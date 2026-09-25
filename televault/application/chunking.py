import hashlib
import math
from pathlib import Path

# Safe limits for standard Telegram MTProto uploads
DEFAULT_MAX_SINGLE_FILE_SIZE = 2000 * 1024 * 1024  # 2.0 GB limit
DEFAULT_CHUNK_SIZE = 1900 * 1024 * 1024            # 1.9 GB per chunk
STREAM_BUFFER_SIZE = 1024 * 1024                    # 1 MB streaming buffer


def split_file_slices(file_size: int, chunk_size: int = DEFAULT_CHUNK_SIZE) -> list[tuple[int, int]]:
    """Calculate byte offset and size for each chunk of a large file.
    
    Returns a list of (offset, size) tuples.
    """
    if file_size <= 0:
        return [(0, 0)]

    slices: list[tuple[int, int]] = []
    offset = 0
    while offset < file_size:
        size = min(chunk_size, file_size - offset)
        slices.append((offset, size))
        offset += size
    return slices


def extract_chunk_to_file(
    source_path: Path,
    offset: int,
    size: int,
    dest_path: Path,
) -> str:
    """Stream a slice from source file into a temporary chunk file.
    
    Computes and returns the SHA-256 of the extracted chunk.
    Maintains O(1) memory overhead by reading in 1 MB buffers.
    """
    dest_path.parent.mkdir(parents=True, exist_ok=True)
    hasher = hashlib.sha256()
    remaining = size

    with open(source_path, "rb") as f_in, open(dest_path, "wb") as f_out:
        f_in.seek(offset)
        while remaining > 0:
            read_size = min(STREAM_BUFFER_SIZE, remaining)
            buf = f_in.read(read_size)
            if not buf:
                break
            f_out.write(buf)
            hasher.update(buf)
            remaining -= len(buf)

    return hasher.hexdigest()


def merge_chunks(chunk_paths: list[Path], dest_path: Path) -> str:
    """Concatenate chunk files into the final restored file in order.
    
    Computes and returns the full SHA-256 checksum of the merged output file.
    """
    dest_path.parent.mkdir(parents=True, exist_ok=True)
    hasher = hashlib.sha256()

    with open(dest_path, "wb") as f_out:
        for chunk_path in chunk_paths:
            if not chunk_path.exists():
                raise FileNotFoundError(f"Missing chunk file: {chunk_path}")
            with open(chunk_path, "rb") as f_in:
                while buf := f_in.read(STREAM_BUFFER_SIZE):
                    f_out.write(buf)
                    hasher.update(buf)

    return hasher.hexdigest()
