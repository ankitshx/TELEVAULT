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
echo                      TELEVAULT v2.0 - WINDOWS 11 LAUNCHER                      
echo         A private, append-only, verifiable backup vault via MTProto            
echo ===============================================================================
echo.
echo Application Interfaces:
echo   [0] Launch Windows 11 Fluent GUI (Desktop App)
echo   [1] Launch Modern Web Dashboard (Browser App)
echo   [2] View Rich Colored Terminal Dashboard (CLI)
echo.
echo Health, Diagnostics & AI:
echo   [3] Run Vault Doctor Diagnostics (doctor)
echo   [4] Run Non-Destructive Recovery Drill (recovery-test)
echo   [5] Verify Cryptographic Manifest Chain (manifest --verify)
echo   [6] Inspect Tamper-Evident Audit Ledger (audit)
echo.
echo Operations:
echo   [7] Connect / Login Telegram MTProto Account (login)
echo   [8] Verify Dual Channel Redundancy & Auto-Heal (verify --heal)
echo   [9] Back Up a File or Folder (backup)
echo   [10] Restore a File by Record ID (restore)
echo   [11] Disaster Recovery Rebuild from Telegram (rebuild)
echo   [12] Show Command Line Help (--help)
echo   [13] Exit
echo.
set /p "CHOICE=Enter choice [0-13] or custom command: "

if "%CHOICE%"=="0" (
    echo.
    echo Launching TeleVault Windows 11 Fluent GUI...
    start "" python -m televault.presentation.gui.app
    goto menu
)
if "%CHOICE%"=="1" (
    echo.
    echo Launching TeleVault Web Dashboard at http://127.0.0.1:8000 ...
    python -m televault.presentation.cli web
    pause
    goto menu
)
if "%CHOICE%"=="2" (
    echo.
    python -m televault.presentation.cli dashboard
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="3" (
    echo.
    python -m televault.presentation.cli doctor
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="4" (
    echo.
    python -m televault.presentation.cli recovery-test
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="5" (
    echo.
    python -m televault.presentation.cli manifest --verify
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="6" (
    echo.
    python -m televault.presentation.cli audit
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="7" (
    echo.
    python -m televault.presentation.cli login
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="8" (
    echo.
    python -m televault.presentation.cli verify --heal
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="9" (
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
if "%CHOICE%"=="10" (
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
if "%CHOICE%"=="11" (
    echo.
    python -m televault.presentation.cli rebuild
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="12" (
    echo.
    python -m televault.presentation.cli --help
    echo.
    pause
    goto menu
)
if "%CHOICE%"=="13" (
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
