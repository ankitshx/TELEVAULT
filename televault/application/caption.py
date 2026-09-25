import json
from typing import Any

from televault.domain.entities import FileRecord
from televault.domain.states import VaultMode

TV1_PREFIX = "tv1 "


def format_tv1_caption(record: FileRecord) -> str:
    """Format a self-describing tv1 JSON metadata caption for Telegram messages.

    Allows complete recovery of index metadata from Telegram channels alone.
    """
    if record.mode == VaultMode.PRIVATE:
        # Private mode hides original filename and plaintext metadata
        payload = {
            "id": record.id,
            "mode": record.mode.value,
            "ver": record.version,
        }
    else:
        payload = {
            "id": record.id,
            "name": record.name,
            "sha256": record.sha256,
            "size": record.size,
            "mtime": round(record.mtime, 3),
            "mode": record.mode.value,
            "ver": record.version,
        }

    return f"{TV1_PREFIX}{json.dumps(payload, separators=(',', ':'))}"


def format_human_size(num_bytes: int) -> str:
    """Format bytes into readable human unit string (KB, MB, GB, TB)."""
    val = float(num_bytes)
    for unit in ["B", "KB", "MB", "GB", "TB"]:
        if abs(val) < 1024.0 or unit == "TB":
            return f"{val:.2f} {unit}" if unit in ("GB", "TB") else f"{val:.1f} {unit}" if unit in ("KB", "MB") else f"{int(val)} B"
        val /= 1024.0
    return f"{val:.2f} TB"


def format_master_post(record: FileRecord, total_parts: int) -> str:
    """Format the Master Announcement Card for a multi-part file in Telegram.
    
    This card represents the whole file in the channel timeline, while chunks
    are uploaded cleanly inside its reply thread.
    """
    human_size = format_human_size(record.size)
    return (
        f"📦 **{record.name}**\n"
        f"━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
        f"📊 **Total Size:** {human_size} ({record.size:,} bytes)\n"
        f"🧩 **Parts:** {total_parts} Chunks (Uploaded in Reply Thread)\n"
        f"🔐 **SHA-256:** `{record.sha256}`\n"
        f"📁 **Storage Mode:** {record.mode.value.title()}\n"
        f"━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
        f"⚡ *Managed by TeleVault Smart Vault Engine*"
    )


def format_chunk_caption(
    record: FileRecord,
    part_number: int,
    total_parts: int,
    chunk_sha256: str,
    chunk_size: int,
) -> str:
    """Format a tv1 JSON caption for an individual chunk part document."""
    payload = {
        "id": record.id,
        "name": record.name,
        "part": part_number,
        "total_parts": total_parts,
        "chunk_sha": chunk_sha256,
        "chunk_size": chunk_size,
        "master_sha": record.sha256,
        "master_size": record.size,
        "mode": record.mode.value,
        "ver": record.version,
    }
    return f"{TV1_PREFIX}{json.dumps(payload, separators=(',', ':'))}"


def parse_tv1_caption(caption: str | None) -> dict[str, Any] | None:
    """Parse a tv1 caption string into a metadata dictionary, or return None if invalid."""
    if not caption or not caption.startswith(TV1_PREFIX):
        return None

    raw_json = caption[len(TV1_PREFIX):].strip()
    try:
        data = json.loads(raw_json)
        if isinstance(data, dict) and "id" in data:
            return data
    except json.JSONDecodeError:
        pass

    return None
