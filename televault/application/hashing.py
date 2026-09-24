from collections.abc import Callable
import hashlib
from pathlib import Path

CHUNK_SIZE = 1024 * 1024  # 1 MB streaming chunks


def calculate_sha256(
    file_path: Path,
    on_progress: Callable[[int, int], None] | None = None,
) -> tuple[str, int]:
    """Compute SHA-256 hash and exact byte size of a file in streaming chunks.

    Never reads the entire file into memory at once.
    Returns: (hex_digest, total_bytes)
    """
    if not file_path.is_file():
        raise FileNotFoundError(f"File not found: {file_path}")

    total_bytes = file_path.stat().st_size
    hasher = hashlib.sha256()
    processed_bytes = 0

    with file_path.open("rb") as f:
        while chunk := f.read(CHUNK_SIZE):
            hasher.update(chunk)
            processed_bytes += len(chunk)
            if on_progress:
                on_progress(processed_bytes, total_bytes)

    if on_progress and processed_bytes == 0:
        on_progress(0, 0)

    return hasher.hexdigest(), total_bytes
