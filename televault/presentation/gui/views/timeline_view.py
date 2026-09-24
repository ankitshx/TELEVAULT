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


class TimelineView(QWidget):
    """Time Machine Visual Version Tree for historical file generations."""

    restore_version_requested = pyqtSignal(FileRecord)

    def __init__(self):
        super().__init__()
        self._all_records: list[FileRecord] = []
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(14)

        # Header
        top = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("Time Machine & Version Tree")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("Append-only historical lineages. Restore any past generation with verified SHA-256 integrity.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top.addLayout(vbox)
        top.addStretch()

        self.filter_input = QLineEdit()
        self.filter_input.setPlaceholderText("Filter by file name...")
        self.filter_input.setFixedWidth(240)
        self.filter_input.textChanged.connect(self._render_timeline)
        top.addWidget(self.filter_input)
        layout.addLayout(top)

        # Visual Version Tree Panel
        self.tree_card = QFrame()
        self.tree_card.setProperty("class", "fluentCard")
        tree_layout = QVBoxLayout(self.tree_card)
        tree_layout.setContentsMargins(16, 12, 16, 12)

        self.tree_banner = QLabel("Select a file to view version tree: v1 → v2 → v3 [CURRENT]")
        self.tree_banner.setStyleSheet("font-size: 13px; font-weight: 600; color: #38BDF8;")
        tree_layout.addWidget(self.tree_banner)
        layout.addWidget(self.tree_card)

        # Table of versions
        self.table = QTableWidget()
        self.table.setColumnCount(6)
        self.table.setHorizontalHeaderLabels([
            "File Name",
            "Version",
            "SHA-256 Fingerprint",
            "Size",
            "Backup Date",
            "Action",
        ])
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.Stretch)
        header.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(4, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(5, QHeaderView.ResizeMode.ResizeToContents)
        self.table.verticalHeader().setVisible(False)
        layout.addWidget(self.table)

    def update_records(self, records: list[FileRecord]):
        self._all_records = records
        self._render_timeline()

    def _render_timeline(self):
        query = self.filter_input.text().strip().lower()
        records = self._all_records
        if query:
            records = [r for r in records if query in r.name.lower()]

        # Group by name to trace versions
        name_groups: dict[str, list[FileRecord]] = {}
        for r in records:
            name_groups.setdefault(r.name, []).append(r)

        multi_ver = [name for name, vlist in name_groups.items() if len(vlist) > 1]
        if multi_ver:
            sample_name = multi_ver[0]
            v_count = len(name_groups[sample_name])
            chain_str = " → ".join(f"v{i}" for i in range(1, v_count + 1))
            self.tree_banner.setText(f"Active Version Lineage: {sample_name} ({chain_str} [CURRENT])")
        else:
            self.tree_banner.setText("All files currently at v1 (Initial generation). Modifying files creates new append-only generations.")

        self.table.setRowCount(len(records))
        for row, rec in enumerate(records):
            item_name = QTableWidgetItem(rec.name)
            item_ver = QTableWidgetItem(f"v{rec.version}")
            item_ver.setTextAlignment(Qt.AlignmentFlag.AlignCenter)
            item_hash = QTableWidgetItem(f"{rec.sha256[:12]}...{rec.sha256[-6:]}")
            item_hash.setTextAlignment(Qt.AlignmentFlag.AlignCenter)
            size_kb = rec.size / 1024
            size_str = f"{size_kb / 1024:.2f} MB" if size_kb >= 1024 else f"{size_kb:.1f} KB"
            item_size = QTableWidgetItem(size_str)
            item_size.setTextAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
            item_date = QTableWidgetItem(rec.created_at.strftime("%Y-%m-%d %H:%M"))
            item_date.setTextAlignment(Qt.AlignmentFlag.AlignCenter)

            self.table.setItem(row, 0, item_name)
            self.table.setItem(row, 1, item_ver)
            self.table.setItem(row, 2, item_hash)
            self.table.setItem(row, 3, item_size)
            self.table.setItem(row, 4, item_date)

            btn_restore = QPushButton("Restore This Version")
            btn_restore.clicked.connect(lambda _, r=rec: self.restore_version_requested.emit(r))
            self.table.setCellWidget(row, 5, btn_restore)
