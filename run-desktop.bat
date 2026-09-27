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
    start "" "%~dp0dist\TELEVAULT.exe" gui
) else if exist "%~dp0release\TELEVAULT-Setup.exe" (
    start "" "%~dp0release\TELEVAULT-Setup.exe"
) else (
    where python >nul 2>&1
    if %ERRORLEVEL% equ 0 (
        start "" python -m televault.presentation.cli gui
    ) else (
        echo [ERROR] No desktop executable or Python environment found.
        echo Please run release\TELEVAULT-Setup.exe or install Python 3.12+.
        pause
    )
)

