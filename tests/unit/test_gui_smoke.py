from pathlib import Path
import sys
import pytest
from PyQt6.QtWidgets import QApplication

from televault.domain.entities import FileRecord
from televault.domain.states import HealthState, LocalFileStatus
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.os.single_instance import SingleInstanceLock
from televault.presentation.gui.dropzone import DropZoneWidget
from televault.presentation.gui.main_window import MainWindow
from televault.presentation.gui.pipeline_strip import PipelineStripWidget
from televault.presentation.gui.vault_table import VaultTableWidget
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.fixture(scope="session")
def qapp():
    app = QApplication.instance()
    if not app:
        app = QApplication(sys.argv)
    yield app


def test_single_instance_lock(tmp_path: Path):
    lock_file = tmp_path / "test_app.lock"
    lock1 = SingleInstanceLock(lock_file)
    lock2 = SingleInstanceLock(lock_file)

    assert lock1.acquire() is True
    # Second acquisition fails because first instance holds lock
    assert lock2.acquire() is False

    lock1.release()
    # Now second instance can acquire
    assert lock2.acquire() is True
    lock2.release()


def test_gui_widgets_instantiation(qapp, tmp_path: Path):
    dropzone = DropZoneWidget()
    assert dropzone.acceptDrops() is True

    pipeline = PipelineStripWidget()
    pipeline.start_pipeline("report.pdf", 1024 * 1024 * 5)
    pipeline.set_upload_progress(500000, 1000000)
    pipeline.set_mirror_stage()
    pipeline.set_verify_stage()
    pipeline.complete_pipeline()

    table = VaultTableWidget()
    sample_records = [
        FileRecord(
            name="test_contract.pdf",
            original_path=str(tmp_path / "test_contract.pdf"),
            sha256="abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890",
            size=1024 * 200,
            state=HealthState.HEALTHY,
            local_status=LocalFileStatus.PRESENT,
        )
    ]
    table.load_records(sample_records)
    assert table.table.rowCount() == 1


def test_main_window_smoke(qapp, tmp_path: Path):
    config = TeleVaultConfig(base_dir=tmp_path / "app_data")
    gateway = FakeTelegramGateway()
    window = MainWindow(gateway=gateway, config=config)

    assert window.windowTitle() == "TeleVault — Resilient Telegram Desktop Backup"
    assert window.minimumWidth() >= 900
    assert window.minimumHeight() >= 600

    window.close()
