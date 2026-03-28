//! Recursive SSH proxy-chain resolution for runtime-ready connection profiles.

use std::collections::HashSet;

use anyhow::{Context, bail};

use crate::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, ResolvedProxyHop};
use crate::shell::assets::AssetTree;

pub fn resolve_proxy_chain(
    tree: &AssetTree,
    profile: &ConnectionProfile,
    max_depth: usize,
) -> anyhow::Result<Vec<ResolvedProxyHop>> {
    let mut visited = HashSet::new();
    if let Some(asset_id) = profile.asset_id.as_deref() {
        visited.insert(asset_id.to_string());
    }

    let mut hops = Vec::new();
    resolve_proxy_profile(tree, &profile.proxy, max_depth, &mut visited, &mut hops)?;
    Ok(hops)
}

fn resolve_proxy_profile(
    tree: &AssetTree,
    proxy: &ConnectionProxyProfile,
    max_depth: usize,
    visited: &mut HashSet<String>,
    hops: &mut Vec<ResolvedProxyHop>,
) -> anyhow::Result<()> {
    match proxy {
        ConnectionProxyProfile::None => Ok(()),
        ConnectionProxyProfile::Socks5 {
            host,
            port,
            username,
            password,
            ..
        } => {
            ensure_proxy_chain_capacity(hops, max_depth)?;
            hops.push(ResolvedProxyHop::Socks5 {
                host: host.clone(),
                port: *port,
                username: username.clone(),
                password: password.clone(),
            });
            Ok(())
        }
        ConnectionProxyProfile::SshAsset { asset_id } => {
            if !visited.insert(asset_id.clone()) {
                bail!("SSH proxy chain contains a cycle");
            }

            let result = (|| {
                let Some(node) = tree.node(asset_id) else {
                    bail!("upstream SSH asset `{asset_id}` was not found");
                };
                let Some(spec) = tree.ssh_connection_spec(asset_id) else {
                    bail!("upstream SSH asset `{asset_id}` was not found");
                };
                let upstream = ConnectionProfile::from_saved_asset(asset_id, &node.title, spec)
                    .with_context(|| {
                        format!("failed to normalize upstream SSH asset `{asset_id}`")
                    })?;

                resolve_proxy_profile(tree, &upstream.proxy, max_depth, visited, hops)?;
                ensure_proxy_chain_capacity(hops, max_depth)?;
                hops.push(ResolvedProxyHop::Ssh(Box::new(ConnectionProfile {
                    resolved_proxy_hops: Vec::new(),
                    ..upstream
                })));
                Ok(())
            })();

            visited.remove(asset_id);
            result
        }
    }
}

fn ensure_proxy_chain_capacity(hops: &[ResolvedProxyHop], max_depth: usize) -> anyhow::Result<()> {
    if hops.len() >= max_depth {
        bail!("SSH proxy chain is too deep");
    }
    Ok(())
}
