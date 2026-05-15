# FeO

FeO is a Rust zsh framework MVP inspired by Starship and oh-my-zsh. It combines a fast prompt renderer, generated zsh integration, built-in plugin/theme management, and a custom terminal UI.

## Features

- `feo init zsh` prints shell code for safe `.zshrc` integration.
- `feo prompt` renders a configurable zsh prompt with directory, git, status, and duration modules.
- `feo plugin` manages allow-listed built-in zsh plugins and themes.
- `feo config` shows, validates, and writes TOML configuration.
- `feo doctor` checks config, zsh availability, terminal settings, and plugin state.
- `feo tui` opens a Ratatui-based configuration UI with prompt preview and plugin toggles.

## Install and setup

Build locally:

```sh
cargo build --release
```

Put `target/release/feo` on your `PATH`, then add this to `~/.zshrc`:

```zsh
eval "$(feo init zsh)"
```

Open a new zsh session or run:

```zsh
source ~/.zshrc
```

The generated zsh integration captures the previous command status before running other hooks, tracks command duration with `preexec`/`precmd`, assigns `PROMPT` from `feo prompt`, and loads only enabled built-in plugin snippets.

## Commands

```sh
feo init zsh
feo prompt --status 0 --duration-ms 2500
feo config path
feo config show
feo config write-default
feo config validate
feo plugin list
feo plugin enable git
feo plugin disable dirs
feo plugin theme minimal
feo plugin source zsh
feo doctor
feo tui
```

Use `--config <path>` with any command to override the config path.

## Configuration

By default FeO reads:

```text
$XDG_CONFIG_HOME/feo/config.toml
```

If `XDG_CONFIG_HOME` is not set, it falls back to:

```text
~/.config/feo/config.toml
```

You can also set `FEO_CONFIG` or pass `--config <path>`.

Example:

```toml
schema = 1

[prompt]
modules = ["directory", "git", "status", "duration"]
add_newline = true
show_duration_over_ms = 2000
scan_limit = 2000
character = ">"

[prompt.style]
directory = "cyan"
git = "green"
error = "red"
duration = "yellow"
symbol = "blue"

[plugins]
enabled = ["history", "dirs"]
theme = "classic"

[tui]
preview_on_start = true
```

Prompt text escapes zsh `%` characters in user-controlled values, so branch and directory names cannot accidentally inject prompt escapes. The git module uses a lightweight pure-Rust repository detector and dirty marker heuristic to keep prompt rendering predictable.

## Built-in plugins and themes

Plugins are allow-listed and shipped in the binary for the MVP. FeO does not fetch or source remote plugin code.

Built-in plugins:

- `history`: shared history defaults.
- `dirs`: directory stack options and parent-directory aliases.
- `git`: small git alias set.
- `feo`: convenience aliases for FeO commands.

Built-in themes:

- `classic`: default balanced prompt colors.
- `minimal`: sparse low-noise theme preset.

Changing the theme updates the prompt module list, layout, and colors for that preset.

## TUI

Run:

```sh
feo tui
```

Controls:

- `Up`/`Down`: select a built-in plugin.
- `t`: toggle the selected plugin.
- `m`: cycle the active theme.
- `s`: save config.
- `q` or `Esc`: quit.

For scripts or CI, use:

```sh
feo tui --dry-run
```

## Development

```sh
cargo fmt --all
cargo check
cargo test
```
