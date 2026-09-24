from datetime import datetime
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from televault.domain.entities import FileRecord
from televault.domain.states import HealthState


class VaultTableWidget(QFrame):
    """Vault metadata table with search, tags, health badges, and actions."""

    restore_clicked = pyqtSignal(str)  # record_id
    backup_again_clicked = pyqtSignal(str)  # record_id
    remove_clicked = pyqtSignal(str)  # record_id

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("panel")
        self._records: list[FileRecord] = []
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(10)

        # Search bar
        search_layout = QHBoxLayout()
        search_layout.setContentsMargins(12, 10, 12, 0)

        self.search_input = QLineEdit()
        self.search_input.setPlaceholderText("Search by name, sha256 or #tag...")
        self.search_input.textChanged.connect(self._filter_table)
        search_layout.addWidget(self.search_input)

        layout.addLayout(search_layout)

        # Table widget
        self.table = QTableWidget()
        self.table.setColumnCount(7)
        self.table.setHorizontalHeaderLabels([
            "Name", "Size", "Backed Up", "Health", "Ver", "Local Status", "Actions"
        ])
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        self.table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(4, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(5, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(6, QHeaderView.ResizeMode.ResizeToContents)
        self.table.verticalHeader().setVisible(False)
        self.table.setSelectionBehavior(QTableWidget.SelectionBehavior.SelectRows)
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)

        layout.addWidget(self.table)

    def load_records(self, records: list[FileRecord]):
        self._records = records
        self._filter_table(self.search_input.text())

    def _filter_table(self, query: str):
        query = query.strip().lower()
        filtered = [
            r for r in self._records
            if not query or query in r.name.lower() or query in r.sha256.lower() or any(query in t.lower() for t in r.tags)
        ]

        self.table.setRowCount(len(filtered))
        for row, rec in enumerate(filtered):
            # Name
            item_name = QTableWidgetItem(rec.name)
            item_name.setToolTip(f"ID: {rec.id}\nSHA-256: {rec.sha256}\nOriginal Path: {rec.original_path}")
            self.table.setItem(row, 0, item_name)

            # Size
            size_mb = rec.size / (1024 * 1024)
            size_str = f"{size_mb:.2f} MB" if size_mb >= 1.0 else f"{rec.size / 1024:.1f} KB"
            item_size = QTableWidgetItem(size_str)
            item_size.setTextAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
            self.table.setItem(row, 1, item_size)

            # Backed Up Date
            date_str = rec.created_at.strftime("%Y-%m-%d %H:%M") if rec.created_at else "—"
            item_date = QTableWidgetItem(date_str)
            self.table.setItem(row, 2, item_date)

            # Health Badge
            health_lbl = QLabel(f"● {rec.state.value}")
            if rec.state == HealthState.HEALTHY:
                health_lbl.setStyleSheet("color: #10B981; font-weight: bold; font-size: 11px;")
            elif rec.state == HealthState.DEGRADED:
                health_lbl.setStyleSheet("color: #F59E0B; font-weight: bold; font-size: 11px;")
            else:
                health_lbl.setStyleSheet("color: #EF4444; font-weight: bold; font-size: 11px;")
            health_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self.table.setCellWidget(row, 3, health_lbl)

            # Version
            item_ver = QTableWidgetItem(f"v{rec.version}")
            item_ver.setTextAlignment(Qt.AlignmentFlag.AlignCenter)
            self.table.setItem(row, 4, item_ver)

            # Local Status
            item_local = QTableWidgetItem(rec.local_status.value)
            if rec.local_status.value == "MISSING":
                item_local.setText("Only in Telegram")
                item_local.setForeground(Qt.GlobalColor.gray)
            elif rec.local_status.value == "CHANGED":
                item_local.setText("Changed on PC")
                item_local.setForeground(Qt.GlobalColor.yellow)
            self.table.setItem(row, 5, item_local)

            # Action Buttons
            action_widget = QWidget()
            action_layout = QHBoxLayout(action_widget)
            action_layout.setContentsMargins(4, 2, 4, 2)
            action_layout.setSpacing(6)

            btn_restore = QPushButton("Restore")
            btn_restore.setStyleSheet("padding: 3px 8px; font-size: 11px;")
            btn_restore.clicked.connect(lambda _, rid=rec.id: self.restore_clicked.emit(rid))
            action_layout.addWidget(btn_restore)

            if rec.local_status.value == "CHANGED":
                btn_backup_again = QPushButton("Back up again")
                btn_backup_again.setObjectName("btnPrimary")
                btn_backup_again.setStyleSheet("padding: 3px 8px; font-size: 11px;")
                btn_backup_again.clicked.connect(lambda _, rid=rec.id: self.backup_again_clicked.emit(rid))
                action_layout.addWidget(btn_backup_again)

            self.table.setCellWidget(row, 6, action_widget)
