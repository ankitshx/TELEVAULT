@echo off
setlocal enabledelayedexpansion

:: TeleVault - Manual-only, append-only resilient desktop backup vault
:: Set working root directory to where this .bat is located
set "REPO_ROOT=%~dp0"
set "PYTHONPATH=%REPO_ROOT%;%PYTHONPATH%"

:: Ensure Python is available
where python >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Python was not found in your system PATH.
    echo Please install Python 3.12 or 3.14 and ensure 'Add Python to PATH' is checked.
    pause
    exit /b 1
)

:: If arguments were provided on the command line, pass them straight through
if not "%~1"=="" (
    python -m televault.presentation.cli %*
    exit /b %ERRORLEVEL%
)

:: Interactive mode when launched without arguments (or double-clicked in Explorer)
:menu
cls
echo ===============================================================================
echo                           TELEVAULT CLI LAUNCHER                              
echo       Resilient, Append-Only Telegram Backup Vault (Windows 11 x64)           
echo ===============================================================================
echo.
echo Available Quick Actions:
echo   [1] List all vault files (ls)
echo   [2] Verify vault health (verify)
echo   [3] Verify and auto-heal degraded files (verify --heal)
echo   [4] Back up a file or folder (backup)
echo   [5] Restore a file by ID (restore)
echo   [6] Rebuild local index from Telegram channels (rebuild)
echo   [7] Create and upload SQLite snapshot (snapshot)
echo   [8] Show full CLI command help (--help)
echo   [9] Exit
echo.
set /p "CHOICE=Enter choice [1-9] or type a custom command: "

if "%CHOICE%"=="1" (
    echo.
    python -m televault.presentation.cli ls
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="2" (
    echo.
    python -m televault.presentation.cli verify
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="3" (
    echo.
    python -m televault.presentation.cli verify --heal
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="4" (
    echo.
    set /p "TARGET_PATH=Enter full path of file or folder to back up: "
    if not "!TARGET_PATH!"=="" (
        python -m televault.presentation.cli backup "!TARGET_PATH!"
    ) else (
        echo Path cannot be empty.
    )
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="5" (
    echo.
    set /p "REC_ID=Enter record ID to restore: "
    set /p "OUT_DIR=Enter destination folder (leave empty for ./restored): "
    if not "!REC_ID!"=="" (
        if not "!OUT_DIR!"=="" (
            python -m televault.presentation.cli restore "!REC_ID!" --dest "!OUT_DIR!"
        ) else (
            python -m televault.presentation.cli restore "!REC_ID!"
        )
    ) else (
        echo Record ID cannot be empty.
    )
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="6" (
    echo.
    python -m televault.presentation.cli rebuild
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="7" (
    echo.
    python -m televault.presentation.cli snapshot
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="8" (
    echo.
    python -m televault.presentation.cli --help
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="9" (
    exit /b 0
)
if /i "%CHOICE%"=="exit" (
    exit /b 0
)
if /i "%CHOICE%"=="quit" (
    exit /b 0
)

:: If the user typed a command directly (e.g., "backup C:\path\file.txt" or "ls")
echo.
python -m televault.presentation.cli %CHOICE%
echo.
pause
goto menu
