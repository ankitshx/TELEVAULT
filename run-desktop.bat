@echo off
setlocal
cd /d "%~dp0"

:: 1. Unblock executables to eliminate Windows Defender spinning wait cursor
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path '%~dp0*.exe' -ErrorAction SilentlyContinue | Unblock-File -ErrorAction SilentlyContinue" >nul 2>&1

:: 2. Ensure TeleVault background service is running on port 8000
powershell -NoProfile -ExecutionPolicy Bypass -Command "$c = Test-NetConnection -ComputerName 127.0.0.1 -Port 8000 -WarningAction SilentlyContinue; if (-not $c.TcpTestSucceeded) { exit 1 }" >nul 2>&1
if %ERRORLEVEL% neq 0 (
    if exist "%~dp0televault.exe" (
        start "" /B "%~dp0televault.exe" web --no-browser
    ) else if exist "%~dp0dist\televault.exe" (
        start "" /B "%~dp0dist\televault.exe" web --no-browser
    )
)

:: 3. Launch TeleCloud Desktop UI
if exist "%~dp0telecloud-desktop.exe" (
    start "" "%~dp0telecloud-desktop.exe"
) else (
    echo [ERROR] telecloud-desktop.exe not found in %~dp0
    pause
)
