# 🦀 Rusty Terminal PSReadLine Module
# Provides Ctrl+K natural language command assistant inside Windows Terminal & PowerShell

if (-not (Get-Command rusty-cli -ErrorAction SilentlyContinue)) {
    $rustyBin = "$env:USERPROFILE\.rusty\bin"
    if (Test-Path "$rustyBin\rusty-cli.exe") {
        $env:PATH += ";$rustyBin"
    } elseif (Test-Path "v:\RustyTerminal\target\debug\rusty-cli.exe") {
        $env:PATH += ";v:\RustyTerminal\target\debug"
    } elseif (Test-Path "v:\RustyTerminal\target\release\rusty-cli.exe") {
        $env:PATH += ";v:\RustyTerminal\target\release"
    }
}

Set-PSReadLineKeyHandler -Chord 'Ctrl+k' -ScriptBlock {
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    
    $rustyExe = Get-Command rusty-cli -ErrorAction SilentlyContinue
    $cmd = $null
    if ($rustyExe) {
        $cmd = & $rustyExe inline $line
    } else {
        $fallback = "$env:USERPROFILE\.rusty\bin\rusty-cli.exe"
        if (-not (Test-Path $fallback)) {
            $fallback = "v:\RustyTerminal\target\debug\rusty-cli.exe"
        }
        if (Test-Path $fallback) {
            $cmd = & $fallback inline $line
        }
    }

    if ($cmd) {
        [Microsoft.PowerShell.PSConsoleReadLine]::Replace(0, $line.Length, $cmd.Trim())
    }
}
