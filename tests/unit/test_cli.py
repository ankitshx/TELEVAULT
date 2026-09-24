import os
from pathlib import Path
import subprocess
import sys

import pytest


def run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    cmd = [sys.executable, "-m", "televault", *args]
    return subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        cwd=str(Path(__file__).resolve().parent.parent.parent),
    )


def test_cli_help():
    proc = run_cli("--help")
    assert proc.returncode == 0
    assert "TeleVault" in proc.stdout
    assert "backup" in proc.stdout
    assert "restore" in proc.stdout
    assert "ls" in proc.stdout


def test_cli_backup_dry_run(tmp_path: Path):
    test_file = tmp_path / "cli_sample.txt"
    test_file.write_bytes(b"CLI dry run sample content")

    proc = run_cli("backup", str(test_file), "--dry-run", "--fake")
    assert proc.returncode == 0
    assert "[DRY-RUN]" in proc.stdout
    assert "cli_sample.txt" in proc.stdout


def test_cli_backup_and_ls(tmp_path: Path, monkeypatch):
    test_file = tmp_path / "test_notes.md"
    test_file.write_bytes(b"# Meeting Notes for TeleVault")

    # Run backup with fake gateway
    proc_b = run_cli("backup", str(test_file), "--fake")
    assert proc_b.returncode == 0
    assert "[DONE]" in proc_b.stdout or "[ALREADY BACKED UP]" in proc_b.stdout

    # Run ls
    proc_ls = run_cli("ls", "--fake")
    assert proc_ls.returncode == 0
    assert "test_notes.md" in proc_ls.stdout
