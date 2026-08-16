# 🦀 Rusty Terminal & CLI Installer for Windows
# Builds and installs rusty.exe (GUI Terminal) and rusty-cli.exe (Wrapper Tool)

Write-Host "🦀 Building Rusty Terminal (GUI) & Rusty CLI (Wrapper)..." -ForegroundColor Cyan
cargo build --release --bins

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Build failed. Please ensure the Rust toolchain is installed." -ForegroundColor Red
    exit 1
}

$targetDir = "$env:USERPROFILE\.rusty\bin"
if (-not (Test-Path $targetDir)) {
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
}

Copy-Item "target\release\rusty.exe" "$targetDir\rusty.exe" -Force
Copy-Item "target\release\rusty-cli.exe" "$targetDir\rusty-cli.exe" -Force

Write-Host "✅ Binaries installed to: $targetDir" -ForegroundColor Green

# Add to User PATH if not present
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$targetDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$targetDir", "User")
    Write-Host "🎉 Added $targetDir to User PATH." -ForegroundColor Green
}

# Run PowerShell setup
& "$targetDir\rusty-cli.exe" setup-ps

Write-Host ""
Write-Host "✨ Rusty Terminal installation complete!" -ForegroundColor Cyan
Write-Host "   • GUI Terminal: Run 'rusty'"
Write-Host "   • CLI Wrapper:  Run 'rusty-cli \"your query\"' or press Ctrl+K in PowerShell"
