from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from televault.infrastructure.os.config import TeleVaultConfig


class SettingsView(QWidget):
    """Settings view for Telegram authentication, channels, and local storage layout."""

    login_requested = pyqtSignal()
    logout_requested = pyqtSignal()

    def __init__(self, config: TeleVaultConfig | None = None):
        super().__init__()
        self.config = config or TeleVaultConfig()
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(16)

        # Header
        top = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("Vault Settings & Configuration")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("Manage your Telegram MTProto connection and Windows storage layout.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top.addLayout(vbox)
        top.addStretch()
        layout.addLayout(top)

        # Telegram Account Card
        tg_card = QFrame()
        tg_card.setProperty("class", "fluentCard")
        tg_layout = QVBoxLayout(tg_card)
        tg_layout.setContentsMargins(18, 16, 18, 16)
        tg_layout.setSpacing(10)

        tg_title = QLabel("Telegram MTProto Connection")
        tg_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        tg_layout.addWidget(tg_title)

        self.lbl_account_status = QLabel("Status: Connected via MTProto Client Session")
        self.lbl_account_status.setStyleSheet("color: #10B981; font-weight: 500;")
        tg_layout.addWidget(self.lbl_account_status)

        h_btn = QHBoxLayout()
        self.btn_auth = QPushButton("Manage Telegram Account")
        self.btn_auth.clicked.connect(self.login_requested.emit)
        h_btn.addWidget(self.btn_auth)

        self.btn_logout = QPushButton("Disconnect & Logout")
        self.btn_logout.clicked.connect(self.logout_requested.emit)
        h_btn.addWidget(self.btn_logout)

        h_btn.addStretch()
        tg_layout.addLayout(h_btn)
        layout.addWidget(tg_card)

        # Storage Directories Card
        dir_card = QFrame()
        dir_card.setProperty("class", "fluentCard")
        d_layout = QVBoxLayout(dir_card)
        d_layout.setContentsMargins(18, 16, 18, 16)
        d_layout.setSpacing(10)

        d_title = QLabel("Windows Local Storage Architecture")
        d_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        d_layout.addWidget(d_title)

        d_text = (
            f"• <b>Base Directory:</b> <code>{self.config.app_dir}</code><br>"
            f"• <b>Database (SQLite):</b> <code>{self.config.db_path}</code><br>"
            f"• <b>Recovery Scratch:</b> <code>{self.config.recovery_dir}</code><br>"
            f"• <b>MTProto Sessions:</b> <code>{self.config.sessions_dir}</code><br>"
            f"• <b>System Logs:</b> <code>{self.config.logs_dir}</code>"
        )
        d_lbl = QLabel(d_text)
        d_lbl.setStyleSheet("color: #E5E7EB; font-size: 12px; line-height: 1.6;")
        d_layout.addWidget(d_lbl)
        layout.addWidget(dir_card)

        layout.addStretch()
