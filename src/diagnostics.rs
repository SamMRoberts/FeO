use std::{env, path::Path};

use crate::config::{LoadedConfig, resolve_config_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

pub fn run(config_path: Option<&Path>) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();
    let resolved_path = resolve_config_path(config_path);

    match LoadedConfig::load(config_path) {
        Ok(loaded) => {
            checks.push(DiagnosticCheck {
                name: "config",
                status: CheckStatus::Pass,
                detail: if loaded.came_from_disk {
                    format!("loaded {}", loaded.path.display())
                } else {
                    format!("using defaults; no config at {}", loaded.path.display())
                },
            });

            checks.push(DiagnosticCheck {
                name: "plugins",
                status: CheckStatus::Pass,
                detail: format!(
                    "{} enabled built-in plugin(s)",
                    loaded.config.plugins.enabled.len()
                ),
            });
        }
        Err(error) => {
            checks.push(DiagnosticCheck {
                name: "config",
                status: CheckStatus::Fail,
                detail: error.to_string(),
            });
        }
    }

    checks.push(DiagnosticCheck {
        name: "config-dir",
        status: if resolved_path.parent().is_some() {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        detail: resolved_path
            .parent()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "config path has no parent directory".to_string()),
    });

    checks.push(DiagnosticCheck {
        name: "zsh",
        status: if command_in_path("zsh") {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        detail: if command_in_path("zsh") {
            "zsh found in PATH".to_string()
        } else {
            "zsh not found in PATH; init output is still available".to_string()
        },
    });

    let term = env::var("TERM").unwrap_or_else(|_| "unknown".to_string());
    checks.push(DiagnosticCheck {
        name: "terminal",
        status: if term == "dumb" {
            CheckStatus::Warn
        } else {
            CheckStatus::Pass
        },
        detail: format!("TERM={term}"),
    });

    let color_term = env::var("COLORTERM").unwrap_or_default();
    checks.push(DiagnosticCheck {
        name: "truecolor",
        status: if color_term.contains("truecolor") || color_term.contains("24bit") {
            CheckStatus::Pass
        } else {
            CheckStatus::Warn
        },
        detail: if color_term.is_empty() {
            "COLORTERM is not set".to_string()
        } else {
            format!("COLORTERM={color_term}")
        },
    });

    checks
}

pub fn format_checks(checks: &[DiagnosticCheck]) -> String {
    let mut out = String::new();

    for check in checks {
        let status = match check.status {
            CheckStatus::Pass => "pass",
            CheckStatus::Warn => "warn",
            CheckStatus::Fail => "fail",
        };
        out.push_str(&format!("[{status}] {} - {}\n", check.name, check.detail));
    }

    out
}

fn command_in_path(command: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path).any(|entry| {
        let candidate = entry.join(command);
        candidate.is_file()
    })
}
