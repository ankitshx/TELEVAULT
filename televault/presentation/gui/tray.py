from PyQt6.QtGui import QAction, QIcon
from PyQt6.QtWidgets import QMenu, QSystemTrayIcon


class TeleVaultTrayIcon(QSystemTrayIcon):
    """System tray icon integration for TeleVault."""

    def __init__(self, main_window, parent=None):
        super().__init__(parent)
        self.main_window = main_window
        self.setToolTip("TeleVault — Resilient Telegram Desktop Backup")

        # Context menu
        menu = QMenu()

        act_show = QAction("Show TeleVault", self)
        act_show.triggered.connect(self.main_window.showNormal)
        menu.addAction(act_show)

        act_verify = QAction("Verify Vault Now", self)
        act_verify.triggered.connect(self.main_window.run_verification)
        menu.addAction(act_verify)

        menu.addSeparator()

        act_exit = QAction("Exit", self)
        act_exit.triggered.connect(self.main_window.close_completely)
        menu.addAction(act_exit)

        self.setContextMenu(menu)
        self.activated.connect(self._on_tray_activated)

    def _on_tray_activated(self, reason):
        if reason == QSystemTrayIcon.ActivationReason.Trigger:
            if self.main_window.isVisible():
                self.main_window.hide()
            else:
                self.main_window.showNormal()
                self.main_window.activateWindow()
