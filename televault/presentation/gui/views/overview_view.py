from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QFrame,
    QGridLayout,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from televault.domain.entities import FileRecord
from televault.domain.states import HealthState


class StatCard(QFrame):
    def __init__(self, label: str, initial_value: str = "0", subtext: str = ""):
        super().__init__()
        self.setProperty("class", "statCard")
        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 14, 16, 14)
        layout.setSpacing(4)

        self.lbl_label = QLabel(label.upper())
        self.lbl_label.setProperty("class", "cardLabel")
        layout.addWidget(self.lbl_label)

        self.lbl_value = QLabel(initial_value)
        self.lbl_value.setProperty("class", "cardValue")
        layout.addWidget(self.lbl_value)

        self.lbl_subtext = QLabel(subtext)
        self.lbl_subtext.setStyleSheet("color: #9CA3AF; font-size: 11px;")
        layout.addWidget(self.lbl_subtext)

    def set_value(self, value: str, subtext: str | None = None):
        self.lbl_value.setText(value)
        if subtext is not None:
            self.lbl_subtext.setText(subtext)


class OverviewView(QWidget):
    """Answers the 5 Fundamental Questions clearly at first glance."""

    backup_requested = pyqtSignal()
    verify_requested = pyqtSignal()
    recovery_drill_requested = pyqtSignal()
    doctor_requested = pyqtSignal()

    def __init__(self):
        super().__init__()
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 24, 28, 24)
        layout.setSpacing(20)

        # Welcome Banner / Title
        title_box = QHBoxLayout()
        vbox = QVBoxLayout()
        h_title = QLabel("Vault Overview")
        h_title.setStyleSheet("font-size: 22px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(h_title)
        sub_title = QLabel("Append-only, verifiable personal backup vault powered by Telegram MTProto.")
        sub_title.setStyleSheet("color: #9CA3AF; font-size: 13px;")
        vbox.addWidget(sub_title)
        title_box.addLayout(vbox)
        title_box.addStretch()

        self.status_pill = QLabel("● ALL SYSTEMS SECURE")
        self.status_pill.setProperty("class", "pillHealthy")
        title_box.addWidget(self.status_pill, alignment=Qt.AlignmentFlag.AlignVCenter)
        layout.addLayout(title_box)

        # 5 Key Question Cards Grid
        grid = QGridLayout()
        grid.setSpacing(14)

        # 1. Protected Files
        self.card_files = StatCard("1. Protected Files", "0", "Indexed in vault")
        grid.addWidget(self.card_files, 0, 0)

        # 2. Cloud Copies (Dual-Write)
        self.card_copies = StatCard("2. Dual-Write Redundancy", "100%", "Primary + Mirror channels")
        grid.addWidget(self.card_copies, 0, 1)

        # 3. Needs Attention
        self.card_attention = StatCard("3. Needs Attention", "0", "Degraded or modified files")
        grid.addWidget(self.card_attention, 0, 2)

        # 4. Recovery Readiness
        self.card_readiness = StatCard("4. Recovery Readiness", "100 / 100", "Verified restore capable")
        grid.addWidget(self.card_readiness, 1, 0)

        # 5. Storage Used
        self.card_storage = StatCard("5. Cloud Storage Used", "0.0 MB", "Telegram MTProto cloud")
        grid.addWidget(self.card_storage, 1, 1)

        # Quick Health Snapshot
        self.card_channels = StatCard("Channel Redundancy", "2 / 2", "Primary & Mirror active")
        grid.addWidget(self.card_channels, 1, 2)

        layout.addLayout(grid)

        # Quick Actions Card
        actions_card = QFrame()
        actions_card.setProperty("class", "fluentCard")
        act_layout = QVBoxLayout(actions_card)
        act_layout.setContentsMargins(18, 16, 18, 16)
        act_layout.setSpacing(12)

        act_title = QLabel("Quick Actions")
        act_title.setStyleSheet("font-size: 14px; font-weight: 600; color: #FFFFFF;")
        act_layout.addWidget(act_title)

        btn_row = QHBoxLayout()
        btn_row.setSpacing(12)

        self.btn_backup = QPushButton("+ Back Up File / Folder")
        self.btn_backup.setProperty("class", "btnAccent")
        self.btn_backup.clicked.connect(self.backup_requested.emit)
        btn_row.addWidget(self.btn_backup)

        self.btn_verify = QPushButton("Verify Vault Health")
        self.btn_verify.clicked.connect(self.verify_requested.emit)
        btn_row.addWidget(self.btn_verify)

        self.btn_drill = QPushButton("Run Recovery Drill")
        self.btn_drill.clicked.connect(self.recovery_drill_requested.emit)
        btn_row.addWidget(self.btn_drill)

        self.btn_doctor = QPushButton("Vault Doctor Scan")
        self.btn_doctor.clicked.connect(self.doctor_requested.emit)
        btn_row.addWidget(self.btn_doctor)

        btn_row.addStretch()
        act_layout.addLayout(btn_row)
        layout.addWidget(actions_card)

        layout.addStretch()

    def update_telemetry(self, records: list[FileRecord]):
        total = len(records)
        healthy = sum(1 for r in records if r.state == HealthState.HEALTHY)
        degraded = sum(1 for r in records if r.state == HealthState.DEGRADED)
        lost = sum(1 for r in records if r.state == HealthState.LOST)
        total_bytes = sum(r.size for r in records)

        self.card_files.set_value(str(total), f"{healthy} healthy files")
        pct = int((healthy / total) * 100) if total > 0 else 100
        self.card_copies.set_value(f"{pct}%", "Primary & Mirror synced")
        self.card_attention.set_value(str(degraded + lost), f"{degraded} degraded, {lost} lost")

        readiness = 100
        if total > 0:
            readiness = int((healthy / total) * 100)
            if lost > 0:
                readiness = max(0, readiness - 30)
        self.card_readiness.set_value(f"{readiness} / 100", "Instant restore capability")

        mb = total_bytes / (1024 * 1024)
        if mb >= 1024:
            self.card_storage.set_value(f"{mb / 1024:.2f} GB", f"{total} documents")
        else:
            self.card_storage.set_value(f"{mb:.2f} MB", f"{total} documents")

        if lost > 0:
            self.status_pill.setText("● ATTENTION REQUIRED")
            self.status_pill.setProperty("class", "pillLost")
        elif degraded > 0:
            self.status_pill.setText("● DEGRADED - HEALING READY")
            self.status_pill.setProperty("class", "pillDegraded")
        else:
            self.status_pill.setText("● ALL SYSTEMS SECURE")
            self.status_pill.setProperty("class", "pillHealthy")
        self.status_pill.style().unpolish(self.status_pill)
        self.status_pill.style().polish(self.status_pill)
