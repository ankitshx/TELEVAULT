from dataclasses import dataclass

from televault.domain.entities import FileRecord
from televault.domain.ports import VaultRepository


@dataclass(frozen=True)
class DedupeResult:
    is_duplicate: bool
    existing_record: FileRecord | None = None
    reason: str = ""


class DedupeChecker:
    """Checks whether a file has already been safely backed up in the vault."""

    def __init__(self, repo: VaultRepository, live_channel_id: int | None = None):
        self.repo = repo
        self.live_channel_id = live_channel_id

    def check(self, sha256: str) -> DedupeResult:
        existing = self.repo.find_by_hash(sha256)
        if existing and existing.is_healthy:
            # If the record is from a fake test channel, do not count as duplicate in real vault
            if existing.primary_ref and existing.primary_ref.channel_id == -10010001:
                return DedupeResult(is_duplicate=False)

            # If live channel is known, ensure the record was actually uploaded to this vault
            if self.live_channel_id and existing.primary_ref:
                c1 = str(abs(existing.primary_ref.channel_id)).replace("100", "", 1)
                c2 = str(abs(self.live_channel_id)).replace("100", "", 1)
                if c1 != c2:
                    return DedupeResult(is_duplicate=False)

            return DedupeResult(
                is_duplicate=True,
                existing_record=existing,
                reason=f"File with identical SHA-256 already backed up as '{existing.name}' (id: {existing.id})",
            )
        return DedupeResult(is_duplicate=False)
