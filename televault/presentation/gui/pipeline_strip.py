from PyQt6.QtCore import Qt
from PyQt6.QtWidgets import QFrame, QHBoxLayout, QLabel, QProgressBar, QVBoxLayout


class PipelineStripWidget(QFrame):
    """Live visual progress strip displaying active backup pipeline stages."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("panel")
        self.setStyleSheet("""
            QFrame#panel {
                background-color: #0E1116;
                border: 1px solid #242A35;
                padding: 12px 16px;
            }
        """)
        self._init_ui()

    def _init_ui(self):
        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(12, 10, 12, 10)
        main_layout.setSpacing(8)

        # Header info line
        header_layout = QHBoxLayout()
        self.lbl_active_file = QLabel("PIPELINE: Idle (Drop files above to initiate backup)")
        self.lbl_active_file.setStyleSheet("font-weight: bold; color: #ECEFF4; font-size: 12px;")
        header_layout.addWidget(self.lbl_active_file)

        header_layout.addStretch()

        self.lbl_telemetry = QLabel("")
        self.lbl_telemetry.setStyleSheet("color: #F59E0B; font-size: 11px;")
        header_layout.addWidget(self.lbl_telemetry)

        main_layout.addLayout(header_layout)

        # 4 Stage indicators
        stages_layout = QHBoxLayout()
        stages_layout.setSpacing(8)

        self.stage_hash = QLabel("1. SHA-256")
        self.stage_upload = QLabel("2. UPLOAD (PRIMARY)")
        self.stage_mirror = QLabel("3. SERVER MIRROR")
        self.stage_verify = QLabel("4. DUAL VERIFY")

        self.all_stages = [
            self.stage_hash,
            self.stage_upload,
            self.stage_mirror,
            self.stage_verify,
        ]

        for s in self.all_stages:
            s.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self._set_stage_state(s, "idle")
            stages_layout.addWidget(s)

        main_layout.addLayout(stages_layout)

        # Progress bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.progress_bar.setTextVisible(True)
        main_layout.addWidget(self.progress_bar)

    def _set_stage_state(self, lbl: QLabel, state: str):
        if state == "done":
            lbl.setStyleSheet("background-color: rgba(16, 185, 129, 0.1); color: #10B981; border: 1px solid #10B981; padding: 4px; font-size: 11px;")
        elif state == "active":
            lbl.setStyleSheet("background-color: rgba(245, 158, 11, 0.15); color: #F59E0B; border: 1px solid #F59E0B; padding: 4px; font-weight: bold; font-size: 11px;")
        else:
            lbl.setStyleSheet("background-color: #14171C; color: #7E889B; border: 1px solid #242A35; padding: 4px; font-size: 11px;")

    def start_pipeline(self, filename: str, size_bytes: int):
        self.lbl_active_file.setText(f"PIPELINE: {filename} ({size_bytes / (1024*1024):.1f} MB)")
        self.lbl_telemetry.setText("Starting...")
        self.progress_bar.setValue(0)
        for s in self.all_stages:
            self._set_stage_state(s, "idle")
        self._set_stage_state(self.stage_hash, "active")

    def set_upload_progress(self, uploaded_bytes: int, total_bytes: int, speed_str: str = ""):
        self._set_stage_state(self.stage_hash, "done")
        self._set_stage_state(self.stage_upload, "active")
        pct = int((uploaded_bytes / total_bytes) * 100) if total_bytes > 0 else 0
        self.progress_bar.setValue(pct)
        if speed_str:
            self.lbl_telemetry.setText(speed_str)

    def set_mirror_stage(self):
        self._set_stage_state(self.stage_upload, "done")
        self._set_stage_state(self.stage_mirror, "active")
        self.lbl_telemetry.setText("Forwarding server-side to Mirror...")

    def set_verify_stage(self):
        self._set_stage_state(self.stage_mirror, "done")
        self._set_stage_state(self.stage_verify, "active")
        self.lbl_telemetry.setText("Verifying dual cloud presence...")

    def complete_pipeline(self):
        for s in self.all_stages:
            self._set_stage_state(s, "done")
        self.progress_bar.setValue(100)
        self.lbl_telemetry.setText("Protected & Verified [OK]")

    def reset_pipeline(self):
        self.lbl_active_file.setText("PIPELINE: Idle (Drop files above to initiate backup)")
        self.lbl_telemetry.setText("")
        self.progress_bar.setValue(0)
        for s in self.all_stages:
            self._set_stage_state(s, "idle")
