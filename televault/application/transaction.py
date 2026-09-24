import logging
from pathlib import Path
from typing import Any
import uuid

from televault.domain.entities import FileRecord, MessageRef, TransactionRecord
from televault.domain.ports import (
    AuditLedger,
    TelegramGateway,
    TransactionJournal,
    VaultRepository,
)
from televault.domain.states import HealthState, LocalFileStatus, TransactionState, VaultMode

logger = logging.getLogger("televault.transaction")


class TransactionManager:
    """Orchestrates crash-safe persistent backup transactions and startup recovery."""

    def __init__(
        self,
        journal: TransactionJournal,
        repo: VaultRepository,
        gateway: TelegramGateway,
        audit_ledger: AuditLedger | None = None,
    ):
        self.journal = journal
        self.repo = repo
        self.gateway = gateway
        self.audit_ledger = audit_ledger

    def begin(self, file_path: Path, mode: VaultMode = VaultMode.ORIGINAL) -> TransactionRecord:
        """Create a new persistent transaction in CREATED state."""
        tx = TransactionRecord(
            id=str(uuid.uuid4()),
            file_path=str(file_path.resolve()),
            file_name=file_path.name,
            size=file_path.stat().st_size if file_path.exists() else 0,
            mode=mode,
            state=TransactionState.CREATED,
        )
        self.journal.create(tx)
        if self.audit_ledger:
            self.audit_ledger.append_event(
                operation="TRANSACTION_CREATED",
                record_id=tx.id,
                result="SUCCESS",
                details=f"Backup transaction started for {tx.file_name}",
            )
        return tx

    def update_hashed(self, tx_id: str, sha256: str, size: int) -> None:
        self.journal.update_state(tx_id, TransactionState.HASHED, sha256=sha256, size=size)

    def update_uploading(self, tx_id: str) -> None:
        self.journal.update_state(tx_id, TransactionState.UPLOADING)

    def update_primary_complete(self, tx_id: str, primary_ref: MessageRef) -> None:
        self.journal.update_state(tx_id, TransactionState.PRIMARY_COMPLETE, primary_ref=primary_ref)

    def update_mirror_complete(self, tx_id: str, mirror_ref: MessageRef) -> None:
        self.journal.update_state(tx_id, TransactionState.MIRROR_COMPLETE, mirror_ref=mirror_ref)

    def complete(self, tx_id: str, record_id: str) -> None:
        self.journal.complete(tx_id, record_id=record_id)
        if self.audit_ledger:
            self.audit_ledger.append_event(
                operation="TRANSACTION_COMPLETED",
                record_id=record_id,
                result="SUCCESS",
                details=f"Transaction {tx_id} completed successfully",
            )

    def fail(self, tx_id: str, error: str) -> None:
        self.journal.fail(tx_id, error=error)
        if self.audit_ledger:
            self.audit_ledger.append_event(
                operation="TRANSACTION_FAILED",
                record_id=tx_id,
                result="ERROR",
                details=error,
            )

    async def recover_interrupted_transactions(self) -> list[str]:
        """Scan pending transactions on startup and recover any interrupted mid-flight operations."""
        pending = self.journal.list_pending()
        recovered_actions: list[str] = []

        for tx in pending:
            if tx.state == TransactionState.PRIMARY_COMPLETE and tx.primary_ref:
                logger.info(f"Crash recovery: Resuming mirror forward for transaction {tx.id} ({tx.file_name})")
                try:
                    channels = await self.gateway.ensure_vaults()
                    mirror_ref = await self.gateway.forward(tx.primary_ref, to_channel=channels.mirror_id)
                    self.journal.update_state(tx.id, TransactionState.MIRROR_COMPLETE, mirror_ref=mirror_ref)

                    rec_id = tx.record_id or str(uuid.uuid4())
                    record = FileRecord(
                        id=rec_id,
                        name=tx.file_name,
                        original_path=tx.file_path,
                        sha256=tx.sha256,
                        size=tx.size,
                        state=HealthState.HEALTHY,
                        primary_ref=tx.primary_ref,
                        mirror_ref=mirror_ref,
                        local_status=LocalFileStatus.PRESENT if Path(tx.file_path).exists() else LocalFileStatus.MISSING,
                        mode=tx.mode,
                    )
                    self.repo.add(record)
                    self.journal.complete(tx.id, record_id=rec_id)

                    action_msg = f"Recovered {tx.file_name}: Primary was intact; mirrored to channel {channels.mirror_id}."
                    recovered_actions.append(action_msg)
                    if self.audit_ledger:
                        self.audit_ledger.append_event(
                            operation="TRANSACTION_RECOVERED",
                            record_id=rec_id,
                            result="SUCCESS",
                            details=action_msg,
                        )
                except Exception as e:
                    logger.error(f"Failed recovering transaction {tx.id}: {e}")
                    self.journal.fail(tx.id, f"Crash recovery failed: {e}")
            elif tx.state in (TransactionState.CREATED, TransactionState.HASHED, TransactionState.UPLOADING):
                # Interrupted before completion of cloud upload; mark failed cleanly
                self.journal.fail(tx.id, "Interrupted by system shutdown before cloud upload completed.")
                recovered_actions.append(f"Cleared incomplete local transaction for {tx.file_name}.")

        return recovered_actions
