//! Embedded Redb persistence store for hosts, keychain metadata, snippets, and settings.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::domain::{AppSettings, HostTree, KeychainEntry, SnippetItem};

const KV_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("mica_workspace_kv");

const KEY_HOSTS: &str = "hosts_tree_v1";
const KEY_KEYCHAIN: &str = "keychain_entries_v1";
const KEY_SNIPPETS: &str = "snippets_v1";
const KEY_RECENT_HOSTS: &str = "recent_host_ids_v1";
const KEY_SETTINGS: &str = "app_settings_v1";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root_dir: PathBuf,
    pub db_path: PathBuf,
    pub secrets_dir: PathBuf,
    pub known_hosts_path: PathBuf,
    pub downloads_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        let root_dir = if let Some(override_dir) = env::var_os("MICA_TERM_DATA_DIR") {
            PathBuf::from(override_dir)
        } else if let Some(portable_dir) = detect_portable_dir() {
            portable_dir
        } else {
            let proj = ProjectDirs::from("dev", "MicaTerm", "MicaTermGpui")
                .context("无法解析系统应用数据目录")?;
            proj.data_local_dir().to_path_buf()
        };

        Self::from_root(root_dir)
    }

    pub fn from_root(root_dir: impl AsRef<Path>) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        let secrets_dir = root_dir.join("secrets");
        let downloads_dir = directories::UserDirs::new()
            .and_then(|u| u.download_dir().map(Path::to_path_buf))
            .unwrap_or_else(|| root_dir.join("downloads"));
        fs::create_dir_all(&root_dir)
            .with_context(|| format!("创建数据目录失败 `{}`", root_dir.display()))?;
        fs::create_dir_all(&secrets_dir)?;

        Ok(Self {
            db_path: root_dir.join("workspace.redb"),
            known_hosts_path: root_dir.join("known_hosts"),
            root_dir,
            secrets_dir,
            downloads_dir,
        })
    }
}

fn detect_portable_dir() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let parent = exe.parent()?;
    if parent.join(".mica-term-portable").exists() {
        Some(parent.join("mica-term-data"))
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSnapshot {
    pub hosts: HostTree,
    pub keychain: Vec<KeychainEntry>,
    pub snippets: Vec<SnippetItem>,
    pub recent_host_ids: Vec<String>,
    pub settings: AppSettings,
}

pub struct WorkspaceStore {
    db: Database,
}

impl WorkspaceStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let db = Database::create(path)
            .with_context(|| format!("打开本地数据库失败 `{}`", path.display()))?;
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(KV_TABLE)?;
        }
        write_txn.commit()?;
        Ok(Self { db })
    }

    pub fn load_snapshot(&self) -> Result<WorkspaceSnapshot> {
        let hosts: HostTree = self.read_json(KEY_HOSTS)?.unwrap_or_default();
        let keychain: Vec<KeychainEntry> = self.read_json(KEY_KEYCHAIN)?.unwrap_or_default();
        let snippets: Vec<SnippetItem> = match self.read_json(KEY_SNIPPETS)? {
            Some(items) => items,
            None => default_starter_snippets(),
        };
        let recent_host_ids: Vec<String> = self.read_json(KEY_RECENT_HOSTS)?.unwrap_or_default();
        let settings: AppSettings = self.read_json(KEY_SETTINGS)?.unwrap_or_default();

        Ok(WorkspaceSnapshot {
            hosts,
            keychain,
            snippets,
            recent_host_ids,
            settings,
        })
    }

    pub fn save_hosts(&self, hosts: &HostTree) -> Result<()> {
        self.write_json(KEY_HOSTS, hosts)
    }

    pub fn save_keychain(&self, keychain: &[KeychainEntry]) -> Result<()> {
        self.write_json(KEY_KEYCHAIN, &keychain)
    }

    pub fn save_snippets(&self, snippets: &[SnippetItem]) -> Result<()> {
        self.write_json(KEY_SNIPPETS, &snippets)
    }

    pub fn save_recent_hosts(&self, ids: &[String]) -> Result<()> {
        self.write_json(KEY_RECENT_HOSTS, &ids)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.write_json(KEY_SETTINGS, settings)
    }

    fn read_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(KV_TABLE)?;
        let Some(guard) = table.get(key)? else {
            return Ok(None);
        };
        let value = serde_json::from_slice::<T>(guard.value())?;
        Ok(Some(value))
    }

    fn write_json<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(KV_TABLE)?;
            table.insert(key, bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}

fn default_starter_snippets() -> Vec<SnippetItem> {
    vec![
        SnippetItem::new("系统资源概览", "常用诊断", "uname -a && uptime && df -h", true),
        SnippetItem::new("查看监听端口", "网络排查", "ss -tulnp", true),
        SnippetItem::new("Docker 容器状态", "容器运维", "docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}'", true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::HostNode;

    #[test]
    fn store_persists_and_reloads_snapshot() {
        let dir = std::env::temp_dir().join(format!("mica-gpui-store-{}", uuid::Uuid::new_v4()));
        let paths = AppPaths::from_root(&dir).unwrap();
        let store = WorkspaceStore::open(&paths.db_path).unwrap();

        let mut tree = HostTree::default();
        tree.upsert(HostNode::new_folder("Dev", None));
        store.save_hosts(&tree).unwrap();

        let mut settings = AppSettings::default();
        settings.font_size = 16.0;
        store.save_settings(&settings).unwrap();

        drop(store);

        let reopened = WorkspaceStore::open(&paths.db_path).unwrap();
        let snap = reopened.load_snapshot().unwrap();
        assert_eq!(snap.hosts.nodes.len(), 1);
        assert_eq!(snap.settings.font_size, 16.0);
        assert_eq!(snap.snippets.len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }
}
