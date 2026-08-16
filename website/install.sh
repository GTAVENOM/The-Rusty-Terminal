#!/usr/bin/env bash
# --- Rusty Terminal & CLI Assistant macOS & Linux Installer (install.sh) ---
set -e

echo "🦀 Installing Rusty Terminal & AI CLI Assistant for macOS/Linux..."

RUSTY_DIR="$HOME/.rusty"
RUSTY_BIN="$RUSTY_DIR/bin"

mkdir -p "$RUSTY_BIN"

REPO_URL="https://raw.githubusercontent.com/GTAVENOM/The-Rusty-Terminal/main"

echo "📥 Downloading Rusty CLI Assistant..."
if curl -fsSL "$REPO_URL/windows/bin/rusty-cli.exe" -o "$RUSTY_BIN/rusty-cli"; then
    chmod +x "$RUSTY_BIN/rusty-cli"
else
    echo "⚠️ Downloading fallback asset..."
    curl -fsSL "https://github.com/GTAVENOM/The-Rusty-Terminal/releases/latest/download/rusty-cli" -o "$RUSTY_BIN/rusty-cli" || true
    chmod +x "$RUSTY_BIN/rusty-cli" || true
fi

# Add to PATH in zshrc or bashrc
SHELL_RC=""
if [ -f "$HOME/.zshrc" ]; then
    SHELL_RC="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
    SHELL_RC="$HOME/.bashrc"
fi

if [ -n "$SHELL_RC" ]; then
    if ! grep -q '$HOME/.rusty/bin' "$SHELL_RC"; then
        echo 'export PATH="$HOME/.rusty/bin:$PATH"' >> "$SHELL_RC"
        echo "✅ Added $RUSTY_BIN to $SHELL_RC"
    fi
fi

echo ""
echo "🎉 Rusty Terminal Installed Successfully!"
echo "--------------------------------------------------------"
echo "  • Type 'rusty-cli \"go to kt\"' in your terminal"
echo "  • Restart your shell or run 'source $SHELL_RC' to activate"
echo "--------------------------------------------------------"
