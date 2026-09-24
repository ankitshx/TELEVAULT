import logging
from pathlib import Path
from typing import Any

from televault.application.heal import HealVaultUseCase
from televault.application.manifest import ManifestService
from televault.domain.entities import DoctorFinding
from televault.domain.ports import (
    AuditLedger,
    ManifestRepository,
    TelegramGateway,
    TransactionJournal,
    VaultRepository,
)
from televault.domain.states import DoctorHealthState, HealthState

logger = logging.getLogger("televault.doctor")


class VaultDoctorUseCase:
    """Comprehensive diagnostic engine inspecting Telegram auth, channels, SQLite, manifests, and transactions."""

    def __init__(
        self,
        gateway: TelegramGateway,
        repo: VaultRepository,
        journal: TransactionJournal | None = None,
        manifest_repo: ManifestRepository | None = None,
        audit_ledger: AuditLedger | None = None,
    ):
        self.gateway = gateway
        self.repo = repo
        self.journal = journal
        self.manifest_repo = manifest_repo
        self.audit_ledger = audit_ledger

    async def diagnose(self, deep: bool = False) -> list[DoctorFinding]:
        """Run all diagnostic checks and return evidence-based structured findings."""
        findings: list[DoctorFinding] = []

        # 1. Telegram Authentication Check
        try:
            channels = await self.gateway.ensure_vaults()
            findings.append(DoctorFinding(
                category="AUTHENTICATION",
                state=DoctorHealthState.HEALTHY,
                title="Telegram Session Active",
                what_happened="Successfully connected to Telegram MTProto session.",
                why="Valid session and credentials present.",
                what_is_safe="Cloud storage operations permitted.",
                recommended_action="None required.",
            ))

            # 2. Vault Channels Check
            if channels.primary_id and channels.mirror_id:
                findings.append(DoctorFinding(
                    category="CHANNELS",
                    state=DoctorHealthState.HEALTHY,
                    title="Dual Channels Configured",
                    what_happened=f"Primary (ID: {channels.primary_id}) and Mirror (ID: {channels.mirror_id}) online.",
                    why="Both private channels exist and are linked.",
                    what_is_safe="Dual-write redundancy operational.",
                    recommended_action="None required.",
                ))
        except Exception as e:
            findings.append(DoctorFinding(
                category="AUTHENTICATION",
                state=DoctorHealthState.DEGRADED,
                title="Telegram Connection Degraded",
                what_happened=f"Could not connect to Telegram: {e}",
                why="Network offline, invalid credentials, or session expired.",
                what_is_safe="Local SQLite database and recovery cache intact.",
                recommended_action="Run 'televault login' to re-authenticate.",
                action_type="LOGIN",
            ))

        # 3. Database & Records Integrity
        records = self.repo.list_all()
        healthy_count = sum(1 for r in records if r.state == HealthState.HEALTHY)
        degraded_count = sum(1 for r in records if r.state == HealthState.DEGRADED)
        lost_count = sum(1 for r in records if r.state == HealthState.LOST)

        if degraded_count > 0:
            findings.append(DoctorFinding(
                category="REDUNDANCY",
                state=DoctorHealthState.DEGRADED,
                title=f"{degraded_count} Degraded File Record(s) Detected",
                what_happened=f"{degraded_count} file(s) have only 1 surviving cloud copy.",
                why="One copy was removed or forward was incomplete.",
                what_is_safe="Files remain fully recoverable from the surviving copy.",
                recommended_action="Run auto-healing to forward surviving copies back to the mirror.",
                action_type="REPAIR_MIRROR",
            ))

        if lost_count > 0:
            findings.append(DoctorFinding(
                category="REDUNDANCY",
                state=DoctorHealthState.LOST,
                title=f"{lost_count} Lost File Record(s)",
                what_happened=f"{lost_count} file(s) have both Primary and Mirror copies missing.",
                why="Both cloud messages were deleted and no recovery copy exists.",
                what_is_safe="Local PC copy may still exist if not deleted locally.",
                recommended_action="Back up the local file again if available.",
                action_type="REBACKUP",
            ))

        if degraded_count == 0 and lost_count == 0:
            findings.append(DoctorFinding(
                category="REDUNDANCY",
                state=DoctorHealthState.HEALTHY,
                title=f"All {len(records)} Files Healthy",
                what_happened="Every protected file has verified dual-write cloud redundancy.",
                why="Primary and Mirror copies match index records.",
                what_is_safe="All protected data is verified and redundant.",
                recommended_action="None required.",
            ))

        # 4. Pending / Interrupted Transactions Check
        if self.journal:
            pending = self.journal.list_pending()
            if pending:
                findings.append(DoctorFinding(
                    category="TRANSACTIONS",
                    state=DoctorHealthState.ATTENTION,
                    title=f"{len(pending)} Interrupted Transaction(s) Found",
                    what_happened=f"{len(pending)} backup operation(s) did not complete due to past process shutdown.",
                    why="Application was terminated mid-upload or mid-mirroring.",
                    what_is_safe="Surviving Primary uploads can be mirrored automatically.",
                    recommended_action="Run transaction recovery to finalize mirror copies.",
                    action_type="RECOVER_TRANSACTIONS",
                ))
            else:
                findings.append(DoctorFinding(
                    category="TRANSACTIONS",
                    state=DoctorHealthState.HEALTHY,
                    title="Transaction Journal Clean",
                    what_happened="No unfinished or interrupted transactions.",
                    why="All past backup operations completed or resolved.",
                    what_is_safe="No transaction leaks.",
                    recommended_action="None required.",
                ))

        # 5. Tamper-Evident Manifest Integrity Check
        if self.manifest_repo:
            manifest_service = ManifestService(self.repo, self.manifest_repo)
            is_valid, msg = manifest_service.verify_chain()
            if is_valid:
                findings.append(DoctorFinding(
                    category="MANIFEST",
                    state=DoctorHealthState.HEALTHY,
                    title="Vault Manifest Chain Intact",
                    what_happened=msg,
                    why="All manifest generations cryptographically hash-chained.",
                    what_is_safe="Zero tampering detected in vault metadata.",
                    recommended_action="None required.",
                ))
            else:
                findings.append(DoctorFinding(
                    category="MANIFEST",
                    state=DoctorHealthState.DEGRADED,
                    title="Manifest Integrity Failure",
                    what_happened=msg,
                    why="Manifest hash chain does not match expected cryptographic signatures.",
                    what_is_safe="Cloud data remains intact; metadata history compromised.",
                    recommended_action="Generate fresh manifest snapshot from verified records.",
                    action_type="REGENERATE_MANIFEST",
                ))

        # 6. Audit Ledger Integrity Check
        if self.audit_ledger:
            is_valid, msg = self.audit_ledger.verify_integrity()
            if is_valid:
                findings.append(DoctorFinding(
                    category="AUDIT_LEDGER",
                    state=DoctorHealthState.HEALTHY,
                    title="Audit Ledger Verified",
                    what_happened=msg,
                    why="Cryptographic hash chaining of all audit events is valid.",
                    what_is_safe="Complete audit trail is authentic and tamper-evident.",
                    recommended_action="None required.",
                ))
            else:
                findings.append(DoctorFinding(
                    category="AUDIT_LEDGER",
                    state=DoctorHealthState.DEGRADED,
                    title="Audit Ledger Integrity Failure",
                    what_happened=msg,
                    why="One or more historical audit records were modified or deleted.",
                    what_is_safe="Cloud files intact; local audit history altered.",
                    recommended_action="Review forensic audit log.",
                    action_type="REVIEW_AUDIT",
                ))

        return findings

    async def repair(self) -> list[str]:
        """Perform non-destructive repairs (heal degraded items and recover transactions)."""
        actions_taken: list[str] = []

        # 1. Recover pending transactions
        if self.journal:
            from televault.application.transaction import TransactionManager
            tm = TransactionManager(self.journal, self.repo, self.gateway, self.audit_ledger)
            tx_recovered = await tm.recover_interrupted_transactions()
            actions_taken.extend(tx_recovered)

        # 2. Heal degraded records
        heal_uc = HealVaultUseCase(self.gateway, self.repo)
        heal_summary = await heal_uc.execute()
        if heal_summary.healed_count > 0:
            actions_taken.append(f"Healed {heal_summary.healed_count} degraded records by re-forwarding surviving copies.")

        return actions_taken
