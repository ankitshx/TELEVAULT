from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QProgressBar,
    QPushButton,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from televault.application.recovery_drill import RecoveryDrillReport


class RecoveryView(QWidget):
    """Recovery Center: Non-destructive recovery drills and disaster recovery rebuild."""

    run_drill_requested = pyqtSignal()
    rebuild_vault_requested = pyqtSignal()

    def __init__(self):
        super().__init__()
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(16)

        # Header
        top = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("Recovery Center & Readiness")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("Verify backup recoverability with non-destructive drills and cloud disaster recovery.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top.addLayout(vbox)
        top.addStretch()

        self.score_pill = QLabel("READINESS: 100 / 100")
        self.score_pill.setProperty("class", "pillHealthy")
        top.addWidget(self.score_pill, alignment=Qt.AlignmentFlag.AlignVCenter)
        layout.addLayout(top)

        # Non-Destructive Recovery Drill Card
        drill_card = QFrame()
        drill_card.setProperty("class", "fluentCard")
        drill_layout = QVBoxLayout(drill_card)
        drill_layout.setContentsMargins(18, 16, 18, 16)
        drill_layout.setSpacing(12)

        drill_title = QLabel("Non-Destructive Recovery Drill")
        drill_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        drill_layout.addWidget(drill_title)

        drill_desc = QLabel(
            "Downloads a random cloud document to a temporary isolated scratch directory, calculates "
            "and verifies its SHA-256 fingerprint against the local index, tests decryption if encrypted, "
            "and deletes the temporary scratch files. Zero risk to existing data."
        )
        drill_desc.setWordWrap(True)
        drill_desc.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        drill_layout.addWidget(drill_desc)

        h_btn = QHBoxLayout()
        self.btn_run_drill = QPushButton("Execute Non-Destructive Recovery Drill")
        self.btn_run_drill.setProperty("class", "btnAccent")
        self.btn_run_drill.clicked.connect(self.run_drill_requested.emit)
        h_btn.addWidget(self.btn_run_drill)
        h_btn.addStretch()
        drill_layout.addLayout(h_btn)

        self.drill_progress = QProgressBar()
        self.drill_progress.setVisible(False)
        self.drill_progress.setRange(0, 0)
        drill_layout.addWidget(self.drill_progress)

        self.drill_output = QTextEdit()
        self.drill_output.setReadOnly(True)
        self.drill_output.setMaximumHeight(130)
        self.drill_output.setPlaceholderText("Drill output and cryptographic verification reports will appear here.")
        drill_layout.addWidget(self.drill_output)

        layout.addWidget(drill_card)

        # Disaster Recovery Card
        dr_card = QFrame()
        dr_card.setProperty("class", "fluentCard")
        dr_layout = QVBoxLayout(dr_card)
        dr_layout.setContentsMargins(18, 16, 18, 16)
        dr_layout.setSpacing(12)

        dr_title = QLabel("Disaster Recovery (Cloud Rebuild)")
        dr_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        dr_layout.addWidget(dr_title)

        dr_desc = QLabel(
            "If your computer is wiped or the local database is lost, TeleVault can rebuild your entire "
            "vault metadata index by scanning your Telegram Primary and Mirror channels and reading "
            "the embedded tv1/tv2 cryptographic captions."
        )
        dr_desc.setWordWrap(True)
        dr_desc.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        dr_layout.addWidget(dr_desc)

        h_dr = QHBoxLayout()
        self.btn_rebuild = QPushButton("Rebuild Local Index from Telegram Channels")
        self.btn_rebuild.clicked.connect(self.rebuild_vault_requested.emit)
        h_dr.addWidget(self.btn_rebuild)
        h_dr.addStretch()
        dr_layout.addLayout(h_dr)

        layout.addWidget(dr_card)
        layout.addStretch()

    def set_drill_running(self, running: bool):
        self.btn_run_drill.setEnabled(not running)
        self.drill_progress.setVisible(running)

    def display_drill_report(self, report: RecoveryDrillReport):
        status_txt = "PASSED ✓" if report.passed else "FAILED ✗"
        color = "#10B981" if report.passed else "#EF4444"
        lines = [
            f"=== RECOVERY DRILL REPORT [{status_txt}] ===",
            f"Target Record:   {report.file_name} ({report.record_id})",
            f"File Size:       {report.file_size} bytes",
            f"Drill Mode:      {report.mode.value}",
            f"Elapsed Time:    {report.duration_seconds:.2f} seconds",
            f"SHA-256 Match:   {'YES (Exact Match)' if report.sha256_matched else 'NO (MISMATCH)'}",
            f"Scratch Cleaned: {'YES (Isolated scratch space deleted)' if report.scratch_cleaned else 'NO'}",
            f"Summary:         {report.message}",
        ]
        self.drill_output.setPlainText("\n".join(lines))
        self.drill_output.setStyleSheet(f"border-color: {color}; font-family: monospace;")
