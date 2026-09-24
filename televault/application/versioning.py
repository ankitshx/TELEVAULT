from dataclasses import dataclass
from pathlib import Path

from televault.application.backup import BackupFileUseCase, BackupResult
from televault.domain.entities import FileRecord
from televault.domain.ports import VaultRepository
from televault.domain.states import VaultMode

DEFAULT_MAX_VERSIONS = 5


class VersionManager:
    """Manages file version lineage and retention policy.

    Guarantees:
    - Never auto-creates versions; versioning occurs only on manual user backup.
    - Preserves older versions in Telegram (Rule 2b: append-only, zero deletions).
    - Tracks version numbers sequentially (1, 2, 3...).
    """

    def __init__(self, repo: VaultRepository, max_versions: int = DEFAULT_MAX_VERSIONS):
        self.repo = repo
        self.max_versions = max_versions

    def get_next_version(self, original_path: str) -> int:
        records = [r for r in self.repo.list_all() if r.original_path == original_path]
        if not records:
            return 1
        return max(r.version for r in records) + 1

    def get_version_history(self, original_path: str) -> list[FileRecord]:
        records = [r for r in self.repo.list_all() if r.original_path == original_path]
        return sorted(records, key=lambda r: r.version, reverse=True)


class BackupNewVersionUseCase:
    """Creates a new version record (N+1) for a modified file."""

    def __init__(self, backup_use_case: BackupFileUseCase, repo: VaultRepository):
        self.backup_use_case = backup_use_case
        self.repo = repo
        self.version_manager = VersionManager(repo)

    async def execute(
        self,
        file_path: Path,
        mode: VaultMode = VaultMode.ORIGINAL,
        dry_run: bool = False,
    ) -> BackupResult:
        file_path = file_path.resolve()
        next_ver = self.version_manager.get_next_version(str(file_path))

        # Perform backup
        result = await self.backup_use_case.execute(
            file_path=file_path,
            mode=mode,
            dry_run=dry_run,
        )

        if not dry_run and not result.is_duplicate:
            result.record.version = next_ver
            self.repo.add(result.record)

        return result
