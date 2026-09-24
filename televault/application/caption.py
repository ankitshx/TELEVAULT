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
