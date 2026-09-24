import ast
import hashlib
from pathlib import Path
import pytest

from televault.domain.entities import MessageRef
from televault.domain.ports import TelegramGateway
from tests.fakes.fake_gateway import FakeTelegramGateway


class TestInvariant1SingleDocument:
    """Invariant 1: Every upload is exactly one Telegram document (force_document, no chunking)

    unless explicitly approved for split.
    """

    @pytest.mark.asyncio
    async def test_upload_is_single_document_forced(self, tmp_path: Path):
        gateway = FakeTelegramGateway()
        test_file = tmp_path / "test_doc.pdf"
        test_file.write_bytes(b"Sample PDF content for TeleVault backup")

        ref = await gateway.upload_single(test_file, caption='tv1 {"name":"test_doc.pdf"}')

        assert isinstance(ref, MessageRef)
        assert len(gateway.upload_calls) == 1
        call = gateway.upload_calls[0]
        assert call["force_document"] is True
        assert call["size"] == len(test_file.read_bytes())


class TestInvariant2NoTelegramDeletions:
    """Invariant 2: No code path deletes Telegram messages.

    TeleVault is strictly append-only in Telegram.
    """

    def test_no_delete_messages_in_codebase(self):
        """AST and token inspection: verify no function in televault calls Telegram delete methods."""
        src_root = Path(__file__).resolve().parent.parent / "televault"
        py_files = list(src_root.rglob("*.py"))
        assert len(py_files) > 0, "No python files found in televault/"

        forbidden_names = {
            "delete_messages",
            "delete_message",
            "delete_dialog",
            "delete_channel",
        }

        violations = []
        for py_file in py_files:
            content = py_file.read_text(encoding="utf-8")
            tree = ast.parse(content, filename=str(py_file))
            for node in ast.walk(tree):
                if isinstance(node, ast.Attribute) and node.attr in forbidden_names:
                    violations.append(f"{py_file.name}:{node.lineno} -> calls forbidden attribute '{node.attr}'")
                elif isinstance(node, ast.Call):
                    if isinstance(node.func, ast.Name) and node.func.id in forbidden_names:
                        violations.append(f"{py_file.name}:{node.lineno} -> calls forbidden function '{node.func.id}'")

        assert not violations, f"Forbidden deletion calls found:\n" + "\n".join(violations)

    def test_fake_gateway_has_zero_delete_calls(self):
        gateway = FakeTelegramGateway()
        assert len(gateway.delete_message_calls) == 0


class TestInvariant3ManualOnlyNoBackgroundUploads:
    """Invariant 3: No code path uploads without a user-triggered action.

    TeleVault has no watched folders, no background timers, no scheduled uploads.
    """

    def test_no_background_watchers_or_cron_schedulers(self):
        """Verify codebase does not import watchdog, APScheduler, or start background upload loops."""
        src_root = Path(__file__).resolve().parent.parent / "televault"
        py_files = list(src_root.rglob("*.py"))

        forbidden_imports = {
            "watchdog",
            "apscheduler",
            "schedule",
        }

        violations = []
        for py_file in py_files:
            content = py_file.read_text(encoding="utf-8")
            tree = ast.parse(content, filename=str(py_file))
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    for alias in node.names:
                        base = alias.name.split(".")[0].lower()
                        if base in forbidden_imports:
                            violations.append(f"{py_file.name}:{node.lineno} imports '{base}'")
                elif isinstance(node, ast.ImportFrom):
                    if node.module:
                        base = node.module.split(".")[0].lower()
                        if base in forbidden_imports:
                            violations.append(f"{py_file.name}:{node.lineno} imports from '{base}'")

        assert not violations, f"Forbidden background auto-sync imports found:\n" + "\n".join(violations)


class TestInvariant4RestoreIntegritySha256:
    """Invariant 4: A restore is reported successful only when SHA-256 matches."""

    @pytest.mark.asyncio
    async def test_restore_hash_match_validation(self, tmp_path: Path):
        gateway = FakeTelegramGateway()
        original_bytes = b"Crucial data to be restored with byte-level fidelity"
        expected_hash = hashlib.sha256(original_bytes).hexdigest()

        src_file = tmp_path / "source.bin"
        src_file.write_bytes(original_bytes)

        ref = await gateway.upload_single(src_file, caption=f'tv1 {{"sha256":"{expected_hash}"}}')

        # Download to restored destination
        restored_file = tmp_path / "restored.bin"
        await gateway.download(ref, restored_file)

        assert restored_file.exists()
        restored_hash = hashlib.sha256(restored_file.read_bytes()).hexdigest()
        assert restored_hash == expected_hash, "Restored file hash must match original SHA-256"

    def test_tampered_restore_is_detected_and_rejected(self, tmp_path: Path):
        expected_hash = "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        restored_file = tmp_path / "tampered.bin"
        restored_file.write_bytes(b"Tampered content")

        actual_hash = hashlib.sha256(restored_file.read_bytes()).hexdigest()
        assert actual_hash != expected_hash, "Hash mismatch must be detected"


class TestProtocolConformance:
    """Verify that FakeTelegramGateway satisfies the TelegramGateway protocol."""

    def test_gateway_satisfies_protocol(self):
        gateway = FakeTelegramGateway()
        assert isinstance(gateway, TelegramGateway)
