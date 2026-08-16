# 🦀 Rusty Terminal & CLI Uninstall Script

Write-Host "🗑️  Uninstalling Rusty Terminal & CLI Assistant..." -ForegroundColor Yellow

# 1. Clean up binaries via CLI uninstall
if (Get-Command rusty-cli -ErrorAction SilentlyContinue) {
    rusty-cli uninstall
} else {
    $targetDir = "$env:USERPROFILE\.rusty"
    if (Test-Path $targetDir) {
        Remove-Item -Path $targetDir -Recurse -Force
        Write-Host "🧹 Removed $targetDir" -ForegroundColor Green
    }
}

# 2. Clean AutoRun registry key for CMD
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Command Processor" -Name "AutoRun" -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "✨ Rusty Terminal has been uninstalled completely." -ForegroundColor Cyan
