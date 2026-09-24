import argparse
import asyncio
import os
from pathlib import Path
import sys

from televault.application.backup import BackupFileUseCase
from televault.application.event_bus import SimpleEventBus
from televault.application.restore import RestoreFileUseCase
from televault.domain.ports import TelegramGateway
from televault.domain.states import HealthState, VaultMode
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.os.keyring_store import KeyringStore
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


def get_gateway(use_fake: bool = False, config: TeleVaultConfig | None = None) -> TelegramGateway:
    """Resolve gateway: FakeTelegramGateway for tests/simulations or TelethonGateway."""
    # Check TV_LIVE_TEST environment flag or explicit --fake argument
    live_test_enabled = os.environ.get("TV_LIVE_TEST", "0") == "1"

    if use_fake or not live_test_enabled:
        return FakeTelegramGateway()

    # If TV_LIVE_TEST=1 is explicitly passed, load live gateway with credentials from keyring
    keyring_store = KeyringStore()
    api_id = keyring_store.get_credential("api_id") or os.environ.get("TELEVAULT_API_ID")
    api_hash = keyring_store.get_credential("api_hash") or os.environ.get("TELEVAULT_API_HASH")

    if not api_id or not api_hash:
        print("Note: Live Telegram credentials not configured. Falling back to FakeTelegramGateway.")
        return FakeTelegramGateway()

    from telethon import TelegramClient
    from televault.infrastructure.telegram.telethon_gateway import TelethonGateway

    session_path = config.session_name if config else "televault_session"
    client = TelegramClient(session_path, int(api_id), api_hash)
    return TelethonGateway(client)


async def handle_backup(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    event_bus = SimpleEventBus()
    gateway = get_gateway(use_fake=args.fake, config=config)

    use_case = BackupFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)
    target_path = Path(args.path)

    if not target_path.exists():
        print(f"Error: Path does not exist: {target_path}", file=sys.stderr)
        return 1

    files_to_backup = [target_path] if target_path.is_file() else list(target_path.rglob("*"))
    files_to_backup = [f for f in files_to_backup if f.is_file()]

    if not files_to_backup:
        print("No files found to back up.")
        return 0

    print(f"Backing up {len(files_to_backup)} file(s)... (dry-run: {args.dry_run})")
    for file_path in files_to_backup:
        print(f"-> Processing: {file_path.name}")
        result = await use_case.execute(
            file_path=file_path,
            mode=VaultMode.PRIVATE if args.private else VaultMode.ORIGINAL,
            dry_run=args.dry_run,
        )
        if result.is_duplicate:
            print(f"   [ALREADY BACKED UP] {result.message}")
        elif result.dry_run:
            print(f"   {result.message}")
        else:
            state_label = "[HEALTHY]" if result.record.state == HealthState.HEALTHY else "[DEGRADED]"
            print(f"   [DONE] {result.record.id} | {state_label} | Size: {result.record.size} bytes")

    return 0


async def handle_restore(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    event_bus = SimpleEventBus()
    gateway = get_gateway(use_fake=args.fake, config=config)

    use_case = RestoreFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)
    dest_dir = Path(args.dest) if args.dest else Path.cwd() / "restored"

    print(f"Restoring record {args.id} to {dest_dir}... (dry-run: {args.dry_run})")
    try:
        result = await use_case.execute(
            record_id=args.id,
            dest_dir=dest_dir,
            dry_run=args.dry_run,
        )
        if result.success:
            print(f"[RESTORE OK] Restored to: {result.restored_path}")
            print(f"             SHA-256 Verified: {result.sha256_matched}")
            return 0
        else:
            print(f"[RESTORE FAILED] {result.message}", file=sys.stderr)
            return 1
    except KeyError:
        print(f"Error: Record with ID '{args.id}' not found in index.", file=sys.stderr)
        return 1


async def handle_verify(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    recovery_cache = RecoveryCache(config.cache_dir)
    gateway = get_gateway(use_fake=args.fake, config=config)

    from televault.application.heal import HealVaultUseCase
    from televault.application.verify import VerifyVaultUseCase

    verify_uc = VerifyVaultUseCase(gateway=gateway, repo=repo, recovery_cache=recovery_cache)
    summary = await verify_uc.execute(dry_run=args.dry_run)

    print(f"Verification Complete: Checked {summary.total_checked} item(s)")
    print(f"  [HEALTHY]:  {summary.healthy_count}")
    print(f"  [DEGRADED]: {summary.degraded_count}")
    print(f"  [LOST]:     {summary.lost_count}")

    for r in summary.results:
        state_tag = f"[{r.new_state.value}]"
        print(f"  -> {r.name:<25} {state_tag:<12} {r.diagnosis}")

    if args.heal and summary.degraded_count > 0:
        print("\nStarting auto-healing for DEGRADED records...")
        heal_uc = HealVaultUseCase(gateway=gateway, repo=repo, recovery_cache=recovery_cache)
        heal_summary = await heal_uc.execute(dry_run=args.dry_run)
        print(f"Healing Complete: {heal_summary.healed_count} healed, {heal_summary.failed_count} failed.")
        for item in heal_summary.results:
            print(f"  -> {item.name}: {item.action_taken}")

    return 0


async def handle_rebuild(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    gateway = get_gateway(use_fake=args.fake, config=config)

    from televault.application.rebuild import RebuildIndexUseCase

    rebuild_uc = RebuildIndexUseCase(gateway=gateway, repo=repo)
    print("Scanning Telegram Primary and Mirror channels for tv1 captions...")
    summary = await rebuild_uc.execute(dry_run=args.dry_run)

    print(f"Rebuild Complete: (dry-run: {args.dry_run})")
    print(f"  Primary messages scanned: {summary.primary_messages_scanned}")
    print(f"  Mirror messages scanned:  {summary.mirror_messages_scanned}")
    print(f"  Records reconstructed:    {summary.records_reconstructed}")
    print(f"    - [HEALTHY]:            {summary.records_healthy}")
    print(f"    - [DEGRADED]:           {summary.records_degraded}")
    return 0


async def handle_snapshot(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    gateway = get_gateway(use_fake=args.fake, config=config)

    from televault.application.snapshot import SnapshotUseCase

    snapshot_uc = SnapshotUseCase(gateway=gateway, repo=repo, temp_dir=config.cache_dir)
    print("Creating SQLite index snapshot and uploading to Primary vault...")
    dest = await snapshot_uc.execute(dry_run=args.dry_run)
    print(f"Snapshot exported: {dest.name}")
    return 0


async def handle_ls(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)

    records = repo.list_all()
    if not records:
        print("Vault is empty. Use 'televault backup <path>' to back up your first file.")
        return 0

    print(f"{'ID':<38} {'NAME':<24} {'SIZE':<12} {'HEALTH':<12} {'LOCAL'}")
    print("-" * 95)
    for r in records:
        size_str = f"{r.size / (1024 * 1024):.2f} MB" if r.size > 1024 * 1024 else f"{r.size / 1024:.1f} KB"
        health_str = f"[{r.state.value}]"
        print(f"{r.id:<38} {r.name[:23]:<24} {size_str:<12} {health_str:<12} {r.local_status.value}")

    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="televault",
        description="TeleVault: Manual-only, append-only resilient desktop backup vault.",
    )
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # backup
    p_backup = subparsers.add_parser("backup", help="Back up a file or directory")
    p_backup.add_argument("path", help="Path of the file or folder to back up")
    p_backup.add_argument("--dry-run", action="store_true", help="Print actions without modifying state")
    p_backup.add_argument("--private", action="store_true", help="Encrypt with Private Mode")
    p_backup.add_argument("--fake", action="store_true", default=True, help="Force in-memory fake gateway")

    # restore
    p_restore = subparsers.add_parser("restore", help="Restore a file from the vault")
    p_restore.add_argument("id", help="Record ID of the file to restore")
    p_restore.add_argument("--dest", help="Destination directory to restore to")
    p_restore.add_argument("--dry-run", action="store_true", help="Simulate restore without downloading")
    p_restore.add_argument("--fake", action="store_true", default=True, help="Force in-memory fake gateway")

    # ls
    p_ls = subparsers.add_parser("ls", help="List all files in the vault")
    p_ls.add_argument("--fake", action="store_true", default=True, help="Use local SQLite database")

    # verify
    p_verify = subparsers.add_parser("verify", help="Verify presence and health of vault files")
    p_verify.add_argument("--heal", action="store_true", help="Automatically heal degraded records")
    p_verify.add_argument("--dry-run", action="store_true", help="Report status without modifying state")
    p_verify.add_argument("--fake", action="store_true", default=True, help="Force in-memory fake gateway")

    # rebuild
    p_rebuild = subparsers.add_parser("rebuild", help="Reconstruct local index from Telegram channels")
    p_rebuild.add_argument("--dry-run", action="store_true", help="Scan channels without modifying local DB")
    p_rebuild.add_argument("--fake", action="store_true", default=True, help="Force in-memory fake gateway")

    # snapshot
    p_snapshot = subparsers.add_parser("snapshot", help="Export and upload an index snapshot to Primary vault")
    p_snapshot.add_argument("--dry-run", action="store_true", help="Export snapshot locally without uploading")
    p_snapshot.add_argument("--fake", action="store_true", default=True, help="Force in-memory fake gateway")

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    if args.command == "backup":
        sys.exit(asyncio.run(handle_backup(args)))
    elif args.command == "restore":
        sys.exit(asyncio.run(handle_restore(args)))
    elif args.command == "ls":
        sys.exit(asyncio.run(handle_ls(args)))
    elif args.command == "verify":
        sys.exit(asyncio.run(handle_verify(args)))
    elif args.command == "rebuild":
        sys.exit(asyncio.run(handle_rebuild(args)))
    elif args.command == "snapshot":
        sys.exit(asyncio.run(handle_snapshot(args)))
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
