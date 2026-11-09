use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub last_sent_date: Option<String>,
}

impl State {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read state: {}", path.display()))?;
        let state = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse state: {}", path.display()))?;
        Ok(state)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self).context("failed to serialize state")?;
        std::fs::write(path, raw)
            .with_context(|| format!("failed to write state: {}", path.display()))?;
        Ok(())
    }

    pub fn sent_today(&self) -> bool {
        let Some(last) = &self.last_sent_date else {
            return false;
        };
        today_local() == *last
    }

    pub fn mark_sent_today(&mut self) {
        self.last_sent_date = Some(today_local());
    }
}

pub fn today_local() -> String {
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        now.month() as u8,
        now.day()
    )
}
