//! Chunked SFTP transfer helpers used by the session binding.

use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::{
    SftpRemoteMetadata, SftpRuntimeHandle, TransferResumeMode, TransferTask, TransferTaskAction,
    TransferTaskState, download_part_path,
};

const TRANSFER_CHUNK_SIZE: usize = 64 * 1024;

pub async fn execute_download_task<F>(
    runtime: &SftpRuntimeHandle,
    task: &mut TransferTask,
    on_progress: &mut F,
) -> Result<()>
where
    F: FnMut(&TransferTask),
{
    let target_path = download_target_path(task)?;
    let part_path = task
        .temp_target_path
        .clone()
        .unwrap_or_else(|| download_part_path(target_path.as_path()));
    let resume_offset = existing_file_len(part_path.as_path())?;
    let remote_meta = runtime
        .stat(&task.source_path)
        .await
        .with_context(|| format!("failed to stat `{}` before download", task.source_path))?;

    if resume_offset > 0 {
        task.state = TransferTaskState::VerifyingResume;
        task.temp_target_path = Some(part_path.clone());
        task.bytes_confirmed = resume_offset;
        task.bytes_transferred = resume_offset;
        on_progress(task);
    }

    validate_download_resume(task, resume_offset, &remote_meta)?;

    task.state = TransferTaskState::Running;
    task.temp_target_path = Some(part_path.clone());
    task.bytes_total = remote_meta
        .size_bytes
        .unwrap_or(task.bytes_total.max(resume_offset));
    task.bytes_confirmed = resume_offset;
    task.bytes_transferred = resume_offset;
    on_progress(task);

    if let Some(parent) = part_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create local temporary download directory `{}`",
                parent.display()
            )
        })?;
    }

    let mut reader = runtime
        .open_file_reader(&task.source_path)
        .await
        .with_context(|| {
            format!(
                "failed to open `{}` for resumable download",
                task.source_path
            )
        })?;
    reader
        .seek(SeekFrom::Start(resume_offset))
        .await
        .with_context(|| format!("failed to seek remote source `{}`", task.source_path))?;

    let mut writer = open_local_part_writer(part_path.as_path(), resume_offset)?;
    let mut buffer = vec![0; TRANSFER_CHUNK_SIZE];

    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .with_context(|| format!("failed to read remote source `{}`", task.source_path))?;
        if read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..read])
            .with_context(|| format!("failed to write local part `{}`", part_path.display()))?;
        task.bytes_transferred += read as u64;
        task.bytes_confirmed = task.bytes_transferred;
        on_progress(task);
    }

    writer
        .flush()
        .with_context(|| format!("failed to flush local part `{}`", part_path.display()))?;
    drop(writer);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create local download directory `{}`",
                parent.display()
            )
        })?;
    }
    fs::rename(&part_path, &target_path).with_context(|| {
        format!(
            "failed to finalize download `{}` -> `{}`",
            part_path.display(),
            target_path.display()
        )
    })?;

    task.state = TransferTaskState::Completed;
    task.bytes_confirmed = task.bytes_transferred;
    task.temp_target_path = None;
    task.error_message = None;
    on_progress(task);

    Ok(())
}

fn download_target_path(task: &TransferTask) -> Result<PathBuf> {
    match &task.action {
        TransferTaskAction::Download { local_path } => Ok(local_path.clone()),
        _ => bail!("download engine received unsupported task action"),
    }
}

fn existing_file_len(path: &Path) -> Result<u64> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect local part `{}`", path.display()))
        }
    }
}

fn validate_download_resume(
    task: &mut TransferTask,
    resume_offset: u64,
    remote_meta: &SftpRemoteMetadata,
) -> Result<()> {
    let remote_size = remote_meta.size_bytes.unwrap_or(task.bytes_total);
    if remote_size < resume_offset {
        task.resume_mode = TransferResumeMode::RestartOnly;
        bail!(
            "remote source `{}` is smaller than the local resume checkpoint; restart required",
            task.source_path
        );
    }

    task.resume_mode = TransferResumeMode::ResumeIfPossible;
    Ok(())
}

fn open_local_part_writer(path: &Path, offset: u64) -> Result<std::fs::File> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("failed to open local part `{}`", path.display()))?;
    file.seek(SeekFrom::Start(offset))
        .with_context(|| format!("failed to seek local part `{}`", path.display()))?;
    Ok(file)
}
