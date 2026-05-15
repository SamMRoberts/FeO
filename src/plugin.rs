use anyhow::{Result, bail};

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinPlugin {
    pub name: &'static str,
    pub description: &'static str,
    pub zsh: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTheme {
    pub name: &'static str,
    pub description: &'static str,
}

const PLUGINS: &[BuiltinPlugin] = &[
    BuiltinPlugin {
        name: "history",
        description: "Safer shared history defaults for interactive zsh sessions.",
        zsh: r#"setopt EXTENDED_HISTORY
setopt HIST_IGNORE_DUPS
setopt HIST_IGNORE_SPACE
setopt SHARE_HISTORY"#,
    },
    BuiltinPlugin {
        name: "dirs",
        description: "Directory stack helpers and concise parent-directory aliases.",
        zsh: r#"setopt AUTO_PUSHD
setopt PUSHD_IGNORE_DUPS
alias ..='cd ..'
alias ...='cd ../..'
alias ....='cd ../../..'"#,
    },
    BuiltinPlugin {
        name: "git",
        description: "Small git alias set for common status, branch, and checkout flows.",
        zsh: r#"alias gst='git status --short'
alias gco='git checkout'
alias gcb='git checkout -b'
alias gl='git log --oneline --decorate --graph -20'"#,
    },
    BuiltinPlugin {
        name: "feo",
        description: "Convenience helpers for reloading and inspecting FeO.",
        zsh: r#"alias feoreload='source ~/.zshrc'
alias feodoctor='feo doctor'"#,
    },
];

const THEMES: &[BuiltinTheme] = &[
    BuiltinTheme {
        name: "classic",
        description: "Balanced colors for the default two-line prompt.",
    },
    BuiltinTheme {
        name: "minimal",
        description: "Sparse prompt intended for low-noise terminal workflows.",
    },
];

pub fn builtin_plugins() -> &'static [BuiltinPlugin] {
    PLUGINS
}

pub fn builtin_themes() -> &'static [BuiltinTheme] {
    THEMES
}

pub fn is_builtin_plugin(name: &str) -> bool {
    PLUGINS.iter().any(|plugin| plugin.name == name)
}

pub fn is_builtin_theme(name: &str) -> bool {
    THEMES.iter().any(|theme| theme.name == name)
}

pub fn enable_plugin(config: &mut Config, name: &str) -> Result<bool> {
    if !is_builtin_plugin(name) {
        bail!("unknown built-in plugin `{name}`");
    }

    if config.plugins.enabled.iter().any(|enabled| enabled == name) {
        return Ok(false);
    }

    config.plugins.enabled.push(name.to_string());
    config.plugins.enabled.sort();
    Ok(true)
}

pub fn disable_plugin(config: &mut Config, name: &str) -> Result<bool> {
    if !is_builtin_plugin(name) {
        bail!("unknown built-in plugin `{name}`");
    }

    let before = config.plugins.enabled.len();
    config.plugins.enabled.retain(|enabled| enabled != name);
    Ok(config.plugins.enabled.len() != before)
}

pub fn set_theme(config: &mut Config, name: &str) -> Result<bool> {
    if !is_builtin_theme(name) {
        bail!("unknown built-in theme `{name}`");
    }

    let changed = config.plugins.theme != name;
    apply_theme(config, name);

    Ok(changed)
}

pub fn apply_theme(config: &mut Config, name: &str) {
    config.plugins.theme = name.to_string();

    match name {
        "classic" => {
            config.prompt.modules = ["directory", "git", "status", "duration"]
                .into_iter()
                .map(String::from)
                .collect();
            config.prompt.add_newline = true;
            config.prompt.character = ">".to_string();
            config.prompt.style.directory = "cyan".to_string();
            config.prompt.style.git = "green".to_string();
            config.prompt.style.error = "red".to_string();
            config.prompt.style.duration = "yellow".to_string();
            config.prompt.style.symbol = "blue".to_string();
        }
        "minimal" => {
            config.prompt.modules = ["directory", "git", "status"]
                .into_iter()
                .map(String::from)
                .collect();
            config.prompt.add_newline = false;
            config.prompt.character = ">".to_string();
            config.prompt.style.directory = "white".to_string();
            config.prompt.style.git = "green".to_string();
            config.prompt.style.error = "red".to_string();
            config.prompt.style.duration = "yellow".to_string();
            config.prompt.style.symbol = "white".to_string();
        }
        _ => {}
    }
}

pub fn zsh_source(config: &Config) -> Result<String> {
    config.validate()?;

    let mut out = String::from("# FeO built-in plugins\n");
    for name in &config.plugins.enabled {
        let Some(plugin) = PLUGINS.iter().find(|plugin| plugin.name == name) else {
            bail!("unknown built-in plugin `{name}`");
        };

        out.push_str("\n# feo plugin: ");
        out.push_str(plugin.name);
        out.push('\n');
        out.push_str(plugin.zsh);
        out.push('\n');
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_only_contains_enabled_plugins() {
        let mut config = Config::default();
        config.plugins.enabled = vec!["git".to_string()];

        let source = zsh_source(&config).unwrap();
        assert!(source.contains("alias gst="));
        assert!(!source.contains("AUTO_PUSHD"));
    }

    #[test]
    fn enable_plugin_is_idempotent() {
        let mut config = Config::default();

        assert!(enable_plugin(&mut config, "git").unwrap());
        assert!(!enable_plugin(&mut config, "git").unwrap());
    }

    #[test]
    fn theme_updates_prompt_preset() {
        let mut config = Config::default();

        assert!(set_theme(&mut config, "minimal").unwrap());
        assert_eq!(config.plugins.theme, "minimal");
        assert!(!config.prompt.add_newline);
        assert!(
            !config
                .prompt
                .modules
                .iter()
                .any(|module| module == "duration")
        );
    }
}
