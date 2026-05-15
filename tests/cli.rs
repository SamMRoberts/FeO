use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn feo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_feo"))
}

#[test]
fn init_zsh_prints_hooks() {
    let output = feo().args(["init", "zsh"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("_feo_precmd"));
    assert!(stdout.contains("local feo_status=$?"));
    assert!(stdout.contains("zmodload zsh/datetime"));
    assert!(stdout.contains("_feo_hook_last precmd _feo_precmd"));
}

#[test]
fn init_zsh_is_valid_zsh_syntax() {
    if !zsh_available() {
        return;
    }

    let init = feo().args(["init", "zsh"]).output().unwrap();
    assert!(init.status.success());

    let init = String::from_utf8(init.stdout).unwrap();
    let output = Command::new("zsh")
        .args(["-f", "-n", "-c", &init])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn init_zsh_repairs_prompt_hook_order() {
    if !zsh_available() {
        return;
    }

    let script = r#"
set -e
eval "$(feo init zsh)"

_other_prompt() {
  PROMPT='standard> '
}

add-zsh-hook precmd _other_prompt

_run_precmds() {
  local fn
  local -a hooks
  hooks=("${precmd_functions[@]}")
  for fn in "${hooks[@]}"; do
    "$fn"
  done
}

_run_precmds
[[ "$PROMPT" == "standard> " ]]
_run_precmds
[[ "$PROMPT" != "standard> " ]]
[[ "$PROMPT" == *"%F{"* ]]
[[ "${precmd_functions[-1]}" == "_feo_precmd" ]]
"#;

    let output = Command::new("zsh")
        .args(["-f", "-c", script])
        .env("PATH", path_with_feo_binary())
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn prompt_renders_status_and_duration() {
    let output = feo()
        .args(["prompt", "--status", "7", "--duration-ms", "2500"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("status:7"));
    assert!(stdout.contains("2.5s"));
}

#[test]
fn plugin_source_uses_config_file() {
    let dir = temp_dir("feo-cli-plugin");
    let config = dir.join("config.toml");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &config,
        r#"
[plugins]
enabled = ["git"]
theme = "classic"
"#,
    )
    .unwrap();

    let output = feo()
        .args([
            "--config",
            config.to_str().unwrap(),
            "plugin",
            "source",
            "zsh",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("alias gst="));
    assert!(!stdout.contains("AUTO_PUSHD"));

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tui_dry_run_exits() {
    let output = feo().args(["tui", "--dry-run"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("FeO TUI preview"));
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    if Path::new(&root).exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    root
}

fn zsh_available() -> bool {
    Command::new("zsh").arg("--version").output().is_ok()
}

fn path_with_feo_binary() -> String {
    let bin_dir = PathBuf::from(env!("CARGO_BIN_EXE_feo"))
        .parent()
        .expect("feo binary should have a parent directory")
        .to_path_buf();
    let current_path = env::var_os("PATH").unwrap_or_default();

    env::join_paths(std::iter::once(bin_dir).chain(env::split_paths(&current_path)))
        .expect("PATH entries should be joinable")
        .to_string_lossy()
        .into_owned()
}
