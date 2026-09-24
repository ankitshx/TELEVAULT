from PyQt6.QtWidgets import (
    QCheckBox,
    QComboBox,
    QDialog,
    QFormLayout,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QSpinBox,
    QVBoxLayout,
)


class SettingsDialog(QDialog):
    """TeleVault user configuration and preferences dialog."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setWindowTitle("TeleVault Settings")
        self.setMinimumWidth(480)
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(16)

        title = QLabel("VAULT SETTINGS & ENCRYPTION")
        title.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #ECEFF4; font-size: 13px;")
        layout.addWidget(title)

        form = QFormLayout()
        form.setSpacing(12)

        # Mode selection
        self.combo_mode = QComboBox()
        self.combo_mode.addItems(["Original Mode (Default)", "Private Mode (AES-256-GCM)"])
        self.combo_mode.currentIndexChanged.connect(self._on_mode_changed)
        form.addRow("Vault Mode:", self.combo_mode)

        # Passphrase (only visible in Private Mode)
        self.txt_passphrase = QLineEdit()
        self.txt_passphrase.setEchoMode(QLineEdit.EchoMode.Password)
        self.txt_passphrase.setPlaceholderText("Enter client-side encryption passphrase")
        self.lbl_passphrase = QLabel("Passphrase:")
        form.addRow(self.lbl_passphrase, self.txt_passphrase)
        self.lbl_passphrase.setVisible(False)
        self.txt_passphrase.setVisible(False)

        # Local recovery copy
        self.chk_recovery = QCheckBox("Keep local recovery cache for files < 500 MB")
        self.chk_recovery.setChecked(True)
        form.addRow("Recovery:", self.chk_recovery)

        # Versions to keep
        self.spin_versions = QSpinBox()
        self.spin_versions.setRange(1, 20)
        self.spin_versions.setValue(5)
        form.addRow("Versions to Retain:", self.spin_versions)

        # Concurrency
        self.spin_concurrency = QSpinBox()
        self.spin_concurrency.setRange(1, 4)
        self.spin_concurrency.setValue(1)
        form.addRow("Max Concurrent Uploads:", self.spin_concurrency)

        # Minimize to tray
        self.chk_tray = QCheckBox("Minimize to system tray on window close")
        self.chk_tray.setChecked(True)
        form.addRow("System Tray:", self.chk_tray)

        layout.addLayout(form)

        # Buttons
        btn_layout = QHBoxLayout()
        btn_layout.addStretch()

        btn_cancel = QPushButton("CANCEL")
        btn_cancel.clicked.connect(self.reject)
        btn_layout.addWidget(btn_cancel)

        btn_save = QPushButton("SAVE SETTINGS")
        btn_save.setObjectName("btnPrimary")
        btn_save.clicked.connect(self.accept)
        btn_layout.addWidget(btn_save)

        layout.addLayout(btn_layout)

    def _on_mode_changed(self, index: int):
        is_private = (index == 1)
        self.lbl_passphrase.setVisible(is_private)
        self.txt_passphrase.setVisible(is_private)
