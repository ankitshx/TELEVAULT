from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QDialog,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
)

from televault.application.verify import RecordVerificationResult


class HealthReportDialog(QDialog):
    """Displays vault health diagnosis and provides a one-click 'Heal Now' action."""

    heal_requested = pyqtSignal()

    def __init__(self, results: list[RecordVerificationResult], parent=None):
        super().__init__(parent)
        self.results = results
        self.setWindowTitle("TeleVault Health & Diagnostics Report")
        self.setMinimumSize(700, 420)
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setSpacing(14)

        header = QLabel("VAULT HEALTH REPORT & REDUNDANCY DIAGNOSTICS")
        header.setStyleSheet("font-family: 'JetBrains Mono'; font-weight: bold; color: #ECEFF4; font-size: 13px;")
        layout.addWidget(header)

        desc = QLabel(
            "TeleVault verifies message presence in Primary and Mirror channels. "
            "Surviving copies are forwarded server-side with zero re-upload bandwidth."
        )
        desc.setStyleSheet("color: #7E889B; font-size: 11px;")
        layout.addWidget(desc)

        # Table
        self.table = QTableWidget()
        self.table.setColumnCount(3)
        self.table.setHorizontalHeaderLabels(["File Name", "Health State", "Diagnosis"])
        self.table.horizontalHeader().setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(1, QHeaderView.ResizeMode.ResizeToContents)
        self.table.horizontalHeader().setSectionResizeMode(2, QHeaderView.ResizeMode.Stretch)
        self.table.verticalHeader().setVisible(False)
        self.table.setEditTriggers(QTableWidget.EditTrigger.NoEditTriggers)

        self._populate_table()
        layout.addWidget(self.table)

        # Footer action line
        footer_layout = QHBoxLayout()
        degraded_count = sum(1 for r in self.results if r.new_state.value == "DEGRADED")

        self.lbl_summary = QLabel(f"{degraded_count} degraded item(s) found eligible for healing.")
        self.lbl_summary.setStyleSheet("color: #F59E0B; font-size: 11px;")
        footer_layout.addWidget(self.lbl_summary)

        footer_layout.addStretch()

        btn_close = QPushButton("CLOSE")
        btn_close.clicked.connect(self.accept)
        footer_layout.addWidget(btn_close)

        self.btn_heal = QPushButton("HEAL NOW (SERVER FORWARD)")
        self.btn_heal.setObjectName("btnPrimary")
        self.btn_heal.setEnabled(degraded_count > 0)
        self.btn_heal.clicked.connect(self._on_heal)
        footer_layout.addWidget(self.btn_heal)

        layout.addLayout(footer_layout)

    def _populate_table(self):
        self.table.setRowCount(len(self.results))
        for row, r in enumerate(self.results):
            self.table.setItem(row, 0, QTableWidgetItem(r.name))

            state_item = QTableWidgetItem(r.new_state.value)
            if r.new_state.value == "HEALTHY":
                state_item.setForeground(Qt.GlobalColor.green)
            elif r.new_state.value == "DEGRADED":
                state_item.setForeground(Qt.GlobalColor.yellow)
            else:
                state_item.setForeground(Qt.GlobalColor.red)
            self.table.setItem(row, 1, state_item)

            self.table.setItem(row, 2, QTableWidgetItem(r.diagnosis))

    def _on_heal(self):
        self.btn_heal.setEnabled(False)
        self.btn_heal.setText("HEALING...")
        self.heal_requested.emit()
