@echo off
rem 🦀 Rusty Terminal & CLI Assistant Installer for cmd.exe

echo 🦀 Installing Rusty Terminal CLI Assistant for cmd.exe...

if not exist "%USERPROFILE%\.rusty\bin" mkdir "%USERPROFILE%\.rusty\bin"

copy /y "target\release\rusty.exe" "%USERPROFILE%\.rusty\bin\rusty.exe" >nul 2>&1
copy /y "target\release\rusty-cli.exe" "%USERPROFILE%\.rusty\bin\rusty-cli.exe" >nul 2>&1

reg add "HKCU\Software\Microsoft\Command Processor" /v AutoRun /t REG_SZ /d "@set PATH=%%USERPROFILE%%\.rusty\bin;%%PATH%%" /f >nul

echo ✅ Installed Rusty binaries to %USERPROFILE%\.rusty\bin
echo ✅ Configured HKCU\Software\Microsoft\Command Processor\AutoRun
echo.
echo ✨ Installation complete! Open any new CMD window and run 'rusty-cli "your query"'
