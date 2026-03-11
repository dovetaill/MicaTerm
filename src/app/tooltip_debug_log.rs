use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct TooltipDebugEvent<'a> {
    pub phase: &'a str,
    pub source_id: &'a str,
    pub text: &'a str,
    pub anchor_x: f32,
    pub anchor_y: f32,
}

#[derive(Debug, Clone)]
pub struct TooltipDebugLog {
    directory: PathBuf,
}

impl TooltipDebugLog {
    pub fn in_directory(directory: PathBuf) -> Result<Self> {
        fs::create_dir_all(&directory)?;
        Ok(Self { directory })
    }

    pub fn for_current_dir() -> Result<Self> {
        let base = std::env::current_dir()?;
        Self::in_directory(base.join("logs"))
    }

    pub fn log_path(&self) -> PathBuf {
        self.directory.join("titlebar-tooltip.log")
    }

    pub fn append(&self, event: TooltipDebugEvent<'_>) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.log_path())?;

        writeln!(
            file,
            "phase={} source_id={} text={:?} anchor_x={} anchor_y={}",
            event.phase, event.source_id, event.text, event.anchor_x, event.anchor_y
        )?;

        Ok(())
    }
}
