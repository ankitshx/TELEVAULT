from pathlib import Path
from PyQt6.QtWidgets import (
    QDialog,
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QProgressBar,
    QPushButton,
    QVBoxLayout,
)

from televault.domain.entities import FileRecord


class RestoreDialog(QDialog):
    """File restoration dialog with folder selection and SHA-256 verification display."""

    def __init__(self, record: FileRecord, default_dest: Path | None = None, parent=None):
        super().__init__(parent)
        self.record = record
        self.setWindowTitle(f"Restore File: {record.name}")
        self.setMinimumWidth(500)
        self.selected_destination = default_dest or Path.home() / "Downloads" / "Restored"

        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(14)

        title = QLabel(f"RESTORE: {self.record.name}")
        title.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #ECEFF4; font-size: 13px;")
        layout.addWidget(title)

        info = QLabel(
            f"Size: {self.record.size / (1024*1024):.2f} MB  |  "
            f"Expected SHA-256: {self.record.sha256[:12]}..."
        )
        info.setStyleSheet("color: #7E889B; font-size: 11px;")
        layout.addWidget(info)

        # Destination input
        dest_lbl = QLabel("DESTINATION FOLDER:")
        dest_lbl.setStyleSheet("font-size: 11px; color: #7E889B;")
        layout.addWidget(dest_lbl)

        path_layout = QHBoxLayout()
        self.txt_dest = QLineEdit(str(self.selected_destination))
        path_layout.addWidget(self.txt_dest)

        btn_browse = QPushButton("BROWSE...")
        btn_browse.clicked.connect(self._browse_folder)
        path_layout.addWidget(btn_browse)

        layout.addLayout(path_layout)

        # Progress bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.progress_bar.setVisible(False)
        layout.addWidget(self.progress_bar)

        # Status result banner
        self.lbl_status = QLabel("")
        self.lbl_status.setWordWrap(True)
        layout.addWidget(self.lbl_status)

        # Buttons
        btn_layout = QHBoxLayout()
        btn_layout.addStretch()

        self.btn_cancel = QPushButton("CANCEL")
        self.btn_cancel.clicked.connect(self.reject)
        btn_layout.addWidget(self.btn_cancel)

        self.btn_confirm = QPushButton("CONFIRM RESTORE")
        self.btn_confirm.setObjectName("btnPrimary")
        self.btn_confirm.clicked.connect(self._on_confirm)
        btn_layout.addWidget(self.btn_confirm)

        layout.addLayout(btn_layout)

    def _browse_folder(self):
        folder = QFileDialog.getExistingDirectory(self, "Select Restore Directory")
        if folder:
            self.txt_dest.setText(folder)
            self.selected_destination = Path(folder)

    def _on_confirm(self):
        self.selected_destination = Path(self.txt_dest.text().strip())
        self.accept()

    def show_progress(self, current: int, total: int):
        self.progress_bar.setVisible(True)
        pct = int((current / total) * 100) if total > 0 else 0
        self.progress_bar.setValue(pct)

    def show_completed(self, success: bool, msg: str):
        self.progress_bar.setVisible(False)
        if success:
            self.lbl_status.setStyleSheet("color: #10B981; font-weight: bold;")
        else:
            self.lbl_status.setStyleSheet("color: #EF4444; font-weight: bold;")
        self.lbl_status.setText(msg)
