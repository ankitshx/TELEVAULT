from dataclasses import dataclass
from pathlib import Path

from televault.application.caption import format_tv1_caption
from televault.application.hashing import calculate_sha256
from televault.domain.entities import FileRecord, MessageRef
from televault.domain.events import FileStateChangedEvent
from televault.domain.ports import EventBus, TelegramGateway, VaultRepository
from televault.domain.states import ExistsStatus, HealthState
from televault.infrastructure.storage.recovery_cache import RecoveryCache


@dataclass
class HealItemResult:
    record_id: str
    name: str
    old_state: HealthState
    new_state: HealthState
    action_taken: str
    success: bool


@dataclass
class HealVaultSummary:
    total_processed: int = 0
    healed_count: int = 0
    failed_count: int = 0
    results: list[HealItemResult] = None  # type: ignore

    def __post_init__(self):
        if self.results is None:
            self.results = []


class HealVaultUseCase:
    """Restores redundancy to DEGRADED vault items by re-forwarding surviving cloud copies

    or re-uploading from local recovery cache.

    Guarantees:
    - Never uploads new local files (only heals existing backed-up items).
    - Uses server-side forward where possible (zero re-upload bandwidth).
    - Re-verifies both channels before marking HEALTHY.
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

    async def heal_record(
        self,
        record: FileRecord,
        dry_run: bool = False,
    ) -> HealItemResult:
        old_state = record.state
        vaults = await self.gateway.ensure_vaults()

        # Check which copies exist
        p_ok = False
        m_ok = False
        if record.primary_ref:
            check_p = await self.gateway.exists(record.primary_ref)
            p_ok = (check_p.status == ExistsStatus.PRESENT)
        if record.mirror_ref:
            check_m = await self.gateway.exists(record.mirror_ref)
            m_ok = (check_m.status == ExistsStatus.PRESENT)

        if p_ok and m_ok:
            return HealItemResult(
                record_id=record.id,
                name=record.name,
                old_state=old_state,
                new_state=HealthState.HEALTHY,
                action_taken="Both copies already present; no healing needed.",
                success=True,
            )

        # Case 1: Primary exists, Mirror missing -> Forward Primary to Mirror
        if p_ok and not m_ok and record.primary_ref:
            if dry_run:
                return HealItemResult(
                    record_id=record.id,
                    name=record.name,
                    old_state=old_state,
                    new_state=HealthState.HEALTHY,
                    action_taken="[DRY-RUN] Would forward surviving Primary copy to Mirror channel.",
                    success=True,
                )

            new_mirror_ref = await self.gateway.forward(
                record.primary_ref, to_channel=vaults.mirror_id
            )
            record.mirror_ref = new_mirror_ref
            record.state = HealthState.HEALTHY
            self.repo.add(record)

            if self.event_bus:
                self.event_bus.publish(
                    FileStateChangedEvent(
                        file_id=record.id,
                        old_state=old_state,
                        new_state=HealthState.HEALTHY,
                        details="Healed by forwarding surviving Primary copy to Mirror.",
                    )
                )

            return HealItemResult(
                record_id=record.id,
                name=record.name,
                old_state=old_state,
                new_state=HealthState.HEALTHY,
                action_taken="Forwarded surviving Primary copy to Mirror channel.",
                success=True,
            )

        # Case 2: Mirror exists, Primary missing -> Forward Mirror to Primary
        if not p_ok and m_ok and record.mirror_ref:
            if dry_run:
                return HealItemResult(
                    record_id=record.id,
                    name=record.name,
                    old_state=old_state,
                    new_state=HealthState.HEALTHY,
                    action_taken="[DRY-RUN] Would forward surviving Mirror copy to Primary channel.",
                    success=True,
                )

            new_primary_ref = await self.gateway.forward(
                record.mirror_ref, to_channel=vaults.primary_id
            )
            record.primary_ref = new_primary_ref
            record.state = HealthState.HEALTHY
            self.repo.add(record)

            if self.event_bus:
                self.event_bus.publish(
                    FileStateChangedEvent(
                        file_id=record.id,
                        old_state=old_state,
                        new_state=HealthState.HEALTHY,
                        details="Healed by forwarding surviving Mirror copy to Primary.",
                    )
                )

            return HealItemResult(
                record_id=record.id,
                name=record.name,
                old_state=old_state,
                new_state=HealthState.HEALTHY,
                action_taken="Forwarded surviving Mirror copy to Primary channel.",
                success=True,
            )

        # Case 3: Both missing -> Try local recovery copy
        recovery_file = (
            self.recovery_cache.get(record.sha256) if self.recovery_cache else None
        )
        if recovery_file and recovery_file.is_file():
            # Validate hash before re-upload
            h, _ = calculate_sha256(recovery_file)
            if h.lower() == record.sha256.lower():
                if dry_run:
                    return HealItemResult(
                        record_id=record.id,
                        name=record.name,
                        old_state=old_state,
                        new_state=HealthState.HEALTHY,
                        action_taken="[DRY-RUN] Would restore and re-upload from local recovery copy.",
                        success=True,
                    )

                caption = format_tv1_caption(record)
                new_p_ref = await self.gateway.upload_single(recovery_file, caption=caption)
                new_m_ref = await self.gateway.forward(new_p_ref, to_channel=vaults.mirror_id)

                record.primary_ref = new_p_ref
                record.mirror_ref = new_m_ref
                record.state = HealthState.HEALTHY
                self.repo.add(record)

                if self.event_bus:
                    self.event_bus.publish(
                        FileStateChangedEvent(
                            file_id=record.id,
                            old_state=old_state,
                            new_state=HealthState.HEALTHY,
                            details="Restored and re-uploaded from local recovery cache.",
                        )
                    )

                return HealItemResult(
                    record_id=record.id,
                    name=record.name,
                    old_state=old_state,
                    new_state=HealthState.HEALTHY,
                    action_taken="Restored and re-uploaded from local recovery cache.",
                    success=True,
                )

        # Unrecoverable
        if not dry_run:
            record.state = HealthState.LOST
            self.repo.add(record)

        return HealItemResult(
            record_id=record.id,
            name=record.name,
            old_state=old_state,
            new_state=HealthState.LOST,
            action_taken="Both copies missing and no local recovery copy found. Marked LOST.",
            success=False,
        )

    async def execute(
        self,
        record_id: str | None = None,
        dry_run: bool = False,
    ) -> HealVaultSummary:
        records = [self.repo.get(record_id)] if record_id else self.repo.list_all()
        # Filter for items that need healing
        targets = [r for r in records if r.state == HealthState.DEGRADED or record_id is not None]

        summary = HealVaultSummary(total_processed=len(targets))
        for rec in targets:
            res = await self.heal_record(rec, dry_run=dry_run)
            summary.results.append(res)
            if res.success and res.new_state == HealthState.HEALTHY:
                summary.healed_count += 1
            else:
                summary.failed_count += 1

        return summary
