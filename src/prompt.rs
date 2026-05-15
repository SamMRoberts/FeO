use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::config::{Config, PromptStyle, home_dir};

const SUPPORTED_MODULES: &[&str] = &["directory", "git", "status", "duration"];

#[derive(Debug, Clone)]
pub struct PromptContext {
    pub cwd: PathBuf,
    pub status: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitInfo {
    branch: String,
    dirty: bool,
    state: Option<String>,
}

pub fn is_supported_module(module: &str) -> bool {
    SUPPORTED_MODULES.contains(&module)
}

pub fn supported_modules() -> &'static [&'static str] {
    SUPPORTED_MODULES
}

pub fn render_prompt(config: &Config, context: &PromptContext) -> String {
    let mut segments = Vec::new();

    for module in &config.prompt.modules {
        let segment = match module.as_str() {
            "directory" => Some(directory_segment(config, &context.cwd)),
            "git" => git_segment(config, &context.cwd),
            "status" => status_segment(config, context.status),
            "duration" => duration_segment(config, context.duration_ms),
            _ => None,
        };

        if let Some(segment) = segment
            && !segment.is_empty()
        {
            segments.push(segment);
        }
    }

    let symbol_color = if context.status == 0 {
        &config.prompt.style.symbol
    } else {
        &config.prompt.style.error
    };
    let symbol = paint(symbol_color, &config.prompt.character);

    if segments.is_empty() {
        return format!("{symbol} ");
    }

    let left = segments.join(" ");
    if config.prompt.add_newline {
        format!("{left}\n{symbol} ")
    } else {
        format!("{left} {symbol} ")
    }
}

pub fn escape_zsh_prompt_text(value: &str) -> String {
    value
        .chars()
        .filter_map(|ch| match ch {
            '%' => Some("%%".to_string()),
            '\n' | '\r' => Some(" ".to_string()),
            '\0' => None,
            _ => Some(ch.to_string()),
        })
        .collect()
}

fn directory_segment(config: &Config, cwd: &Path) -> String {
    paint(&config.prompt.style.directory, &display_directory(cwd))
}

fn git_segment(config: &Config, cwd: &Path) -> Option<String> {
    let git = detect_git(cwd, config.prompt.scan_limit)?;
    let mut value = format!("git:{}", git.branch);

    if let Some(state) = git.state {
        value.push(':');
        value.push_str(&state);
    }

    if git.dirty {
        value.push('*');
    }

    Some(paint(&config.prompt.style.git, &value))
}

fn status_segment(config: &Config, status: i32) -> Option<String> {
    (status != 0).then(|| paint(&config.prompt.style.error, &format!("status:{status}")))
}

fn duration_segment(config: &Config, duration_ms: u64) -> Option<String> {
    (duration_ms >= config.prompt.show_duration_over_ms)
        .then(|| paint(&config.prompt.style.duration, &format_duration(duration_ms)))
}

fn paint(color: &str, value: &str) -> String {
    format!("%F{{{}}}{}%f", color, escape_zsh_prompt_text(value))
}

fn display_directory(cwd: &Path) -> String {
    if let Some(home) = home_dir() {
        if cwd == home {
            return "~".to_string();
        }

        if let Ok(stripped) = cwd.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }

    if cwd.parent().is_none() {
        return cwd.display().to_string();
    }

    cwd.file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| cwd.display().to_string(), ToString::to_string)
}

fn format_duration(duration_ms: u64) -> String {
    if duration_ms < 1_000 {
        return format!("{duration_ms}ms");
    }

    let seconds = duration_ms as f64 / 1_000.0;
    if seconds < 10.0 {
        format!("{seconds:.1}s")
    } else {
        format!("{seconds:.0}s")
    }
}

fn detect_git(cwd: &Path, scan_limit: usize) -> Option<GitInfo> {
    let (worktree_root, git_marker) = find_git_marker(cwd)?;
    let git_dir = resolve_git_dir(&worktree_root, &git_marker)?;
    let branch = read_branch(&git_dir)?;
    let state = read_git_state(&git_dir);
    let dirty = has_worktree_changes_since_index(&worktree_root, &git_dir, scan_limit);

    Some(GitInfo {
        branch,
        dirty,
        state,
    })
}

fn find_git_marker(cwd: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut current = if cwd.is_file() { cwd.parent()? } else { cwd };

    loop {
        let marker = current.join(".git");
        if marker.exists() {
            return Some((current.to_path_buf(), marker));
        }

        current = current.parent()?;
    }
}

fn resolve_git_dir(worktree_root: &Path, marker: &Path) -> Option<PathBuf> {
    if marker.is_dir() {
        return Some(marker.to_path_buf());
    }

    let raw = fs::read_to_string(marker).ok()?;
    let path = raw.strip_prefix("gitdir:")?.trim();
    let git_dir = PathBuf::from(path);

    Some(if git_dir.is_absolute() {
        git_dir
    } else {
        worktree_root.join(git_dir)
    })
}

fn read_branch(git_dir: &Path) -> Option<String> {
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();

    if let Some(reference) = head.strip_prefix("ref: ") {
        return reference
            .strip_prefix("refs/heads/")
            .or_else(|| reference.strip_prefix("refs/tags/"))
            .or(Some(reference))
            .map(ToString::to_string);
    }

    let short = head.chars().take(7).collect::<String>();
    (!short.is_empty()).then_some(short)
}

fn read_git_state(git_dir: &Path) -> Option<String> {
    if git_dir.join("MERGE_HEAD").exists() {
        return Some("merge".to_string());
    }

    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        return Some("rebase".to_string());
    }

    if git_dir.join("CHERRY_PICK_HEAD").exists() {
        return Some("cherry-pick".to_string());
    }

    None
}

fn has_worktree_changes_since_index(root: &Path, git_dir: &Path, scan_limit: usize) -> bool {
    let Some(index_modified) = fs::metadata(git_dir.join("index"))
        .and_then(|metadata| metadata.modified())
        .ok()
    else {
        return false;
    };

    let mut visited = 0;
    tree_has_newer_files(root, index_modified, scan_limit, &mut visited)
}

fn tree_has_newer_files(
    dir: &Path,
    index_modified: SystemTime,
    scan_limit: usize,
    visited: &mut usize,
) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };

    for entry in entries.flatten() {
        if *visited >= scan_limit {
            return false;
        }

        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" || name == "target" {
            continue;
        }

        *visited += 1;

        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        if metadata.is_file() {
            if metadata
                .modified()
                .map(|modified| modified > index_modified)
                .unwrap_or(false)
            {
                return true;
            }
        } else if metadata.is_dir()
            && tree_has_newer_files(&path, index_modified, scan_limit, visited)
        {
            return true;
        }
    }

    false
}

#[allow(dead_code)]
fn _style_is_used_for_docs(_: &PromptStyle) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn escapes_percent_for_zsh_prompt() {
        assert_eq!(escape_zsh_prompt_text("feat/%branch"), "feat/%%branch");
    }

    #[test]
    fn renders_status_and_duration() {
        let config = Config::default();
        let context = PromptContext {
            cwd: PathBuf::from("/tmp/project"),
            status: 2,
            duration_ms: 2_500,
        };

        let prompt = render_prompt(&config, &context);
        assert!(prompt.contains("status:2"));
        assert!(prompt.contains("2.5s"));
        assert!(prompt.ends_with(">%f "));
    }

    #[test]
    fn renders_git_branch_from_head() {
        let root = temp_path("feo-git-test");
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(git_dir.join("index"), "").unwrap();

        let config = Config::default();
        let context = PromptContext {
            cwd: root.clone(),
            status: 0,
            duration_ms: 0,
        };

        let prompt = render_prompt(&config, &context);
        assert!(prompt.contains("git:main"));

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{name}-{}-{unique}", std::process::id()))
    }
}
