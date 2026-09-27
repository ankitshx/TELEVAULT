@echo off
setlocal
cd /d "%~dp0"
title TELEVAULT Desktop — Created by Ankit Sharma (@ankitshx)

:: 1. Unblock executables to eliminate Windows Defender spinning wait cursor
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path '%~dp0*.exe' -Recurse -ErrorAction SilentlyContinue | Unblock-File -ErrorAction SilentlyContinue" >nul 2>&1

:: 2. Launch TELEVAULT Application
if exist "%LOCALAPPDATA%\Programs\TELEVAULT\TELEVAULT.exe" (
    start "" "%LOCALAPPDATA%\Programs\TELEVAULT\TELEVAULT.exe"
) else if exist "%~dp0dist\TELEVAULT.exe" (
    start "" "%~dp0dist\TELEVAULT.exe"
) else if exist "%~dp0target\release\TELEVAULT.exe" (
    start "" "%~dp0target\release\TELEVAULT.exe"
) else if exist "%~dp0release\TELEVAULT-Setup.exe" (
    start "" "%~dp0release\TELEVAULT-Setup.exe"
) else (
    echo [ERROR] No TELEVAULT desktop executable found.
    echo Please run release\TELEVAULT-Setup.exe to install.
    pause
)

