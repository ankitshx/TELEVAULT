import hashlib
from pathlib import Path
import pytest

from televault.application.hashing import calculate_sha256


def test_streaming_hash_calculation(tmp_path: Path):
    test_data = b"Arbitrary binary content for TeleVault streaming hash test." * 500
    test_file = tmp_path / "sample.bin"
    test_file.write_bytes(test_data)

    expected_hash = hashlib.sha256(test_data).hexdigest()
    expected_size = len(test_data)

    progress_calls = []

    def on_progress(current: int, total: int):
        progress_calls.append((current, total))

    digest, size = calculate_sha256(test_file, on_progress=on_progress)

    assert digest == expected_hash
    assert size == expected_size
    assert len(progress_calls) > 0
    assert progress_calls[-1] == (expected_size, expected_size)


def test_hash_non_existent_file_raises_error(tmp_path: Path):
    missing_file = tmp_path / "does_not_exist.dat"
    with pytest.raises(FileNotFoundError):
        calculate_sha256(missing_file)
