from enum import Enum
from PyQt6.QtWidgets import QDialog, QHBoxLayout, QLabel, QPushButton, QVBoxLayout


class OverLimitChoice(str, Enum):
    CANCEL = "cancel"
    SPLIT = "split"


class OverLimitDialog(QDialog):
    """Shown when a file exceeds Telegram's single-file limit (2 GB Free / 4 GB Premium).

    Default choice is Cancel (never split silently).
    """

    def __init__(self, filename: str, file_size_bytes: int, limit_bytes: int, parent=None):
        super().__init__(parent)
        self.setWindowTitle("File Exceeds Telegram Account Limit")
        self.setMinimumWidth(450)
        self.choice = OverLimitChoice.CANCEL

        layout = QVBoxLayout(self)
        layout.setSpacing(16)

        title = QLabel("FILE EXCEEDS MAXIMUM UPLOAD LIMIT")
        title.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #EF4444; font-size: 14px;")
        layout.addWidget(title)

        size_gb = file_size_bytes / (1024 * 1024 * 1024)
        limit_gb = limit_bytes / (1024 * 1024 * 1024)

        desc = QLabel(
            f"The file '<strong>{filename}</strong>' is <strong>{size_gb:.2f} GB</strong>, "
            f"which exceeds your Telegram limit of <strong>{limit_gb:.1f} GB</strong>."
        )
        desc.setWordWrap(True)
        layout.addWidget(desc)

        advice = QLabel(
            "TeleVault non-negotiable rule: One file = One Telegram document. "
            "Files are never split without your explicit consent."
        )
        advice.setStyleSheet("color: #7E889B; font-size: 11px;")
        advice.setWordWrap(True)
        layout.addWidget(advice)

        btn_layout = QHBoxLayout()
        btn_layout.addStretch()

        self.btn_cancel = QPushButton("CANCEL (RECOMMENDED)")
        self.btn_cancel.setObjectName("btnPrimary")
        self.btn_cancel.clicked.connect(self._on_cancel)
        btn_layout.addWidget(self.btn_cancel)

        self.btn_split = QPushButton("SPLIT INTO PARTS (.part001...)")
        self.btn_split.clicked.connect(self._on_split)
        btn_layout.addWidget(self.btn_split)

        layout.addLayout(btn_layout)

    def _on_cancel(self):
        self.choice = OverLimitChoice.CANCEL
        self.reject()

    def _on_split(self):
        self.choice = OverLimitChoice.SPLIT
        self.accept()
