# --- Rusty Terminal & CLI Assistant Windows Installer (install.ps1) ---
$ErrorActionPreference = "Stop"

Write-Host "🦀 Installing Rusty Terminal & AI CLI Assistant..." -ForegroundColor Cyan

$rustyDir = "$env:USERPROFILE\.rusty"
$rustyBin = "$rustyDir\bin"

if (-not (Test-Path $rustyBin)) {
    New-Item -ItemType Directory -Force -Path $rustyBin | Out-Null
}

$repoUrl = "https://raw.githubusercontent.com/GTAVENOM/The-Rusty-Terminal/main"

$cliTarget = "$rustyBin\rusty-cli.exe"
$guiTarget = "$rustyBin\rusty.exe"

Write-Host "📥 Downloading Rusty CLI Assistant (rusty-cli.exe)..." -ForegroundColor Yellow
try {
    Invoke-WebRequest -Uri "$repoUrl/windows/bin/rusty-cli.exe" -OutFile $cliTarget -UseBasicParsing
} catch {
    Write-Host "⚠️ Downloading from fallback release asset..." -ForegroundColor Gray
    Invoke-WebRequest -Uri "https://github.com/GTAVENOM/The-Rusty-Terminal/releases/latest/download/rusty-cli.exe" -OutFile $cliTarget -UseBasicParsing
}

Write-Host "📥 Downloading Rusty Terminal GUI App (rusty.exe)..." -ForegroundColor Yellow
try {
    Invoke-WebRequest -Uri "$repoUrl/windows/bin/rusty.exe" -OutFile $guiTarget -UseBasicParsing
} catch {
    Write-Host "⚠️ Downloading from fallback release asset..." -ForegroundColor Gray
    Invoke-WebRequest -Uri "https://github.com/GTAVENOM/The-Rusty-Terminal/releases/latest/download/rusty.exe" -OutFile $guiTarget -UseBasicParsing
}

# Update User PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$rustyBin*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$rustyBin", "User")
    $env:PATH += ";$rustyBin"
    Write-Host "✅ Added $rustyBin to User PATH environment variable." -ForegroundColor Green
}

# Setup PowerShell PSReadLine Ctrl+V integration
if (Test-Path $cliTarget) {
    Write-Host "⚙️ Registering Ctrl+V PSReadLine keybinding in PowerShell profile..." -ForegroundColor Yellow
    & $cliTarget setup-ps | Out-Null
}

Write-Host ""
Write-Host "🎉 Rusty Terminal Installed Successfully!" -ForegroundColor Green
Write-Host "--------------------------------------------------------" -ForegroundColor DarkGray
Write-Host "  • Type 'rusty-cli \"go to kt\"' in any shell" -ForegroundColor White
Write-Host "  • Press 'Ctrl+V' for inline AI suggestions" -ForegroundColor White
Write-Host "  • Press 'Ctrl+Shift+R' or run 'rusty-cli overlay' for ANSI TUI Panel" -ForegroundColor White
Write-Host "  • Restart your terminal or run '. `$PROFILE' to activate" -ForegroundColor Cyan
Write-Host "--------------------------------------------------------" -ForegroundColor DarkGray
