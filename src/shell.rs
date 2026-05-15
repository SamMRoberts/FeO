pub fn zsh_init() -> &'static str {
    r#"# FeO zsh initialization
# Source this from ~/.zshrc with:
#   eval "$(feo init zsh)"

if command -v feo >/dev/null 2>&1; then
  autoload -Uz add-zsh-hook

  _feo_preexec() {
    typeset -g FEO_CMD_START="${EPOCHREALTIME:-0}"
  }

  _feo_precmd() {
    local feo_status=$?
    local feo_duration_ms=0

    if [[ -n "${FEO_CMD_START:-}" && -n "${EPOCHREALTIME:-}" ]]; then
      feo_duration_ms=$(printf "%.0f" $(( (${EPOCHREALTIME} - ${FEO_CMD_START}) * 1000 )))
    fi

    unset FEO_CMD_START
    PROMPT="$(feo prompt --status "${feo_status}" --duration-ms "${feo_duration_ms}")"
  }

  add-zsh-hook -d preexec _feo_preexec 2>/dev/null || true
  add-zsh-hook -d precmd _feo_precmd 2>/dev/null || true
  add-zsh-hook preexec _feo_preexec
  add-zsh-hook precmd _feo_precmd

  eval "$(feo plugin source zsh 2>/dev/null)"
fi
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_init_captures_status_before_prompt() {
        let init = zsh_init();

        assert!(init.contains("local feo_status=$?"));
        assert!(init.contains("add-zsh-hook precmd _feo_precmd"));
        assert!(init.contains("add-zsh-hook preexec _feo_preexec"));
        assert!(init.contains("feo plugin source zsh"));
    }
}
