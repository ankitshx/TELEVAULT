from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from televault.domain.entities import FileRecord
from televault.domain.states import HealthState, LocalFileStatus, VaultMode
from televault.presentation.gui.dropzone import DropZoneWidget
from televault.presentation.gui.pipeline_strip import PipelineStripWidget
from televault.presentation.gui.vault_table import VaultTableWidget


class FilesView(QWidget):
    """Files view with Search (Ctrl+F), filter chips, dropzone, and vault table."""

    restore_requested = pyqtSignal(FileRecord)
    verify_single_requested = pyqtSignal(FileRecord)
    backup_file_requested = pyqtSignal(str, bool)  # path, is_private

    def __init__(self):
        super().__init__()
        self._all_records: list[FileRecord] = []
        self._active_filter: str = "ALL"
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(14)

        # Header with Search & Filter
        top_row = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("Vault Files")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("Search, inspect, and restore files from Telegram cloud.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top_row.addLayout(vbox)
        top_row.addStretch()

        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("Search by file name or SHA-256 (Ctrl+F)...")
        self.search_input.setFixedWidth(280)
        self.search_input.textChanged.connect(self._apply_filter)
        top_row.addWidget(self.search_input)

        layout.addLayout(top_row)

        # Filter Chips Row
        chips_row = QHBoxLayout()
        chips_row.setSpacing(8)

        self.btn_all = QPushButton("All")
        self.btn_all.setCheckable(True)
        self.btn_all.setChecked(True)
        self.btn_all.clicked.connect(lambda: self._set_filter("ALL"))
        chips_row.addWidget(self.btn_all)

        self.btn_healthy = QPushButton("Healthy")
        self.btn_healthy.setCheckable(True)
        self.btn_healthy.clicked.connect(lambda: self._set_filter("HEALTHY"))
        chips_row.addWidget(self.btn_healthy)

        self.btn_changed = QPushButton("Changed on PC")
        self.btn_changed.setCheckable(True)
        self.btn_changed.clicked.connect(lambda: self._set_filter("CHANGED"))
        chips_row.addWidget(self.btn_changed)

        self.btn_degraded = QPushButton("Degraded")
        self.btn_degraded.setCheckable(True)
        self.btn_degraded.clicked.connect(lambda: self._set_filter("DEGRADED"))
        chips_row.addWidget(self.btn_degraded)

        self.btn_private = QPushButton("Private Mode")
        self.btn_private.setCheckable(True)
        self.btn_private.clicked.connect(lambda: self._set_filter("PRIVATE"))
        chips_row.addWidget(self.btn_private)

        chips_row.addStretch()
        layout.addLayout(chips_row)

        # Dropzone for Drag-and-Drop
        self.dropzone = DropZoneWidget()
        self.dropzone.files_dropped.connect(self._on_files_dropped)
        layout.addWidget(self.dropzone)

        # Live Progress Pipeline Strip
        self.pipeline_strip = PipelineStripWidget()
        layout.addWidget(self.pipeline_strip)

        # Vault Table
        self.table = VaultTableWidget()
        self.table.restore_clicked.connect(self.restore_requested.emit)
        layout.addWidget(self.table)

    def _on_files_dropped(self, paths: list):
        for p in paths:
            self.backup_file_requested.emit(str(p), False)

    def _set_filter(self, filter_name: str):
        self._active_filter = filter_name
        self.btn_all.setChecked(filter_name == "ALL")
        self.btn_healthy.setChecked(filter_name == "HEALTHY")
        self.btn_changed.setChecked(filter_name == "CHANGED")
        self.btn_degraded.setChecked(filter_name == "DEGRADED")
        self.btn_private.setChecked(filter_name == "PRIVATE")
        self._apply_filter()

    def _apply_filter(self):
        query = self.search_input.text().strip().lower()
        filtered = self._all_records

        if query:
            filtered = [r for r in filtered if query in r.name.lower() or query in r.sha256.lower()]

        if self._active_filter == "HEALTHY":
            filtered = [r for r in filtered if r.state == HealthState.HEALTHY]
        elif self._active_filter == "CHANGED":
            filtered = [r for r in filtered if r.local_status == LocalFileStatus.CHANGED]
        elif self._active_filter == "DEGRADED":
            filtered = [r for r in filtered if r.state == HealthState.DEGRADED]
        elif self._active_filter == "PRIVATE":
            filtered = [r for r in filtered if r.mode == VaultMode.PRIVATE]

        self.table.load_records(filtered)

    def update_records(self, records: list[FileRecord]):
        self._all_records = records
        self._apply_filter()
