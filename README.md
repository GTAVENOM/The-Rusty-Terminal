# 🦀 Rusty Terminal — Cross-Platform AI Terminal App & Shell Wrapper

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.92%2B-orange.svg)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-blue.svg)](https://github.com/GTAVENOM/The-Rusty-Terminal)
[![Website](https://img.shields.io/badge/Website-rustyterminal.vercel.app-brightgreen.svg)](https://rustyterminal.vercel.app)

> **Plain English → Instant, Safe Shell Commands with Built-in 3-Tier Safety & Sub-1GB Offline AI**

Rusty Terminal is a high-speed, **offline-first AI terminal wrapper and independent terminal app** built for **Windows (PowerShell & `cmd.exe`)** and **macOS (`zsh` & `bash`)**. Seamlessly turn natural English intent into executable shell commands with zero cloud latency or privacy risk.

---

## ⚡ Quick Start & Installation

### 📦 Simple Package Manager Install Commands

#### 1. 📦 npm (Global Install)
```bash
npm i -g rusty-terminal
```

#### 2. ⚡ npx (Run without installing)
```bash
npx rusty-terminal
```

#### 3. 🪟 Windows PowerShell One-Liner
```powershell
irm rustyterminal.vercel.app/i | iex
```

#### 4. 🪟 WinGet (Windows Package Manager)
```cmd
winget install rusty-terminal
```

#### 5. 🍎 macOS / Linux (zsh / bash)
```bash
curl -fsSL rustyterminal.vercel.app/mac | sh
```

---

## 🔥 Key Architectural Features

| Feature | Description | Example Command / Shortcut |
| :--- | :--- | :--- |
| **⚡ Smart Command Alias Generator** | Synthesize custom shortcuts for multi-step terminal tasks stored in SQLite. | `rusty alias build-all "cargo build && npm run build"` |
| **📜 Cross-Shell History Synchronizer** | Search 3-way historical execution logs across PowerShell, CMD, and WSL with safety tier records. | `rusty history git` |
| **🔍 Interactive Dry-Run Sandbox** | Preview generated execution plans and safety tier classifications without running anything on disk. | `rusty dry-run "remove temp build files"` |
| **💡 Live Log Stream Error Healing** | Pattern matches non-zero exit codes & stderr to surface instant 1-line remediation suggestions. | `rusty error-help "python app.py" 1 "ModuleNotFoundError"` |
| **📂 Multi-File Code Gen Pipeline** | Natural language goals generate clean code files directly into `.rusty_scratch/`. | `rusty scratch clean` |
| **🖥️ ANSI Ratatui TUI Overlay** | Full interactive ANSI terminal interface with Settings, AI Chat, and History tabs. | `rusty overlay` (or `Ctrl+Shift+R`) |
| **⌨️ Everywhere Autocomplete** | Live inline ghost-text suggestions via PSReadLine on Windows & zsh/bash on macOS. | `Ctrl+V` |
| **🛡️ 3-Tier Safety Gates** | **Tier 1**: Read-only auto-executes.<br>**Tier 2**: Idempotent prompts `[Y/n]`.<br>**Tier 3**: Destructive commands displayed as read-only text and blocked. | Automatic classification |

---

## 📋 15 Working `rusty "<PROMPT>"` Prompt Examples

```powershell
# 1. Network & IP Configuration
rusty "please show my ip address"

# 2. File System Listing & Sorting
rusty "list the 10 largest files in V:\ drive"

# 3. Dynamic Extension Search
rusty "find all .rs files"

# 4. Directory Target Inspection
rusty "what is inside desktop folder"

# 5. Disk Space & Storage
rusty "how much free disk space is left"

# 6. Active System Processes
rusty "list running processes"

# 7. Network Port Lookup
rusty "find process on port 8080"

# 8. Git Status
rusty "check git status"

# 9. Git Commit History
rusty "show recent commits"

# 10. System Specifications
rusty "show system info"

# 11. Multi-File Code Scaffolding
rusty "please create a html file stating all features"

# 12. Interactive Dry-Run Sandbox
rusty dry-run "remove temporary build files"

# 13. Smart Command Alias Generator
rusty alias dev-build "cargo build && cargo test"

# 14. Cross-Shell History Search
rusty history

# 15. Live Error Diagnosis & Fix
rusty error-help "python app.py" 1 "ModuleNotFoundError: No module named 'requests'"
```

---

## 📁 Repository Structure

```
The-Rusty-Terminal/
├── README.md                 # Main GitHub README (this file)
├── COMMANDS.md               # Complete 15-command prompt reference guide
├── package.json              # NPM package definition for npm i -g rusty-terminal
├── bin/                      # NPM binary installer wrappers
│   ├── install.js
│   └── rusty.js
├── windows/                  # Windows Rust Engine & GUI/TUI codebase
│   ├── Cargo.toml            # Rust cargo manifest
│   ├── src/                  # Core Rust source modules
│   │   ├── bin/rusty_cli.rs  # Main rusty CLI entry point
│   │   ├── execution/        # Command planner & heuristics
│   │   ├── intent/           # Code generation & local GGUF model handler
│   │   ├── safety/           # 3-Tier safety classifier
│   │   ├── error_help/       # Exit code anomaly detector
│   │   ├── learning/         # SQLite history & alias database
│   │   └── ui/               # egui GUI & Ratatui ANSI TUI overlay
│   └── website/              # Public landing page deployed to Vercel
└── macos/                    # macOS zsh/bash integration module
```

---

## 🌐 Public Website

Visit the official website: [rustyterminal.vercel.app](https://rustyterminal.vercel.app)

Run locally:
```powershell
rusty website
```

---

## 📜 License

Distributed under the **MIT License**. See `LICENSE` for more information.
