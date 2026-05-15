use std::{
    io::{self, IsTerminal},
    path::Path,
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use crate::{
    config::LoadedConfig,
    diagnostics,
    plugin::{self, builtin_plugins},
    prompt::{PromptContext, render_prompt},
};

struct App {
    loaded: LoadedConfig,
    selected_plugin: usize,
    message: String,
}

pub fn run(config_path: Option<&Path>, dry_run: bool) -> Result<()> {
    let loaded = LoadedConfig::load(config_path)?;
    let preview = preview_text(&loaded);

    if dry_run || !io::stdout().is_terminal() {
        println!("{preview}");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App {
        loaded,
        selected_plugin: 0,
        message: "Press q to quit, t to toggle plugin, m to cycle theme, s to save config"
            .to_string(),
    };

    loop {
        terminal.draw(|frame| draw(frame, &app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Down => {
                    app.selected_plugin =
                        (app.selected_plugin + 1).min(builtin_plugins().len().saturating_sub(1));
                }
                KeyCode::Up => {
                    app.selected_plugin = app.selected_plugin.saturating_sub(1);
                }
                KeyCode::Char('t') => toggle_selected_plugin(&mut app),
                KeyCode::Char('m') => cycle_theme(&mut app),
                KeyCode::Char('s') => {
                    app.loaded.save()?;
                    app.message = format!("Saved {}", app.loaded.path.display());
                }
                _ => {}
            }
        }
    }

    Ok(())
}

pub fn preview_text(loaded: &LoadedConfig) -> String {
    let context = PromptContext {
        cwd: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        status: 0,
        duration_ms: loaded.config.prompt.show_duration_over_ms,
    };

    format!(
        "FeO TUI preview\nConfig: {}\nPrompt:\n{}\nEnabled plugins: {}\nTheme: {}\n",
        loaded.path.display(),
        render_prompt(&loaded.config, &context),
        if loaded.config.plugins.enabled.is_empty() {
            "none".to_string()
        } else {
            loaded.config.plugins.enabled.join(", ")
        },
        loaded.config.plugins.theme
    )
}

fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            "FeO",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " zsh framework | theme: {}",
            app.loaded.config.plugins.theme
        )),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let context = PromptContext {
        cwd: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        status: 0,
        duration_ms: app.loaded.config.prompt.show_duration_over_ms,
    };
    let preview = Paragraph::new(render_prompt(&app.loaded.config, &context))
        .block(
            Block::default()
                .title("Prompt preview")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(preview, chunks[1]);

    let plugins = builtin_plugins()
        .iter()
        .enumerate()
        .map(|(index, plugin)| {
            let enabled = app
                .loaded
                .config
                .plugins
                .enabled
                .iter()
                .any(|name| name == plugin.name);
            let marker = if enabled { "[x]" } else { "[ ]" };
            let mut item = ListItem::new(format!(
                "{marker} {:<8} {}",
                plugin.name, plugin.description
            ));
            if index == app.selected_plugin {
                item = item.style(Style::default().fg(Color::Yellow));
            }
            item
        })
        .collect::<Vec<_>>();
    let plugin_list = List::new(plugins).block(
        Block::default()
            .title("Built-in plugins")
            .borders(Borders::ALL),
    );
    frame.render_widget(plugin_list, chunks[2]);

    let diagnostics = diagnostics::run(Some(&app.loaded.path));
    let failures = diagnostics
        .iter()
        .filter(|check| matches!(check.status, diagnostics::CheckStatus::Fail))
        .count();
    let footer = Paragraph::new(format!(
        "{} | diagnostics: {} check(s), {} failure(s)",
        app.message,
        diagnostics.len(),
        failures
    ))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[3]);
}

fn toggle_selected_plugin(app: &mut App) {
    let Some(plugin) = builtin_plugins().get(app.selected_plugin) else {
        return;
    };

    let enabled = app
        .loaded
        .config
        .plugins
        .enabled
        .iter()
        .any(|name| name == plugin.name);

    let changed = if enabled {
        plugin::disable_plugin(&mut app.loaded.config, plugin.name)
    } else {
        plugin::enable_plugin(&mut app.loaded.config, plugin.name)
    };

    match changed {
        Ok(true) if enabled => app.message = format!("Disabled {}", plugin.name),
        Ok(true) => app.message = format!("Enabled {}", plugin.name),
        Ok(false) => app.message = format!("No change for {}", plugin.name),
        Err(error) => app.message = error.to_string(),
    }
}

fn cycle_theme(app: &mut App) {
    let themes = plugin::builtin_themes();
    let current = themes
        .iter()
        .position(|theme| theme.name == app.loaded.config.plugins.theme)
        .unwrap_or(0);
    let next = themes[(current + 1) % themes.len()].name;

    match plugin::set_theme(&mut app.loaded.config, next) {
        Ok(true) => app.message = format!("Theme set to {next}"),
        Ok(false) => app.message = format!("Theme {next} already active"),
        Err(error) => app.message = error.to_string(),
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
