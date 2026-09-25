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

from televault.application.backup import BackupFileUseCase
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
from televault.infrastructure.telegram.auth_service import TelegramAuthService
from televault.infrastructure.telegram.telethon_gateway import TelethonGateway
from televault.presentation.gui.async_bridge import AsyncBridge
from televault.presentation.gui.dialogs.health_dialog import HealthReportDialog
from televault.presentation.gui.dialogs.login_dialog import TelegramLoginDialog
from televault.presentation.gui.dialogs.restore_dialog import RestoreDialog
from televault.presentation.gui.dialogs.settings_dialog import SettingsDialog
from televault.presentation.gui.theme import THEME_STYLESHEET
from televault.presentation.gui.tray import TeleVaultTrayIcon
from televault.presentation.gui.views.files_view import FilesView
from televault.presentation.gui.views.overview_view import OverviewView
from televault.presentation.gui.views.settings_view import SettingsView
from televault.presentation.gui.views.timeline_view import TimelineView
from tests.fakes.fake_gateway import FakeTelegramGateway


class MainWindow(QMainWindow):
    """Main application shell for TeleVault featuring Windows 11 Fluent UI,
    fast drive file management, MTProto authentication, and dual-write health.
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
        self.auth_service = TelegramAuthService(self.config)

        # Default to passed gateway or offline simulation until MTProto is verified
        self.gateway = gateway or FakeTelegramGateway()
        self.is_live_telegram = isinstance(self.gateway, TelethonGateway)
        self.current_user: dict | None = None

        # Core Application Use Cases
        self._init_use_cases()

        self._active_worker: AsyncBridge | None = None
        self._init_ui()
        self._refresh_vault_table()

        # Probe background Telegram authorization if no explicit gateway provided
        if gateway is None:
            self._check_initial_auth()

    def _init_use_cases(self):
        self.backup_uc = BackupFileUseCase(self.gateway, self.repo, self.event_bus)
        self.restore_uc = RestoreFileUseCase(self.gateway, self.repo, self.event_bus)
        self.verify_uc = VerifyVaultUseCase(self.gateway, self.repo, self.recovery_cache, self.event_bus)
        self.heal_uc = HealVaultUseCase(self.gateway, self.repo, self.recovery_cache, self.event_bus)
        self.rebuild_uc = RebuildIndexUseCase(self.gateway, self.repo)
        self.recovery_drill_uc = RecoveryDrillUseCase(self.gateway, self.repo, self.config.recovery_dir, self.repo)
        self.manifest_service = ManifestService(self.repo, self.repo, vault_id="v_main", audit_ledger=self.repo)

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

        sub_logo = QLabel("TELEGRAM CLOUD DRIVE")
        sub_logo.setObjectName("sidebarSubtitle")
        sidebar_layout.addWidget(sub_logo)
        sidebar_layout.addSpacing(14)

        # Navigation Buttons Group
        self.nav_group = QButtonGroup(self)
        self.nav_buttons: list[QPushButton] = []

        nav_items = [
            ("🗁 My Drive", 0),
            ("⊞ Health & Stats", 1),
            ("◷ Time Machine", 2),
            ("⚙ Settings & Account", 3),
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
        self.sidebar_conn = QLabel("○ Telegram Offline")
        self.sidebar_conn.setStyleSheet("color: #9CA3AF; font-size: 11px; font-weight: 600; padding: 6px 8px;")
        self.sidebar_conn.setCursor(Qt.CursorShape.PointingHandCursor)
        sidebar_layout.addWidget(self.sidebar_conn)

        main_h_layout.addWidget(sidebar)

        # 2. Main Stacked Views Container
        self.stack = QStackedWidget()

        # View 0: Files (My Drive)
        self.files_view = FilesView()
        self.files_view.restore_requested.connect(lambda r: self._handle_restore(r.id))
        self.files_view.verify_single_requested.connect(self._handle_verify_single)
        self.files_view.backup_file_requested.connect(self._handle_backup_path)
        # Compatibility aliases
        self.drop_zone = self.files_view.dropzone
        self.pipeline_strip = self.files_view.pipeline_strip
        self.vault_table = self.files_view.table
        self.drop_zone.files_dropped.connect(self._handle_files_dropped)
        self.vault_table.restore_clicked.connect(self._handle_restore)
        self.vault_table.backup_again_clicked.connect(self._handle_backup_again)
        self.stack.addWidget(self.files_view)

        # View 1: Overview & Health
        self.overview_view = OverviewView()
        self.overview_view.backup_requested.connect(self._prompt_backup_file)
        self.overview_view.verify_requested.connect(self.run_verification)
        self.overview_view.heal_requested.connect(self._run_auto_heal)
        self.overview_view.rebuild_requested.connect(self._run_rebuild)
        self.overview_view.recovery_drill_requested.connect(self._run_recovery_drill)
        self.stack.addWidget(self.overview_view)

        # View 2: Timeline
        self.timeline_view = TimelineView()
        self.timeline_view.restore_version_requested.connect(lambda r: self._handle_restore(r.id))
        self.stack.addWidget(self.timeline_view)

        # View 3: Settings & Account
        self.settings_view = SettingsView(self.config)
        self.settings_view.login_requested.connect(self._open_login_dialog)
        self.settings_view.logout_requested.connect(self._handle_logout)
        self.stack.addWidget(self.settings_view)

        # Header for Global Badge & Quick Actions
        content_box = QVBoxLayout()
        content_box.setContentsMargins(0, 0, 0, 0)
        content_box.setSpacing(0)

        header = QFrame()
        header.setObjectName("headerPanel")
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(20, 10, 20, 10)
        header_layout.setSpacing(12)

        self.brand_title = QLabel("TeleVault Drive")
        self.brand_title.setObjectName("brandTitle")
        header_layout.addWidget(self.brand_title)

        self.badge_global = QLabel("● ALL FILES SAFE")
        self.badge_global.setObjectName("badgeHealthy")
        header_layout.addWidget(self.badge_global)

        header_layout.addStretch()

        # Connect / Account Button in Header
        self.btn_auth_header = QPushButton("🔑 Connect Telegram")
        self.btn_auth_header.setObjectName("btnPrimary")
        self.btn_auth_header.setCursor(Qt.CursorShape.PointingHandCursor)
        self.btn_auth_header.clicked.connect(self._open_login_dialog)
        header_layout.addWidget(self.btn_auth_header)

        btn_add = QPushButton("+ Protect File")
        btn_add.setCursor(Qt.CursorShape.PointingHandCursor)
        btn_add.clicked.connect(self._prompt_backup_file)
        header_layout.addWidget(btn_add)

        btn_verify = QPushButton("Verify Vault")
        btn_verify.setCursor(Qt.CursorShape.PointingHandCursor)
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

    def _check_initial_auth(self):
        async def _check():
            if await self.auth_service.is_authorized():
                gw = await self.auth_service.get_live_gateway()
                user = await self.auth_service.get_current_user()
                return gw, user
            return None, None

        worker = AsyncBridge(_check)

        def _done(result):
            gw, user = result
            if gw:
                self._apply_live_gateway(gw, user)
            else:
                self._update_auth_ui(False, None)

        worker.task_completed.connect(_done)
        worker.task_failed.connect(lambda _: self._update_auth_ui(False, None))
        worker.start()

    def _open_login_dialog(self):
        dlg = TelegramLoginDialog(self.auth_service, parent=self)
        dlg.login_success.connect(self._apply_live_gateway)
        dlg.exec()

    def _apply_live_gateway(self, gateway: TelethonGateway, user_info: dict | None = None):
        self.gateway = gateway
        self.is_live_telegram = True
        self.current_user = user_info or {}
        self._init_use_cases()
        self._update_auth_ui(True, self.current_user)

    def _update_auth_ui(self, is_connected: bool, user_info: dict | None):
        creds = self.auth_service.get_stored_credentials()
        channels = {
            "primary_id": creds.get("primary_channel_id"),
            "mirror_id": creds.get("mirror_channel_id"),
        }
        self.settings_view.update_account_info(is_connected, user_info, channels)

        if is_connected and user_info:
            handle = f"@{user_info.get('username')}" if user_info.get("username") else user_info.get("first_name", "Telegram")
            self.sidebar_conn.setText(f"● {handle}")
            self.sidebar_conn.setStyleSheet("color: #10B981; font-size: 11px; font-weight: 600; padding: 6px 8px;")
            self.btn_auth_header.setText(f"● {handle}")
            self.btn_auth_header.setStyleSheet("background-color: #064E3B; color: #34D399; font-weight: 600; border: 1px solid #059669;")
        else:
            self.sidebar_conn.setText("○ Telegram Disconnected")
            self.sidebar_conn.setStyleSheet("color: #EF4444; font-size: 11px; font-weight: 600; padding: 6px 8px;")
            self.btn_auth_header.setText("🔑 Connect Telegram")
            self.btn_auth_header.setObjectName("btnPrimary")
            self.btn_auth_header.setStyleSheet("")

    def _handle_logout(self):
        confirm = QMessageBox.question(
            self, "Confirm Disconnect",
            "Are you sure you want to disconnect your Telegram account?\n"
            "Your existing files will remain intact in Telegram and your local database.",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
        )
        if confirm != QMessageBox.StandardButton.Yes:
            return

        self.auth_service.clear_credentials()
        self.gateway = FakeTelegramGateway()
        self.is_live_telegram = False
        self.current_user = None
        self._init_use_cases()
        self._update_auth_ui(False, None)
        QMessageBox.information(self, "Disconnected", "Telegram session cleared successfully.")

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
        self._switch_view(1)

        async def _run():
            return await self.recovery_drill_uc.execute()

        worker = AsyncBridge(_run)

        def _done(report):
            self._refresh_vault_table()
            QMessageBox.information(
                self, "Recovery Drill Completed",
                f"Drill finished successfully!\nReadiness Score: {report.readiness_score}/100\n"
                f"Files Verified: {report.total_files_tested}\nSurviving: {report.surviving_files}"
            )

        def _failed(err):
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
