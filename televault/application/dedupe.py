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

    def __init__(self, repo: VaultRepository):
        self.repo = repo

    def check(self, sha256: str) -> DedupeResult:
        existing = self.repo.find_by_hash(sha256)
        if existing and existing.is_healthy:
            return DedupeResult(
                is_duplicate=True,
                existing_record=existing,
                reason=f"File with identical SHA-256 already backed up as '{existing.name}' (id: {existing.id})",
            )
        return DedupeResult(is_duplicate=False)
