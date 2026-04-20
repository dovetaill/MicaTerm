//! Persisted SFTP transfer task store backed by `redb`.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use redb::{Database, ReadableTable, TableDefinition};

use super::queue::{TransferTask, TransferTaskState};

const TRANSFER_STORE_SCHEMA_VERSION: u64 = 1;
const METADATA_SCHEMA_VERSION_KEY: &str = "schema_version";

const METADATA_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("sftp_transfer_metadata");
const TASKS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("sftp_transfer_tasks");

pub struct RedbTransferStore {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl RedbTransferStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            database_path: data_dir.join("transfers.redb"),
            data_dir,
        }
    }

    pub fn load_tasks(&self) -> Result<Vec<TransferTask>> {
        if !self.database_path.exists() {
            return Ok(Vec::new());
        }

        let database = self.open_database()?;
        let read_txn = database.begin_read()?;
        let metadata = read_txn.open_table(METADATA_TABLE)?;
        let tasks_table = read_txn.open_table(TASKS_TABLE)?;

        let schema_version = metadata
            .get(METADATA_SCHEMA_VERSION_KEY)?
            .map(|value| decode_schema_version(value.value()))
            .transpose()?
            .unwrap_or(TRANSFER_STORE_SCHEMA_VERSION);
        if schema_version > TRANSFER_STORE_SCHEMA_VERSION {
            bail!(
                "transfer store schema {} is newer than supported schema {}",
                schema_version,
                TRANSFER_STORE_SCHEMA_VERSION
            );
        }

        let mut tasks = Vec::new();
        for entry in tasks_table.iter()? {
            let (_, value) = entry?;
            tasks.push(decode_task(value.value())?);
        }

        Ok(tasks)
    }

    pub fn save_tasks(&self, tasks: &[TransferTask]) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        let database = self.open_database()?;
        let write_txn = database.begin_write()?;

        {
            let mut metadata = write_txn.open_table(METADATA_TABLE)?;
            let schema_version = encode_schema_version(TRANSFER_STORE_SCHEMA_VERSION);
            metadata.insert(METADATA_SCHEMA_VERSION_KEY, schema_version.as_slice())?;
        }

        {
            let mut task_table = write_txn.open_table(TASKS_TABLE)?;
            let existing_ids = task_table
                .iter()?
                .map(|entry| entry.map(|(key, _)| key.value().to_string()))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for task_id in existing_ids {
                task_table.remove(task_id.as_str())?;
            }

            for task in tasks {
                let encoded = encode_task(task)?;
                task_table.insert(task.id.as_str(), encoded.as_slice())?;
            }
        }

        write_txn.commit()?;
        Ok(())
    }

    pub fn clear_completed(&self) -> Result<()> {
        if !self.database_path.exists() {
            return Ok(());
        }

        let database = self.open_database()?;
        let write_txn = database.begin_write()?;
        {
            let mut task_table = write_txn.open_table(TASKS_TABLE)?;
            let removable_ids = task_table
                .iter()?
                .filter_map(|entry| entry.ok())
                .filter_map(|(key, value)| {
                    let task = decode_task(value.value()).ok()?;
                    matches!(task.state, TransferTaskState::Completed | TransferTaskState::Cancelled)
                        .then(|| key.value().to_string())
                })
                .collect::<Vec<_>>();

            for task_id in removable_ids {
                task_table.remove(task_id.as_str())?;
            }
        }

        write_txn.commit()?;
        Ok(())
    }

    fn open_database(&self) -> Result<Database> {
        Database::create(&self.database_path).context("failed to open SFTP transfer database")
    }
}

fn encode_schema_version(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn decode_schema_version(bytes: &[u8]) -> Result<u64> {
    let raw: [u8; 8] = bytes
        .try_into()
        .context("invalid SFTP transfer schema version payload")?;
    Ok(u64::from_le_bytes(raw))
}

fn encode_task(task: &TransferTask) -> Result<Vec<u8>> {
    bincode::serialize(task).context("failed to encode SFTP transfer task")
}

fn decode_task(bytes: &[u8]) -> Result<TransferTask> {
    bincode::deserialize(bytes).context("failed to decode SFTP transfer task")
}
