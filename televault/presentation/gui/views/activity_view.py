from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import (
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
    QWidget,
)

from televault.domain.entities import AuditEvent


class ActivityView(QWidget):
    """Activity stream & tamper-evident audit ledger viewer."""

    def __init__(self):
        super().__init__()
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(14)

        # Header
        top = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("Activity & Audit Ledger")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("Append-only, SHA-256 hash-chained forensic audit trail of all vault operations.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top.addLayout(vbox)
        top.addStretch()

        self.integrity_pill = QLabel("CHAIN INTEGRITY: VERIFIED")
        self.integrity_pill.setProperty("class", "pillHealthy")
        top.addWidget(self.integrity_pill, alignment=Qt.AlignmentFlag.AlignVCenter)
        layout.addLayout(top)

        # Table of audit events
        self.table = QTableWidget()
        self.table.setColumnCount(5)
        self.table.setHorizontalHeaderLabels([
            "Timestamp",
            "Action",
            "Entity ID",
            "Result",
            "Forensic Details",
        ])
        header = self.table.horizontalHeader()
        header.setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        header.setSectionResizeMode(4, QHeaderView.ResizeMode.Stretch)
        self.table.verticalHeader().setVisible(False)
        layout.addWidget(self.table)

    def update_events(self, events: list[AuditEvent], is_valid: bool = True):
        color_class = "pillHealthy" if is_valid else "pillLost"
        self.integrity_pill.setText(f"CHAIN INTEGRITY: {'VERIFIED' if is_valid else 'TAMPERED'}")
        self.integrity_pill.setProperty("class", color_class)
        self.integrity_pill.style().unpolish(self.integrity_pill)
        self.integrity_pill.style().polish(self.integrity_pill)

        self.table.setRowCount(len(events))
        for row, e in enumerate(events):
            t_item = QTableWidgetItem(e.timestamp.strftime("%Y-%m-%d %H:%M:%S"))
            a_item = QTableWidgetItem(e.action)
            a_item.setStyleSheet("font-weight: 600;")
            ent_item = QTableWidgetItem(e.entity_id or "-")
            r_item = QTableWidgetItem(e.result)
            if e.result == "SUCCESS":
                r_item.setForeground(Qt.GlobalColor.green)
            else:
                r_item.setForeground(Qt.GlobalColor.red)
            d_item = QTableWidgetItem(e.details or "-")

            self.table.setItem(row, 0, t_item)
            self.table.setItem(row, 1, a_item)
            self.table.setItem(row, 2, ent_item)
            self.table.setItem(row, 3, r_item)
            self.table.setItem(row, 4, d_item)
