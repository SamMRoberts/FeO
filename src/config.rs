use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{plugin, prompt};

pub const CONFIG_ENV: &str = "FEO_CONFIG";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub path: PathBuf,
    pub came_from_disk: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "schema_version")]
    pub schema: u32,
    #[serde(default)]
    pub prompt: PromptConfig,
    #[serde(default)]
    pub plugins: PluginConfig,
    #[serde(default)]
    pub tui: TuiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptConfig {
    #[serde(default = "default_prompt_modules")]
    pub modules: Vec<String>,
    #[serde(default = "default_add_newline")]
    pub add_newline: bool,
    #[serde(default = "default_duration_threshold")]
    pub show_duration_over_ms: u64,
    #[serde(default = "default_scan_limit")]
    pub scan_limit: usize,
    #[serde(default = "default_prompt_character")]
    pub character: String,
    #[serde(default)]
    pub style: PromptStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptStyle {
    #[serde(default = "color_cyan")]
    pub directory: String,
    #[serde(default = "color_green")]
    pub git: String,
    #[serde(default = "color_red")]
    pub error: String,
    #[serde(default = "color_yellow")]
    pub duration: String,
    #[serde(default = "color_blue")]
    pub symbol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginConfig {
    #[serde(default = "default_enabled_plugins")]
    pub enabled: Vec<String>,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiConfig {
    #[serde(default = "default_preview_on_start")]
    pub preview_on_start: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            prompt: PromptConfig::default(),
            plugins: PluginConfig::default(),
            tui: TuiConfig::default(),
        }
    }
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            modules: default_prompt_modules(),
            add_newline: default_add_newline(),
            show_duration_over_ms: default_duration_threshold(),
            scan_limit: default_scan_limit(),
            character: default_prompt_character(),
            style: PromptStyle::default(),
        }
    }
}

impl Default for PromptStyle {
    fn default() -> Self {
        Self {
            directory: color_cyan(),
            git: color_green(),
            error: color_red(),
            duration: color_yellow(),
            symbol: color_blue(),
        }
    }
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled_plugins(),
            theme: default_theme(),
        }
    }
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            preview_on_start: default_preview_on_start(),
        }
    }
}

impl LoadedConfig {
    pub fn load(path_override: Option<&Path>) -> Result<Self> {
        let path = resolve_config_path(path_override);

        if !path.exists() {
            let config = Config::default();
            config.validate()?;
            return Ok(Self {
                config,
                path,
                came_from_disk: false,
            });
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let config: Config = toml::from_str(&raw)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;
        config.validate()?;

        Ok(Self {
            config,
            path,
            came_from_disk: true,
        })
    }

    pub fn save(&self) -> Result<()> {
        write_config(&self.path, &self.config)
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA_VERSION {
            bail!(
                "unsupported config schema {}; expected {}",
                self.schema,
                SCHEMA_VERSION
            );
        }

        if self.prompt.modules.is_empty() {
            bail!("prompt.modules must include at least one module");
        }

        for module in &self.prompt.modules {
            if !prompt::is_supported_module(module) {
                bail!("unsupported prompt module `{module}`");
            }
        }

        if self.prompt.scan_limit == 0 {
            bail!("prompt.scan_limit must be greater than zero");
        }

        for (name, color) in [
            ("prompt.style.directory", &self.prompt.style.directory),
            ("prompt.style.git", &self.prompt.style.git),
            ("prompt.style.error", &self.prompt.style.error),
            ("prompt.style.duration", &self.prompt.style.duration),
            ("prompt.style.symbol", &self.prompt.style.symbol),
        ] {
            validate_zsh_color(name, color)?;
        }

        for plugin_name in &self.plugins.enabled {
            if !plugin::is_builtin_plugin(plugin_name) {
                bail!("unsupported plugin `{plugin_name}`");
            }
        }

        if !plugin::is_builtin_theme(&self.plugins.theme) {
            bail!("unsupported theme `{}`", self.plugins.theme);
        }

        Ok(())
    }
}

pub fn resolve_config_path(path_override: Option<&Path>) -> PathBuf {
    if let Some(path) = path_override {
        return path.to_path_buf();
    }

    if let Ok(path) = env::var(CONFIG_ENV)
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }

    if let Ok(config_home) = env::var("XDG_CONFIG_HOME")
        && !config_home.trim().is_empty()
    {
        return PathBuf::from(config_home).join("feo").join("config.toml");
    }

    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("feo")
        .join("config.toml")
}

pub fn write_config(path: &Path, config: &Config) -> Result<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    let raw = toml::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(path, raw).with_context(|| format!("failed to write config at {}", path.display()))
}

pub fn default_config_toml() -> Result<String> {
    toml::to_string_pretty(&Config::default()).context("failed to serialize default config")
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn validate_zsh_color(name: &str, color: &str) -> Result<()> {
    if color.is_empty() {
        bail!("{name} cannot be empty");
    }

    let valid = color
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '#'));
    if !valid {
        bail!("{name} contains characters that are unsafe for zsh prompt escapes");
    }

    Ok(())
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

fn default_prompt_modules() -> Vec<String> {
    ["directory", "git", "status", "duration"]
        .into_iter()
        .map(String::from)
        .collect()
}

fn default_enabled_plugins() -> Vec<String> {
    ["history", "dirs"].into_iter().map(String::from).collect()
}

fn default_add_newline() -> bool {
    true
}

fn default_duration_threshold() -> u64 {
    2_000
}

fn default_scan_limit() -> usize {
    2_000
}

fn default_prompt_character() -> String {
    ">".to_string()
}

fn default_theme() -> String {
    "classic".to_string()
}

fn default_preview_on_start() -> bool {
    true
}

fn color_cyan() -> String {
    "cyan".to_string()
}

fn color_green() -> String {
    "green".to_string()
}

fn color_red() -> String {
    "red".to_string()
}

fn color_yellow() -> String {
    "yellow".to_string()
}

fn color_blue() -> String {
    "blue".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn explicit_config_path_wins() {
        let path = Path::new("/tmp/feo-test/config.toml");
        assert_eq!(resolve_config_path(Some(path)), path);
    }

    #[test]
    fn rejects_unknown_prompt_modules() {
        let mut config = Config::default();
        config.prompt.modules.push("network".to_string());

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("unsupported prompt module"));
    }

    #[test]
    fn rejects_unsafe_color_names() {
        let mut config = Config::default();
        config.prompt.style.directory = "red}%{bad".to_string();

        let error = config.validate().unwrap_err().to_string();
        assert!(error.contains("unsafe"));
    }
}
