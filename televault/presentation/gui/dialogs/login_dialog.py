import asyncio
from typing import Any
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QDesktopServices
from PyQt6.QtCore import QUrl
from PyQt6.QtWidgets import (
    QDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from televault.infrastructure.telegram.auth_service import TelegramAuthService
from televault.presentation.gui.async_bridge import AsyncBridge


class TelegramLoginDialog(QDialog):
    """Modern Fluent-style interactive Telegram MTProto login dialog."""

    login_success = pyqtSignal(object, dict)  # (TelethonGateway, user_dict)

    def __init__(self, auth_service: TelegramAuthService, parent=None):
        super().__init__(parent)
        self.auth_service = auth_service
        self.setWindowTitle("Connect Telegram Account — TeleVault")
        self.setMinimumSize(480, 420)
        self.setWindowFlags(self.windowFlags() & ~Qt.WindowType.WindowContextHelpButtonHint)

        self._pending_phone_hash: str | None = None
        self._worker: AsyncBridge | None = None

        self._init_ui()
        self._load_stored_credentials()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 24, 28, 24)
        layout.setSpacing(18)

        # Header Title
        title_box = QVBoxLayout()
        title_box.setSpacing(4)
        title = QLabel("Connect Your Telegram Account")
        title.setStyleSheet("font-size: 18px; font-weight: 700; color: #FFFFFF;")
        title_box.addWidget(title)

        desc = QLabel(
            "TeleVault connects directly via MTProto client session to protect your files.\n"
            "Your login credentials are encrypted locally in Windows Keyring."
        )
        desc.setStyleSheet("color: #9CA3AF; font-size: 12px; line-height: 1.4;")
        desc.setWordWrap(True)
        title_box.addWidget(desc)
        layout.addLayout(title_box)

        # Stack for Step 1 (Phone & API keys) vs Step 2 (Code & 2FA)
        self.stack = QStackedWidget()

        # --- STEP 1: Phone + API ID + API Hash ---
        step1 = QWidget()
        s1_layout = QVBoxLayout(step1)
        s1_layout.setContentsMargins(0, 0, 0, 0)
        s1_layout.setSpacing(12)

        # Help Card for API Credentials
        help_card = QFrame()
        help_card.setStyleSheet("background-color: #1F2937; border-radius: 8px; padding: 10px;")
        h_layout = QHBoxLayout(help_card)
        h_layout.setContentsMargins(10, 8, 10, 8)
        lbl_help = QLabel("Need Telegram API keys?")
        lbl_help.setStyleSheet("color: #D1D5DB; font-size: 11px;")
        h_layout.addWidget(lbl_help)

        btn_my_tg = QPushButton("Open my.telegram.org")
        btn_my_tg.setCursor(Qt.CursorShape.PointingHandCursor)
        btn_my_tg.setStyleSheet("background: transparent; color: #3B82F6; font-size: 11px; text-decoration: underline; border: none; padding: 0;")
        btn_my_tg.clicked.connect(lambda: QDesktopServices.openUrl(QUrl("https://my.telegram.org")))
        h_layout.addWidget(btn_my_tg)
        h_layout.addStretch()
        s1_layout.addWidget(help_card)

        # Phone Number
        lbl_phone = QLabel("Phone Number (with country code):")
        lbl_phone.setStyleSheet("color: #E5E7EB; font-weight: 600; font-size: 12px;")
        s1_layout.addWidget(lbl_phone)
        self.txt_phone = QLineEdit()
        self.txt_phone.setPlaceholderText("+1234567890 or +919876543210")
        s1_layout.addWidget(self.txt_phone)

        # API ID
        lbl_api_id = QLabel("Telegram API ID:")
        lbl_api_id.setStyleSheet("color: #E5E7EB; font-weight: 600; font-size: 12px;")
        s1_layout.addWidget(lbl_api_id)
        self.txt_api_id = QLineEdit()
        self.txt_api_id.setPlaceholderText("e.g. 12345678")
        s1_layout.addWidget(self.txt_api_id)

        # API Hash
        lbl_api_hash = QLabel("Telegram API Hash:")
        lbl_api_hash.setStyleSheet("color: #E5E7EB; font-weight: 600; font-size: 12px;")
        s1_layout.addWidget(lbl_api_hash)
        self.txt_api_hash = QLineEdit()
        self.txt_api_hash.setEchoMode(QLineEdit.EchoMode.Password)
        self.txt_api_hash.setPlaceholderText("e.g. 0123456789abcdef0123456789abcdef")
        s1_layout.addWidget(self.txt_api_hash)

        s1_layout.addStretch()

        # Step 1 Action Buttons
        b1_box = QHBoxLayout()
        b1_box.addStretch()

        self.btn_cancel1 = QPushButton("Cancel")
        self.btn_cancel1.clicked.connect(self.reject)
        b1_box.addWidget(self.btn_cancel1)

        self.btn_send_code = QPushButton("Send Login Code →")
        self.btn_send_code.setObjectName("btnPrimary")
        self.btn_send_code.setCursor(Qt.CursorShape.PointingHandCursor)
        self.btn_send_code.clicked.connect(self._handle_send_code)
        b1_box.addWidget(self.btn_send_code)
        s1_layout.addLayout(b1_box)

        self.stack.addWidget(step1)

        # --- STEP 2: Code Verification & 2FA Password ---
        step2 = QWidget()
        s2_layout = QVBoxLayout(step2)
        s2_layout.setContentsMargins(0, 0, 0, 0)
        s2_layout.setSpacing(12)

        self.lbl_code_info = QLabel("A login code has been sent to your Telegram app.")
        self.lbl_code_info.setStyleSheet("color: #10B981; font-weight: 600; font-size: 12px;")
        s2_layout.addWidget(self.lbl_code_info)

        lbl_code = QLabel("Telegram Login Code:")
        lbl_code.setStyleSheet("color: #E5E7EB; font-weight: 600; font-size: 12px;")
        s2_layout.addWidget(lbl_code)
        self.txt_code = QLineEdit()
        self.txt_code.setPlaceholderText("Enter the code received in Telegram")
        s2_layout.addWidget(self.txt_code)

        lbl_password = QLabel("2-Step Verification Password (Optional):")
        lbl_password.setStyleSheet("color: #E5E7EB; font-weight: 600; font-size: 12px;")
        s2_layout.addWidget(lbl_password)
        self.txt_password = QLineEdit()
        self.txt_password.setEchoMode(QLineEdit.EchoMode.Password)
        self.txt_password.setPlaceholderText("Leave blank if you don't use 2FA Cloud Password")
        s2_layout.addWidget(self.txt_password)

        s2_layout.addStretch()

        # Step 2 Action Buttons
        b2_box = QHBoxLayout()
        self.btn_back = QPushButton("← Change Details")
        self.btn_back.clicked.connect(lambda: self.stack.setCurrentIndex(0))
        b2_box.addWidget(self.btn_back)

        b2_box.addStretch()

        self.btn_sign_in = QPushButton("Verify & Sign In")
        self.btn_sign_in.setObjectName("btnPrimary")
        self.btn_sign_in.setCursor(Qt.CursorShape.PointingHandCursor)
        self.btn_sign_in.clicked.connect(self._handle_complete_login)
        b2_box.addWidget(self.btn_sign_in)
        s2_layout.addLayout(b2_box)

        self.stack.addWidget(step2)

        layout.addWidget(self.stack)

        # Status / Spinner Label
        self.lbl_status = QLabel("")
        self.lbl_status.setStyleSheet("color: #F59E0B; font-size: 11px; font-weight: 600;")
        self.lbl_status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(self.lbl_status)

    def _load_stored_credentials(self):
        creds = self.auth_service.get_stored_credentials()
        if creds.get("phone"):
            self.txt_phone.setText(str(creds["phone"]))
        if creds.get("api_id"):
            self.txt_api_id.setText(str(creds["api_id"]))
        if creds.get("api_hash"):
            self.txt_api_hash.setText(str(creds["api_hash"]))

    def _set_loading(self, is_loading: bool, message: str = ""):
        self.lbl_status.setText(message)
        self.btn_send_code.setEnabled(not is_loading)
        self.btn_sign_in.setEnabled(not is_loading)
        self.btn_back.setEnabled(not is_loading)
        self.txt_phone.setEnabled(not is_loading)
        self.txt_api_id.setEnabled(not is_loading)
        self.txt_api_hash.setEnabled(not is_loading)
        self.txt_code.setEnabled(not is_loading)
        self.txt_password.setEnabled(not is_loading)

    def _handle_send_code(self):
        phone = self.txt_phone.text().strip()
        api_id = self.txt_api_id.text().strip()
        api_hash = self.txt_api_hash.text().strip()

        if not phone or not api_id or not api_hash:
            QMessageBox.warning(self, "Missing Credentials", "Please enter Phone Number, API ID, and API Hash.")
            return

        self._set_loading(True, "Connecting to Telegram & requesting verification code...")

        async def _run():
            return await self.auth_service.send_login_code(api_id=api_id, api_hash=api_hash, phone=phone)

        self._worker = AsyncBridge(_run)

        def _on_success(res: dict):
            self._set_loading(False)
            self._pending_phone_hash = res.get("phone_code_hash")
            self.lbl_code_info.setText(f"Code sent to {phone}. Check your Telegram app.")
            self.stack.setCurrentIndex(1)
            self.txt_code.setFocus()

        def _on_failure(err: str):
            self._set_loading(False)
            QMessageBox.critical(self, "Telegram Connection Error", f"Failed to send code:\n{err}")

        self._worker.task_completed.connect(_on_success)
        self._worker.task_failed.connect(_on_failure)
        self._worker.start()

    def _handle_complete_login(self):
        code = self.txt_code.text().strip()
        password = self.txt_password.text().strip() or None

        if not code:
            QMessageBox.warning(self, "Verification Code Required", "Please enter the code sent to your Telegram account.")
            return

        self._set_loading(True, "Verifying code & bootstrapping private vault channels...")

        async def _run():
            res = await self.auth_service.complete_login(
                code=code,
                phone_code_hash=self._pending_phone_hash,
                password=password,
            )
            if res.get("status") == "2fa_required":
                return res

            gateway = await self.auth_service.get_live_gateway()
            user_info = res.get("user") or {}
            return {"status": "success", "gateway": gateway, "user": user_info}

        self._worker = AsyncBridge(_run)

        def _on_success(res: dict):
            self._set_loading(False)
            if res.get("status") == "2fa_required":
                QMessageBox.warning(
                    self, "2-Step Verification Required",
                    "This Telegram account has Two-Factor Authentication enabled.\nPlease enter your cloud password below."
                )
                self.txt_password.setFocus()
                return

            gateway = res.get("gateway")
            user = res.get("user") or {}
            self.login_success.emit(gateway, user)
            QMessageBox.information(
                self, "Connected to Telegram",
                f"Welcome {user.get('first_name', '')} (@{user.get('username', 'user')})!\n"
                "TeleVault channels have been initialized and verified."
            )
            self.accept()

        def _on_failure(err: str):
            self._set_loading(False)
            QMessageBox.critical(self, "Login Failed", f"Could not complete sign in:\n{err}")

        self._worker.task_completed.connect(_on_success)
        self._worker.task_failed.connect(_on_failure)
        self._worker.start()
