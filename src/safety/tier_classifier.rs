//! The three-tier command safety classifier.
//!
//! This is a STATIC pattern-match classifier — not a risk score, not an AI
//! judgment call. Its verdict gates what Rusty is allowed to *suggest and
//! inject*; it is never used to intercept, block, or modify commands the
//! user types themselves.
//!
//! - Tier 1 (read-only): may be suggested and auto-inserted into the input
//!   line; execution always requires the user's own Enter.
//! - Tier 2 (idempotent / low blast radius): may be suggested; requires one
//!   explicit confirmation before execution.
//! - Tier 3 (destructive/irreversible, incl. remote sessions): never
//!   suggested, never auto-completed, never injected — under any
//!   circumstance. Unknown commands default to Tier 2.
//!
//! Zero dependencies on UI/intent code: pure functions, fully unit-tested.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Read-only, no side effects.
    ReadOnly = 1,
    /// State-changing but idempotent / easily reversible.
    Idempotent = 2,
    /// Destructive or irreversible. Never assisted.
    Destructive = 3,
}

/// Classify a full input line. Compound commands (`&&`, `||`, `;`, `|`)
/// are split and the whole line takes the tier of its most dangerous
/// segment: `git status && rm -rf x` is Tier 3.
pub fn classify(command: &str) -> Tier {
    split_compound(command)
        .iter()
        .map(|seg| classify_single(seg))
        .max()
        .unwrap_or(Tier::Idempotent)
}

/// Split a command line on shell chaining/piping operators, respecting
/// single and double quotes.
fn split_compound(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(c);
            },
            '"' if !in_single => {
                in_double = !in_double;
                current.push(c);
            },
            '&' | '|' if !in_single && !in_double => {
                // `&&` / `||` / `|` / `&` all end the current segment.
                if chars.peek() == Some(&c) {
                    chars.next();
                }
                segments.push(current.clone());
                current.clear();
            },
            ';' if !in_single && !in_double => {
                segments.push(current.clone());
                current.clear();
            },
            _ => current.push(c),
        }
    }
    segments.push(current);
    segments
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Tokenize a single command: lowercase, whitespace-split.
fn tokens(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .collect()
}

/// Resolve common PowerShell aliases for destructive cmdlets before
/// matching (`ri`, `rd`, `del`, `erase` → remove-item, etc.).
fn resolve_alias(cmd: &str) -> &str {
    match cmd {
        "ri" | "rd" | "del" | "erase" | "rmdir" => "remove-item",
        "rm" => "rm", // unix rm and PS alias — both destructive anyway
        "gci" | "dir" | "ls" => "get-childitem",
        "gc" | "type" | "cat" => "get-content",
        "gps" | "ps" => "get-process",
        "md" | "mkdir" => "new-item-directory",
        "cp" | "copy" | "cpi" => "copy-item",
        "mv" | "move" | "mi" => "move-item",
        other => other,
    }
}

/// Strip a leading path from a command token (`C:\tools\rm.exe` → `rm.exe`,
/// `./rm` → `rm`) and a trailing `.exe`.
fn base_command(token: &str) -> String {
    let base = token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .trim_end_matches(".exe")
        .to_string();
    base
}

fn classify_single(segment: &str) -> Tier {
    let toks = tokens(segment);
    if toks.is_empty() {
        return Tier::Idempotent;
    }
    let cmd = base_command(&toks[0]);
    let cmd = resolve_alias(&cmd);
    let args: Vec<&str> = toks[1..].iter().map(|s| s.as_str()).collect();

    if is_tier3(cmd, &args) {
        return Tier::Destructive;
    }
    if is_tier1(cmd, &args) {
        return Tier::ReadOnly;
    }
    if is_tier2(cmd, &args) {
        return Tier::Idempotent;
    }
    // Unknown commands default to Tier 2: suggestible only with explicit
    // confirmation.
    Tier::Idempotent
}

fn has_flag(args: &[&str], long: &str, short: Option<&str>) -> bool {
    args.iter().any(|a| {
        *a == long
            || short.is_some_and(|s| *a == s)
            // combined short flags: `-rf`, `-fr`, `-force` style
            || (short.is_some_and(|s| s.len() == 2)
                && a.starts_with('-')
                && !a.starts_with("--")
                && a.contains(short.unwrap().trim_start_matches('-')))
    })
}

/// The static Tier 3 denylist.
fn is_tier3(cmd: &str, args: &[&str]) -> bool {
    // mkfs comes in variants: mkfs.ext4, mkfs.ntfs, ...
    if cmd.starts_with("mkfs") {
        return true;
    }
    match cmd {
        // File deletion, any platform, any spelling.
        "rm" | "remove-item" | "unlink" | "shred" => true,
        // Disk/filesystem destruction.
        "format" | "dd" | "diskpart" | "fdisk" => true,
        // Privilege-holders that commonly wrap destructive intent.
        "sudo" => true,
        // Remote/production hosts: never assisted.
        "ssh" | "enter-pssession" | "new-pssession" | "invoke-command"
        | "scp" | "sftp" => true,
        // Registry editing.
        "reg" if args.first() == Some(&"delete") => true,
        // Process killing en masse.
        "taskkill" | "stop-process" | "kill" => true,
        "git" => {
            let rest = args.join(" ");
            (args.contains(&"push")
                && (args.contains(&"--force")
                    || args.contains(&"-f")
                    || args.contains(&"--force-with-lease")))
                || (args.contains(&"reset") && args.contains(&"--hard"))
                || (args.contains(&"clean")
                    && args.iter().any(|a| {
                        *a == "-f"
                            || *a == "--force"
                            || (a.starts_with('-')
                                && !a.starts_with("--")
                                && a.contains('f'))
                    }))
                || (args.contains(&"branch")
                    && (args.contains(&"-d")
                        || args.contains(&"-D")
                        || args.contains(&"--delete")))
                || rest.starts_with("checkout -- ")
                || (args.contains(&"stash")
                    && (args.contains(&"drop") || args.contains(&"clear")))
        },
        "docker" => {
            (args.contains(&"system") && args.contains(&"prune"))
                || args.first() == Some(&"prune")
                || (args.contains(&"rm")
                    && has_flag(args, "--force", Some("-f")))
                || (args.contains(&"volume") && args.contains(&"rm"))
                || (args.contains(&"rmi"))
                || (args.contains(&"image")
                    && (args.contains(&"rm") || args.contains(&"prune")))
                || (args.contains(&"container")
                    && (args.contains(&"rm") || args.contains(&"prune")))
                || (args.contains(&"network")
                    && (args.contains(&"rm") || args.contains(&"prune")))
        },
        "kubectl" => args.first() == Some(&"delete"),
        _ => false,
    }
}

/// Tier 1 allowlist: read-only commands.
fn is_tier1(cmd: &str, args: &[&str]) -> bool {
    match cmd {
        "get-childitem" | "get-content" | "get-process" | "get-location"
        | "get-nettcpconnection" | "get-item" | "get-itemproperty"
        | "pwd" | "echo" | "write-output" | "hostname" | "whoami"
        | "netstat" | "findstr" | "grep" | "find" | "where"
        | "select-string" | "tree" | "head" | "tail" | "less" | "more"
        | "which" | "ver" | "systeminfo" | "tasklist" | "explorer"
        // Read-only pipeline cmdlets (common tails of a piped read).
        | "select-object" | "where-object" | "sort-object"
        | "format-table" | "format-list" | "measure-object"
        | "group-object" | "out-string" | "ss" | "sed" | "awk"
        | "wc" | "sort" | "uniq" | "cut" => true,
        // `set` with no args lists env vars (cmd); with args it mutates.
        "set" => args.is_empty(),
        "env" | "printenv" => true,
        "git" => matches!(
            args.first(),
            Some(&"status")
                | Some(&"log")
                | Some(&"diff")
                | Some(&"show")
                | Some(&"branch")
                | Some(&"remote")
                | Some(&"stash")
                | Some(&"blame")
                | Some(&"describe")
                | Some(&"rev-parse")
                | Some(&"config")
        ) && !args.contains(&"--delete")
            && !args.contains(&"-d")
            && !args.contains(&"-D")
            && !args.contains(&"drop")
            && !args.contains(&"clear")
            // `git config` with a value assignment mutates; only `--get`/
            // `--list` style is read-only. Be conservative: only allow
            // explicit reads.
            && (args.first() != Some(&"config")
                || args.contains(&"--list")
                || args.contains(&"--get")),
        "docker" => {
            matches!(
                args.first(),
                Some(&"ps") | Some(&"images") | Some(&"version")
                    | Some(&"info") | Some(&"top") | Some(&"stats")
            ) || (args.first() == Some(&"logs"))
                || (args.first() == Some(&"image")
                    && args.get(1) == Some(&"ls"))
                || (args.first() == Some(&"container")
                    && args.get(1) == Some(&"ls"))
                || (args.first() == Some(&"compose")
                    && matches!(args.get(1), Some(&"ps") | Some(&"logs")))
        },
        _ => false,
    }
}

/// Tier 2 allowlist: idempotent / low-blast-radius commands.
fn is_tier2(cmd: &str, args: &[&str]) -> bool {
    match cmd {
        "new-item-directory" | "cd" | "set-location" | "pushd" | "popd"
        | "cls" | "clear" | "copy-item" | "move-item" | "start"
        | "start-process" | "code" | "notepad" => true,
        "new-item" => !args.contains(&"-force"),
        "npm" | "pnpm" | "yarn" => matches!(
            args.first(),
            Some(&"install") | Some(&"ci") | Some(&"run") | Some(&"start")
                | Some(&"test") | Some(&"build") | Some(&"list")
        ),
        "cargo" => matches!(
            args.first(),
            Some(&"build") | Some(&"check") | Some(&"test") | Some(&"run")
                | Some(&"fmt") | Some(&"clippy") | Some(&"doc")
                | Some(&"tree") | Some(&"metadata")
        ),
        "pip" | "pip3" => matches!(
            args.first(),
            Some(&"install") | Some(&"list") | Some(&"show") | Some(&"freeze")
        ),
        "git" => matches!(
            args.first(),
            Some(&"pull") | Some(&"fetch") | Some(&"checkout")
                | Some(&"switch") | Some(&"add") | Some(&"commit")
                | Some(&"merge") | Some(&"init") | Some(&"clone")
                | Some(&"push")
        ) && !args.contains(&"--force")
            && !args.contains(&"-f")
            && !args.contains(&"--force-with-lease")
            && !args.contains(&"--hard")
            // `git checkout -- <path>` discards working changes: Tier 3
            // (handled above); plain checkout of a ref is Tier 2.
            && !args.contains(&"--"),
        "docker" => {
            (args.first() == Some(&"compose")
                && matches!(
                    args.get(1),
                    Some(&"up") | Some(&"start") | Some(&"stop")
                        | Some(&"restart") | Some(&"build") | Some(&"pull")
                ))
                || matches!(
                    args.first(),
                    Some(&"start") | Some(&"stop") | Some(&"restart")
                        | Some(&"pull") | Some(&"build") | Some(&"pause")
                        | Some(&"unpause")
                )
        },
        "wsl" => !args.contains(&"--unregister"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(cmd: &str) -> Tier {
        classify(cmd)
    }

    // ---- Tier 3: the denylist from the brief ----

    #[test]
    fn rm_variants_are_destructive() {
        assert_eq!(t("rm -rf node_modules"), Tier::Destructive);
        assert_eq!(t("rm file.txt"), Tier::Destructive);
        assert_eq!(t("del /q *.tmp"), Tier::Destructive);
        assert_eq!(t("rd /s /q build"), Tier::Destructive);
        assert_eq!(t("rmdir /s dist"), Tier::Destructive);
        assert_eq!(t("Remove-Item -Recurse -Force out"), Tier::Destructive);
        assert_eq!(t("ri -r out"), Tier::Destructive);
        assert_eq!(t("erase important.doc"), Tier::Destructive);
    }

    #[test]
    fn git_destructive_forms() {
        assert_eq!(t("git push --force"), Tier::Destructive);
        assert_eq!(t("git push -f origin main"), Tier::Destructive);
        assert_eq!(t("git push origin main --force"), Tier::Destructive);
        assert_eq!(t("git push --force-with-lease"), Tier::Destructive);
        assert_eq!(t("git reset --hard HEAD~3"), Tier::Destructive);
        assert_eq!(t("git clean -fd"), Tier::Destructive);
        assert_eq!(t("git branch -D feature"), Tier::Destructive);
        assert_eq!(t("git branch --delete feature"), Tier::Destructive);
        assert_eq!(t("git stash drop"), Tier::Destructive);
        assert_eq!(t("git stash clear"), Tier::Destructive);
        assert_eq!(t("git checkout -- ."), Tier::Destructive);
    }

    #[test]
    fn docker_destructive_forms() {
        assert_eq!(t("docker system prune"), Tier::Destructive);
        assert_eq!(t("docker system prune -a"), Tier::Destructive);
        assert_eq!(t("docker rm -f api"), Tier::Destructive);
        assert_eq!(t("docker rm --force api"), Tier::Destructive);
        assert_eq!(t("docker volume rm data"), Tier::Destructive);
        assert_eq!(t("docker rmi old-image"), Tier::Destructive);
        assert_eq!(t("docker image prune"), Tier::Destructive);
        assert_eq!(t("docker container prune"), Tier::Destructive);
    }

    #[test]
    fn kubectl_delete_is_destructive() {
        assert_eq!(t("kubectl delete pod api-1"), Tier::Destructive);
        assert_eq!(t("kubectl delete deployment web"), Tier::Destructive);
    }

    #[test]
    fn disk_and_system_destruction() {
        assert_eq!(t("format d:"), Tier::Destructive);
        assert_eq!(t("mkfs.ext4 /dev/sda1"), Tier::Destructive);
        assert_eq!(t("dd if=/dev/zero of=/dev/sda"), Tier::Destructive);
        assert_eq!(t("diskpart"), Tier::Destructive);
        assert_eq!(t("taskkill /f /im chrome.exe"), Tier::Destructive);
        assert_eq!(t("Stop-Process -Name chrome"), Tier::Destructive);
    }

    #[test]
    fn remote_hosts_never_assisted() {
        assert_eq!(t("ssh prod-server"), Tier::Destructive);
        assert_eq!(t("Enter-PSSession -ComputerName prod"), Tier::Destructive);
        assert_eq!(t("scp file.txt user@host:/tmp"), Tier::Destructive);
        assert_eq!(
            t("Invoke-Command -ComputerName srv -ScriptBlock {ls}"),
            Tier::Destructive
        );
    }

    // ---- Compound commands: max of segments ----

    #[test]
    fn compound_takes_most_dangerous_tier() {
        assert_eq!(t("git status && rm -rf x"), Tier::Destructive);
        assert_eq!(t("rm -rf x && git status"), Tier::Destructive);
        assert_eq!(t("git status; del temp.txt"), Tier::Destructive);
        assert_eq!(t("echo y | del /q *"), Tier::Destructive);
        assert_eq!(t("git status && git pull"), Tier::Idempotent);
        assert_eq!(t("git status && git log"), Tier::ReadOnly);
        assert_eq!(t("ls | grep foo"), Tier::ReadOnly);
    }

    #[test]
    fn quoted_operators_do_not_split() {
        // The && inside quotes is data, not an operator.
        assert_eq!(t("echo \"a && rm -rf x\""), Tier::ReadOnly);
        assert_eq!(t("echo 'del things'"), Tier::ReadOnly);
    }

    // ---- Case-insensitivity & path/extension stripping ----

    #[test]
    fn case_variants_match() {
        assert_eq!(t("RM -RF x"), Tier::Destructive);
        assert_eq!(t("Del temp.txt"), Tier::Destructive);
        assert_eq!(t("GIT PUSH --FORCE"), Tier::Destructive);
        assert_eq!(t("Docker System Prune"), Tier::Destructive);
    }

    #[test]
    fn path_prefixed_commands_match() {
        assert_eq!(t("C:\\bin\\rm.exe -rf x"), Tier::Destructive);
        assert_eq!(t("./rm -rf x"), Tier::Destructive);
        assert_eq!(t("/usr/bin/ssh host"), Tier::Destructive);
    }

    // ---- Tier 1 ----

    #[test]
    fn read_only_commands() {
        assert_eq!(t("git status"), Tier::ReadOnly);
        assert_eq!(t("git log --oneline -10"), Tier::ReadOnly);
        assert_eq!(t("git diff HEAD~1"), Tier::ReadOnly);
        assert_eq!(t("git branch"), Tier::ReadOnly);
        assert_eq!(t("docker ps -a"), Tier::ReadOnly);
        assert_eq!(t("docker logs api --tail 50"), Tier::ReadOnly);
        assert_eq!(t("docker images"), Tier::ReadOnly);
        assert_eq!(t("docker image ls"), Tier::ReadOnly);
        assert_eq!(t("ls -la"), Tier::ReadOnly);
        assert_eq!(t("dir"), Tier::ReadOnly);
        assert_eq!(t("Get-ChildItem"), Tier::ReadOnly);
        assert_eq!(t("cat file.txt"), Tier::ReadOnly);
        assert_eq!(t("type file.txt"), Tier::ReadOnly);
        assert_eq!(t("netstat -ano"), Tier::ReadOnly);
        assert_eq!(t("Get-NetTCPConnection -LocalPort 3000"), Tier::ReadOnly);
        assert_eq!(t("Get-Process"), Tier::ReadOnly);
        assert_eq!(t("set"), Tier::ReadOnly); // bare set lists env
        assert_eq!(t("echo hello"), Tier::ReadOnly);
        assert_eq!(t("explorer ."), Tier::ReadOnly);
    }

    #[test]
    fn git_branch_delete_is_not_tier1() {
        assert_eq!(t("git branch -d old"), Tier::Destructive);
        assert_eq!(t("git branch -D old"), Tier::Destructive);
    }

    // ---- Tier 2 ----

    #[test]
    fn idempotent_commands() {
        assert_eq!(t("git pull"), Tier::Idempotent);
        assert_eq!(t("git fetch --all"), Tier::Idempotent);
        assert_eq!(t("git checkout main"), Tier::Idempotent);
        assert_eq!(t("git push"), Tier::Idempotent); // plain push: T2
        assert_eq!(t("docker compose up -d"), Tier::Idempotent);
        assert_eq!(t("docker start api"), Tier::Idempotent);
        assert_eq!(t("docker stop api"), Tier::Idempotent);
        assert_eq!(t("mkdir new-folder"), Tier::Idempotent);
        assert_eq!(t("md new-folder"), Tier::Idempotent);
        assert_eq!(t("cd .."), Tier::Idempotent);
        assert_eq!(t("npm install"), Tier::Idempotent);
        assert_eq!(t("cargo build --release"), Tier::Idempotent);
        assert_eq!(t("set FOO=bar"), Tier::Idempotent); // set with args
    }

    // ---- Unknown commands default to Tier 2 ----

    #[test]
    fn unknown_defaults_to_tier2() {
        assert_eq!(t("some-unknown-tool --flag"), Tier::Idempotent);
        assert_eq!(t("terraform plan"), Tier::Idempotent);
        assert_eq!(t(""), Tier::Idempotent);
    }

    // ---- Edge cases ----

    #[test]
    fn sudo_wrapped_commands_are_destructive() {
        assert_eq!(t("sudo rm -rf /"), Tier::Destructive);
        assert_eq!(t("sudo apt install thing"), Tier::Destructive);
    }

    #[test]
    fn flag_order_does_not_matter() {
        assert_eq!(t("git push -f origin"), Tier::Destructive);
        assert_eq!(t("git push origin -f"), Tier::Destructive);
        assert_eq!(t("docker rm api -f"), Tier::Destructive);
        assert_eq!(t("docker rm -f api"), Tier::Destructive);
    }

    #[test]
    fn read_only_pipelines_stay_tier1() {
        assert_eq!(
            t("Get-NetTCPConnection -LocalPort 3000 | Select-Object -Property LocalPort,State"),
            Tier::ReadOnly
        );
        assert_eq!(
            t("Get-ChildItem Env: | Where-Object Name -like '*PATH*'"),
            Tier::ReadOnly
        );
        assert_eq!(t("docker ps | findstr api"), Tier::ReadOnly);
    }

    #[test]
    fn destructive_tail_of_a_pipeline_still_wins() {
        assert_eq!(
            t("Get-ChildItem *.tmp | Remove-Item"),
            Tier::Destructive
        );
        assert_eq!(t("docker ps -q | docker rm -f"), Tier::Destructive);
    }

    #[test]
    fn multiple_chained_segments() {
        assert_eq!(
            t("cd repo && git pull && cargo build && rm -rf target"),
            Tier::Destructive
        );
        assert_eq!(
            t("cd repo && git pull && cargo build"),
            Tier::Idempotent
        );
    }
}
