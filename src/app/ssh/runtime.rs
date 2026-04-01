//! SSH session runtime primitives and terminal-state wrapper.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{error::Error as StdError, fmt};

use anyhow::{Context, Result, anyhow, bail};
use russh::Channel;
use russh::ChannelMsg;
use russh::Disconnect;
use russh::client;
use russh::client::AuthResult;
use russh::keys::{self, PrivateKeyWithHashAlg};
use russh_sftp::client::SftpSession;
use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers as KeyModifiers};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{Sleep, sleep};
use uuid::Uuid;
use wezterm_surface::{CursorShape, CursorVisibility};
use wezterm_term::color::{ColorAttribute, ColorPalette, SrgbaTuple};
use wezterm_term::{Intensity, Line, Terminal, TerminalConfiguration, TerminalSize, Underline};

use crate::app::sftp::{SftpBackend, SftpDirectoryEntry, SftpOperationFuture, SftpRuntimeHandle};
use crate::app::ssh::connection_progress::{
    ConnectionHeadlineState, ConnectionProgressEvent, ConnectionStepState, ConnectionStepStateItem,
};
use crate::app::ssh::credentials::{
    CredentialStore, StoredSecretLookupError, StoredSshSecretBundle, SystemCredentialStore,
    load_secret_bundle_with_diagnostics, required_secret_bundle_field,
};
use crate::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService, default_known_hosts_path};
use crate::app::ssh::profile::{ConnectionProfile, ResolvedProxyHop, SshAuthMethod};
use crate::app::ssh::session_manager::{EnhancedSessionState, SessionRuntimeControl};
use crate::app::ssh::shell_integration::runtime_shell_events;
use crate::app::terminal_theme::{palette_for_theme_mode, preset_for_theme_mode};
use crate::theme::ThemeMode;

const DEFAULT_TERMINAL_ROWS: usize = 24;
const DEFAULT_TERMINAL_COLS: usize = 80;
const TERMINAL_SCROLLBACK_LINES: usize = 3_500;
const SSH_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSH_KEEPALIVE_MAX_MISSES: usize = 3;
const SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(40);
const WORKING_SET_TRIM_IDLE_INTERVAL: Duration = Duration::from_secs(2);
const WORKING_SET_TRIM_MIN_OUTPUT_BYTES: usize = 1024 * 1024;
const FILTERED_EXACT_BANNER: &str =
    "Activate the web console with: systemctl enable --now cockpit.socket";

fn ssh_client_config() -> client::Config {
    client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(SSH_KEEPALIVE_INTERVAL),
        keepalive_max: SSH_KEEPALIVE_MAX_MISSES,
        nodelay: true,
        ..Default::default()
    }
}

trait TransportStream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> TransportStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

type BoxedTransportStream = Box<dyn TransportStream>;

#[derive(Default)]
struct TransportChainGuard {
    upstream_handles: Vec<client::Handle<RuntimeClientHandler>>,
}

fn configure_transport_nodelay(stream: &TcpStream, enabled: bool) {
    if enabled && let Err(error) = stream.set_nodelay(true) {
        tracing::warn!("set_nodelay() failed for SSH transport stream: {error:?}");
    }
}

async fn connect_direct_tcp_stream(
    host: &str,
    port: u16,
    enable_nodelay: bool,
) -> Result<TcpStream> {
    let stream = TcpStream::connect((host, port))
        .await
        .with_context(|| format!("failed to connect to SSH server `{}:{}`", host, port))?;
    configure_transport_nodelay(&stream, enable_nodelay);
    Ok(stream)
}

fn next_chain_target(profile: &ConnectionProfile, hop_index: usize) -> Result<(&str, u16)> {
    match profile.resolved_proxy_hops.get(hop_index + 1) {
        Some(ResolvedProxyHop::Ssh(upstream)) => Ok((upstream.host.as_str(), upstream.port)),
        Some(ResolvedProxyHop::Socks5 { .. } | ResolvedProxyHop::Http { .. }) => {
            bail!(
                "HTTP and SOCKS5 hops must be the outermost transport in resolved SSH proxy chains"
            )
        }
        None => Ok((profile.host.as_str(), profile.port)),
    }
}

async fn connect_ssh_handle_over_stream(
    config: Arc<client::Config>,
    stream: BoxedTransportStream,
    profile: &ConnectionProfile,
) -> Result<client::Handle<RuntimeClientHandler>> {
    let handler = RuntimeClientHandler {
        host: profile.host.clone(),
        port: profile.port,
        known_hosts: KnownHostsService::new(default_known_hosts_path()?),
    };
    let handle = client::connect_stream(config, stream, handler)
        .await
        .with_context(|| {
            format!(
                "failed to connect to SSH server `{}:{}`",
                profile.host, profile.port
            )
        })?;
    Ok(handle)
}

async fn open_direct_tcpip_stream(
    upstream_handle: &client::Handle<RuntimeClientHandler>,
    upstream_profile: &ConnectionProfile,
    target_host: &str,
    target_port: u16,
) -> Result<BoxedTransportStream> {
    match upstream_handle
        .channel_open_direct_tcpip(target_host, u32::from(target_port), "127.0.0.1", 0)
        .await
    {
        Ok(channel) => Ok(Box::new(channel.into_stream())),
        Err(russh::Error::ChannelOpenFailure(_)) => {
            bail!(
                "SSH upstream '{}' rejected direct-tcpip forwarding",
                upstream_profile.name
            )
        }
        Err(error) => Err(anyhow!(error)).with_context(|| {
            format!(
                "failed to open direct-tcpip channel via SSH upstream `{}`",
                upstream_profile.name
            )
        }),
    }
}

async fn connect_proxy_tcp_stream(
    proxy_host: &str,
    proxy_port: u16,
    enable_nodelay: bool,
    proxy_kind: &str,
) -> Result<TcpStream> {
    let stream = TcpStream::connect((proxy_host, proxy_port))
        .await
        .with_context(|| {
            format!("failed to open TCP stream to {proxy_kind} proxy `{proxy_host}:{proxy_port}`")
        })?;
    configure_transport_nodelay(&stream, enable_nodelay);
    Ok(stream)
}

async fn connect_target_handle_for_profile(
    config: Arc<client::Config>,
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
    progress: &mut ConnectionProgressReporter,
) -> Result<(TransportChainGuard, client::Handle<RuntimeClientHandler>)> {
    let mut chain_guard = TransportChainGuard::default();

    let mut current_stream: BoxedTransportStream =
        match profile.resolved_proxy_hops.first() {
            Some(ResolvedProxyHop::Socks5 {
                host,
                port,
                username,
                password,
            }) => {
                let connect_step = progress.start_step(
                    "connect-proxy",
                    "Connect Proxy",
                    format!("Connecting to SOCKS5 proxy {host}:{port}"),
                    "Proxy",
                );
                let (next_host, next_port) = next_chain_target(profile, 0)?;
                let mut stream =
                    match connect_proxy_tcp_stream(host, *port, config.as_ref().nodelay, "SOCKS5")
                        .await
                    {
                        Ok(stream) => {
                            connect_step.finish(format!("Connected to SOCKS5 proxy {host}:{port}"));
                            stream
                        }
                        Err(err) => {
                            connect_step.fail(err.to_string());
                            return Err(err);
                        }
                    };
                let negotiate_step = progress.start_step(
                    "proxy-negotiate",
                    "Negotiate Proxy Tunnel",
                    format!("Negotiating SOCKS5 tunnel to {next_host}:{next_port}"),
                    "Proxy",
                );
                match negotiate_socks5_proxy_tunnel(
                    &mut stream,
                    username.as_deref(),
                    password.as_deref(),
                    next_host,
                    next_port,
                )
                .await
                .with_context(|| format!("failed to negotiate SOCKS5 proxy `{}:{}`", host, port))
                {
                    Ok(()) => {
                        negotiate_step.finish(format!(
                            "Established SOCKS5 tunnel to {next_host}:{next_port}"
                        ));
                        Box::new(stream)
                    }
                    Err(err) => {
                        negotiate_step.fail(err.to_string());
                        return Err(err);
                    }
                }
            }
            Some(ResolvedProxyHop::Http {
                host,
                port,
                username,
                password,
            }) => {
                let connect_step = progress.start_step(
                    "connect-proxy",
                    "Connect Proxy",
                    format!("Connecting to HTTP proxy {host}:{port}"),
                    "Proxy",
                );
                let (next_host, next_port) = next_chain_target(profile, 0)?;
                let mut stream =
                    match connect_proxy_tcp_stream(host, *port, config.as_ref().nodelay, "HTTP")
                        .await
                    {
                        Ok(stream) => {
                            connect_step.finish(format!("Connected to HTTP proxy {host}:{port}"));
                            stream
                        }
                        Err(err) => {
                            connect_step.fail(err.to_string());
                            return Err(err);
                        }
                    };
                let negotiate_step = progress.start_step(
                    "proxy-negotiate",
                    "Negotiate Proxy Tunnel",
                    format!("Negotiating HTTP CONNECT tunnel to {next_host}:{next_port}"),
                    "Proxy",
                );
                match negotiate_http_connect_tunnel(
                    &mut stream,
                    username.as_deref(),
                    password.as_deref(),
                    next_host,
                    next_port,
                )
                .await
                .with_context(|| format!("failed to negotiate HTTP proxy `{}:{}`", host, port))
                {
                    Ok(()) => {
                        negotiate_step.finish(format!(
                            "Established HTTP CONNECT tunnel to {next_host}:{next_port}"
                        ));
                        Box::new(stream)
                    }
                    Err(err) => {
                        negotiate_step.fail(err.to_string());
                        return Err(err);
                    }
                }
            }
            Some(ResolvedProxyHop::Ssh(upstream)) => Box::new(
                connect_direct_tcp_stream(
                    upstream.host.as_str(),
                    upstream.port,
                    config.as_ref().nodelay,
                )
                .await?,
            ),
            None => Box::new(
                connect_direct_tcp_stream(
                    profile.host.as_str(),
                    profile.port,
                    config.as_ref().nodelay,
                )
                .await?,
            ),
        };

    let mut jump_host_index = 0usize;
    for (hop_index, hop) in profile.resolved_proxy_hops.iter().enumerate() {
        match hop {
            ResolvedProxyHop::Socks5 { .. } | ResolvedProxyHop::Http { .. } => {
                if hop_index != 0 {
                    bail!(
                        "HTTP and SOCKS5 hops must be the outermost transport in resolved SSH proxy chains"
                    );
                }
            }
            ResolvedProxyHop::Ssh(upstream) => {
                jump_host_index = jump_host_index.saturating_add(1);
                let upstream_profile = upstream.as_ref();
                let hop_label = format!("Jump Host {jump_host_index}");
                let connect_step = progress.start_step(
                    "connect-jump-host",
                    "Connect Jump Host",
                    format!("Opening SSH transport to {}", upstream_profile.host),
                    hop_label.clone(),
                );
                let mut upstream_handle = match connect_ssh_handle_over_stream(
                    Arc::clone(&config),
                    current_stream,
                    upstream_profile,
                )
                .await
                {
                    Ok(handle) => {
                        connect_step.finish(format!(
                            "Connected to jump host {}:{}",
                            upstream_profile.host, upstream_profile.port
                        ));
                        handle
                    }
                    Err(err) => {
                        if let Some(unknown) = err.downcast_ref::<UnknownHostKeyError>() {
                            connect_step.finish(format!(
                                "Connected to jump host {}:{}",
                                upstream_profile.host, upstream_profile.port
                            ));
                            progress.set_headline(ConnectionHeadlineState::WaitingUser);
                            let verify_step = progress.start_step(
                                "verify-host-key",
                                "Verify Host Key",
                                format!("Verifying host key for {}", upstream_profile.host),
                                hop_label.clone(),
                            );
                            verify_step.block(format!(
                                "Host key verification required for {}:{} ({})",
                                unknown.host, unknown.port, unknown.fingerprint
                            ));
                        } else {
                            connect_step.fail(err.to_string());
                        }
                        return Err(err);
                    }
                };
                let verify_step = progress.start_step(
                    "verify-host-key",
                    "Verify Host Key",
                    format!("Verifying host key for {}", upstream_profile.host),
                    hop_label.clone(),
                );
                verify_step.finish(format!(
                    "Verified host key for {}:{}",
                    upstream_profile.host, upstream_profile.port
                ));
                let auth_step = progress.start_step(
                    "authenticate-jump-host",
                    "Authenticate Jump Host",
                    format!("Authenticating to {}", upstream_profile.user),
                    hop_label.clone(),
                );
                if let Err(err) =
                    authenticate_client(&mut upstream_handle, upstream_profile, credential_store)
                        .await
                {
                    auth_step.fail(err.to_string());
                    return Err(err);
                }
                auth_step.finish(format!(
                    "Authenticated jump host {} as {}",
                    upstream_profile.host, upstream_profile.user
                ));
                let (next_host, next_port) = next_chain_target(profile, hop_index)?;
                let direct_tcpip_step = progress.start_step(
                    "open-direct-tcpip",
                    "Open Direct TCPIP",
                    format!("Opening SSH tunnel to {next_host}:{next_port}"),
                    hop_label,
                );
                current_stream = match open_direct_tcpip_stream(
                    &upstream_handle,
                    upstream_profile,
                    next_host,
                    next_port,
                )
                .await
                {
                    Ok(stream) => {
                        direct_tcpip_step.finish(format!(
                            "Opened direct-tcpip tunnel to {next_host}:{next_port}"
                        ));
                        stream
                    }
                    Err(err) => {
                        direct_tcpip_step.fail(err.to_string());
                        return Err(err);
                    }
                };
                chain_guard.upstream_handles.push(upstream_handle);
            }
        }
    }

    let connect_target_step = progress.start_step(
        "connect-target",
        "Connect Target",
        format!("Opening SSH transport to {}", profile.host),
        "Target",
    );
    let mut handle = match connect_ssh_handle_over_stream(config, current_stream, profile).await {
        Ok(handle) => {
            connect_target_step.finish(format!(
                "Connected to target {}:{}",
                profile.host, profile.port
            ));
            handle
        }
        Err(err) => {
            if let Some(unknown) = err.downcast_ref::<UnknownHostKeyError>() {
                connect_target_step.finish(format!(
                    "Connected to target {}:{}",
                    profile.host, profile.port
                ));
                progress.set_headline(ConnectionHeadlineState::WaitingUser);
                let verify_step = progress.start_step(
                    "verify-host-key",
                    "Verify Host Key",
                    format!("Verifying host key for {}", profile.host),
                    "Target",
                );
                verify_step.block(format!(
                    "Host key verification required for {}:{} ({})",
                    unknown.host, unknown.port, unknown.fingerprint
                ));
            } else {
                connect_target_step.fail(err.to_string());
            }
            return Err(err);
        }
    };
    let verify_step = progress.start_step(
        "verify-host-key",
        "Verify Host Key",
        format!("Verifying host key for {}", profile.host),
        "Target",
    );
    verify_step.finish(format!(
        "Verified host key for {}:{}",
        profile.host, profile.port
    ));
    let auth_step = progress.start_step(
        "authenticate-target",
        "Authenticate Target",
        format!("Authenticating to {}", profile.user),
        "Target",
    );
    if let Err(err) = authenticate_client(&mut handle, profile, credential_store).await {
        auth_step.fail(err.to_string());
        return Err(err);
    }
    auth_step.finish(format!(
        "Authenticated target {} as {}",
        profile.host, profile.user
    ));
    Ok((chain_guard, handle))
}

async fn negotiate_socks5_proxy_tunnel(
    stream: &mut TcpStream,
    username: Option<&str>,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<()> {
    let requires_password_auth = username.is_some() || password.is_some();
    let mut methods = vec![0x00];
    if requires_password_auth {
        match (username, password) {
            (Some(_), Some(_)) => methods.push(0x02),
            _ => bail!("SOCKS5 username/password auth requires both username and password"),
        }
    }

    stream
        .write_all(&[0x05, methods.len() as u8])
        .await
        .context("failed to write SOCKS5 greeting")?;
    stream
        .write_all(&methods)
        .await
        .context("failed to write SOCKS5 auth methods")?;

    let reply_version = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 greeting response version")?;
    let selected_method = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 selected auth method")?;
    if reply_version != 0x05 {
        bail!("unexpected SOCKS5 greeting response version: {reply_version:#04x}");
    }

    match selected_method {
        0x00 => {}
        0x02 => {
            let username = username.expect("username/password auth validated above");
            let password = password.expect("username/password auth validated above");
            authenticate_socks5_username_password(stream, username, password).await?;
        }
        0xFF => bail!("SOCKS5 proxy rejected all advertised authentication methods"),
        other => bail!("SOCKS5 proxy selected unsupported auth method: {other:#04x}"),
    }

    write_socks5_connect_request(stream, target_host, target_port).await?;
    read_socks5_connect_reply(stream).await?;

    Ok(())
}

async fn negotiate_http_connect_tunnel(
    stream: &mut TcpStream,
    username: Option<&str>,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<()> {
    let target_authority = format_proxy_authority(target_host, target_port);
    let mut request =
        format!("CONNECT {target_authority} HTTP/1.1\r\nHost: {target_authority}\r\n");

    if username.is_some() || password.is_some() {
        let (username, password) = match (username, password) {
            (Some(username), Some(password)) => (username, password),
            _ => bail!("HTTP proxy basic auth requires both username and password"),
        };
        request.push_str("Proxy-Authorization: Basic ");
        request.push_str(encode_basic_auth_header(username, password).as_str());
        request.push_str("\r\n");
    }
    request.push_str("\r\n");

    stream
        .write_all(request.as_bytes())
        .await
        .context("failed to write HTTP CONNECT request")?;

    let response = read_http_connect_response(stream).await?;
    let status = parse_http_connect_status(response.as_str())?;
    if !(200..300).contains(&status) {
        bail!("HTTP CONNECT request failed with status: {status}");
    }

    Ok(())
}

fn format_proxy_authority(target_host: &str, target_port: u16) -> String {
    if target_host.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("[{target_host}]:{target_port}")
    } else {
        format!("{target_host}:{target_port}")
    }
}

fn encode_basic_auth_header(username: &str, password: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let raw = format!("{username}:{password}");
    let bytes = raw.as_bytes();
    let mut encoded = String::new();
    let mut index = 0;
    while index < bytes.len() {
        let first = bytes[index];
        let second = bytes.get(index + 1).copied();
        let third = bytes.get(index + 2).copied();

        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0x03) << 4) | (second.unwrap_or(0) >> 4)) as usize] as char);
        match second {
            Some(second) => {
                encoded.push(
                    TABLE[(((second & 0x0f) << 2) | (third.unwrap_or(0) >> 6)) as usize] as char,
                );
            }
            None => encoded.push('='),
        }
        match third {
            Some(third) => encoded.push(TABLE[(third & 0x3f) as usize] as char),
            None => encoded.push('='),
        }

        index += 3;
    }

    encoded
}

async fn read_http_connect_response(stream: &mut TcpStream) -> Result<String> {
    const MAX_HTTP_CONNECT_RESPONSE_BYTES: usize = 16 * 1024;

    let mut response = Vec::new();
    while !response.ends_with(b"\r\n\r\n") {
        if response.len() >= MAX_HTTP_CONNECT_RESPONSE_BYTES {
            bail!("HTTP CONNECT response headers exceeded the maximum supported size");
        }
        response.push(
            stream
                .read_u8()
                .await
                .context("failed to read HTTP CONNECT response")?,
        );
    }

    String::from_utf8(response).context("failed to decode HTTP CONNECT response")
}

fn parse_http_connect_status(response: &str) -> Result<u16> {
    let status_line = response
        .lines()
        .next()
        .ok_or_else(|| anyhow!("HTTP CONNECT response was empty"))?;
    let mut parts = status_line.split_whitespace();
    let protocol = parts
        .next()
        .ok_or_else(|| anyhow!("HTTP CONNECT response is missing its protocol version"))?;
    if !protocol.starts_with("HTTP/") {
        bail!("unexpected HTTP CONNECT response protocol: {protocol}");
    }
    let status = parts
        .next()
        .ok_or_else(|| anyhow!("HTTP CONNECT response is missing its status code"))?;
    status
        .parse::<u16>()
        .with_context(|| format!("invalid HTTP CONNECT status code: {status}"))
}

async fn authenticate_socks5_username_password(
    stream: &mut TcpStream,
    username: &str,
    password: &str,
) -> Result<()> {
    let username_len = u8::try_from(username.len())
        .context("SOCKS5 username exceeds the protocol length limit")?;
    let password_len = u8::try_from(password.len())
        .context("SOCKS5 password exceeds the protocol length limit")?;

    stream
        .write_all(&[0x01, username_len])
        .await
        .context("failed to write SOCKS5 username/password auth header")?;
    stream
        .write_all(username.as_bytes())
        .await
        .context("failed to write SOCKS5 username")?;
    stream
        .write_all(&[password_len])
        .await
        .context("failed to write SOCKS5 password length")?;
    stream
        .write_all(password.as_bytes())
        .await
        .context("failed to write SOCKS5 password")?;

    let reply_version = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 username/password auth version")?;
    let status = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 username/password auth status")?;
    if reply_version != 0x01 {
        bail!("unexpected SOCKS5 username/password auth version: {reply_version:#04x}");
    }
    if status != 0x00 {
        bail!("SOCKS5 username/password authentication was rejected");
    }

    Ok(())
}

async fn write_socks5_connect_request(
    stream: &mut TcpStream,
    target_host: &str,
    target_port: u16,
) -> Result<()> {
    let mut request = vec![0x05, 0x01, 0x00];
    if let Ok(ipv4) = target_host.parse::<std::net::Ipv4Addr>() {
        request.push(0x01);
        request.extend_from_slice(&ipv4.octets());
    } else if let Ok(ipv6) = target_host.parse::<std::net::Ipv6Addr>() {
        request.push(0x04);
        request.extend_from_slice(&ipv6.octets());
    } else {
        let host_bytes = target_host.as_bytes();
        let host_len = u8::try_from(host_bytes.len())
            .context("SOCKS5 target host exceeds the protocol length limit")?;
        request.push(0x03);
        request.push(host_len);
        request.extend_from_slice(host_bytes);
    }
    request.extend_from_slice(&target_port.to_be_bytes());

    stream
        .write_all(&request)
        .await
        .context("failed to write SOCKS5 CONNECT request")?;

    Ok(())
}

async fn read_socks5_connect_reply(stream: &mut TcpStream) -> Result<()> {
    let reply_version = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 CONNECT reply version")?;
    let reply_code = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 CONNECT reply code")?;
    let reserved = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 CONNECT reserved byte")?;
    let address_type = stream
        .read_u8()
        .await
        .context("failed to read SOCKS5 CONNECT bound address type")?;

    if reply_version != 0x05 {
        bail!("unexpected SOCKS5 CONNECT reply version: {reply_version:#04x}");
    }
    if reserved != 0x00 {
        bail!("unexpected SOCKS5 CONNECT reserved byte: {reserved:#04x}");
    }

    match address_type {
        0x01 => {
            let mut addr = [0_u8; 4];
            stream
                .read_exact(&mut addr)
                .await
                .context("failed to read SOCKS5 bound IPv4 address")?;
        }
        0x03 => {
            let host_len = stream
                .read_u8()
                .await
                .context("failed to read SOCKS5 bound domain length")?;
            let mut host = vec![0_u8; host_len as usize];
            stream
                .read_exact(&mut host)
                .await
                .context("failed to read SOCKS5 bound domain")?;
        }
        0x04 => {
            let mut addr = [0_u8; 16];
            stream
                .read_exact(&mut addr)
                .await
                .context("failed to read SOCKS5 bound IPv6 address")?;
        }
        other => bail!("unexpected SOCKS5 CONNECT address type: {other:#04x}"),
    }
    let _bound_port = stream
        .read_u16()
        .await
        .context("failed to read SOCKS5 bound port")?;

    if reply_code != 0x00 {
        bail!("SOCKS5 CONNECT request failed with status: {reply_code:#04x}");
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSurfaceState {
    pub session_id: Uuid,
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    pub visible_rows: Vec<TerminalRowState>,
    pub visible_lines: Vec<String>,
    pub cells: Vec<TerminalCellState>,
    pub cursor: TerminalCursorState,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub bracketed_paste_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSurfaceSignature {
    pub session_id: Uuid,
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    pub cursor_row: u32,
    pub cursor_col: u32,
    pub cursor_visible: bool,
    pub cursor_blinking: bool,
    pub cursor_shape: TerminalCursorShape,
    pub cursor_fg_rgba: u32,
    pub cursor_bg_rgba: u32,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub bracketed_paste_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRowState {
    pub index: u32,
    pub text: String,
    pub wrapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCellState {
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub text: String,
    pub bold: bool,
    pub underline: bool,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCursorState {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub blinking: bool,
    pub shape: TerminalCursorShape,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseEventKind {
    Down,
    Up,
    Move,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMouseInput {
    pub kind: TerminalMouseEventKind,
    pub button: TerminalMouseButton,
    pub row: u32,
    pub col: u32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKeyKind {
    Named(&'static str),
    Function(u8),
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalKeyEvent {
    pub key: TerminalKeyKind,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl TerminalKeyEvent {
    pub fn named(key_name: &'static str, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Named(key_name),
            alt,
            ctrl,
            shift,
        }
    }

    pub fn function(number: u8, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Function(number),
            alt,
            ctrl,
            shift,
        }
    }

    pub fn character(ch: char, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Char(ch),
            alt,
            ctrl,
            shift,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeEvent {
    Connected,
    ConnectionProgress(ConnectionProgressEvent),
    EnhancedSessionStateChanged(EnhancedSessionState),
    CurrentDirectoryChanged(String),
    SurfaceChanged(TerminalSurfaceState),
    SurfaceDirty,
    Disconnected,
    Error(String),
}

pub struct SshSessionRuntime {
    session_id: Uuid,
    #[allow(dead_code)]
    profile: ConnectionProfile,
    terminal: Arc<Mutex<TerminalSession>>,
    command_tx: mpsc::UnboundedSender<RuntimeCommand>,
    sftp_runtime: SftpRuntimeHandle,
}

struct RusshSftpBackend {
    handle: Arc<client::Handle<RuntimeClientHandler>>,
}

impl RusshSftpBackend {
    async fn open_sftp_session(&self) -> Result<SftpSession> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel for SFTP subsystem")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("failed to request SFTP subsystem")?;
        SftpSession::new(channel.into_stream())
            .await
            .context("failed to initialize SFTP client session")
    }
}

impl SftpBackend for RusshSftpBackend {
    fn read_dir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, Vec<SftpDirectoryEntry>> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let mut read_dir = sftp
                .read_dir(path)
                .await
                .with_context(|| format!("failed to read remote directory `{path}`"))?;
            let mut entries = Vec::new();

            for entry in &mut read_dir {
                let name = entry.file_name();
                let child_path = remote_child_path(path, &name);
                let metadata = entry.metadata();
                let kind = if metadata.is_dir() {
                    crate::app::sftp::SftpDirectoryEntryKind::Directory
                } else if metadata.is_symlink() {
                    crate::app::sftp::SftpDirectoryEntryKind::Symlink
                } else if metadata.is_regular() {
                    crate::app::sftp::SftpDirectoryEntryKind::File
                } else {
                    crate::app::sftp::SftpDirectoryEntryKind::Unknown
                };
                entries.push(SftpDirectoryEntry {
                    id: child_path.clone(),
                    name,
                    path: child_path,
                    kind,
                    modified_unix_seconds: metadata.mtime.map(u64::from),
                    size_bytes: metadata.size,
                });
            }

            Ok(entries)
        })
    }

    fn mkdir<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.create_dir(path)
                .await
                .with_context(|| format!("failed to create remote directory `{path}`"))?;
            Ok(())
        })
    }

    fn rename<'a>(&'a self, from: &'a str, to: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.rename(from, to)
                .await
                .with_context(|| format!("failed to rename remote path `{from}` -> `{to}`"))?;
            Ok(())
        })
    }

    fn path_exists<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, bool> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.try_exists(path)
                .await
                .with_context(|| format!("failed to check remote path `{path}`"))
        })
    }

    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> SftpOperationFuture<'a, u64> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            let mut file = sftp
                .create(remote_path)
                .await
                .with_context(|| format!("failed to create remote file `{remote_path}`"))?;
            file.write_all(&data)
                .await
                .with_context(|| format!("failed to write remote file `{remote_path}`"))?;
            Ok(data.len() as u64)
        })
    }

    fn download_file<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.read(remote_path)
                .await
                .with_context(|| format!("failed to read remote file `{remote_path}`"))
        })
    }

    fn remove_file<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.remove_file(remote_path)
                .await
                .with_context(|| format!("failed to remove remote file `{remote_path}`"))?;
            Ok(())
        })
    }

    fn remove_dir<'a>(&'a self, remote_path: &'a str) -> SftpOperationFuture<'a, ()> {
        Box::pin(async move {
            let sftp = self.open_sftp_session().await?;
            sftp.remove_dir(remote_path)
                .await
                .with_context(|| format!("failed to remove remote directory `{remote_path}`"))?;
            Ok(())
        })
    }
}

fn remote_child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{}", name.trim_start_matches('/'))
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

struct ConnectionProgressReporter {
    attempt_id: Uuid,
    next_step_index: usize,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

struct ConnectionProgressStep {
    attempt_id: Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    step_id: String,
    step_kind: String,
    title: String,
    hop_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownHostKeyError {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub public_key_openssh: String,
}

impl fmt::Display for UnknownHostKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown SSH host key for `{}`:{} ({})",
            self.host, self.port, self.fingerprint
        )
    }
}

impl StdError for UnknownHostKeyError {}

#[derive(Debug)]
enum RuntimeCommand {
    TextInput(String),
    KeyInput(TerminalKeyEvent),
    MouseInput(TerminalMouseInput),
    Paste(String),
    Resize { rows: u32, cols: u32 },
    Disconnect,
}

struct RuntimeClientHandler {
    host: String,
    port: u16,
    known_hosts: KnownHostsService,
}

impl ConnectionProgressReporter {
    fn new(
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        headline: ConnectionHeadlineState,
    ) -> Self {
        let reporter = Self {
            attempt_id,
            next_step_index: 0,
            event_tx,
        };
        let _ = reporter
            .event_tx
            .send(SessionRuntimeEvent::ConnectionProgress(
                ConnectionProgressEvent::AttemptStarted {
                    attempt_id,
                    headline,
                },
            ));
        reporter
    }

    fn start_step(
        &mut self,
        step_kind: &str,
        title: impl Into<String>,
        detail: impl Into<String>,
        hop_label: impl Into<String>,
    ) -> ConnectionProgressStep {
        let step = ConnectionStepStateItem {
            step_id: format!("{:02}-{}", self.next_step_index, step_kind),
            step_kind: step_kind.to_string(),
            title: title.into(),
            detail: detail.into(),
            hop_label: hop_label.into(),
            state: ConnectionStepState::Running,
        };
        self.next_step_index = self.next_step_index.saturating_add(1);
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: step.clone(),
            },
        ));
        ConnectionProgressStep {
            attempt_id: self.attempt_id,
            event_tx: self.event_tx.clone(),
            step_id: step.step_id,
            step_kind: step.step_kind,
            title: step.title,
            hop_label: step.hop_label,
        }
    }

    fn set_headline(&self, headline: ConnectionHeadlineState) {
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::HeadlineChanged {
                attempt_id: self.attempt_id,
                headline,
            },
        ));
    }
}

impl ConnectionProgressStep {
    fn finish(self, detail: impl Into<String>) {
        let detail = detail.into();
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: ConnectionStepStateItem {
                    step_id: self.step_id.clone(),
                    step_kind: self.step_kind.clone(),
                    title: self.title.clone(),
                    detail: detail.clone(),
                    hop_label: self.hop_label.clone(),
                    state: ConnectionStepState::Done,
                },
            },
        ));
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::DiagnosticAppended {
                attempt_id: self.attempt_id,
                message: detail,
            },
        ));
    }

    fn fail(self, detail: impl Into<String>) {
        let detail = detail.into();
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: ConnectionStepStateItem {
                    step_id: self.step_id.clone(),
                    step_kind: self.step_kind.clone(),
                    title: self.title.clone(),
                    detail: detail.clone(),
                    hop_label: self.hop_label.clone(),
                    state: ConnectionStepState::Failed,
                },
            },
        ));
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::DiagnosticAppended {
                attempt_id: self.attempt_id,
                message: detail,
            },
        ));
    }

    fn block(self, detail: impl Into<String>) {
        let detail = detail.into();
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::StepUpdated {
                attempt_id: self.attempt_id,
                step: ConnectionStepStateItem {
                    step_id: self.step_id.clone(),
                    step_kind: self.step_kind.clone(),
                    title: self.title.clone(),
                    detail: detail.clone(),
                    hop_label: self.hop_label.clone(),
                    state: ConnectionStepState::Blocked,
                },
            },
        ));
        let _ = self.event_tx.send(SessionRuntimeEvent::ConnectionProgress(
            ConnectionProgressEvent::DiagnosticAppended {
                attempt_id: self.attempt_id,
                message: detail,
            },
        ));
    }
}

impl client::Handler for RuntimeClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        match self
            .known_hosts
            .check(&self.host, self.port, server_public_key)?
        {
            KnownHostCheck::Trusted => Ok(true),
            KnownHostCheck::Unknown { fingerprint } => Err(UnknownHostKeyError {
                host: self.host.clone(),
                port: self.port,
                fingerprint,
                public_key_openssh: server_public_key
                    .to_openssh()
                    .context("failed to encode unknown SSH host key")?,
            }
            .into()),
            KnownHostCheck::Changed { expected, actual } => bail!(
                "SSH host key changed for `{}`:{} (expected {}, got {})",
                self.host,
                self.port,
                expected,
                actual
            ),
        }
    }
}

impl SshSessionRuntime {
    pub async fn connect(
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Result<Self> {
        Self::connect_with_credential_store(
            profile,
            session_id,
            attempt_id,
            event_tx,
            Arc::new(SystemCredentialStore),
        )
        .await
    }

    pub async fn connect_with_credential_store(
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        credential_store: Arc<dyn CredentialStore>,
    ) -> Result<Self> {
        let mut progress = ConnectionProgressReporter::new(
            attempt_id,
            event_tx.clone(),
            ConnectionHeadlineState::Connecting,
        );
        let resolve_step = progress.start_step(
            "resolve-profile",
            "Resolve Profile",
            format!("Resolving connection profile for {}", profile.name),
            "Target",
        );
        let terminal = Arc::new(Mutex::new(TerminalSession::new(
            DEFAULT_TERMINAL_ROWS,
            DEFAULT_TERMINAL_COLS,
        )));
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let config = Arc::new(ssh_client_config());
        resolve_step.finish(format!("Resolved connection profile for {}", profile.name));
        let (transport_chain_guard, handle) = connect_target_handle_for_profile(
            Arc::clone(&config),
            &profile,
            credential_store.as_ref(),
            &mut progress,
        )
        .await?;

        let open_session_step = progress.start_step(
            "open-session-channel",
            "Open Session Channel",
            format!("Opening SSH session channel for {}", profile.host),
            "Target",
        );
        let mut channel = handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel")?;
        open_session_step.finish(format!("Opened SSH session channel for {}", profile.host));
        let pty_step = progress.start_step(
            "request-pty",
            "Request PTY",
            "Requesting terminal PTY".to_string(),
            "Target",
        );
        channel
            .request_pty(
                true,
                "xterm-256color",
                DEFAULT_TERMINAL_COLS as u32,
                DEFAULT_TERMINAL_ROWS as u32,
                (DEFAULT_TERMINAL_COLS * 8) as u32,
                (DEFAULT_TERMINAL_ROWS * 16) as u32,
                &[],
            )
            .await
            .context("failed to request SSH PTY")?;

        let mut pending_output = Vec::new();
        await_channel_success(&mut channel, "pty", &mut pending_output).await?;
        pty_step.finish("SSH PTY request accepted");
        negotiate_terminal_environment(&mut channel, &mut pending_output).await;

        let shell_step = progress.start_step(
            "request-shell",
            "Request Shell",
            "Requesting interactive shell".to_string(),
            "Target",
        );
        channel
            .request_shell(true)
            .await
            .context("failed to request remote shell")?;
        await_channel_success(&mut channel, "shell", &mut pending_output).await?;
        shell_step.finish("Interactive shell request accepted");

        progress.set_headline(ConnectionHeadlineState::Connected);
        let _ = event_tx.send(SessionRuntimeEvent::Connected);
        if !pending_output.is_empty() {
            apply_remote_output(&terminal, &pending_output);
        }

        let handle = Arc::new(handle);
        let sftp_runtime = SftpRuntimeHandle::new(Arc::new(RusshSftpBackend {
            handle: Arc::clone(&handle),
        }));

        let runtime = Self {
            session_id,
            profile: profile.clone(),
            terminal: Arc::clone(&terminal),
            command_tx,
            sftp_runtime,
        };

        tokio::spawn(run_channel_pump(
            session_id,
            handle,
            channel,
            terminal,
            event_tx,
            command_rx,
            transport_chain_guard,
        ));

        Ok(runtime)
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn terminal(&self) -> Arc<Mutex<TerminalSession>> {
        Arc::clone(&self.terminal)
    }

    pub fn send_text_input(&self, text: String) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::TextInput(text))
            .map_err(|_| anyhow!("ssh runtime text input channel is closed"))
    }

    pub fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::KeyInput(event))
            .map_err(|_| anyhow!("ssh runtime key input channel is closed"))
    }

    pub fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Resize { rows, cols })
            .map_err(|_| anyhow!("ssh runtime resize channel is closed"))
    }

    pub fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::MouseInput(event))
            .map_err(|_| anyhow!("ssh runtime mouse input channel is closed"))
    }

    pub fn send_paste(&self, text: String) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Paste(text))
            .map_err(|_| anyhow!("ssh runtime paste channel is closed"))
    }

    pub fn update_theme_mode(&self, mode: ThemeMode) -> Result<TerminalSurfaceState> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for theme update"))?;
        terminal.set_theme_mode(mode);
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for local scrollback"))?;
        terminal.scroll_viewport_lines(delta);
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        let terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for surface snapshot"))?;
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn disconnect(&self) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Disconnect)
            .map_err(|_| anyhow!("ssh runtime disconnect channel is closed"))
    }
}

impl SessionRuntimeControl for SshSessionRuntime {
    fn disconnect(&self) -> Result<()> {
        SshSessionRuntime::disconnect(self)
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        SshSessionRuntime::send_text_input(self, text)
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        SshSessionRuntime::send_key_input(self, event)
    }

    fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        SshSessionRuntime::resize(self, rows, cols)
    }

    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        SshSessionRuntime::send_mouse_input(self, event)
    }

    fn send_paste(&self, text: String) -> Result<()> {
        SshSessionRuntime::send_paste(self, text)
    }

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::terminal_surface(self)
    }

    fn update_theme_mode(&self, mode: ThemeMode) -> Result<Option<TerminalSurfaceState>> {
        SshSessionRuntime::update_theme_mode(self, mode).map(Some)
    }

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::scroll_viewport_lines(self, delta)
    }

    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        Some(self.sftp_runtime.clone())
    }
}

async fn authenticate_client(
    handle: &mut client::Handle<RuntimeClientHandler>,
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    match profile.auth_method {
        SshAuthMethod::Password => {
            let password = match profile
                .password
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                Some(password) => password,
                None => {
                    let stored_bundle = load_required_stored_secret_bundle(
                        profile,
                        credential_store,
                        "SSH password secret",
                    )?;
                    require_profile_secret_field(
                        profile,
                        "SSH password secret",
                        stored_bundle.as_ref(),
                        "password",
                    )?
                }
            };
            let auth_result = handle
                .authenticate_password(profile.user.clone(), password)
                .await
                .context("password authentication failed")?;
            ensure_auth_success(auth_result, "password")?;
        }
        SshAuthMethod::PrivateKeyPath => {
            let private_key_path = profile
                .private_key_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| anyhow!("missing private key path for `{}`", profile.name))?;
            let stored_bundle = if profile
                .passphrase
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                None
            } else {
                load_optional_stored_secret_bundle(profile, credential_store).map_err(|err| {
                    anyhow!(stored_secret_lookup_message(
                        profile,
                        "SSH passphrase secret",
                        &err,
                    ))
                })?
            };
            let passphrase = profile
                .passphrase
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    stored_bundle
                        .as_ref()
                        .and_then(|(_, bundle)| non_empty_secret(bundle.passphrase.as_deref()))
                });
            let private_key = keys::load_secret_key(private_key_path, passphrase.as_deref())
                .with_context(|| {
                    format!("failed to load SSH private key from `{private_key_path}`")
                })?;
            let auth_result = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(private_key),
                        handle.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
                .context("private key path authentication failed")?;
            ensure_auth_success(auth_result, "private key path")?;
        }
        SshAuthMethod::PrivateKeyContent => {
            let stored_bundle = if profile
                .private_key_content
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                load_optional_stored_secret_bundle(profile, credential_store).map_err(|err| {
                    anyhow!(stored_secret_lookup_message(
                        profile,
                        "SSH inline private key secret",
                        &err,
                    ))
                })?
            } else {
                load_required_stored_secret_bundle(
                    profile,
                    credential_store,
                    "SSH inline private key secret",
                )?
            };
            let private_key_content = match profile
                .private_key_content
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                Some(private_key_content) => private_key_content,
                None => require_profile_secret_field(
                    profile,
                    "SSH inline private key secret",
                    stored_bundle.as_ref(),
                    "private_key_content",
                )?,
            };
            let passphrase = profile
                .passphrase
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    stored_bundle
                        .as_ref()
                        .and_then(|(_, bundle)| non_empty_secret(bundle.passphrase.as_deref()))
                });
            let private_key = keys::decode_secret_key(&private_key_content, passphrase.as_deref())
                .context("failed to decode inline SSH private key")?;
            let auth_result = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(private_key),
                        handle.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
                .context("inline private key authentication failed")?;
            ensure_auth_success(auth_result, "inline private key")?;
        }
    }

    Ok(())
}

fn ensure_auth_success(result: AuthResult, method: &str) -> Result<()> {
    if result.success() {
        Ok(())
    } else {
        bail!("SSH authentication was rejected for {method}")
    }
}

pub(crate) fn load_optional_stored_secret_bundle(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> std::result::Result<Option<(String, StoredSshSecretBundle)>, StoredSecretLookupError> {
    let Some(credential_ref) = profile.credential_ref.as_deref() else {
        return Ok(None);
    };

    let bundle = load_secret_bundle_with_diagnostics(credential_store, Some(credential_ref))?;
    let bundle = match profile.auth_method {
        SshAuthMethod::Password => bundle,
        SshAuthMethod::PrivateKeyContent
            if bundle
                .private_key_content
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()) =>
        {
            bundle
        }
        SshAuthMethod::PrivateKeyContent => StoredSshSecretBundle {
            private_key_content: bundle.password,
            passphrase: bundle.passphrase,
            proxy_socks5_password: None,
            ..StoredSshSecretBundle::default()
        },
        SshAuthMethod::PrivateKeyPath => bundle,
    };
    Ok(Some((credential_ref.to_string(), bundle)))
}

fn load_required_stored_secret_bundle(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
    secret_label: &str,
) -> Result<Option<(String, StoredSshSecretBundle)>> {
    load_optional_stored_secret_bundle(profile, credential_store)
        .map_err(|err| anyhow!(stored_secret_lookup_message(profile, secret_label, &err)))
}

fn require_profile_secret_field(
    profile: &ConnectionProfile,
    secret_label: &str,
    stored_bundle: Option<&(String, StoredSshSecretBundle)>,
    field: &'static str,
) -> Result<String> {
    let Some((credential_ref, bundle)) = stored_bundle else {
        return Err(anyhow!(stored_secret_lookup_message(
            profile,
            secret_label,
            &StoredSecretLookupError::MissingCredentialRef,
        )));
    };

    required_secret_bundle_field(bundle, credential_ref, field)
        .map_err(|err| anyhow!(stored_secret_lookup_message(profile, secret_label, &err)))
}

pub(crate) fn stored_secret_lookup_message(
    profile: &ConnectionProfile,
    secret_label: &str,
    error: &StoredSecretLookupError,
) -> String {
    match error {
        StoredSecretLookupError::MissingCredentialRef => format!(
            "missing credential binding for {secret_label} on `{}`",
            profile.name
        ),
        StoredSecretLookupError::MissingEntry { credential_ref } => format!(
            "missing saved entry `{credential_ref}` for {secret_label} on `{}`",
            profile.name
        ),
        StoredSecretLookupError::ReadFailed {
            credential_ref,
            message,
        } => format!(
            "failed to read saved entry `{credential_ref}` for {secret_label} on `{}`: {message}",
            profile.name
        ),
        StoredSecretLookupError::EmptyBundleField {
            credential_ref,
            field,
        } => format!(
            "saved entry `{credential_ref}` for `{}` is missing field `{field}` required by {secret_label}",
            profile.name
        ),
    }
}

fn non_empty_secret(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

async fn await_channel_success(
    channel: &mut Channel<client::Msg>,
    request_label: &str,
    pending_output: &mut Vec<u8>,
) -> Result<()> {
    loop {
        let Some(message) = channel.wait().await else {
            bail!("SSH channel closed before `{request_label}` completed");
        };

        match message {
            ChannelMsg::Success => return Ok(()),
            ChannelMsg::Failure => bail!("SSH channel rejected `{request_label}` request"),
            ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                pending_output.extend_from_slice(data.as_ref());
            }
            ChannelMsg::Close | ChannelMsg::Eof => {
                bail!("SSH channel closed during `{request_label}` request");
            }
            _ => {}
        }
    }
}

pub fn negotiated_terminal_environment() -> [(&'static str, &'static str); 1] {
    [("COLORTERM", "truecolor")]
}

async fn negotiate_terminal_environment(
    channel: &mut Channel<client::Msg>,
    pending_output: &mut Vec<u8>,
) {
    for (variable_name, variable_value) in negotiated_terminal_environment() {
        if let Err(err) = channel.set_env(true, variable_name, variable_value).await {
            tracing::warn!(
                variable_name,
                variable_value,
                error = %err,
                "failed to send negotiated terminal environment request",
            );
            continue;
        }

        let request_label = format!("env {variable_name}");
        if let Err(err) = await_channel_success(channel, &request_label, pending_output).await {
            tracing::warn!(
                variable_name,
                variable_value,
                error = %err,
                "SSH server rejected negotiated terminal environment request",
            );
        }
    }
}

async fn run_channel_pump(
    session_id: Uuid,
    handle: Arc<client::Handle<RuntimeClientHandler>>,
    mut channel: Channel<client::Msg>,
    terminal: Arc<Mutex<TerminalSession>>,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    mut command_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
    _transport_chain_guard: TransportChainGuard,
) {
    let mut command_channel_open = true;
    let mut dirty_notifier = SurfaceDirtyNotifier::default();
    let mut dirty_timer: Option<std::pin::Pin<Box<Sleep>>> = None;
    let mut working_set_trim_scheduler = WorkingSetTrimScheduler::default();
    let mut working_set_trim_timer: Option<std::pin::Pin<Box<Sleep>>> = None;

    loop {
        tokio::select! {
            maybe_command = command_rx.recv(), if command_channel_open => {
                match maybe_command {
                    Some(RuntimeCommand::TextInput(text)) => {
                        let bytes = text.into_bytes();
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::KeyInput(event)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.send_key_event(event) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode key input for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for key input".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} key bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::MouseInput(event)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.send_mouse_input(event) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode mouse input for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for mouse input".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} mouse bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Paste(text)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.encode_paste(&text) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode paste for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for paste".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} paste bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Resize { rows, cols }) => {
                        if let Ok(mut terminal) = terminal.lock() {
                            terminal.resize(rows as usize, cols as usize);
                        }
                        if let Some(surface) = snapshot_terminal_surface(&terminal, session_id) {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
                        }
                        if let Err(err) = channel
                            .window_change(cols, rows, cols.saturating_mul(8), rows.saturating_mul(16))
                            .await
                        {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to resize SSH PTY: {err}"
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Disconnect) => {
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        let _ = handle
                            .disconnect(Disconnect::ByApplication, "session closed", "en-US")
                            .await;
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    None => {
                        command_channel_open = false;
                    }
                }
            }
            maybe_message = channel.wait() => {
                match maybe_message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let parsed = runtime_shell_events(data.as_ref());
                        if let Some(cwd) = parsed.cwd {
                            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd));
                        }
                        if !parsed.sanitized_bytes.is_empty() {
                            apply_remote_output(&terminal, &parsed.sanitized_bytes);
                            working_set_trim_scheduler.record_output(parsed.sanitized_bytes.len());
                            working_set_trim_timer =
                                Some(Box::pin(sleep(WORKING_SET_TRIM_IDLE_INTERVAL)));
                            if dirty_notifier.record_output() {
                                dirty_timer = Some(Box::pin(sleep(SURFACE_DIRTY_NOTIFICATION_INTERVAL)));
                            }
                        }
                    }
                    Some(ChannelMsg::Close) | Some(ChannelMsg::Eof) | None => {
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    Some(ChannelMsg::Failure) => {
                        if dirty_notifier.take_pending() {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                        }
                        let _ = event_tx.send(SessionRuntimeEvent::Error(
                            "remote SSH channel reported failure".into()
                        ));
                        break;
                    }
                    Some(_) => {}
                }
            }
            () = async { if let Some(timer) = dirty_timer.as_mut() { timer.await } }, if dirty_timer.is_some() => {
                dirty_timer = None;
                if dirty_notifier.flush_due() {
                    let _ = event_tx.send(SessionRuntimeEvent::SurfaceDirty);
                }
            }
            () = async { if let Some(timer) = working_set_trim_timer.as_mut() { timer.await } }, if working_set_trim_timer.is_some() => {
                working_set_trim_timer = None;
                if working_set_trim_scheduler.trim_due() {
                    crate::app::memory::trim_process_working_set();
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct SurfaceDirtyNotifier {
    dirty: bool,
    notification_armed: bool,
}

impl SurfaceDirtyNotifier {
    fn record_output(&mut self) -> bool {
        self.dirty = true;
        if self.notification_armed {
            false
        } else {
            self.notification_armed = true;
            true
        }
    }

    fn flush_due(&mut self) -> bool {
        self.notification_armed = false;
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }

    fn take_pending(&mut self) -> bool {
        self.notification_armed = false;
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Default)]
struct WorkingSetTrimScheduler {
    pending_output_bytes: usize,
}

impl WorkingSetTrimScheduler {
    fn record_output(&mut self, bytes: usize) {
        self.pending_output_bytes = self.pending_output_bytes.saturating_add(bytes);
    }

    fn trim_due(&mut self) -> bool {
        let should_trim = self.pending_output_bytes >= WORKING_SET_TRIM_MIN_OUTPUT_BYTES;
        self.pending_output_bytes = 0;
        should_trim
    }
}

fn apply_remote_output(terminal: &Arc<Mutex<TerminalSession>>, bytes: &[u8]) {
    if let Ok(mut terminal) = terminal.lock() {
        terminal.apply_remote_bytes(bytes);
    }
}

pub fn extract_current_working_directory_from_osc7(bytes: &[u8]) -> Option<String> {
    const PREFIX: &[u8] = b"\x1b]7;file://";

    let start = bytes
        .windows(PREFIX.len())
        .position(|window| window == PREFIX)?;
    let payload_start = start + PREFIX.len();
    let payload_end = bytes[payload_start..]
        .iter()
        .position(|byte| *byte == 0x07)
        .map(|offset| payload_start + offset)
        .or_else(|| {
            bytes[payload_start..]
                .windows(2)
                .position(|window| window == b"\x1b\\")
                .map(|offset| payload_start + offset)
        })?;

    let payload = std::str::from_utf8(&bytes[payload_start..payload_end]).ok()?;
    let path_start = payload.find('/')?;
    let decoded = percent_decode_path(&payload[path_start..])?;

    if decoded.starts_with('/') {
        Some(decoded)
    } else {
        None
    }
}

fn percent_decode_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            let value = (hex_value(high)? << 4) | hex_value(low)?;
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn snapshot_terminal_surface(
    terminal: &Arc<Mutex<TerminalSession>>,
    session_id: Uuid,
) -> Option<TerminalSurfaceState> {
    terminal
        .lock()
        .ok()
        .map(|terminal| terminal.surface_state(session_id))
}

pub struct TerminalSession {
    terminal: Terminal,
    config: Arc<SessionTerminalConfig>,
    writer: SharedWriteBuffer,
    fallback_mouse_button: Option<TerminalMouseButton>,
    pending_remote_line_buffer: PendingRemoteLineBuffer,
    pending_paste_highlight_filter: Option<PendingPasteHighlightFilter>,
    keyboard_modes: TerminalKeyboardModes,
    mouse_modes: TerminalMouseModes,
    viewport_offset_lines: usize,
}

impl TerminalSession {
    pub fn new(rows: usize, cols: usize) -> Self {
        let writer = SharedWriteBuffer::default();
        let config = Arc::new(SessionTerminalConfig::new(ThemeMode::Dark));
        let terminal = Terminal::new(
            TerminalSize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
                dpi: 96,
            },
            config.clone(),
            "MicaTerm",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer.clone()),
        );

        Self {
            terminal,
            config,
            writer,
            fallback_mouse_button: None,
            pending_remote_line_buffer: PendingRemoteLineBuffer::default(),
            pending_paste_highlight_filter: None,
            keyboard_modes: TerminalKeyboardModes::default(),
            mouse_modes: TerminalMouseModes::default(),
            viewport_offset_lines: 0,
        }
    }

    pub fn sequence_number(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        let filtered = self.pending_remote_line_buffer.push_and_filter(bytes);
        let filtered = if let Some(filter) = self.pending_paste_highlight_filter.as_mut() {
            let filtered = filter.filter(filtered.as_slice());
            if filter.is_finished() {
                self.pending_paste_highlight_filter = None;
            }
            filtered
        } else {
            filtered
        };
        self.keyboard_modes.observe(filtered.as_slice());
        self.mouse_modes.observe(filtered.as_slice());
        if !filtered.is_empty() {
            let was_at_bottom = self.viewport_offset_lines == 0;
            let previous_total_rows = self.terminal.screen().scrollback_rows();
            self.terminal.advance_bytes(filtered.as_slice());
            if !was_at_bottom {
                let next_total_rows = self.terminal.screen().scrollback_rows();
                let appended_rows = next_total_rows.saturating_sub(previous_total_rows);
                self.viewport_offset_lines =
                    self.viewport_offset_lines.saturating_add(appended_rows);
            }
        }
        self.clamp_viewport_offset();
    }

    pub fn screen_text(&self) -> String {
        self.visible_lines().join("\n")
    }

    pub fn visible_rows(&self) -> Vec<TerminalRowState> {
        let size = self.terminal.get_size();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let mut rows = Vec::with_capacity(size.rows.max(1));
        self.terminal.screen().for_each_phys_line(|phys_idx, line| {
            if phys_idx < visible_start || phys_idx >= visible_end {
                return;
            }

            rows.push(project_terminal_row(
                line,
                (phys_idx - visible_start) as u32,
                size.cols.max(1),
            ));
        });

        while rows.len() < size.rows.max(1) {
            rows.push(TerminalRowState {
                index: rows.len() as u32,
                text: String::new(),
                wrapped: false,
            });
        }

        rows
    }

    pub fn visible_lines(&self) -> Vec<String> {
        visible_lines_from_rows(&self.visible_rows())
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.terminal.resize(TerminalSize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: cols.max(1) * 8,
            pixel_height: rows.max(1) * 16,
            dpi: 96,
        });
        self.clamp_viewport_offset();
    }

    pub fn surface_state(&self, session_id: Uuid) -> TerminalSurfaceState {
        let size = self.terminal.get_size();
        let palette = self.terminal.palette();
        let preset = preset_for_theme_mode(self.config.theme_mode());
        let visible_rows = self.visible_rows();
        let visible_lines = visible_lines_from_rows(&visible_rows);
        let cells = self.visible_cells(&palette);
        let cursor = self.cursor_state(&palette);
        TerminalSurfaceState {
            session_id,
            seqno: self.sequence_number(),
            rows: size.rows as u32,
            cols: size.cols as u32,
            default_fg_rgba: color_to_rgba_u32(palette.foreground),
            default_bg_rgba: color_to_rgba_u32(palette.background),
            row_bg_even_rgba: 0xff00_0000 | preset.row_band_even,
            row_bg_odd_rgba: 0xff00_0000 | preset.row_band_odd,
            viewport_offset_lines: self.viewport_offset_lines as u32,
            viewport_max_offset_lines: self.max_viewport_offset_lines() as u32,
            viewport_at_bottom: self.viewport_offset_lines == 0,
            visible_lines,
            visible_rows,
            cells,
            cursor,
            alternate_screen_active: self.terminal.is_alt_screen_active(),
            mouse_grabbed: self.terminal.is_mouse_grabbed(),
            bracketed_paste_enabled: self.terminal.bracketed_paste_enabled(),
        }
    }

    pub fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
        let key = match event.key {
            TerminalKeyKind::Named(name) => match named_key_code(name) {
                Some(key) => key,
                None => bail!("unsupported named terminal key `{name}`"),
            },
            TerminalKeyKind::Function(number) => KeyCode::Function(number),
            TerminalKeyKind::Char(ch) => KeyCode::Char(ch),
        };

        self.send_key_down(key, key_modifiers(event.alt, event.ctrl, event.shift))
    }

    pub fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        let sanitized = strip_bracketed_paste_markers(text);
        if self.terminal.bracketed_paste_enabled() {
            self.pending_paste_highlight_filter = PendingPasteHighlightFilter::arm(&sanitized);
            return Ok(format!("\x1b[200~{sanitized}\x1b[201~").into_bytes());
        }

        self.pending_paste_highlight_filter = None;
        Ok(sanitized.into_bytes())
    }

    pub fn scroll_viewport_lines(&mut self, delta: i32) {
        if delta > 0 {
            self.viewport_offset_lines = self.viewport_offset_lines.saturating_add(delta as usize);
        } else if delta < 0 {
            self.viewport_offset_lines = self
                .viewport_offset_lines
                .saturating_sub(delta.unsigned_abs() as usize);
        }
        self.clamp_viewport_offset();
    }

    pub fn scroll_viewport_to_top(&mut self) {
        self.viewport_offset_lines = self.max_viewport_offset_lines();
    }

    pub fn scroll_viewport_to_bottom(&mut self) {
        self.viewport_offset_lines = 0;
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        if self.config.set_theme_mode(mode) {
            self.terminal.increment_seqno();
        }
    }

    pub fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        let encoded = key.encode(
            modifiers,
            KeyCodeEncodeModes {
                encoding: KeyboardEncoding::Xterm,
                newline_mode: false,
                application_cursor_keys: self.keyboard_modes.application_cursor_keys,
                modify_other_keys: self.keyboard_modes.modify_other_keys,
            },
            true,
        )?;
        let bytes = encoded.into_bytes();

        let mut writer = self.writer.clone();
        writer.write_all(&bytes)?;
        writer.flush()?;

        Ok(self.writer.take())
    }

    pub fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        let fallback_button = self.resolve_fallback_mouse_button(event);
        self.terminal.mouse_event(wezterm_term::MouseEvent {
            kind: match event.kind {
                TerminalMouseEventKind::Down => wezterm_term::MouseEventKind::Press,
                TerminalMouseEventKind::Up => wezterm_term::MouseEventKind::Release,
                TerminalMouseEventKind::Move => wezterm_term::MouseEventKind::Move,
                TerminalMouseEventKind::Scroll => wezterm_term::MouseEventKind::Press,
            },
            x: event.col as usize,
            y: event.row as i64,
            x_pixel_offset: 0,
            y_pixel_offset: 0,
            button: match event.button {
                TerminalMouseButton::Left => wezterm_term::MouseButton::Left,
                TerminalMouseButton::Middle => wezterm_term::MouseButton::Middle,
                TerminalMouseButton::Right => wezterm_term::MouseButton::Right,
                TerminalMouseButton::WheelUp => wezterm_term::MouseButton::WheelUp(1),
                TerminalMouseButton::WheelDown => wezterm_term::MouseButton::WheelDown(1),
                TerminalMouseButton::None => wezterm_term::MouseButton::None,
            },
            modifiers: mouse_modifiers(event),
        })?;

        let bytes = self.writer.take();
        if !self.terminal.is_mouse_grabbed() {
            return Ok(bytes);
        }

        match event.kind {
            TerminalMouseEventKind::Down | TerminalMouseEventKind::Scroll if !bytes.is_empty() => {
                return Ok(bytes);
            }
            TerminalMouseEventKind::Move
                if matches!(fallback_button, TerminalMouseButton::None)
                    && !self.mouse_modes.any_event_mouse =>
            {
                return Ok(bytes);
            }
            TerminalMouseEventKind::Up if matches!(fallback_button, TerminalMouseButton::None) => {
                return Ok(bytes);
            }
            _ => {}
        }

        Ok(encode_sgr_mouse_fallback(event, fallback_button))
    }

    fn visible_cells(&self, palette: &ColorPalette) -> Vec<TerminalCellState> {
        let size = self.terminal.get_size();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let mut cells = Vec::new();

        self.terminal.screen().for_each_phys_line(|phys_idx, line| {
            if phys_idx < visible_start || phys_idx >= visible_end {
                return;
            }

            let row = (phys_idx - visible_start) as u32;
            for cell in line.visible_cells() {
                if cell.cell_index() >= size.cols {
                    continue;
                }

                let attrs = cell.attrs();
                let (fg_rgba, bg_rgba) = resolve_cell_colors(palette, attrs);
                cells.push(TerminalCellState {
                    row,
                    col: cell.cell_index() as u32,
                    width: cell.width() as u32,
                    text: cell.str().to_string(),
                    bold: matches!(attrs.intensity(), Intensity::Bold),
                    underline: attrs.underline() != Underline::None,
                    fg_rgba,
                    bg_rgba,
                });
            }
        });

        cells
    }

    fn cursor_state(&self, palette: &ColorPalette) -> TerminalCursorState {
        let cursor = self.terminal.cursor_pos();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let cursor_phys = self.terminal.screen().phys_row(cursor.y);
        let cursor_visible = matches!(cursor.visibility, CursorVisibility::Visible)
            && cursor_phys >= visible_start
            && cursor_phys < visible_end;
        TerminalCursorState {
            row: cursor_phys.saturating_sub(visible_start) as u32,
            col: cursor.x as u32,
            visible: cursor_visible,
            blinking: cursor_shape_blinks(cursor.shape),
            shape: project_cursor_shape(cursor.shape),
            fg_rgba: pack_color(palette.cursor_fg),
            bg_rgba: pack_color(palette.cursor_bg),
        }
    }

    fn resolve_fallback_mouse_button(&mut self, event: TerminalMouseInput) -> TerminalMouseButton {
        match event.kind {
            TerminalMouseEventKind::Down => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Move => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Up => {
                let effective = if event.button != TerminalMouseButton::None {
                    event.button
                } else {
                    self.fallback_mouse_button
                        .unwrap_or(TerminalMouseButton::None)
                };
                self.fallback_mouse_button = None;
                effective
            }
            TerminalMouseEventKind::Scroll => event.button,
        }
    }

    fn visible_phys_row_bounds(&self) -> (usize, usize) {
        let size = self.terminal.get_size();
        let visible_rows = size.rows.max(1);
        let visible_start = self
            .terminal
            .screen()
            .scrollback_or_visible_row(-(self.viewport_offset_lines as i32));
        let visible_end = visible_start.saturating_add(visible_rows);
        (visible_start, visible_end)
    }

    fn max_viewport_offset_lines(&self) -> usize {
        let size = self.terminal.get_size();
        self.terminal
            .screen()
            .scrollback_rows()
            .saturating_sub(size.rows.max(1))
    }

    fn clamp_viewport_offset(&mut self) {
        self.viewport_offset_lines = self
            .viewport_offset_lines
            .min(self.max_viewport_offset_lines());
    }

    #[allow(dead_code)]
    fn snap_viewport_to_bottom(&mut self) {
        if self.viewport_offset_lines > 0 {
            self.scroll_viewport_to_bottom();
        }
    }
}

#[derive(Debug, Default)]
struct PendingRemoteLineBuffer {
    bytes: Vec<u8>,
    passthrough_until_newline: bool,
}

impl PendingRemoteLineBuffer {
    fn push_and_filter(&mut self, incoming: &[u8]) -> Vec<u8> {
        let mut forwarded = Vec::with_capacity(incoming.len());

        for &byte in incoming {
            if self.passthrough_until_newline {
                forwarded.push(byte);
                if byte == b'\n' {
                    self.passthrough_until_newline = false;
                }
                continue;
            }

            self.bytes.push(byte);
            if byte == b'\n' {
                if !matches_filtered_exact_banner(&self.bytes) {
                    forwarded.extend_from_slice(&self.bytes);
                }
                self.bytes.clear();
                continue;
            }

            if !matches_filtered_banner_prefix(&self.bytes) {
                forwarded.extend_from_slice(&self.bytes);
                self.bytes.clear();
                self.passthrough_until_newline = true;
            }
        }

        forwarded
    }
}

#[derive(Debug)]
struct PendingPasteHighlightFilter {
    expected_echo: Vec<u8>,
    observed_output: Vec<u8>,
    pending_bytes: Vec<u8>,
    highlight_active: bool,
    finished: bool,
}

impl PendingPasteHighlightFilter {
    const MAX_OBSERVED_BYTES: usize = 4096;

    fn arm(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }

        Some(Self {
            expected_echo: text.as_bytes().to_vec(),
            observed_output: Vec::new(),
            pending_bytes: Vec::new(),
            highlight_active: false,
            finished: false,
        })
    }

    fn filter(&mut self, incoming: &[u8]) -> Vec<u8> {
        if self.finished {
            return incoming.to_vec();
        }

        if !self.pending_bytes.is_empty() {
            self.pending_bytes.extend_from_slice(incoming);
        } else {
            self.pending_bytes = incoming.to_vec();
        }

        let mut output = Vec::with_capacity(self.pending_bytes.len());
        let mut index = 0;

        while index < self.pending_bytes.len() {
            match classify_sgr_sequence(&self.pending_bytes[index..]) {
                SgrSequenceKind::ReverseOn(len) => {
                    self.highlight_active = true;
                    index += len;
                }
                SgrSequenceKind::ReverseOff(len) if self.highlight_active => {
                    self.highlight_active = false;
                    index += len;
                }
                SgrSequenceKind::ReverseOff(len) => {
                    output.extend_from_slice(&self.pending_bytes[index..index + len]);
                    index += len;
                }
                SgrSequenceKind::Other(len) => {
                    output.extend_from_slice(&self.pending_bytes[index..index + len]);
                    index += len;
                }
                SgrSequenceKind::Partial => break,
                SgrSequenceKind::None => {
                    output.push(self.pending_bytes[index]);
                    index += 1;
                }
            }
        }

        self.pending_bytes.drain(..index);
        self.record_output(output.as_slice());
        output
    }

    fn is_finished(&self) -> bool {
        self.finished
    }

    fn record_output(&mut self, output: &[u8]) {
        if output.is_empty() {
            return;
        }

        self.observed_output.extend_from_slice(output);
        if self.observed_output.len() > Self::MAX_OBSERVED_BYTES {
            let drain_len = self.observed_output.len() - Self::MAX_OBSERVED_BYTES;
            self.observed_output.drain(..drain_len);
        }

        if contains_subslice(&self.observed_output, &self.expected_echo)
            || self.observed_output.len() >= self.expected_echo.len().saturating_add(1024)
        {
            self.finished = true;
            self.pending_bytes.clear();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SgrSequenceKind {
    None,
    Partial,
    Other(usize),
    ReverseOn(usize),
    ReverseOff(usize),
}

fn classify_sgr_sequence(bytes: &[u8]) -> SgrSequenceKind {
    if !bytes.starts_with(b"\x1b[") {
        return SgrSequenceKind::None;
    }

    let Some(final_index) = bytes
        .iter()
        .enumerate()
        .skip(2)
        .find_map(|(index, byte)| ((*byte >= 0x40) && (*byte <= 0x7e)).then_some(index))
    else {
        return SgrSequenceKind::Partial;
    };
    if bytes[final_index] != b'm' {
        return SgrSequenceKind::Other(final_index + 1);
    }
    let sequence_len = final_index + 1;
    let params = &bytes[2..final_index];

    if params.is_empty() {
        return SgrSequenceKind::ReverseOff(sequence_len);
    }

    let values = params
        .split(|byte| *byte == b';')
        .map(|value| {
            std::str::from_utf8(value)
                .ok()
                .and_then(|value| value.parse::<i16>().ok())
        })
        .collect::<Option<Vec<_>>>();
    let Some(values) = values else {
        return SgrSequenceKind::Other(sequence_len);
    };

    if values.contains(&7) {
        return SgrSequenceKind::ReverseOn(sequence_len);
    }
    if values.iter().any(|value| matches!(*value, 0 | 27)) {
        return SgrSequenceKind::ReverseOff(sequence_len);
    }

    SgrSequenceKind::Other(sequence_len)
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn strip_bracketed_paste_markers(text: &str) -> String {
    text.replace("\x1b[200~", "").replace("\x1b[201~", "")
}

#[derive(Debug, Default)]
struct TerminalKeyboardModes {
    application_cursor_keys: bool,
    modify_other_keys: Option<i64>,
    trailing_bytes: Vec<u8>,
}

impl TerminalKeyboardModes {
    fn observe(&mut self, incoming: &[u8]) {
        let mut observed = Vec::with_capacity(self.trailing_bytes.len() + incoming.len());
        observed.extend_from_slice(&self.trailing_bytes);
        observed.extend_from_slice(incoming);

        for index in 0..observed.len() {
            let remaining = &observed[index..];
            if remaining.starts_with(b"\x1b[?1h") {
                self.application_cursor_keys = true;
                continue;
            }
            if remaining.starts_with(b"\x1b[?1l") {
                self.application_cursor_keys = false;
                continue;
            }
        }

        const MAX_TRAILING_BYTES: usize = 8;
        if observed.len() <= MAX_TRAILING_BYTES {
            self.trailing_bytes = observed;
        } else {
            self.trailing_bytes = observed[observed.len() - MAX_TRAILING_BYTES..].to_vec();
        }
    }
}

#[derive(Debug, Default)]
struct TerminalMouseModes {
    any_event_mouse: bool,
    trailing_bytes: Vec<u8>,
}

impl TerminalMouseModes {
    fn observe(&mut self, incoming: &[u8]) {
        let mut observed = Vec::with_capacity(self.trailing_bytes.len() + incoming.len());
        observed.extend_from_slice(&self.trailing_bytes);
        observed.extend_from_slice(incoming);

        for index in 0..observed.len() {
            let remaining = &observed[index..];
            if remaining.starts_with(b"\x1b[?1003h") {
                self.any_event_mouse = true;
                continue;
            }
            if remaining.starts_with(b"\x1b[?1003l") {
                self.any_event_mouse = false;
                continue;
            }
        }

        const MAX_TRAILING_BYTES: usize = 10;
        if observed.len() <= MAX_TRAILING_BYTES {
            self.trailing_bytes = observed;
        } else {
            self.trailing_bytes = observed[observed.len() - MAX_TRAILING_BYTES..].to_vec();
        }
    }
}

impl TerminalSurfaceState {
    pub fn signature(&self) -> TerminalSurfaceSignature {
        TerminalSurfaceSignature {
            session_id: self.session_id,
            seqno: self.seqno,
            rows: self.rows,
            cols: self.cols,
            default_fg_rgba: self.default_fg_rgba,
            default_bg_rgba: self.default_bg_rgba,
            row_bg_even_rgba: self.row_bg_even_rgba,
            row_bg_odd_rgba: self.row_bg_odd_rgba,
            viewport_offset_lines: self.viewport_offset_lines,
            viewport_max_offset_lines: self.viewport_max_offset_lines,
            viewport_at_bottom: self.viewport_at_bottom,
            cursor_row: self.cursor.row,
            cursor_col: self.cursor.col,
            cursor_visible: self.cursor.visible,
            cursor_blinking: self.cursor.blinking,
            cursor_shape: self.cursor.shape,
            cursor_fg_rgba: self.cursor.fg_rgba,
            cursor_bg_rgba: self.cursor.bg_rgba,
            alternate_screen_active: self.alternate_screen_active,
            mouse_grabbed: self.mouse_grabbed,
            bracketed_paste_enabled: self.bracketed_paste_enabled,
        }
    }

    pub fn from_visible_lines(
        session_id: Uuid,
        seqno: usize,
        rows: u32,
        cols: u32,
        visible_lines: Vec<String>,
    ) -> Self {
        Self {
            session_id,
            seqno,
            rows,
            cols,
            default_fg_rgba: 0xff00_0000,
            default_bg_rgba: 0xffff_ffff,
            row_bg_even_rgba: 0xffff_ffff,
            row_bg_odd_rgba: 0xffff_ffff,
            viewport_offset_lines: 0,
            viewport_max_offset_lines: 0,
            viewport_at_bottom: true,
            visible_rows: visible_lines
                .iter()
                .enumerate()
                .map(|(index, text)| TerminalRowState {
                    index: index as u32,
                    text: text.clone(),
                    wrapped: false,
                })
                .collect(),
            visible_lines,
            cells: Vec::new(),
            cursor: TerminalCursorState {
                row: 0,
                col: 0,
                visible: false,
                blinking: false,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0xff00_0000,
                bg_rgba: 0xff52_ad70,
            },
            alternate_screen_active: false,
            mouse_grabbed: false,
            bracketed_paste_enabled: false,
        }
    }

    pub fn selection_text(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> String {
        let ((start_row, start_col), (end_row, end_col)) =
            normalized_selection((start_row, start_col), (end_row, end_col));
        let mut text = String::new();

        for row in start_row..=end_row {
            let row_start = if row == start_row { start_col } else { 0 };
            let row_end = if row == end_row {
                end_col
            } else {
                self.cols.saturating_sub(1)
            };
            let mut row_text = String::new();

            for cell in self.cells.iter().filter(|cell| cell.row == row) {
                let cell_start = cell.col;
                let cell_end = cell.col.saturating_add(cell.width.saturating_sub(1));
                if cell_end < row_start || cell_start > row_end {
                    continue;
                }
                row_text.push_str(&cell.text);
            }

            text.push_str(row_text.trim_end_matches(' '));
            let wrapped = self
                .visible_rows
                .iter()
                .find(|visible_row| visible_row.index == row)
                .map(|visible_row| visible_row.wrapped)
                .unwrap_or(false);
            if row < end_row && !wrapped {
                text.push('\n');
            }
        }

        text
    }
}

pub fn encode_named_key_input(
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) -> Result<Option<Vec<u8>>> {
    let Some(key) = named_key_code(key_name) else {
        return Ok(None);
    };

    let mut session = TerminalSession::new(DEFAULT_TERMINAL_ROWS, DEFAULT_TERMINAL_COLS);
    let bytes = session.send_key_down(key, key_modifiers(alt, ctrl, shift))?;
    Ok(Some(bytes))
}

#[derive(Debug)]
struct SessionTerminalConfig {
    state: Mutex<SessionTerminalConfigState>,
}

#[derive(Debug, Clone, Copy)]
struct SessionTerminalConfigState {
    theme_mode: ThemeMode,
    generation: usize,
}

impl SessionTerminalConfig {
    fn new(theme_mode: ThemeMode) -> Self {
        Self {
            state: Mutex::new(SessionTerminalConfigState {
                theme_mode,
                generation: 0,
            }),
        }
    }

    fn set_theme_mode(&self, theme_mode: ThemeMode) -> bool {
        let mut state = self.state.lock().expect("lock session terminal config");
        if state.theme_mode == theme_mode {
            return false;
        }
        state.theme_mode = theme_mode;
        state.generation = state.generation.saturating_add(1);
        true
    }

    fn theme_mode(&self) -> ThemeMode {
        self.state
            .lock()
            .expect("lock session terminal config")
            .theme_mode
    }
}

impl TerminalConfiguration for SessionTerminalConfig {
    fn generation(&self) -> usize {
        self.state
            .lock()
            .expect("lock session terminal config")
            .generation
    }

    fn scrollback_size(&self) -> usize {
        TERMINAL_SCROLLBACK_LINES
    }

    fn color_palette(&self) -> ColorPalette {
        palette_for_theme_mode(self.theme_mode())
    }
}

fn project_terminal_row(line: &Line, index: u32, cols: usize) -> TerminalRowState {
    TerminalRowState {
        index,
        text: line.columns_as_str(0..cols).trim_end().to_string(),
        wrapped: line.last_cell_was_wrapped(),
    }
}

fn visible_lines_from_rows(rows: &[TerminalRowState]) -> Vec<String> {
    let mut lines = rows.iter().map(|row| row.text.clone()).collect::<Vec<_>>();
    while lines.first().is_some_and(String::is_empty) {
        let _ = lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        let _ = lines.pop();
    }
    lines
}

fn matches_filtered_exact_banner(bytes: &[u8]) -> bool {
    normalized_remote_line(bytes) == FILTERED_EXACT_BANNER.as_bytes()
}

fn matches_filtered_banner_prefix(bytes: &[u8]) -> bool {
    let normalized = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    FILTERED_EXACT_BANNER.as_bytes().starts_with(normalized)
}

fn normalized_remote_line(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

fn named_key_code(key_name: &str) -> Option<KeyCode> {
    match key_name {
        "enter" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "escape" => Some(KeyCode::Escape),
        "backspace" => Some(KeyCode::Backspace),
        "insert" => Some(KeyCode::Insert),
        "delete" => Some(KeyCode::Delete),
        "up" => Some(KeyCode::UpArrow),
        "down" => Some(KeyCode::DownArrow),
        "left" => Some(KeyCode::LeftArrow),
        "right" => Some(KeyCode::RightArrow),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "page-up" => Some(KeyCode::PageUp),
        "page-down" => Some(KeyCode::PageDown),
        _ => None,
    }
}

fn key_modifiers(alt: bool, ctrl: bool, shift: bool) -> KeyModifiers {
    let mut modifiers = KeyModifiers::NONE;
    if alt {
        modifiers |= KeyModifiers::ALT;
    }
    if ctrl {
        modifiers |= KeyModifiers::CTRL;
    }
    if shift {
        modifiers |= KeyModifiers::SHIFT;
    }
    modifiers
}

fn resolve_cell_colors(palette: &ColorPalette, attrs: &wezterm_term::CellAttributes) -> (u32, u32) {
    let mut fg = resolve_palette_color(palette, attrs.foreground(), false);
    let mut bg = resolve_palette_color(palette, attrs.background(), true);
    if attrs.reverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if attrs.invisible() {
        fg = bg;
    }
    (fg, bg)
}

fn resolve_palette_color(palette: &ColorPalette, color: ColorAttribute, background: bool) -> u32 {
    let rgba = if background {
        palette.resolve_bg(color)
    } else {
        palette.resolve_fg(color)
    };
    pack_color(rgba)
}

fn color_to_rgba_u32(color: SrgbaTuple) -> u32 {
    pack_color(color)
}

fn pack_color(color: SrgbaTuple) -> u32 {
    let channel = |value: f32| -> u32 { (value.clamp(0.0, 1.0) * 255.0).round() as u32 };
    let r = channel(color.0);
    let g = channel(color.1);
    let b = channel(color.2);
    let a = channel(color.3);
    (a << 24) | (r << 16) | (g << 8) | b
}

fn project_cursor_shape(shape: CursorShape) -> TerminalCursorShape {
    match shape {
        CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline => {
            TerminalCursorShape::Underline
        }
        CursorShape::BlinkingBar | CursorShape::SteadyBar => TerminalCursorShape::Bar,
        CursorShape::Default | CursorShape::BlinkingBlock | CursorShape::SteadyBlock => {
            TerminalCursorShape::Block
        }
    }
}

fn cursor_shape_blinks(shape: CursorShape) -> bool {
    matches!(
        shape,
        CursorShape::Default
            | CursorShape::BlinkingBlock
            | CursorShape::BlinkingUnderline
            | CursorShape::BlinkingBar
    )
}

fn normalized_selection(start: (u32, u32), end: (u32, u32)) -> ((u32, u32), (u32, u32)) {
    if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
        (start, end)
    } else {
        (end, start)
    }
}

fn mouse_modifiers(event: TerminalMouseInput) -> wezterm_term::KeyModifiers {
    let mut modifiers = wezterm_term::KeyModifiers::NONE;
    if event.shift {
        modifiers |= wezterm_term::KeyModifiers::SHIFT;
    }
    if event.ctrl {
        modifiers |= wezterm_term::KeyModifiers::CTRL;
    }
    if event.alt {
        modifiers |= wezterm_term::KeyModifiers::ALT;
    }
    modifiers
}

fn encode_sgr_mouse_fallback(event: TerminalMouseInput, button: TerminalMouseButton) -> Vec<u8> {
    let mut code = match button {
        TerminalMouseButton::Left => 0,
        TerminalMouseButton::Middle => 1,
        TerminalMouseButton::Right => 2,
        TerminalMouseButton::WheelUp => 64,
        TerminalMouseButton::WheelDown => 65,
        TerminalMouseButton::None => 3,
    };
    if event.shift {
        code += 4;
    }
    if event.alt {
        code += 8;
    }
    if event.ctrl {
        code += 16;
    }
    if matches!(event.kind, TerminalMouseEventKind::Move) {
        code += 32;
    }

    format!(
        "\x1b[<{};{};{}{}",
        code,
        event.col + 1,
        event.row + 1,
        if matches!(event.kind, TerminalMouseEventKind::Up) {
            "m"
        } else {
            "M"
        }
    )
    .into_bytes()
}

#[derive(Clone, Debug, Default)]
struct SharedWriteBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedWriteBuffer {
    fn take(&self) -> Vec<u8> {
        let mut buffer = self.inner.lock().expect("lock terminal write buffer");
        std::mem::take(&mut *buffer)
    }
}

impl Write for SharedWriteBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer = self.inner.lock().expect("lock terminal write buffer");
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_client_config_uses_keepalive_without_inactivity_disconnects() {
        let config = ssh_client_config();

        assert_eq!(config.inactivity_timeout, None);
        assert_eq!(config.keepalive_interval, Some(SSH_KEEPALIVE_INTERVAL));
        assert_eq!(config.keepalive_max, SSH_KEEPALIVE_MAX_MISSES);
        assert!(config.nodelay);
    }

    #[test]
    fn visible_lines_from_rows_trims_only_outer_empty_rows() {
        let rows = vec![
            TerminalRowState {
                index: 0,
                text: String::new(),
                wrapped: false,
            },
            TerminalRowState {
                index: 1,
                text: "top".into(),
                wrapped: false,
            },
            TerminalRowState {
                index: 2,
                text: String::new(),
                wrapped: false,
            },
            TerminalRowState {
                index: 3,
                text: "bottom".into(),
                wrapped: false,
            },
            TerminalRowState {
                index: 4,
                text: String::new(),
                wrapped: false,
            },
        ];

        assert_eq!(
            visible_lines_from_rows(&rows),
            vec!["top".to_string(), String::new(), "bottom".to_string()]
        );
    }

    #[test]
    fn surface_dirty_notifier_coalesces_repeated_output_until_flush() {
        let mut notifier = SurfaceDirtyNotifier::default();

        assert!(notifier.record_output());
        assert!(!notifier.record_output());
        assert!(!notifier.record_output());
        assert!(notifier.flush_due());
        assert!(!notifier.flush_due());
    }

    #[test]
    fn surface_dirty_notifier_rearms_after_flush() {
        let mut notifier = SurfaceDirtyNotifier::default();

        assert!(notifier.record_output());
        assert!(notifier.flush_due());
        assert!(notifier.record_output());
        assert!(notifier.take_pending());
        assert!(!notifier.take_pending());
    }

    #[test]
    fn working_set_trim_scheduler_ignores_small_idle_output() {
        let mut scheduler = WorkingSetTrimScheduler::default();

        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 4);

        assert!(!scheduler.trim_due());
        assert!(!scheduler.trim_due());
    }

    #[test]
    fn working_set_trim_scheduler_requests_trim_after_large_idle_output() {
        let mut scheduler = WorkingSetTrimScheduler::default();

        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(WORKING_SET_TRIM_MIN_OUTPUT_BYTES / 2);
        scheduler.record_output(1);

        assert!(scheduler.trim_due());
        assert!(!scheduler.trim_due());
    }
}
