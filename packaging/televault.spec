# -*- mode: python ; coding: utf-8 -*-
import sys
from pathlib import Path

block_cipher = None

a = Analysis(
    ['../televault/__main__.py'],
    pathex=['..'],
    binaries=[],
    datas=[
        ('../televault/infrastructure/storage/migrations/*.sql', 'televault/infrastructure/storage/migrations'),
        ('../assets', 'assets'),
    ],
    hiddenimports=[
        'televault.infrastructure.telegram.telethon_gateway',
        'televault.infrastructure.storage.sqlite_repo',
        'televault.infrastructure.crypto.stream_cipher',
        'televault.infrastructure.os.keyring_store',
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
    entitlements_file=None,
    icon='../assets/icons/televault.ico' if Path('../assets/icons/televault.ico').exists() else None,
)
