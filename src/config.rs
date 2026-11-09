use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub messages: Vec<String>,
    pub messages_url: String,
    pub headless: bool,
    pub browser_args: Vec<String>,
    pub delay_between_ms: (u64, u64),
    pub typing_delay_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            messages: vec![
                "desenrolado".to_string(),
                "orea seca".to_string(),
                "fogo".to_string(),
                "sossegado".to_string(),
                "feijao com farinha".to_string(),
                "moleculas aahh".to_string(),
            ],
            messages_url: "https://www.tiktok.com/messages?lang=en".to_string(),
            headless: false,
            browser_args: Vec::new(),
            delay_between_ms: (1500, 3000),
            typing_delay_ms: 120,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config: {}", path.display()))?;
        let cfg: Self = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config: {}", path.display()))?;
        if cfg.messages.is_empty() {
            anyhow::bail!("config.messages must not be empty");
        }
        Ok(cfg)
    }

    pub fn save_default(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cfg = Self::default();
        let body = toml::to_string_pretty(&cfg).context("failed to serialize default config")?;
        std::fs::write(path, body)
            .with_context(|| format!("failed to write config: {}", path.display()))?;
        Ok(())
    }
}

pub fn data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("TIKTOK_TTSK_DATA_DIR") {
        let p = PathBuf::from(dir);
        std::fs::create_dir_all(&p)?;
        return Ok(p);
    }
    let base = if cfg!(windows) {
        dirs::data_dir()
    } else {
        dirs::data_local_dir().or_else(dirs::data_dir)
    }
    .context("could not determine a data directory")?;
    let dir = base.join("tiktok-ttsk");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
