from pathlib import Path
import sys
from PyQt6.QtCore import Qt
from PyQt6.QtGui import QIcon
from PyQt6.QtWidgets import QApplication

from televault.presentation.gui.main_window import MainWindow


def run_gui() -> int:
    """Launch the native TeleVault PyQt6 application."""
    app = QApplication.instance()
    if not app:
        app = QApplication(sys.argv)
    app.setApplicationName("TeleVault")
    app.setOrganizationName("TeleVault")

    if getattr(sys, "frozen", False) and hasattr(sys, "_MEIPASS"):
        base_dir = Path(sys._MEIPASS)
    else:
        base_dir = Path(__file__).resolve().parents[3]
    icon_path = base_dir / "assets" / "icons" / "televault.ico"
    if not icon_path.exists():
        icon_path = base_dir / "assets" / "icons" / "televault.png"
    if icon_path.exists():
        app.setWindowIcon(QIcon(str(icon_path)))

    window = MainWindow()
    if icon_path.exists():
        window.setWindowIcon(QIcon(str(icon_path)))
    window.show()

    return app.exec()


if __name__ == "__main__":
    sys.exit(run_gui())
