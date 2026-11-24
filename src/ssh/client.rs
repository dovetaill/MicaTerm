//! Async SSH client (`russh` 0.65) with SOCKS5, HTTP CONNECT, recursive Jump Hosts,
//! interactive TOFU host-key confirmation, PTY shell pump, and SFTP channel sharing.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use russh::client::{self, AuthResult};
use russh::keys::{self, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use crate::domain::expand_tilde_path;
use crate::domain::models::{ResolvedAuthMaterial, ResolvedProxyHop, ResolvedSshConfig};
use crate::sftp::SftpService;
use crate::ssh::known_hosts::{KnownHostStatus, KnownHostsStore};

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);

trait AsyncStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncStream for T {}
type BoxedStream = Box<dyn AsyncStream>;

#[derive(Debug)]
pub struct SshHostKeyPrompt {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub openssh_key: String,
    pub decision_tx: oneshot::Sender<bool>,
}

pub enum SshSessionEvent {
    Step(String),
    HostKeyPrompt(SshHostKeyPrompt),
    Connected {
        sftp: SftpService,
        initial_dir: String,
        home_dir: Option<String>,
    },
    Output(Vec<u8>),
    Disconnected(Option<String>),
    Error(String),
}

pub enum SshClientCommand {
    Write(Vec<u8>),
    Resize { rows: u16, cols: u16 },
    Disconnect,
}

pub(crate) struct SshClientHandler {
    host: String,
    port: u16,
    known_hosts: KnownHostsStore,
    event_tx: mpsc::UnboundedSender<SshSessionEvent>,
}

impl client::Handler for SshClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let pk = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(cert) => {
                russh::keys::PublicKey::from(cert.public_key().clone())
            }
        };

        match self.known_hosts.verify(&self.host, self.port, &pk)? {
            KnownHostStatus::Trusted => Ok(true),
            KnownHostStatus::Unknown {
                fingerprint,
                openssh_key,
            } => {
                let (tx, rx) = oneshot::channel();
                let _ = self
                    .event_tx
                    .send(SshSessionEvent::HostKeyPrompt(SshHostKeyPrompt {
                        host: self.host.clone(),
                        port: self.port,
                        fingerprint,
                        openssh_key: openssh_key.clone(),
                        decision_tx: tx,
                    }));
                let accepted = rx.await.unwrap_or(false);
                if accepted {
                    self.known_hosts
                        .trust_openssh_key(&self.host, self.port, &openssh_key)?;
                    Ok(true)
                } else {
                    bail!("已拒绝未知主机指纹 `{}:{}`", self.host, self.port)
                }
            }
            KnownHostStatus::Changed {
                expected_fingerprint,
                actual_fingerprint,
            } => bail!(
                "警告：远程主机 `{}:{}` 的指纹发生变化！\n预期：{}\n实际：{}\n连接已中止以防中间人攻击。",
                self.host,
                self.port,
                expected_fingerprint,
                actual_fingerprint
            ),
        }
    }
}

pub struct SshClientSession {
    cmd_tx: mpsc::UnboundedSender<SshClientCommand>,
}

impl SshClientSession {
    pub fn connect_in_runtime(
        rt: &tokio::runtime::Handle,
        config: ResolvedSshConfig,
        known_hosts: KnownHostsStore,
        rows: u16,
        cols: u16,
        event_tx: mpsc::UnboundedSender<SshSessionEvent>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let tx_err = event_tx.clone();
        rt.spawn(async move {
            tracing::info!(
                host = %config.host,
                port = config.port,
                user = %config.user,
                proxy_hops = config.proxy_hops.len(),
                "后台任务开始建立 SSH 会话"
            );
            if let Err(err) =
                run_ssh_session(config, known_hosts, rows, cols, event_tx, cmd_rx).await
            {
                tracing::error!(error = ?err, "SSH 会话建立或运行失败");
                let _ = tx_err.send(SshSessionEvent::Error(format!("{err:#}")));
            }
        });
        Self { cmd_tx }
    }

    pub fn write(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() {
            let _ = self.cmd_tx.send(SshClientCommand::Write(bytes));
        }
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let _ = self.cmd_tx.send(SshClientCommand::Resize {
            rows: rows.max(2),
            cols: cols.max(4),
        });
    }

    pub fn disconnect(&self) {
        let _ = self.cmd_tx.send(SshClientCommand::Disconnect);
    }
}

impl Drop for SshClientSession {
    fn drop(&mut self) {
        self.disconnect();
    }
}

async fn run_ssh_session(
    config: ResolvedSshConfig,
    known_hosts: KnownHostsStore,
    rows: u16,
    cols: u16,
    event_tx: mpsc::UnboundedSender<SshSessionEvent>,
    mut cmd_rx: mpsc::UnboundedReceiver<SshClientCommand>,
) -> Result<()> {
    let client_cfg = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    });

    let mut upstream_handles: Vec<client::Handle<SshClientHandler>> = Vec::new();
    let first_target = next_hop_endpoint(&config, 0);

    let mut stream: BoxedStream = match config.proxy_hops.first() {
        Some(ResolvedProxyHop::Socks5 {
            host,
            port,
            username,
            password,
        }) => {
            let _ = event_tx.send(SshSessionEvent::Step(format!(
                "正在连接 SOCKS5 代理 {host}:{port}..."
            )));
            let mut tcp = TcpStream::connect((host.as_str(), *port))
                .await
                .with_context(|| format!("连接 SOCKS5 代理 `{host}:{port}` 失败"))?;
            let _ = tcp.set_nodelay(true);
            negotiate_socks5(
                &mut tcp,
                username.as_deref(),
                password.as_deref(),
                first_target.0,
                first_target.1,
            )
            .await?;
            Box::new(tcp)
        }
        Some(ResolvedProxyHop::Http {
            host,
            port,
            username,
            password,
        }) => {
            let _ = event_tx.send(SshSessionEvent::Step(format!(
                "正在连接 HTTP 代理 {host}:{port}..."
            )));
            let mut tcp = TcpStream::connect((host.as_str(), *port))
                .await
                .with_context(|| format!("连接 HTTP 代理 `{host}:{port}` 失败"))?;
            let _ = tcp.set_nodelay(true);
            negotiate_http_connect(
                &mut tcp,
                username.as_deref(),
                password.as_deref(),
                first_target.0,
                first_target.1,
            )
            .await?;
            Box::new(tcp)
        }
        Some(ResolvedProxyHop::JumpHost(jump)) => {
            let _ = event_tx.send(SshSessionEvent::Step(format!(
                "正在连接跳板机 {}:{}...",
                jump.host, jump.port
            )));
            let tcp = TcpStream::connect((jump.host.as_str(), jump.port))
                .await
                .with_context(|| format!("连接跳板机 `{}:{}` 失败", jump.host, jump.port))?;
            let _ = tcp.set_nodelay(true);
            Box::new(tcp)
        }
        None => {
            let _ = event_tx.send(SshSessionEvent::Step(format!(
                "正在建立 TCP 连接 {}:{}...",
                config.host, config.port
            )));
            let tcp = TcpStream::connect((config.host.as_str(), config.port))
                .await
                .with_context(|| format!("无法连接到 `{}:{}`", config.host, config.port))?;
            let _ = tcp.set_nodelay(true);
            Box::new(tcp)
        }
    };

    for (idx, hop) in config.proxy_hops.iter().enumerate() {
        if let ResolvedProxyHop::JumpHost(jump) = hop {
            let _ = event_tx.send(SshSessionEvent::Step(format!(
                "正在认证跳板机 {}@{}...",
                jump.user, jump.host
            )));
            let handler = SshClientHandler {
                host: jump.host.clone(),
                port: jump.port,
                known_hosts: known_hosts.clone(),
                event_tx: event_tx.clone(),
            };
            let mut jump_handle = client::connect_stream(Arc::clone(&client_cfg), stream, handler)
                .await
                .with_context(|| format!("跳板机 `{}` SSH 握手失败", jump.host))?;
            authenticate_ssh(&mut jump_handle, jump).await?;

            let (next_host, next_port) = next_hop_endpoint(&config, idx + 1);
            let _ = event_tx.send(SshSessionEvent::Step(format!(
                "正在通过跳板机转发至 {next_host}:{next_port}..."
            )));
            let tunnel = jump_handle
                .channel_open_direct_tcpip(next_host, u32::from(next_port), "127.0.0.1", 0)
                .await
                .with_context(|| format!("跳板机 `{}` 拒绝端口转发", jump.name))?;
            stream = Box::new(tunnel.into_stream());
            upstream_handles.push(jump_handle);
        }
    }

    let _ = event_tx.send(SshSessionEvent::Step(format!(
        "正在进行 SSH 握手与认证 {}@{}...",
        config.user, config.host
    )));
    tracing::debug!(host = %config.host, port = config.port, "TCP 流就绪，开始 russh 协议握手");
    let handler = SshClientHandler {
        host: config.host.clone(),
        port: config.port,
        known_hosts,
        event_tx: event_tx.clone(),
    };
    let mut handle = client::connect_stream(client_cfg, stream, handler)
        .await
        .with_context(|| format!("SSH 握手失败 `{}:{}`", config.host, config.port))?;
    tracing::debug!(host = %config.host, user = %config.user, "SSH 密钥交换完成，开始用户身份认证");
    authenticate_ssh(&mut handle, &config).await?;
    tracing::info!(host = %config.host, user = %config.user, "SSH 用户认证通过，正在打开 PTY 与 SFTP 通道");

    let handle = Arc::new(handle);
    let channel = handle
        .channel_open_session()
        .await
        .context("打开 SSH 会话通道失败")?;
    channel
        .request_pty(
            false,
            "xterm-256color",
            u32::from(cols.max(4)),
            u32::from(rows.max(2)),
            0,
            0,
            &[],
        )
        .await
        .context("请求远程 PTY 失败")?;
    let _ = channel.set_env(false, "COLORTERM", "truecolor").await;
    let _ = channel.set_env(false, "TERM_PROGRAM", "mica-term").await;
    channel
        .request_shell(true)
        .await
        .context("启动远程 Shell 失败")?;

    let sftp = SftpService::from_ssh_handle(Arc::clone(&handle));
    let home_dir = sftp.canonicalize(".").await.ok();
    let initial_dir = if !config.default_remote_dir.is_empty() {
        config.default_remote_dir.clone()
    } else {
        home_dir.clone().unwrap_or_else(|| "/".to_string())
    };

    let _ = event_tx.send(SshSessionEvent::Connected {
        sftp,
        initial_dir,
        home_dir,
    });

    let (mut read_half, write_half) = channel.split();
    let mut disconnect_reason: Option<String> = None;

    loop {
        tokio::select! {
            biased;
            msg = read_half.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        let _ = event_tx.send(SshSessionEvent::Output(data.to_vec()));
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let _ = event_tx.send(SshSessionEvent::Output(data.to_vec()));
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        disconnect_reason = Some(format!("远程进程已退出 (code {exit_status})"));
                    }
                    Some(ChannelMsg::Eof | ChannelMsg::Close) | None => {
                        break;
                    }
                    _ => {}
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(SshClientCommand::Write(bytes)) => {
                        if write_half.data(&bytes[..]).await.is_err() {
                            disconnect_reason = Some("SSH 写入通道已关闭".to_string());
                            break;
                        }
                    }
                    Some(SshClientCommand::Resize { rows, cols }) => {
                        let _ = write_half
                            .window_change(u32::from(cols), u32::from(rows), 0, 0)
                            .await;
                    }
                    Some(SshClientCommand::Disconnect) | None => {
                        let _ = write_half.eof().await;
                        let _ = handle.disconnect(Disconnect::ByApplication, "closed by user", "en").await;
                        break;
                    }
                }
            }
        }
    }

    drop(upstream_handles);
    let _ = event_tx.send(SshSessionEvent::Disconnected(disconnect_reason));
    Ok(())
}

fn next_hop_endpoint(config: &ResolvedSshConfig, from_hop_index: usize) -> (&str, u16) {
    for hop in config.proxy_hops.iter().skip(from_hop_index) {
        if let ResolvedProxyHop::JumpHost(jump) = hop {
            return (jump.host.as_str(), jump.port);
        }
    }
    (config.host.as_str(), config.port)
}

async fn authenticate_ssh(
    handle: &mut client::Handle<SshClientHandler>,
    cfg: &ResolvedSshConfig,
) -> Result<()> {
    let result: AuthResult = match &cfg.auth {
        ResolvedAuthMaterial::Password(password) => handle
            .authenticate_password(cfg.user.clone(), password.clone())
            .await
            .context("SSH 密码认证请求失败")?,
        ResolvedAuthMaterial::PrivateKeyPath { path, passphrase } => {
            let expanded = expand_tilde_path(path);
            let key = keys::load_secret_key(&expanded, passphrase.as_deref())
                .with_context(|| format!("读取私钥文件 `{expanded}` 失败"))?;
            let rsa_hash = handle.best_supported_rsa_hash().await?.flatten();
            handle
                .authenticate_publickey(
                    cfg.user.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash),
                )
                .await
                .context("SSH 私钥文件认证失败")?
        }
        ResolvedAuthMaterial::PrivateKeyInline {
            private_key_pem,
            passphrase,
        } => {
            let key = keys::decode_secret_key(private_key_pem, passphrase.as_deref())
                .context("解析内联 SSH 私钥失败（请检查密钥格式或密码短语）")?;
            let rsa_hash = handle.best_supported_rsa_hash().await?.flatten();
            handle
                .authenticate_publickey(
                    cfg.user.clone(),
                    PrivateKeyWithHashAlg::new(Arc::new(key), rsa_hash),
                )
                .await
                .context("SSH 内联私钥认证失败")?
        }
    };

    if !result.success() {
        bail!("服务器拒绝了用户 `{}` 的认证凭据", cfg.user);
    }
    Ok(())
}

async fn negotiate_socks5(
    stream: &mut TcpStream,
    username: Option<&str>,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<()> {
    let has_auth = username.is_some();
    if has_auth {
        stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await?;
    } else {
        stream.write_all(&[0x05, 0x01, 0x00]).await?;
    }
    let ver = stream.read_u8().await?;
    let method = stream.read_u8().await?;
    if ver != 0x05 {
        bail!("无效的 SOCKS5 协议响应版本: {ver:#x}");
    }
    match method {
        0x00 => {}
        0x02 => {
            let u = username.unwrap_or_default().as_bytes();
            let p = password.unwrap_or_default().as_bytes();
            let mut pkt = Vec::with_capacity(3 + u.len() + p.len());
            pkt.push(0x01);
            pkt.push(u8::try_from(u.len()).context("SOCKS5 用户名过长")?);
            pkt.extend_from_slice(u);
            pkt.push(u8::try_from(p.len()).context("SOCKS5 密码过长")?);
            pkt.extend_from_slice(p);
            stream.write_all(&pkt).await?;
            let _ = stream.read_u8().await?;
            let status = stream.read_u8().await?;
            if status != 0x00 {
                bail!("SOCKS5 代理认证被拒绝");
            }
        }
        _ => bail!("SOCKS5 代理不支持请求的认证方式"),
    }

    let mut req = vec![0x05, 0x01, 0x00];
    if let Ok(ipv4) = target_host.parse::<std::net::Ipv4Addr>() {
        req.push(0x01);
        req.extend_from_slice(&ipv4.octets());
    } else if let Ok(ipv6) = target_host.parse::<std::net::Ipv6Addr>() {
        req.push(0x04);
        req.extend_from_slice(&ipv6.octets());
    } else {
        let hb = target_host.as_bytes();
        req.push(0x03);
        req.push(u8::try_from(hb.len()).context("目标主机域名过长")?);
        req.extend_from_slice(hb);
    }
    req.extend_from_slice(&target_port.to_be_bytes());
    stream.write_all(&req).await?;

    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[1] != 0x00 {
        bail!("SOCKS5 代理连接目标失败 (code {:#x})", head[1]);
    }
    match head[3] {
        0x01 => {
            let mut skip = [0u8; 6];
            stream.read_exact(&mut skip).await?;
        }
        0x03 => {
            let len = stream.read_u8().await? as usize;
            let mut skip = vec![0u8; len + 2];
            stream.read_exact(&mut skip).await?;
        }
        0x04 => {
            let mut skip = [0u8; 18];
            stream.read_exact(&mut skip).await?;
        }
        other => bail!("未知的 SOCKS5 地址类型: {other:#x}"),
    }
    Ok(())
}

async fn negotiate_http_connect(
    stream: &mut TcpStream,
    username: Option<&str>,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<()> {
    let authority = if target_host.contains(':') {
        format!("[{target_host}]:{target_port}")
    } else {
        format!("{target_host}:{target_port}")
    };
    let mut req = format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n");
    if let Some(u) = username {
        let p = password.unwrap_or_default();
        let auth = base64_encode(format!("{u}:{p}").as_bytes());
        req.push_str(&format!("Proxy-Authorization: Basic {auth}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).await?;

    let mut buf = Vec::new();
    while !buf.ends_with(b"\r\n\r\n") {
        if buf.len() > 16 * 1024 {
            bail!("HTTP CONNECT 响应头过大");
        }
        buf.push(stream.read_u8().await?);
    }
    let text = String::from_utf8_lossy(&buf);
    let line = text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("空的 HTTP 代理响应"))?;
    let code: u16 = line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow!("无效的 HTTP 代理状态行: {line}"))?;
    if !(200..300).contains(&code) {
        bail!("HTTP 代理拒绝隧道请求: {line}");
    }
    Ok(())
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
