import asyncio
from pathlib import Path
import sys
from PyQt6.QtCore import Qt
from PyQt6.QtGui import QCloseEvent
from PyQt6.QtWidgets import (
    QButtonGroup,
    QFileDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QStackedWidget,
    QVBoxLayout,
    QWidget,
)

from televault.ai.orchestrator import AIOrchestrator
from televault.application.backup import BackupFileUseCase
from televault.application.doctor import VaultDoctorUseCase
from televault.application.event_bus import SimpleEventBus
from televault.application.heal import HealVaultUseCase
from televault.application.manifest import ManifestService
from televault.application.rebuild import RebuildIndexUseCase
from televault.application.recovery_drill import RecoveryDrillUseCase
from televault.application.restore import RestoreFileUseCase
from televault.application.verify import VerifyVaultUseCase
from televault.domain.entities import FileRecord
from televault.domain.ports import TelegramGateway
from televault.domain.states import HealthState, VaultMode
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from televault.presentation.gui.async_bridge import AsyncBridge
from televault.presentation.gui.dialogs.health_dialog import HealthReportDialog
from televault.presentation.gui.dialogs.restore_dialog import RestoreDialog
from televault.presentation.gui.dialogs.settings_dialog import SettingsDialog
from televault.presentation.gui.theme import THEME_STYLESHEET
from televault.presentation.gui.tray import TeleVaultTrayIcon
from televault.presentation.gui.views.activity_view import ActivityView
from televault.presentation.gui.views.ai_doctor_view import AIDoctorView
from televault.presentation.gui.views.files_view import FilesView
from televault.presentation.gui.views.overview_view import OverviewView
from televault.presentation.gui.views.recovery_view import RecoveryView
from televault.presentation.gui.views.security_view import SecurityView
from televault.presentation.gui.views.settings_view import SettingsView
from televault.presentation.gui.views.timeline_view import TimelineView
from tests.fakes.fake_gateway import FakeTelegramGateway


class MainWindow(QMainWindow):
    """Main application shell for TeleVault featuring Windows 11 Fluent UI,
    compact sidebar navigation, 8 specialized views, and AI Doctor integration.
    """

    def __init__(self, gateway: TelegramGateway | None = None, config: TeleVaultConfig | None = None):
        super().__init__()
        self.setWindowTitle("TeleVault — Resilient Telegram Desktop Backup")
        self.setMinimumSize(1080, 720)
        self.setStyleSheet(THEME_STYLESHEET)

        self.config = config or TeleVaultConfig()
        self.config.ensure_directories()

        self.repo = SQLiteVaultRepository(self.config.db_path)
        self.recovery_cache = RecoveryCache(self.config.cache_dir)
        self.event_bus = SimpleEventBus()
        self.gateway = gateway or FakeTelegramGateway()

        # Core Application Use Cases
        self.backup_uc = BackupFileUseCase(self.gateway, self.repo, self.event_bus)
        self.restore_uc = RestoreFileUseCase(self.gateway, self.repo, self.event_bus)
        self.verify_uc = VerifyVaultUseCase(self.gateway, self.repo, self.recovery_cache, self.event_bus)
        self.heal_uc = HealVaultUseCase(self.gateway, self.repo, self.recovery_cache, self.event_bus)
        self.rebuild_uc = RebuildIndexUseCase(self.gateway, self.repo)
        self.recovery_drill_uc = RecoveryDrillUseCase(self.gateway, self.repo, self.config.recovery_dir, self.repo)
        self.manifest_service = ManifestService(self.repo, self.repo, vault_id="v_main", audit_ledger=self.repo)
        self.doctor_uc = VaultDoctorUseCase(self.gateway, self.repo, self.repo, self.repo, self.repo)

        # AI Advisory Orchestration
        self.ai_orchestrator = AIOrchestrator(
            repo=self.repo,
            gateway=self.gateway,
            manifest_repo=self.repo,
            audit_ledger=self.repo,
        )

        self._active_worker: AsyncBridge | None = None
        self._init_ui()
        self._refresh_vault_table()

    def _init_ui(self):
        central = QWidget()
        self.setCentralWidget(central)
        main_h_layout = QHBoxLayout(central)
        main_h_layout.setContentsMargins(0, 0, 0, 0)
        main_h_layout.setSpacing(0)

        # 1. Left Sidebar Navigation
        sidebar = QFrame()
        sidebar.setObjectName("sidebarPanel")
        sidebar.setFixedWidth(220)
        sidebar_layout = QVBoxLayout(sidebar)
        sidebar_layout.setContentsMargins(12, 16, 12, 16)
        sidebar_layout.setSpacing(6)

        # Brand Logo
        logo = QLabel("TELEVAULT")
        logo.setObjectName("sidebarLogo")
        sidebar_layout.addWidget(logo)

        sub_logo = QLabel("v2.0 • FLUID RECOVERY")
        sub_logo.setObjectName("sidebarSubtitle")
        sidebar_layout.addWidget(sub_logo)
        sidebar_layout.addSpacing(14)

        # Navigation Buttons Group
        self.nav_group = QButtonGroup(self)
        self.nav_buttons: list[QPushButton] = []

        nav_items = [
            ("⊞ Overview", 0),
            ("🗁 Files", 1),
            ("◷ Timeline", 2),
            ("⛨ Recovery", 3),
            ("⚕ AI Doctor", 4),
            ("🔒 Security", 5),
            ("📝 Activity", 6),
            ("⚙ Settings", 7),
        ]

        for text, index in nav_items:
            btn = QPushButton(text)
            btn.setProperty("class", "navBtn")
            btn.setCheckable(True)
            if index == 0:
                btn.setChecked(True)
            btn.clicked.connect(lambda _, idx=index: self._switch_view(idx))
            self.nav_group.addButton(btn)
            self.nav_buttons.append(btn)
            sidebar_layout.addWidget(btn)

        sidebar_layout.addStretch()

        # Connection status pill in sidebar footer
        self.sidebar_conn = QLabel("● Telegram Online")
        self.sidebar_conn.setStyleSheet("color: #10B981; font-size: 11px; font-weight: 600; padding: 6px 8px;")
        sidebar_layout.addWidget(self.sidebar_conn)

        main_h_layout.addWidget(sidebar)

        # 2. Main Stacked Views Container
        self.stack = QStackedWidget()

        # View 0: Overview
        self.overview_view = OverviewView()
        self.overview_view.backup_requested.connect(self._prompt_backup_file)
        self.overview_view.verify_requested.connect(self.run_verification)
        self.overview_view.recovery_drill_requested.connect(self._run_recovery_drill)
        self.overview_view.doctor_requested.connect(lambda: self._switch_view(4))
        self.stack.addWidget(self.overview_view)

        # View 1: Files
        self.files_view = FilesView()
        self.files_view.restore_requested.connect(lambda r: self._handle_restore(r.id))
        self.files_view.verify_single_requested.connect(self._handle_verify_single)
        self.files_view.backup_file_requested.connect(self._handle_backup_path)
        # Compatibility aliases for tests
        self.drop_zone = self.files_view.dropzone
        self.pipeline_strip = self.files_view.pipeline_strip
        self.vault_table = self.files_view.table
        self.drop_zone.files_dropped.connect(self._handle_files_dropped)
        self.vault_table.restore_clicked.connect(self._handle_restore)
        self.vault_table.backup_again_clicked.connect(self._handle_backup_again)
        self.stack.addWidget(self.files_view)

        # View 2: Timeline
        self.timeline_view = TimelineView()
        self.timeline_view.restore_version_requested.connect(lambda r: self._handle_restore(r.id))
        self.stack.addWidget(self.timeline_view)

        # View 3: Recovery
        self.recovery_view = RecoveryView()
        self.recovery_view.run_drill_requested.connect(self._run_recovery_drill)
        self.recovery_view.rebuild_vault_requested.connect(self._run_rebuild)
        self.stack.addWidget(self.recovery_view)

        # View 4: AI Doctor
        self.ai_doctor_view = AIDoctorView()
        self.ai_doctor_view.query_requested.connect(self._handle_ai_query)
        self.ai_doctor_view.proposal_approved.connect(self._handle_proposal_approved)
        self.ai_doctor_view.proposal_rejected.connect(self._handle_proposal_rejected)
        self.stack.addWidget(self.ai_doctor_view)

        # View 5: Security
        self.security_view = SecurityView()
        self.security_view.verify_manifest_requested.connect(self._verify_manifest)
        self.security_view.generate_manifest_requested.connect(self._generate_manifest)
        self.stack.addWidget(self.security_view)

        # View 6: Activity
        self.activity_view = ActivityView()
        self.stack.addWidget(self.activity_view)

        # View 7: Settings
        self.settings_view = SettingsView(self.config)
        self.settings_view.login_requested.connect(self._open_settings)
        self.stack.addWidget(self.settings_view)

        # Header for Global Badge
        content_box = QVBoxLayout()
        content_box.setContentsMargins(0, 0, 0, 0)
        content_box.setSpacing(0)

        header = QFrame()
        header.setObjectName("headerPanel")
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(20, 10, 20, 10)

        self.brand_title = QLabel("Personal Vault")
        self.brand_title.setObjectName("brandTitle")
        header_layout.addWidget(self.brand_title)

        self.badge_global = QLabel("● ALL FILES SAFE")
        self.badge_global.setObjectName("badgeHealthy")
        header_layout.addWidget(self.badge_global)

        header_layout.addStretch()

        btn_verify = QPushButton("Verify Vault")
        btn_verify.clicked.connect(self.run_verification)
        header_layout.addWidget(btn_verify)

        content_box.addWidget(header)
        content_box.addWidget(self.stack)

        main_h_layout.addLayout(content_box)

        # System Tray icon
        self.tray = TeleVaultTrayIcon(self)
        self.tray.show()

    def _switch_view(self, index: int):
        self.stack.setCurrentIndex(index)
        for i, btn in enumerate(self.nav_buttons):
            btn.setChecked(i == index)
        self._refresh_vault_table()

    def _refresh_vault_table(self):
        records = self.repo.list_all()
        self.files_view.update_records(records)
        self.overview_view.update_telemetry(records)
        self.timeline_view.update_records(records)

        # Activity view
        try:
            events = self.repo.list_events(limit=50)
            is_valid, _ = self.repo.verify_integrity()
            self.activity_view.update_events(events, is_valid)
        except Exception:
            pass

        # Global Badge status
        any_lost = any(r.state == HealthState.LOST for r in records)
        any_degraded = any(r.state == HealthState.DEGRADED for r in records)

        if any_lost:
            self.badge_global.setText("▲ 1+ FILES LOST")
            self.badge_global.setObjectName("badgeLost")
        elif any_degraded:
            self.badge_global.setText("▲ DEGRADED (HEALABLE)")
            self.badge_global.setObjectName("badgeDegraded")
        else:
            self.badge_global.setText("● ALL FILES SAFE")
            self.badge_global.setObjectName("badgeHealthy")

    def _prompt_backup_file(self):
        file_path, _ = QFileDialog.getOpenFileName(self, "Select File to Protect in TeleVault")
        if file_path:
            self._start_backup_task(Path(file_path))

    def _handle_backup_path(self, path_str: str, is_private: bool):
        p = Path(path_str)
        if p.is_file():
            mode = VaultMode.PRIVATE if is_private else VaultMode.ORIGINAL
            self._start_backup_task(p, mode=mode)

    def _handle_files_dropped(self, paths: list[Path]):
        for p in paths:
            if p.is_file():
                self._start_backup_task(p)

    def _start_backup_task(self, file_path: Path, mode: VaultMode = VaultMode.ORIGINAL):
        self.files_view.pipeline_strip.start_pipeline(file_path.name, file_path.stat().st_size)

        async def _run():
            return await self.backup_uc.execute(file_path, mode=mode)

        self._active_worker = AsyncBridge(_run)
        self._active_worker.task_completed.connect(self._on_backup_completed)
        self._active_worker.task_failed.connect(self._on_task_failed)
        self._active_worker.start()

    def _on_backup_completed(self, result):
        self.files_view.pipeline_strip.complete_pipeline()
        self._refresh_vault_table()
        if result.is_duplicate:
            QMessageBox.information(self, "Deduplication", result.message)

    def _handle_restore(self, record_id: str):
        record = self.repo.get(record_id)
        dlg = RestoreDialog(record, parent=self)
        if dlg.exec():
            dest_dir = dlg.selected_destination

            async def _run():
                return await self.restore_uc.execute(record_id, dest_dir=dest_dir)

            worker = AsyncBridge(_run)

            def _done(res):
                dlg.show_completed(res.success, res.message)
                if res.success:
                    QMessageBox.information(
                        self, "Restore Completed",
                        f"File restored cleanly to:\n{res.restored_path}\n\nSHA-256 integrity match confirmed."
                    )

            worker.task_completed.connect(_done)
            worker.task_failed.connect(lambda err: dlg.show_completed(False, err))
            worker.start()

    def _handle_verify_single(self, record: FileRecord):
        self.run_verification()

    def _handle_backup_again(self, record_id: str):
        record = self.repo.get(record_id)
        if record.original_path and Path(record.original_path).is_file():
            self._start_backup_task(Path(record.original_path))

    def run_verification(self):
        async def _run():
            return await self.verify_uc.execute()

        worker = AsyncBridge(_run)

        def _done(summary):
            self._refresh_vault_table()
            dlg = HealthReportDialog(summary.results, parent=self)
            dlg.heal_requested.connect(self._run_auto_heal)
            dlg.exec()

        worker.task_completed.connect(_done)
        worker.task_failed.connect(self._on_task_failed)
        worker.start()

    def _run_auto_heal(self):
        async def _run():
            return await self.heal_uc.execute()

        worker = AsyncBridge(_run)

        def _done(summary):
            self._refresh_vault_table()
            QMessageBox.information(
                self, "Self-Healing Completed",
                f"Successfully healed {summary.healed_count} item(s) by forwarding surviving cloud copies."
            )

        worker.task_completed.connect(_done)
        worker.task_failed.connect(self._on_task_failed)
        worker.start()

    def _run_recovery_drill(self):
        self._switch_view(3)
        self.recovery_view.set_drill_running(True)

        async def _run():
            return await self.recovery_drill_uc.execute()

        worker = AsyncBridge(_run)

        def _done(report):
            self.recovery_view.set_drill_running(False)
            self.recovery_view.display_drill_report(report)
            self._refresh_vault_table()

        def _failed(err):
            self.recovery_view.set_drill_running(False)
            QMessageBox.critical(self, "Recovery Drill Failed", f"Drill encountered an error:\n{err}")

        worker.task_completed.connect(_done)
        worker.task_failed.connect(_failed)
        worker.start()

    def _run_rebuild(self):
        confirm = QMessageBox.question(
            self, "Confirm Disaster Recovery Rebuild",
            "Scan Telegram channels and reconstruct local index from tv1/tv2 captions?",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if confirm != QMessageBox.StandardButton.Yes:
            return

        async def _run():
            return await self.rebuild_uc.execute()

        worker = AsyncBridge(_run)

        def _done(res):
            self._refresh_vault_table()
            QMessageBox.information(
                self, "Disaster Recovery Complete",
                f"Scanned {res.total_messages_scanned} messages. Indexed {res.records_indexed} files."
            )

        worker.task_completed.connect(_done)
        worker.task_failed.connect(self._on_task_failed)
        worker.start()

    def _handle_ai_query(self, role: str, prompt: str):
        async def _run():
            return await self.ai_orchestrator.query_agent(role, prompt)

        worker = AsyncBridge(_run)

        def _done(resp):
            self.ai_doctor_view.append_response(resp.agent_name, resp.message)
            self.ai_doctor_view.display_proposals(resp.proposals)

        worker.task_completed.connect(_done)
        worker.task_failed.connect(lambda err: self.ai_doctor_view.append_response("System Error", err))
        worker.start()

    def _handle_proposal_approved(self, proposal_id: str):
        prop = self.ai_orchestrator.approve_proposal(proposal_id)
        if not prop:
            return

        if "Heal" in prop.action_title:
            self._run_auto_heal()
        elif "Recovery Drill" in prop.action_title:
            self._run_recovery_drill()
        elif "Rebuild" in prop.action_title:
            self._run_rebuild()

        self.ai_doctor_view.display_proposals(self.ai_orchestrator.get_pending_proposals())

    def _handle_proposal_rejected(self, proposal_id: str):
        self.ai_orchestrator.reject_proposal(proposal_id)
        self.ai_doctor_view.display_proposals(self.ai_orchestrator.get_pending_proposals())

    def _verify_manifest(self):
        is_valid, msg = self.manifest_service.verify_chain()
        self.security_view.set_manifest_status(is_valid, msg)
        self._refresh_vault_table()

    def _generate_manifest(self):
        m = self.manifest_service.generate_manifest()
        self.security_view.set_manifest_status(
            True,
            f"Generation {m.generation} created ({m.total_files} files, {m.total_bytes} bytes)."
        )
        self._refresh_vault_table()

    def _open_settings(self):
        dlg = SettingsDialog(self)
        dlg.exec()

    def _on_task_failed(self, error_msg: str):
        self.files_view.pipeline_strip.reset_pipeline()
        QMessageBox.critical(self, "Operation Error", f"An error occurred:\n{error_msg}")

    def closeEvent(self, event: QCloseEvent):
        if self.tray.isVisible():
            self.hide()
            event.ignore()
        else:
            event.accept()

    def close_completely(self):
        self.tray.hide()
        self.close()
        sys.exit(0)
