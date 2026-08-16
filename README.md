# 🦀 Rusty — an AI-assisted terminal for Windows

Rusty is a tabbed, split-capable terminal emulator for Windows with a
local learning engine and a Claude-powered intent engine. You type
plain English, Rusty suggests the *exact* shell command — it never runs
anything without your own Enter.

Built on [`egui`](https://github.com/emilk/egui) +
[`egui_term`](https://github.com/emilk/egui_term) +
[`alacritty_terminal`](https://github.com/alacritty/alacritty) (vendored,
with a small patch to surface shell-integration marks).

## Features

- **Tabs & split panes** — Ctrl+T new tab, Ctrl+W close, Ctrl+Shift+Pane
  (→/↑/↓/←) to split, Ctrl+Tab / Ctrl+Shift+Tab to cycle.
- **Search** — Ctrl+F to find within a pane's scrollback.
- **Local learning engine** — Rusty watches command boundaries (via OSC
  133) and remembers what you run. Ctrl+R gives a fuzzy history palette;
  run a 2–4 command sequence together often enough and it offers to save
  it as a named **shortcut** (Ctrl+Shift+S to invoke). Shortcut commands
  are inserted one at a time, each requiring your own Enter.
- **Intent engine (Ctrl+K)** — describe what you want ("show me the last
  50 docker logs for the api container") and Rusty maps it to a fixed
  intent, renders the literal command, and — depending on tier —
  inserts it or asks you to confirm.
- **Shell integration** — command boundaries + exit codes via OSC 133.
  PowerShell installs with one click (⚙ in the tab bar); WSL is a
  copy-paste snippet; cmd.exe gets boundary marks automatically via the
  `PROMPT` variable (no exit codes — a cmd limitation).
- **Session restore** — your tabs, splits, shells, and working
  directories come back on next launch (saved on graceful close).
- **Themes** — built-in dark and light, plus custom JSON themes.

## Building

Requirements:

- Rust toolchain (stable). **Windows target must be MSVC or GNU with the
  matching linker.** If you build with the GNU toolchain you'll need a
  MinGW-w64 linker on `PATH` (e.g. WinLibs), because `alacritty_terminal`
  links against Windows console APIs.

```bash
git clone <repo>
cd RustyTerminal
cargo build --release
./target/release/rusty.exe
```

Run tests:

```bash
cargo test
```

## Setting up the intent engine (Ctrl+K)

1. Get an [Anthropic API key](https://console.anthropic.com/).
2. Open Rusty, click **⚙** in the tab bar → paste the key into the
   *Anthropic API key* field → **Save encrypted**.
   The key is stored DPAPI-encrypted under your Windows user account —
   never in the database, never logged. You can also set the
   `ANTHROPIC_API_KEY` environment variable instead.

Then press **Ctrl+K** and describe what you want.

## The safety model (three tiers)

Rusty can *suggest* and *insert* commands, but never runs one without
your explicit keystroke. Every suggestion is classified into one of
three tiers, and the tier is enforced **structurally**:

| Tier | Meaning | What Rusty may do |
|------|---------|-------------------|
| **1 — read-only** | no side effects (`git status`, `docker ps`, `dir`, …) | insert into your input line |
| **2 — idempotent** | changes state but safe to re-run (`git pull`, `docker compose up -d`) | insert **only after you click Run** on a confirmation dialog |
| **3 — destructive/irreversible** | `rm -rf`, `git push --force`, `docker system prune`, `kubectl delete`, `ssh` to remote hosts, `sudo`, `format`, … | **never** suggested, inserted, or completed |

Guard rails:

- The intent **schema** physically has no Tier-3 intents — the model
  cannot express one through it.
- A static **tier classifier** re-checks every rendered command at
  insertion time; anything matching a destructive pattern is refused
  with a banner.
- **Exactly one** pending confirmation at a time — there is no queue, so
  multi-step autonomous execution is impossible.
- The only code path that can append an Enter to a *suggested* command
  consumes a non-cloneable `ConfirmationToken` minted exclusively by the
  confirm dialog's Run button. Circumventing it is a type error.

### Supported intents

| Intent | Example phrase | Tier | Rendered (PowerShell) |
|--------|----------------|------|-----------------------|
| `list_files` | "what's in this folder?" | 1 | `Get-ChildItem` |
| `git_status` | "show git status" | 1 | `git status` |
| `git_log` | "show the last 10 commits" | 1 | `git log --oneline -10` |
| `git_diff` | "what changed since last commit?" | 1 | `git diff HEAD~1` |
| `git_branch_list` | "which branches exist?" | 1 | `git branch` |
| `docker_ps` | "show running containers" | 1 | `docker ps -a` |
| `docker_logs` | "logs for the api container, last 100 lines" | 1 | `docker logs --tail 100 api` |
| `open_folder` | "open this folder in Explorer" | 1 | `explorer .` |
| `find_process_by_port` | "what's using port 3000?" | 1 | `Get-NetTCPConnection -LocalPort 3000 …` |
| `show_env_vars` | "show PATH" | 1 | `Get-ChildItem Env: …` |
| `git_pull` | "pull the latest" | 2 | `git pull origin main` |
| `docker_compose_up` | "start the dev services" | 2 | `docker compose up -d` |

Intents owned by the **Git** and **Docker plugins** are promoted to the
front of the model's tool list when the working directory contains the
matching marker (`.git`, `docker-compose.yml`).

## Shell integration

Rusty tracks command boundaries with OSC 133 marks. Without it, command
history is still captured (via the input gate), but exit codes and
reliable boundary timing are lost.

- **PowerShell** — ⚙ → *Install into PowerShell profile*. Adds a marked
  block to `$PROFILE`. Works for Windows PowerShell 5.x
  (`Documents\WindowsPowerShell\`). PowerShell 7 users should put the
  same snippet in `Documents\PowerShell\profile.ps1`.
- **WSL / bash / zsh** — copy the bash snippet from ⚙ into your
  `~/.bashrc` / `~/.zshrc` inside WSL (Rusty doesn't write into the WSL
  filesystem).
- **cmd.exe** — boundary marks are injected via the `PROMPT` env var;
  no exit codes (cmd doesn't expose them).

## Keyboard shortcuts

| Keys | Action |
|------|--------|
| `Ctrl+T` | new tab |
| `Ctrl+W` | close tab / pane |
| `Ctrl+Shift+→/↑/↓/←` | split pane in that direction |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | next / previous tab |
| `Ctrl+1..9` | jump to tab |
| `Ctrl+F` | search scrollback |
| `Ctrl+R` | fuzzy command-history palette |
| `Ctrl+K` | natural-language intent overlay |
| `Ctrl+Shift+S` | shortcut palette |

## Themes

⚙ → theme section toggles built-in **Dark** / **Light**. For a custom
theme, write `%APPDATA%\RustyTerminal\theme.json`:

```json
{
  "name": "solarized-dark",
  "accent": "#268bd2",
  "tab_bar_bg": "#002b36",
  "mode": "dark",
  "palette": [
    "#073642", "#dc322f", "#859900", "#b58900",
    "#268bd2", "#d33682", "#2aa198", "#eee8d5",
    "#586e75", "#cb4b16", "#859900", "#b58900",
    "#268bd2", "#6c71c4", "#2aa198", "#fdf6e3"
  ]
}
```

`accent` and `tab_bar_bg` are required; `palette` (ANSI 16) and `mode`
are optional (`mode` defaults to `dark`).

## Project layout

```
src/
  app.rs                App wiring: hotkeys, dialogs, event drains, safety gate
  config/theme.rs       Dark/light + custom JSON themes
  context/scanner.rs    Static project-marker detection (cwd + 2 parents)
  intent/               Intent schema, renderer, Anthropic client, DPAPI key storage
  learning/             SQLite learning DB, sequence tracker, shortcuts, OSC parser
  plugins/              git + docker plugins (intent ownership, relevance)
  safety/tier_classifier.rs  Static 3-tier command classifier (pure, unit-tested)
  session/restore.rs    Tab/pane layout save & restore
  terminal/             PTY backend, panes, shells, input gate
  ui/                   Tabs, panes, search, history palette, shortcuts, overlays, settings
```

## Data & privacy

- Learning data lives in `%APPDATA%\RustyTerminal\rusty.db` (SQLite,
  WAL mode).
- The API key is DPAPI-encrypted at `%APPDATA%\RustyTerminal\api_key.bin`
  (or read from `ANTHROPIC_API_KEY`).
- Session layout is saved to `%APPDATA%\RustyTerminal\session.json`.
- Intent requests send your phrase, current working directory, shell,
  and detected project markers to `api.anthropic.com`. Nothing else.
