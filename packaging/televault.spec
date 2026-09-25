# -*- mode: python ; coding: utf-8 -*-
import sys
from pathlib import Path

block_cipher = None

# Resolve repository root from spec file location
SPEC_DIR = Path(SPECPATH).resolve() if "SPECPATH" in globals() else Path(__file__).resolve().parent
REPO_ROOT = SPEC_DIR.parent if SPEC_DIR.name == "packaging" else SPEC_DIR

a = Analysis(
    [str(REPO_ROOT / 'televault' / '__main__.py')],
    pathex=[str(REPO_ROOT)],
    binaries=[],
    datas=[
        (str(REPO_ROOT / 'televault' / 'infrastructure' / 'storage' / 'migrations' / '*.sql'), 'televault/infrastructure/storage/migrations'),
        (str(REPO_ROOT / 'televault' / 'presentation' / 'web' / 'static'), 'televault/presentation/web/static'),
        (str(REPO_ROOT / 'assets'), 'assets'),
    ],
    hiddenimports=[
        'televault.infrastructure.telegram.telethon_gateway',
        'televault.infrastructure.telegram.auth_service',
        'televault.infrastructure.storage.sqlite_repo',
        'televault.infrastructure.crypto.stream_cipher',
        'televault.infrastructure.os.keyring_store',
        'televault.application.chunking',
        'televault.application.caption',
        'televault.application.manifest',
        'televault.application.doctor',
        'televault.application.recovery_drill',
        'televault.application.transaction',
        'televault.application.caption_v2',
        'televault.presentation.gui.dialogs.login_dialog',
        'televault.ai.orchestrator',
        'televault.ai.provider',
        'televault.ai.agents',
        'telethon',
        'keyring',
        'fastapi',
        'uvicorn',
        'rich',
        'cryptography',
        'argon2',
        'PyQt6',
        'PyQt6.QtCore',
        'PyQt6.QtGui',
        'PyQt6.QtWidgets',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'pytest',
        'pytest_asyncio',
        'ruff',
        'mypy',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='televault',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    icon=str(REPO_ROOT / 'assets' / 'icons' / 'televault.ico') if (REPO_ROOT / 'assets' / 'icons' / 'televault.ico').exists() else None,
)
