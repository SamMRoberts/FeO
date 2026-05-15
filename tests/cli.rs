use std::{path::Path, process::Command};

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
