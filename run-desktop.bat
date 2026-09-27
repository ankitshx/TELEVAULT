@echo off
setlocal
cd /d "%~dp0"
title TELEVAULT Desktop — Created by Ankit Sharma (@ankitshx)

:: 1. Unblock executables to eliminate Windows Defender spinning wait cursor
powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -Path '%~dp0*.exe' -Recurse -ErrorAction SilentlyContinue | Unblock-File -ErrorAction SilentlyContinue" >nul 2>&1

:: 2. Launch TELEVAULT Standalone Application
if exist "%~dp0TELEVAULT.exe" (
    start "" "%~dp0TELEVAULT.exe"
) else if exist "%~dp0release\TELEVAULT.exe" (
    start "" "%~dp0release\TELEVAULT.exe"
) else if exist "%~dp0target\release\TELEVAULT.exe" (
    start "" "%~dp0target\release\TELEVAULT.exe"
) else (
    echo [ERROR] TELEVAULT.exe not found!
    echo Please make sure TELEVAULT.exe is present in this folder.
    pause
)

