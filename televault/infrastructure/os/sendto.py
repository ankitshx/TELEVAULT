import os
from pathlib import Path
import sys


def install_sendto_shortcut(executable_path: Path | None = None) -> bool:
    """Install a Windows Explorer Send-To shortcut for TeleVault."""
    if os.name != "nt":
        return False

    appdata = os.environ.get("APPDATA")
    if not appdata:
        return False

    sendto_dir = Path(appdata) / "Microsoft" / "Windows" / "SendTo"
    if not sendto_dir.is_dir():
        return False

    shortcut_path = sendto_dir / "TeleVault.bat"
    target_exe = executable_path or Path(sys.executable)

    # Simple portable script wrapper for Send-To
    bat_content = f'@echo off\n"{target_exe}" -m televault backup %*\n'
    try:
        shortcut_path.write_text(bat_content, encoding="utf-8")
        return True
    except OSError:
        return False


def uninstall_sendto_shortcut() -> bool:
    """Remove Windows Explorer Send-To shortcut for TeleVault."""
    if os.name != "nt":
        return False

    appdata = os.environ.get("APPDATA")
    if not appdata:
        return False

    shortcut_path = Path(appdata) / "Microsoft" / "Windows" / "SendTo" / "TeleVault.bat"
    if shortcut_path.is_file():
        try:
            shortcut_path.unlink()
            return True
        except OSError:
            return False
    return True
