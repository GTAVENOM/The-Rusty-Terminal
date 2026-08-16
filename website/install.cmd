@echo off
:: --- Rusty Terminal & CLI Assistant Windows CMD Installer (install.cmd) ---
echo 🦀 Installing Rusty Terminal & AI CLI Assistant for CMD...

if not exist "%USERPROFILE%\.rusty\bin" mkdir "%USERPROFILE%\.rusty\bin"

powershell -Command "irm https://rustyterminal.vercel.app/install.ps1 | iex"

echo ✅ Installation complete! Restart your CMD prompt session.
