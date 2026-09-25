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
    """Settings view for Telegram authentication, vault channels, and local storage layout."""

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
        lbl = QLabel("Vault Settings & Account")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("Manage your Telegram MTProto connection, vault redundancy channels, and storage.")
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

        self.lbl_account_status = QLabel("● Checking Telegram authorization...")
        self.lbl_account_status.setStyleSheet("color: #F59E0B; font-weight: 500; font-size: 12px;")
        tg_layout.addWidget(self.lbl_account_status)

        self.lbl_account_details = QLabel("Session: Local simulation (FakeGateway)")
        self.lbl_account_details.setStyleSheet("color: #9CA3AF; font-size: 11px;")
        tg_layout.addWidget(self.lbl_account_details)

        h_btn = QHBoxLayout()
        self.btn_auth = QPushButton("🔑 Connect Telegram Account")
        self.btn_auth.setObjectName("btnPrimary")
        self.btn_auth.setCursor(Qt.CursorShape.PointingHandCursor)
        self.btn_auth.clicked.connect(self.login_requested.emit)
        h_btn.addWidget(self.btn_auth)

        self.btn_logout = QPushButton("Disconnect Session")
        self.btn_logout.setCursor(Qt.CursorShape.PointingHandCursor)
        self.btn_logout.clicked.connect(self.logout_requested.emit)
        h_btn.addWidget(self.btn_logout)

        h_btn.addStretch()
        tg_layout.addLayout(h_btn)
        layout.addWidget(tg_card)

        # Vault Redundancy Channels Card
        chan_card = QFrame()
        chan_card.setProperty("class", "fluentCard")
        c_layout = QVBoxLayout(chan_card)
        c_layout.setContentsMargins(18, 16, 18, 16)
        c_layout.setSpacing(8)

        c_title = QLabel("Dual-Write Vault Channels")
        c_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        c_layout.addWidget(c_title)

        self.lbl_channels = QLabel(
            "• <b>Primary Vault Channel:</b> <code>Not bound (auto-created on sign in)</code><br>"
            "• <b>Mirror Vault Channel:</b> <code>Not bound (auto-created on sign in)</code>"
        )
        self.lbl_channels.setStyleSheet("color: #E5E7EB; font-size: 12px; line-height: 1.6;")
        c_layout.addWidget(self.lbl_channels)
        layout.addWidget(chan_card)

        # Storage Directories Card
        dir_card = QFrame()
        dir_card.setProperty("class", "fluentCard")
        d_layout = QVBoxLayout(dir_card)
        d_layout.setContentsMargins(18, 16, 18, 16)
        d_layout.setSpacing(8)

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

    def update_account_info(
        self,
        is_connected: bool,
        user_info: dict | None = None,
        channels: dict | None = None,
    ):
        if is_connected and user_info:
            first_name = user_info.get("first_name", "")
            username = user_info.get("username")
            user_display = f"@{username}" if username else first_name or "Authorized User"
            phone = user_info.get("phone", "")
            phone_str = f" ({phone})" if phone else ""
            prem = " [Telegram Premium]" if user_info.get("is_premium") else ""

            self.lbl_account_status.setText(f"● Connected as {user_display}{phone_str}{prem}")
            self.lbl_account_status.setStyleSheet("color: #10B981; font-weight: 600; font-size: 12px;")
            self.lbl_account_details.setText("Client session active. All uploads sync directly to your Telegram cloud.")
            self.btn_auth.setText("Switch Telegram Account")
            self.btn_logout.setEnabled(True)
        else:
            self.lbl_account_status.setText("○ Disconnected (Local Offline Simulation)")
            self.lbl_account_status.setStyleSheet("color: #EF4444; font-weight: 600; font-size: 12px;")
            self.lbl_account_details.setText("Connect your Telegram MTProto credentials to enable real dual-write cloud sync.")
            self.btn_auth.setText("🔑 Connect Telegram Account")
            self.btn_logout.setEnabled(False)

        if channels and channels.get("primary_id"):
            p_id = channels.get("primary_id")
            m_id = channels.get("mirror_id")
            self.lbl_channels.setText(
                f"• <b>Primary Vault Channel:</b> <code>{p_id} (TeleVault Primary)</code><br>"
                f"• <b>Mirror Vault Channel:</b> <code>{m_id} (TeleVault Mirror)</code>"
            )
        else:
            self.lbl_channels.setText(
                "• <b>Primary Vault Channel:</b> <code>Pending login</code><br>"
                "• <b>Mirror Vault Channel:</b> <code>Pending login</code>"
            )
