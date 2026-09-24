@echo off
setlocal enabledelayedexpansion

:: TeleVault - Manual-only, append-only resilient desktop backup vault
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
echo   [0] Launch Web Dashboard (Modern Browser App)
echo   [1] View Rich Color Dashboard & File List (dashboard / ls)
echo   [2] Log in to Telegram Account via MTProto (login)
echo   [3] Disconnect Telegram Account (logout)
echo   [4] Verify Vault Health across Telegram Channels (verify)
echo   [5] Verify and Auto-Heal Degraded Files (verify --heal)
echo   [6] Back up a File or Folder (backup)
echo   [7] Restore a File by ID (restore)
echo   [8] Rebuild Local Index from Telegram Channels (rebuild)
echo   [9] Create and Upload SQLite Snapshot (snapshot)
echo   [10] Show Full Command Line Help (--help)
echo   [11] Exit
echo.
set /p "CHOICE=Enter choice [0-11] or type a command: "

if "%CHOICE%"=="0" (
    echo.
    echo Launching TeleVault Web Dashboard at http://127.0.0.1:8000 ...
    python -m televault.presentation.cli web
    pause
    goto menu
)
if "%CHOICE%"=="1" (
    echo.
    python -m televault.presentation.cli dashboard
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="2" (
    echo.
    python -m televault.presentation.cli login
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="3" (
    echo.
    python -m televault.presentation.cli logout
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="4" (
    echo.
    python -m televault.presentation.cli verify
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="5" (
    echo.
    python -m televault.presentation.cli verify --heal
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="6" (
    echo.
    set /p "TARGET_PATH=Enter full path of file or folder to back up: "
    set /p "IS_PRIV=Enable Private Mode encryption? (y/n): "
    if not "!TARGET_PATH!"=="" (
        if /i "!IS_PRIV!"=="y" (
            python -m televault.presentation.cli backup "!TARGET_PATH!" --private
        ) else (
            python -m televault.presentation.cli backup "!TARGET_PATH!"
        )
    ) else (
        echo Path cannot be empty.
    )
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="7" (
    echo.
    set /p "REC_ID=Enter record ID to restore: "
    set /p "OUT_DIR=Enter destination folder (leave blank for ./restored): "
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
if "%CHOICE%"=="8" (
    echo.
    python -m televault.presentation.cli rebuild
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="9" (
    echo.
    python -m televault.presentation.cli snapshot
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="10" (
    echo.
    python -m televault.presentation.cli --help
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="11" (
    exit /b 0
)
if /i "%CHOICE%"=="exit" (
    exit /b 0
)
if /i "%CHOICE%"=="quit" (
    exit /b 0
)

:: Custom CLI command typed by user
echo.
python -m televault.presentation.cli %CHOICE%
echo.
pause
goto menu
