//! Shell-integration snippets and OSC 133 mark parsing.
//!
//! Rusty never modifies shell profiles silently: the snippets below are
//! shown to the user with a one-click install offer. Without them, command
//! capture degrades gracefully (input-gate shadow buffer only, no exit
//! codes).

/// A parsed OSC 133 / cwd-report mark from the PTY output stream.
#[derive(Debug, Clone, PartialEq)]
pub enum ShellMark {
    /// 133;A — prompt start
    PromptStart,
    /// 133;B — command input start
    CommandStart,
    /// 133;C — command execution start (pre-exec)
    PreExec,
    /// 133;D;<code> — command finished
    Finished { exit_code: Option<i32> },
    /// OSC 9;9;<cwd> (Windows Terminal convention) or OSC 7;file://host/path
    Cwd(String),
}

/// Parse the body of an OSC sequence (without ESC ] prefix / terminator)
/// into a shell mark, if it is one.
pub fn parse_osc_body(body: &str) -> Option<ShellMark> {
    if let Some(rest) = body.strip_prefix("133;") {
        let mut parts = rest.split(';');
        return match parts.next()? {
            "A" => Some(ShellMark::PromptStart),
            "B" => Some(ShellMark::CommandStart),
            "C" => Some(ShellMark::PreExec),
            "D" => {
                let exit_code = parts.next().and_then(|c| c.parse().ok());
                Some(ShellMark::Finished { exit_code })
            },
            _ => None,
        };
    }
    if let Some(rest) = body.strip_prefix("9;9;") {
        let cwd = rest.trim_matches('"').to_string();
        if !cwd.is_empty() {
            return Some(ShellMark::Cwd(cwd));
        }
        return None;
    }
    if let Some(rest) = body.strip_prefix("7;") {
        // OSC 7: file://hostname/path — used by WSL shells.
        if let Some(path) = rest.strip_prefix("file://") {
            let path = match path.find('/') {
                Some(idx) => &path[idx..],
                None => path,
            };
            if !path.is_empty() {
                return Some(ShellMark::Cwd(path.to_string()));
            }
        }
        return None;
    }
    None
}

/// PowerShell profile snippet: wraps `prompt` to emit OSC 133 marks and an
/// OSC 9;9 cwd report. Offered for one-click install into $PROFILE.
pub const POWERSHELL_SNIPPET: &str = r#"# --- Rusty Terminal shell integration (begin) ---
# Emits OSC 133 prompt marks + OSC 9;9 cwd reports so Rusty Terminal can
# track command boundaries and exit codes. Safe to remove at any time.
$Global:__RustyOriginalPrompt = $function:prompt
function prompt {
    $gle = $Global:LASTEXITCODE
    $ec = if ($? -eq $false -and $null -eq $gle) { 1 } elseif ($null -eq $gle) { 0 } else { $gle }
    $esc = [char]27
    $bel = [char]7
    # D: previous command finished; A: prompt start; 9;9: cwd
    Write-Host -NoNewline "$esc]133;D;$ec$bel$esc]133;A$bel$esc]9;9;`"$($executionContext.SessionState.Path.CurrentLocation.Path)`"$bel"
    $out = & $Global:__RustyOriginalPrompt
    # B: command input starts after the prompt text
    "$out$esc]133;B$bel"
}
# --- Rusty Terminal shell integration (end) ---"#;

/// bash/zsh snippet for WSL shells (~/.bashrc / ~/.zshrc).
pub const BASH_SNIPPET: &str = r#"# --- Rusty Terminal shell integration (begin) ---
__rusty_precmd() {
    local ec=$?
    printf '\e]133;D;%s\a\e]133;A\a\e]7;file://%s%s\a' "$ec" "$(hostname)" "$PWD"
}
__rusty_preexec() {
    printf '\e]133;C\a'
}
if [ -n "$ZSH_VERSION" ]; then
    precmd_functions+=(__rusty_precmd)
    preexec_functions+=(__rusty_preexec)
elif [ -n "$BASH_VERSION" ]; then
    PROMPT_COMMAND="__rusty_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
PS1="${PS1}\[\e]133;B\a\]"
# --- Rusty Terminal shell integration (end) ---"#;

/// cmd.exe has no prompt function; the best available is a PROMPT variable
/// that emits marks ($e expands to ESC). Set via the spawned environment —
/// boundaries only, no exit codes (documented limitation).
pub const CMD_PROMPT_VALUE: &str = "$e]133;D$e\\$e]133;A$e\\$e]9;9;\"$P\"$e\\$P$G$e]133;B$e\\";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prompt_marks() {
        assert_eq!(parse_osc_body("133;A"), Some(ShellMark::PromptStart));
        assert_eq!(parse_osc_body("133;B"), Some(ShellMark::CommandStart));
        assert_eq!(parse_osc_body("133;C"), Some(ShellMark::PreExec));
        assert_eq!(
            parse_osc_body("133;D;0"),
            Some(ShellMark::Finished { exit_code: Some(0) })
        );
        assert_eq!(
            parse_osc_body("133;D;127"),
            Some(ShellMark::Finished {
                exit_code: Some(127)
            })
        );
        assert_eq!(
            parse_osc_body("133;D"),
            Some(ShellMark::Finished { exit_code: None })
        );
    }

    #[test]
    fn parses_cwd_reports() {
        assert_eq!(
            parse_osc_body("9;9;\"C:\\Users\\dev\""),
            Some(ShellMark::Cwd("C:\\Users\\dev".to_string()))
        );
        assert_eq!(
            parse_osc_body("7;file://host/home/dev"),
            Some(ShellMark::Cwd("/home/dev".to_string()))
        );
    }

    #[test]
    fn rejects_unrelated_oscs() {
        assert_eq!(parse_osc_body("0;window title"), None);
        assert_eq!(parse_osc_body("133;Z"), None);
        assert_eq!(parse_osc_body("9;9;"), None);
    }
}
