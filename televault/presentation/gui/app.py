import sys
from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import QApplication

from televault.presentation.gui.main_window import MainWindow


def run_gui() -> int:
    """Launch the native TeleVault PyQt6 application."""
    app = QApplication.instance()
    if not app:
        app = QApplication(sys.argv)
    app.setApplicationName("TeleVault")
    app.setOrganizationName("TeleVault")

    window = MainWindow()
    window.show()

    return app.exec()


if __name__ == "__main__":
    sys.exit(run_gui())
