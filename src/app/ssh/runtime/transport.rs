//! SSH runtime transport chain and proxy tunnel helpers.

use std::sync::Arc;

use anyhow::{Context, Result, anyhow, bail};
use russh::client;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::app::ssh::connection_progress::ConnectionHeadlineState;
use crate::app::ssh::credentials::CredentialStore;
use crate::app::ssh::known_hosts::{KnownHostsService, default_known_hosts_path};
use crate::app::ssh::profile::{ConnectionProfile, ResolvedProxyHop};

use super::auth::{
    ConnectionProgressReporter, RuntimeClientHandler, UnknownHostKeyError,
    authenticate_client,
};
use super::{SSH_KEEPALIVE_INTERVAL, SSH_KEEPALIVE_MAX_MISSES};

trait TransportStream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> TransportStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

type BoxedTransportStream = Box<dyn TransportStream>;

#[derive(Default)]
pub(super) struct TransportChainGuard {
    upstream_handles: Vec<client::Handle<RuntimeClientHandler>>,
}

pub(super) fn ssh_client_config() -> client::Config {
    client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(SSH_KEEPALIVE_INTERVAL),
        keepalive_max: SSH_KEEPALIVE_MAX_MISSES,
        nodelay: true,
        ..Default::default()
    }
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

pub(super) async fn connect_target_handle_for_profile(
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
