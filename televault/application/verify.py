from dataclasses import dataclass
from pathlib import Path

from televault.application.hashing import calculate_sha256
from televault.domain.entities import FileRecord
from televault.domain.events import FileStateChangedEvent, VerificationResultEvent
from televault.domain.ports import EventBus, TelegramGateway, VaultRepository
from televault.domain.states import ExistsStatus, HealthState, LocalFileStatus
from televault.infrastructure.storage.recovery_cache import RecoveryCache


@dataclass
class RecordVerificationResult:
    record_id: str
    name: str
    old_state: HealthState
    new_state: HealthState
    local_status: LocalFileStatus
    primary_exists: bool
    mirror_exists: bool
    diagnosis: str


@dataclass
class VerifyVaultSummary:
    total_checked: int = 0
    healthy_count: int = 0
    degraded_count: int = 0
    lost_count: int = 0
    results: list[RecordVerificationResult] = None  # type: ignore

    def __post_init__(self):
        if self.results is None:
            self.results = []


class VerifyVaultUseCase:
    """Verifies the health and integrity of all items in the vault against Telegram and local disk.

    Strict Manual-Only Rules (Rule 2b):
    - Deleting a file on PC never touches Telegram (cloud copy stays forever).
    - If a file is missing locally, local status is updated to MISSING ("Only in Telegram").
      Never auto-downloads or deletes cloud copies.
    - If a local file has changed, local status is updated to CHANGED.
      Never auto-uploads.
    """

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        recovery_cache: RecoveryCache | None = None,
        event_bus: EventBus | None = None,
    ):
        self.gateway = gateway
        self.repo = repo
        self.recovery_cache = recovery_cache
        self.event_bus = event_bus

    async def verify_record(
        self,
        record: FileRecord,
        dry_run: bool = False,
    ) -> RecordVerificationResult:
        old_state = record.state

        # Step 1: Check Local File Status on PC (Status only, never triggers network actions)
        local_status = record.local_status
        if record.original_path:
            local_path = Path(record.original_path)
            if not local_path.exists():
                local_status = LocalFileStatus.MISSING  # "Only in Telegram"
            else:
                # Check if file has changed
                try:
                    current_hash, _ = calculate_sha256(local_path)
                    if current_hash.lower() == record.sha256.lower():
                        local_status = LocalFileStatus.PRESENT
                    else:
                        local_status = LocalFileStatus.CHANGED  # "Changed since last backup"
                except Exception:
                    local_status = LocalFileStatus.CHANGED

        # Step 2: Check Cloud Presence in Telegram
        p_ok = False
        m_ok = False

        if record.primary_ref:
            check_p = await self.gateway.exists(record.primary_ref)
            if check_p.status == ExistsStatus.PRESENT:
                p_ok = True

        if record.mirror_ref:
            check_m = await self.gateway.exists(record.mirror_ref)
            if check_m.status == ExistsStatus.PRESENT:
                m_ok = True

        # Step 3: Determine Health State
        if p_ok and m_ok:
            new_state = HealthState.HEALTHY
            diagnosis = "Both Primary and Mirror copies verified present and intact."
        elif p_ok and not m_ok:
            new_state = HealthState.DEGRADED
            diagnosis = "Primary copy present; Mirror copy missing. Eligible for auto-healing."
        elif not p_ok and m_ok:
            new_state = HealthState.DEGRADED
            diagnosis = "Primary copy missing; Mirror copy present. Eligible for auto-healing."
        else:
            # Both missing in Telegram
            has_recovery = bool(
                self.recovery_cache and self.recovery_cache.has(record.sha256)
            )
            if has_recovery:
                new_state = HealthState.DEGRADED
                diagnosis = "Both cloud copies missing, but local recovery copy is available for restore."
            else:
                new_state = HealthState.LOST
                diagnosis = "Both Primary and Mirror copies missing. Unrecoverable (LOST)."

        # Step 4: Persist State (unless dry-run)
        if not dry_run:
            record.state = new_state
            record.local_status = local_status
            self.repo.add(record)

            if self.event_bus and old_state != new_state:
                self.event_bus.publish(
                    FileStateChangedEvent(
                        file_id=record.id,
                        old_state=old_state,
                        new_state=new_state,
                        details=diagnosis,
                    )
                )

        return RecordVerificationResult(
            record_id=record.id,
            name=record.name,
            old_state=old_state,
            new_state=new_state,
            local_status=local_status,
            primary_exists=p_ok,
            mirror_exists=m_ok,
            diagnosis=diagnosis,
        )

    async def execute(
        self,
        record_id: str | None = None,
        dry_run: bool = False,
    ) -> VerifyVaultSummary:
        records = [self.repo.get(record_id)] if record_id else self.repo.list_all()
        summary = VerifyVaultSummary(total_checked=len(records))

        for rec in records:
            res = await self.verify_record(rec, dry_run=dry_run)
            summary.results.append(res)
            if res.new_state == HealthState.HEALTHY:
                summary.healthy_count += 1
            elif res.new_state == HealthState.DEGRADED:
                summary.degraded_count += 1
            elif res.new_state == HealthState.LOST:
                summary.lost_count += 1

        if self.event_bus:
            self.event_bus.publish(
                VerificationResultEvent(
                    healthy_count=summary.healthy_count,
                    degraded_count=summary.degraded_count,
                    lost_count=summary.lost_count,
                )
            )

        return summary
