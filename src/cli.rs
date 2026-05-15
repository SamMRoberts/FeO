use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use crate::{
    config::{self, LoadedConfig},
    diagnostics, plugin,
    prompt::{PromptContext, render_prompt},
    shell, tui,
};

#[derive(Debug, Parser)]
#[command(
    name = "feo",
    version,
    about = "A Rust zsh framework with prompt, plugins, and a TUI"
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(about = "Print shell initialization code")]
    Init {
        #[command(subcommand)]
        shell: InitShell,
    },
    #[command(about = "Render the configured prompt")]
    Prompt(PromptArgs),
    #[command(about = "Inspect or write configuration")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(about = "Manage built-in plugins and themes")]
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    #[command(about = "Run setup diagnostics")]
    Doctor,
    #[command(about = "Open the configuration TUI")]
    Tui {
        #[arg(long, help = "Print a non-interactive preview and exit")]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
enum InitShell {
    Zsh,
}

#[derive(Debug, clap::Args)]
struct PromptArgs {
    #[arg(
        long,
        default_value_t = 0,
        help = "Exit status of the previous command"
    )]
    status: i32,
    #[arg(
        long,
        default_value_t = 0,
        help = "Previous command duration in milliseconds"
    )]
    duration_ms: u64,
    #[arg(long, value_name = "PATH", help = "Working directory to render for")]
    cwd: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    #[command(about = "Print the resolved config path")]
    Path,
    #[command(about = "Print the loaded config, including defaults")]
    Show,
    #[command(about = "Write a default config file")]
    WriteDefault {
        #[arg(long, help = "Overwrite an existing config file")]
        force: bool,
    },
    #[command(about = "Validate the config file")]
    Validate,
}

#[derive(Debug, Subcommand)]
enum PluginCommand {
    #[command(about = "List built-in plugins and themes")]
    List,
    #[command(about = "Enable a built-in plugin")]
    Enable { name: String },
    #[command(about = "Disable a built-in plugin")]
    Disable { name: String },
    #[command(about = "Set the active built-in theme")]
    Theme { name: String },
    #[command(about = "Print shell code for enabled built-in plugins")]
    Source {
        #[command(subcommand)]
        shell: PluginShell,
    },
}

#[derive(Debug, Subcommand)]
enum PluginShell {
    Zsh,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    run_with_cli(cli)
}

fn run_with_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init {
            shell: InitShell::Zsh,
        } => {
            print!("{}", shell::zsh_init());
        }
        Commands::Prompt(args) => {
            let loaded = LoadedConfig::load(cli.config.as_deref())?;
            let cwd = args
                .cwd
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
            let context = PromptContext {
                cwd,
                status: args.status,
                duration_ms: args.duration_ms,
            };
            print!("{}", render_prompt(&loaded.config, &context));
        }
        Commands::Config { command } => handle_config(cli.config, command)?,
        Commands::Plugin { command } => handle_plugin(cli.config, command)?,
        Commands::Doctor => {
            let checks = diagnostics::run(cli.config.as_deref());
            print!("{}", diagnostics::format_checks(&checks));
            if checks
                .iter()
                .any(|check| matches!(check.status, diagnostics::CheckStatus::Fail))
            {
                bail!("one or more diagnostics failed");
            }
        }
        Commands::Tui { dry_run } => tui::run(cli.config.as_deref(), dry_run)?,
    }

    Ok(())
}

fn handle_config(config_path: Option<PathBuf>, command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Path => {
            println!(
                "{}",
                config::resolve_config_path(config_path.as_deref()).display()
            );
        }
        ConfigCommand::Show => {
            let loaded = LoadedConfig::load(config_path.as_deref())?;
            println!("{}", toml::to_string_pretty(&loaded.config)?);
        }
        ConfigCommand::WriteDefault { force } => {
            let path = config::resolve_config_path(config_path.as_deref());
            if path.exists() && !force {
                bail!(
                    "config already exists at {}; pass --force to overwrite",
                    path.display()
                );
            }

            config::write_config(&path, &config::Config::default())?;
            println!("Wrote {}", path.display());
        }
        ConfigCommand::Validate => {
            let loaded = LoadedConfig::load(config_path.as_deref())?;
            loaded.config.validate()?;
            println!("Config OK: {}", loaded.path.display());
        }
    }

    Ok(())
}

fn handle_plugin(config_path: Option<PathBuf>, command: PluginCommand) -> Result<()> {
    match command {
        PluginCommand::List => {
            let loaded = LoadedConfig::load(config_path.as_deref())?;
            println!("Plugins:");
            for plugin in plugin::builtin_plugins() {
                let enabled = loaded
                    .config
                    .plugins
                    .enabled
                    .iter()
                    .any(|name| name == plugin.name);
                let marker = if enabled { "*" } else { " " };
                println!("  [{marker}] {:<8} {}", plugin.name, plugin.description);
            }

            println!("\nThemes:");
            for theme in plugin::builtin_themes() {
                let marker = if loaded.config.plugins.theme == theme.name {
                    "*"
                } else {
                    " "
                };
                println!("  [{marker}] {:<8} {}", theme.name, theme.description);
            }
        }
        PluginCommand::Enable { name } => {
            let mut loaded = LoadedConfig::load(config_path.as_deref())?;
            let changed = plugin::enable_plugin(&mut loaded.config, &name)?;
            loaded.save()?;
            if changed {
                println!("Enabled plugin `{name}` in {}", loaded.path.display());
            } else {
                println!("Plugin `{name}` was already enabled");
            }
        }
        PluginCommand::Disable { name } => {
            let mut loaded = LoadedConfig::load(config_path.as_deref())?;
            let changed = plugin::disable_plugin(&mut loaded.config, &name)?;
            loaded.save()?;
            if changed {
                println!("Disabled plugin `{name}` in {}", loaded.path.display());
            } else {
                println!("Plugin `{name}` was already disabled");
            }
        }
        PluginCommand::Theme { name } => {
            let mut loaded = LoadedConfig::load(config_path.as_deref())?;
            let changed = plugin::set_theme(&mut loaded.config, &name)?;
            loaded.save()?;
            if changed {
                println!("Set theme `{name}` in {}", loaded.path.display());
            } else {
                println!("Theme `{name}` was already active");
            }
        }
        PluginCommand::Source {
            shell: PluginShell::Zsh,
        } => {
            let loaded = LoadedConfig::load(config_path.as_deref())
                .context("failed to load config for plugin source")?;
            print!("{}", plugin::zsh_source(&loaded.config)?);
        }
    }

    Ok(())
}
