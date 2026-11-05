use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
pub struct Config {
    #[serde(default = "default_fade_ms")]
    pub fade_ms: u64,
    #[serde(default = "default_unfocused")]
    pub default_unfocused: f32,
    #[serde(default, rename = "program")]
    pub programs: Vec<ProgramCfg>,
}

#[derive(Deserialize, Clone)]
pub struct ProgramCfg {
    pub exe: String,
    pub unfocused: Option<f32>,
}

fn default_fade_ms() -> u64 {
    250
}
fn default_unfocused() -> f32 {
    0.2
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str("").unwrap()
    }
}

const DEFAULT_FILE: &str = r#"# SoundShift config. Volumes are 0.0 - 1.0.
# Use "Reload config" in the tray menu after editing.

fade_ms = 250            # fade duration, 0 = instant
default_unfocused = 0.2  # used when a program has no "unfocused" value

[[program]]
exe = "firefox.exe"      # executable name, case-insensitive
unfocused = 0.1

[[program]]
exe = "chrome.exe"
"#;

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("SoundShift")
}

pub fn path() -> PathBuf {
    dir().join("config.toml")
}

/// Loads the config, creating the default file first if it doesn't exist.
pub fn load() -> Result<Config, String> {
    let p = path();
    if !p.exists() {
        let _ = std::fs::create_dir_all(dir());
        std::fs::write(&p, DEFAULT_FILE).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}:\n{e}", p.display()))
}
