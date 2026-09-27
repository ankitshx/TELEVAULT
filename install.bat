@echo off
setlocal
cd /d "%~dp0"
title TeleCloud One-Click Installer

echo ================================================================
echo  TeleCloud Automated Windows Setup
echo ================================================================
echo.
echo Installing TeleCloud and verifying all dependencies...
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"

if %ERRORLEVEL% neq 0 (
    echo.
    echo [!] Setup encountered a warning or error. Press any key to close.
    pause
)
