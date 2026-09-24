from PyQt6.QtWidgets import (
    QComboBox,
    QDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)


class SetupWizardDialog(QDialog):
    """First-run setup wizard for configuring Telegram MTProto credentials and vaults."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("TeleVault Setup Wizard")
        self.setMinimumSize(540, 360)
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(16)

        # Wizard pages
        self.stack = QStackedWidget()

        # Page 1: API ID & API Hash
        p1 = QWidget()
        l1 = QVBoxLayout(p1)
        h1 = QLabel("STEP 1: TELEGRAM MTPROTO CREDENTIALS")
        h1.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #ECEFF4; font-size: 13px;")
        l1.addWidget(h1)

        d1 = QLabel(
            "TeleVault uses an MTProto user session directly from your PC (no Bot limits).\n"
            "Get your API credentials for free from https://my.telegram.org."
        )
        d1.setStyleSheet("color: #7E889B; font-size: 11px;")
        d1.setWordWrap(True)
        l1.addWidget(d1)

        f1 = QFormLayout()
        self.txt_api_id = QLineEdit()
        self.txt_api_id.setPlaceholderText("e.g. 1234567")
        f1.addRow("API ID:", self.txt_api_id)

        self.txt_api_hash = QLineEdit()
        self.txt_api_hash.setPlaceholderText("e.g. 0123456789abcdef0123456789abcdef")
        f1.addRow("API Hash:", self.txt_api_hash)
        l1.addLayout(f1)
        l1.addStretch()
        self.stack.addWidget(p1)

        # Page 2: Phone & Auth
        p2 = QWidget()
        l2 = QVBoxLayout(p2)
        h2 = QLabel("STEP 2: TELEGRAM ACCOUNT LOGIN")
        h2.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #ECEFF4; font-size: 13px;")
        l2.addWidget(h2)

        d2 = QLabel("Your session is stored strictly in the Windows Credential Manager keyring.")
        d2.setStyleSheet("color: #7E889B; font-size: 11px;")
        l2.addWidget(d2)

        f2 = QFormLayout()
        self.txt_phone = QLineEdit()
        self.txt_phone.setPlaceholderText("+1234567890")
        f2.addRow("Phone Number:", self.txt_phone)
        l2.addLayout(f2)
        l2.addStretch()
        self.stack.addWidget(p2)

        # Page 3: Vault Mode
        p3 = QWidget()
        l3 = QVBoxLayout(p3)
        h3 = QLabel("STEP 3: VAULT CONFIGURATION")
        h3.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #ECEFF4; font-size: 13px;")
        l3.addWidget(h3)

        d3 = QLabel("TeleVault will create private channels 'TeleVault Primary' and 'TeleVault Mirror'.")
        d3.setStyleSheet("color: #7E889B; font-size: 11px;")
        l3.addWidget(d3)

        f3 = QFormLayout()
        self.combo_wizard_mode = QComboBox()
        self.combo_wizard_mode.addItems(["Original Mode (Direct viewing)", "Private Mode (Client-side AES-256-GCM)"])
        f3.addRow("Vault Mode:", self.combo_wizard_mode)
        l3.addLayout(f3)
        l3.addStretch()
        self.stack.addWidget(p3)

        layout.addWidget(self.stack)

        # Navigation buttons
        nav_layout = QHBoxLayout()
        self.btn_back = QPushButton("BACK")
        self.btn_back.clicked.connect(self._prev_page)
        self.btn_back.setEnabled(False)
        nav_layout.addWidget(self.btn_back)

        nav_layout.addStretch()

        self.btn_next = QPushButton("NEXT")
        self.btn_next.setObjectName("btnPrimary")
        self.btn_next.clicked.connect(self._next_page)
        nav_layout.addWidget(self.btn_next)

        layout.addLayout(nav_layout)

    def _next_page(self):
        curr = self.stack.currentIndex()
        if curr < self.stack.count() - 1:
            self.stack.setCurrentIndex(curr + 1)
            self.btn_back.setEnabled(True)
            if self.stack.currentIndex() == self.stack.count() - 1:
                self.btn_next.setText("FINISH SETUP")
        else:
            self.accept()

    def _prev_page(self):
        curr = self.stack.currentIndex()
        if curr > 0:
            self.stack.setCurrentIndex(curr - 1)
            self.btn_next.setText("NEXT")
            if self.stack.currentIndex() == 0:
                self.btn_back.setEnabled(False)
