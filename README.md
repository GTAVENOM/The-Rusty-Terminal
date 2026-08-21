# 🦀 Rusty Terminal — Cross-Platform AI Terminal App & Shell Engine

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)
[![Website](https://img.shields.io/badge/Website-rustyterminal.vercel.app-brightgreen.svg)](https://rustyterminal.vercel.app)
[![GTAVENOM Repo](https://img.shields.io/badge/GitHub-GTAVENOM%2FThe--Rusty--Terminal-blue.svg)](https://github.com/GTAVENOM/The-Rusty-Terminal)
[![KrishkkT Repo](https://img.shields.io/badge/GitHub-KrishkkT%2Ftherustyterminal-purple.svg)](https://github.com/KrishkkT/therustyterminal)

> **Plain English → Instant Shell Commands with Built-in 3-Tier Safety & Sub-1GB Offline AI**

Rusty Terminal is a high-speed, **offline-first AI terminal wrapper and independent terminal app** built for **Windows (PowerShell & `cmd.exe`)** and **macOS (`zsh` & `bash`)**. Seamlessly convert natural English intent into executable shell commands with zero cloud latency or privacy risks.

---

## 👥 Authors & Maintainers

| Author / Maintainer | Role | GitHub Profile |
| :--- | :--- | :--- |
| **GTAVENOM** | Core Architect & Public Release Repository Lead | [@GTAVENOM](https://github.com/GTAVENOM) |
| **KrishkkT** | Engine Developer & Web Deployment Maintainer | [@KrishkkT](https://github.com/KrishkkT) |

* **Official Website**: [rustyterminal.vercel.app](https://rustyterminal.vercel.app)
* **Main Public GitHub Repository**: [GTAVENOM/The-Rusty-Terminal](https://github.com/GTAVENOM/The-Rusty-Terminal)
* **Development & Deployment Repository**: [KrishkkT/therustyterminal](https://github.com/KrishkkT/therustyterminal)

---

## 🚀 Simple 1-Line Installation Commands

| Method | OS / Shell | Command |
| :--- | :--- | :--- |
| **📦 npm** | Cross-Platform | `npm i -g rusty-terminal` |
| **⚡ npx** | Instant Run | `npx rusty-terminal` |
| **🪟 WinGet** | Windows | `winget install rusty-terminal` |
| **🪟 PowerShell** | Windows | `irm rustyterminal.vercel.app/i \| iex` |
| **🍎 macOS / Linux** | zsh / bash | `curl -fsSL rustyterminal.vercel.app/mac \| sh` |

---

## 🔥 Key Architectural Features

1. **⚡ Smart Command Alias Generator**: Synthesize custom shortcuts for multi-step terminal tasks stored in SQLite (`rusty alias build-all "cargo build && npm run build"`).
2. **📜 Cross-Shell History Synchronizer**: Search unified execution logs across PowerShell, CMD, and WSL with safety tier records (`rusty history git`).
3. **🔍 Interactive Dry-Run Sandbox**: Preview generated execution plans and safety tier classifications without running anything on disk (`rusty dry-run "<prompt>"`).
4. **💡 Live Log Stream Error Healing**: Pattern matches non-zero exit codes & stderr to surface instant 1-line fix suggestions (`rusty error-help`).
5. **📂 Multi-File Code Generation Pipeline**: Natural language goals generate clean code files directly into `.rusty_scratch/` (`rusty scratch clean`).
6. **🖥️ ANSI Ratatui TUI Overlay**: Press **Ctrl+Shift+R** or run `rusty overlay` to launch the interactive ANSI terminal interface with Settings, AI Chat, and History.
7. **⌨️ Everywhere Autocomplete**: Live inline ghost-text suggestions via PSReadLine on Windows & zsh/bash on macOS (`Ctrl+V`).
8. **🛡️ 3-Tier Safety Gates**: Auto-executes Tier 1 Read-Only commands, prompts `[Y/n]` for Tier 2 Idempotent commands, and blocks Tier 3 Destructive commands.

---

## 📋 15 Working `rusty "<PROMPT>"` Prompt Examples

```powershell
# 1. Network & IP Configuration
rusty "please show my ip address"

# 2. Drive & File Storage Search
rusty "list the 10 largest files in V:\ drive"

# 3. Dynamic Pattern Search
rusty "find all .rs files"

# 4. Folder Inspection
rusty "what is inside desktop folder"

# 5. Storage Inspection
rusty "how much free disk space is left"

# 6. Active System Processes
rusty "list running processes"

# 7. Network Port Lookup
rusty "find process on port 8080"

# 8. Git Status
rusty "check git status"

# 9. Git Commit History
rusty "show recent commits"

# 10. System Information
rusty "show system info"

# 11. Multi-File Code Scaffolding
rusty "please create a html file stating all features"

# 12. Interactive Dry-Run Sandbox
rusty dry-run "remove temporary build files"

# 13. Smart Command Alias Generator
rusty alias dev-build "cargo build && cargo test"

# 14. Cross-Shell History Search
rusty history

# 15. Live Error Diagnosis & Remediation
rusty error-help "python app.py" 1 "ModuleNotFoundError: No module named 'requests'"
```

---

## 📁 Repository Structure Overview

### 1. `GTAVENOM / The-Rusty-Terminal` (Public Release Repo)
* `windows/`: Windows engine source code & submodules.
* `package.json`: Global npm package configuration.
* `README.md`: Project overview & installation guide.

### 2. `KrishkkT / therustyterminal` (Development & Website Repo)
* `src/`: Core Rust CLI engine, heuristics, safety gates, and TUI.
* `website/`: Public landing page deployed at `rustyterminal.vercel.app`.
* `scripts/`: PowerShell & CMD installer scripts.

---

## 📜 License

Distributed under the **MIT License**. Created & Maintained by **GTAVENOM** & **KrishkkT**.
