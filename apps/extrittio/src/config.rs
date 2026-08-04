use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct CliConfig {
    pub(crate) url: Option<String>,
    pub(crate) token: Option<String>,
}

pub(crate) fn config_path(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    if let Ok(path) = env::var("EXTRITTIO_CLI_CONFIG") {
        return Ok(PathBuf::from(path));
    }

    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(dir).join("extrittio").join("cli.json"));
    }

    let home = env::var("HOME").context("HOME is not set; pass --config explicitly")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("extrittio")
        .join("cli.json"))
}

pub(crate) fn load_config(path: &Path) -> Result<CliConfig> {
    if !path.exists() {
        return Ok(CliConfig::default());
    }

    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

pub(crate) fn save_config(path: &Path, config: &CliConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, format!("{contents}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub(crate) fn normalize_url(url: &str) -> String {
    url.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_url;

    #[test]
    fn normalize_url_removes_trailing_slashes() {
        assert_eq!(
            normalize_url("http://localhost:8080///"),
            "http://localhost:8080"
        );
    }

    #[test]
    fn normalize_url_preserves_urls_without_trailing_slashes() {
        assert_eq!(
            normalize_url("https://hub.example.com/api"),
            "https://hub.example.com/api"
        );
    }
}
