import json
import logging
from typing import Any

logger = logging.getLogger("televault.caption")

KNOWN_PROTOCOLS = {"tv1", "tv2"}


def format_tv2_caption(
    vault_id: str,
    record_id: str,
    version: int,
    sha256: str,
    size: int,
    timestamp: float,
    mode: str = "original",
    name: str | None = None,
    parent_hash: str | None = None,
    key_id: str | None = None,
    manifest_gen: int = 1,
) -> str:
    """Format a tamper-resistant tv2 metadata caption for Telegram cloud documents."""
    payload: dict[str, Any] = {
        "v_id": vault_id,
        "id": record_id,
        "ver": version,
        "sha256": sha256,
        "size": size,
        "time": timestamp,
        "mode": mode,
        "m_gen": manifest_gen,
    }
    # In Original Mode, include filename; in Private Mode, name is strictly omitted for zero-knowledge
    if mode == "original" and name:
        payload["name"] = name
    if parent_hash:
        payload["parent"] = parent_hash
    if key_id:
        payload["key_id"] = key_id

    return f"tv2 {json.dumps(payload, separators=(',', ':'))}"


def parse_caption(caption: str | None) -> dict[str, Any] | None:
    """Safely parse both tv1 and tv2 captions.
    
    Fails safely on corrupted or unknown protocol versions without guessing.
    """
    if not caption or not isinstance(caption, str):
        return None

    caption = caption.strip()
    prefix = caption[:4].strip()

    if prefix not in KNOWN_PROTOCOLS:
        return None

    try:
        raw_json = caption[len(prefix):].strip()
        data = json.loads(raw_json)
        if not isinstance(data, dict):
            return None

        # Normalize fields between tv1 and tv2
        data["_protocol"] = prefix
        if prefix == "tv1":
            data.setdefault("ver", data.get("version", 1))
            data.setdefault("m_gen", 1)
        elif prefix == "tv2":
            data.setdefault("version", data.get("ver", 1))

        return data
    except (json.JSONDecodeError, UnicodeDecodeError, ValueError) as e:
        logger.debug(f"Failed to parse caption: {e}")
        return None
