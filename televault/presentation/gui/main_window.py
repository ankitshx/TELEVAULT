import asyncio
from pathlib import Path
import sys
from PyQt6.QtCore import Qt
from PyQt6.QtGui import QCloseEvent
from PyQt6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from televault.application.backup import BackupFileUseCase
from televault.application.event_bus import SimpleEventBus
from televault.application.heal import HealVaultUseCase
from televault.application.restore import RestoreFileUseCase
from televault.application.verify import VerifyVaultUseCase
from televault.domain.ports import TelegramGateway
from televault.domain.states import HealthState
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.os.keyring_store import KeyringStore
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from televault.presentation.gui.async_bridge import AsyncBridge
from televault.presentation.gui.dialogs.health_dialog import HealthReportDialog
from televault.presentation.gui.dialogs.restore_dialog import RestoreDialog
from televault.presentation.gui.dialogs.settings_dialog import SettingsDialog
from televault.presentation.gui.dialogs.wizard import SetupWizardDialog
from televault.presentation.gui.dropzone import DropZoneWidget
from televault.presentation.gui.pipeline_strip import PipelineStripWidget
from televault.presentation.gui.theme import THEME_STYLESHEET
from televault.presentation.gui.tray import TeleVaultTrayIcon
from televault.presentation.gui.vault_table import VaultTableWidget
from tests.fakes.fake_gateway import FakeTelegramGateway


class MainWindow(QMainWindow):
    """Main application window for TeleVault desktop."""

    def __init__(self, gateway: TelegramGateway | None = None, config: TeleVaultConfig | None = None):
        super().__init__()
        self.setWindowTitle("TeleVault — Resilient Telegram Desktop Backup")
        self.setMinimumSize(960, 640)
        self.setStyleSheet(THEME_STYLESHEET)

        self.config = config or TeleVaultConfig()
        self.config.ensure_directories()

        self.repo = SQLiteVaultRepository(self.config.db_path)
        self.recovery_cache = RecoveryCache(self.config.cache_dir)
        self.event_bus = SimpleEventBus()
        self.gateway = gateway or FakeTelegramGateway()

        self.backup_uc = BackupFileUseCase(self.gateway, self.repo, self.event_bus)
        self.restore_uc = RestoreFileUseCase(self.gateway, self.repo, self.event_bus)
        self.verify_uc = VerifyVaultUseCase(self.gateway, self.repo, self.recovery_cache, self.event_bus)
        self.heal_uc = HealVaultUseCase(self.gateway, self.repo, self.recovery_cache, self.event_bus)

        self._active_worker: AsyncBridge | None = None
        self._init_ui()
        self._refresh_vault_table()

    def _init_ui(self):
        central = QWidget()
        self.setCentralWidget(central)
        root_layout = QVBoxLayout(central)
        root_layout.setContentsMargins(0, 0, 0, 0)
        root_layout.setSpacing(0)

        # 1. Header Frame
        header = QFrame()
        header.setObjectName("headerPanel")
        header_layout = QHBoxLayout(header)
        header_layout.setContentsMargins(20, 14, 20, 14)

        brand = QLabel("TELEVAULT")
        brand.setObjectName("brandTitle")
        header_layout.addWidget(brand)

        self.badge_global = QLabel("● ALL FILES SAFE")
        self.badge_global.setObjectName("badgeHealthy")
        header_layout.addWidget(self.badge_global)

        header_layout.addStretch()

        btn_verify = QPushButton("VERIFY VAULT")
        btn_verify.clicked.connect(self.run_verification)
        header_layout.addWidget(btn_verify)

        btn_settings = QPushButton("SETTINGS")
        btn_settings.clicked.connect(self._open_settings)
        header_layout.addWidget(btn_settings)

        root_layout.addWidget(header)

        # 2. Main Content Body
        body = QWidget()
        body_layout = QVBoxLayout(body)
        body_layout.setContentsMargins(20, 16, 20, 16)
        body_layout.setSpacing(14)

        # Drop Zone (Top)
        self.drop_zone = DropZoneWidget()
        self.drop_zone.files_dropped.connect(self._handle_files_dropped)
        body_layout.addWidget(self.drop_zone)

        # Live Pipeline Strip (Middle)
        self.pipeline_strip = PipelineStripWidget()
        body_layout.addWidget(self.pipeline_strip)

        # Vault Table (Bottom)
        self.vault_table = VaultTableWidget()
        self.vault_table.restore_clicked.connect(self._handle_restore)
        self.vault_table.backup_again_clicked.connect(self._handle_backup_again)
        body_layout.addWidget(self.vault_table)

        root_layout.addWidget(body)

        # 3. Footer Status Bar
        footer = QFrame()
        footer.setStyleSheet("background-color: #0B0C0E; border-top: 1px solid #242A35; padding: 6px 20px;")
        footer_layout = QHBoxLayout(footer)
        footer_layout.setContentsMargins(10, 4, 10, 4)

        self.lbl_footer_stats = QLabel("TOTAL: 0 FILES  |  VAULT SIZE: 0.0 MB")
        self.lbl_footer_stats.setStyleSheet("color: #7E889B; font-size: 11px;")
        footer_layout.addWidget(self.lbl_footer_stats)

        footer_layout.addStretch()

        self.lbl_footer_mirror = QLabel("MIRROR: TELEVAULT MIRROR (ONLINE)")
        self.lbl_footer_mirror.setStyleSheet("color: #10B981; font-size: 11px; font-weight: bold;")
        footer_layout.addWidget(self.lbl_footer_mirror)

        root_layout.addWidget(footer)

        # Tray icon
        self.tray = TeleVaultTrayIcon(self)
        self.tray.show()

    def _refresh_vault_table(self):
        records = self.repo.list_all()
        self.vault_table.load_records(records)

        total_bytes = sum(r.size for r in records)
        total_mb = total_bytes / (1024 * 1024)
        self.lbl_footer_stats.setText(f"TOTAL: {len(records)} FILES  |  VAULT SIZE: {total_mb:.1f} MB")

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
        self.badge_global.setStyleSheet(self.badge_global.styleSheet())  # Trigger update

    def _handle_files_dropped(self, paths: list[Path]):
        for p in paths:
            if p.is_file():
                self._start_backup_task(p)

    def _start_backup_task(self, file_path: Path):
        self.pipeline_strip.start_pipeline(file_path.name, file_path.stat().st_size)

        async def _run():
            return await self.backup_uc.execute(file_path)

        self._active_worker = AsyncBridge(_run)
        self._active_worker.task_completed.connect(self._on_backup_completed)
        self._active_worker.task_failed.connect(self._on_task_failed)
        self._active_worker.start()

    def _on_backup_completed(self, result):
        self.pipeline_strip.complete_pipeline()
        self._refresh_vault_table()
        if result.is_duplicate:
            QMessageBox.information(self, "Duplicate File", result.message)

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
                    QMessageBox.information(self, "Restore Success", f"File restored to:\n{res.restored_path}\n\nSHA-256 match confirmed.")

            worker.task_completed.connect(_done)
            worker.task_failed.connect(lambda err: dlg.show_completed(False, err))
            worker.start()

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
                self, "Auto-Heal Complete",
                f"Successfully healed {summary.healed_count} item(s) by forwarding surviving cloud copies."
            )

        worker.task_completed.connect(_done)
        worker.task_failed.connect(self._on_task_failed)
        worker.start()

    def _open_settings(self):
        dlg = SettingsDialog(self)
        dlg.exec()

    def _on_task_failed(self, error_msg: str):
        self.pipeline_strip.reset_pipeline()
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
