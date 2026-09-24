import argparse
import asyncio
import os
from pathlib import Path
import sys
import webbrowser

from rich import box
from rich.console import Console
from rich.panel import Panel
from rich.progress import (
    BarColumn,
    DownloadColumn,
    Progress,
    SpinnerColumn,
    TextColumn,
    TimeRemainingColumn,
    TransferSpeedColumn,
)
from rich.prompt import Prompt
from rich.table import Table
from rich.text import Text

from televault.application.backup import BackupFileUseCase
from televault.application.event_bus import SimpleEventBus
from televault.application.restore import RestoreFileUseCase
from televault.domain.ports import TelegramGateway
from televault.domain.states import HealthState, VaultMode
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from televault.infrastructure.telegram.auth_service import TelegramAuthService
from tests.fakes.fake_gateway import FakeTelegramGateway

# Safe console for Windows cp1252 and UTF-8
console = Console(highlight=False)


async def get_gateway(use_fake: bool = False, config: TeleVaultConfig | None = None) -> TelegramGateway:
    """Resolve gateway: live TelethonGateway if user is authorized, else FakeTelegramGateway."""
    if use_fake:
        return FakeTelegramGateway()

    auth_service = TelegramAuthService(config)
    if await auth_service.is_authorized():
        live_gateway = await auth_service.get_live_gateway()
        if live_gateway:
            return live_gateway

    return FakeTelegramGateway()


async def handle_login(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    auth_service = TelegramAuthService(config)

    console.print(Panel.fit(
        "[bold cyan]TeleVault Telegram Authentication[/bold cyan]\n"
        "[dim]Connect as an MTProto client session. Credentials from https://my.telegram.org[/dim]",
        border_style="cyan",
        box=box.ASCII,
    ))

    stored = auth_service.get_stored_credentials()
    default_api_id = stored.get("api_id") or ""
    default_phone = stored.get("phone") or ""

    api_id = Prompt.ask("[bold]Enter Telegram API ID[/bold]", default=default_api_id)
    api_hash = Prompt.ask("[bold]Enter Telegram API Hash[/bold]", password=True)
    phone = Prompt.ask("[bold]Enter Phone Number (e.g. +1234567890)[/bold]", default=default_phone)

    with console.status("[bold green]Sending Telegram confirmation code...[/bold green]"):
        try:
            send_res = await auth_service.send_login_code(
                api_id=api_id.strip(),
                api_hash=api_hash.strip(),
                phone=phone.strip(),
            )
        except Exception as e:
            console.print(f"[bold red]Failed to send code:[/bold red] {e}")
            return 1

    console.print(f"[green]Confirmation code sent to {phone} via Telegram.[/green]")
    code = Prompt.ask("[bold]Enter confirmation code received in Telegram[/bold]")

    password = None
    with console.status("[bold green]Verifying and connecting to Telegram...[/bold green]"):
        try:
            res = await auth_service.complete_login(
                code=code.strip(),
                phone_code_hash=send_res["phone_code_hash"],
                password=password,
            )
        except Exception as e:
            console.print(f"[bold red]Login error:[/bold red] {e}")
            return 1

    if res.get("status") == "2fa_required":
        console.print("[yellow]Two-step verification (Cloud 2FA password) is enabled on this account.[/yellow]")
        password = Prompt.ask("[bold]Enter your 2FA password[/bold]", password=True)
        with console.status("[bold green]Verifying 2FA password...[/bold green]"):
            try:
                res = await auth_service.complete_login(
                    code=code.strip(),
                    phone_code_hash=send_res["phone_code_hash"],
                    password=password,
                )
            except Exception as e:
                console.print(f"[bold red]2FA error:[/bold red] {e}")
                return 1

    if res.get("status") == "success":
        u = res["user"]
        prem_badge = " [bold yellow]* Premium[/bold yellow]" if u.get("is_premium") else ""
        console.print(Panel.fit(
            f"[bold green]Successfully Logged In![/bold green]\n"
            f"[bold white]Name:[/bold white] {u.get('first_name')} {u.get('last_name') or ''}{prem_badge}\n"
            f"[bold white]Username:[/bold white] @{u.get('username') or 'N/A'}\n"
            f"[bold white]Phone:[/bold white] {u.get('phone')}\n\n"
            f"[bold cyan]Storage Vaults:[/bold cyan]\n"
            f"  Primary Channel: {res['channels']['primary_id']}\n"
            f"  Mirror Channel:  {res['channels']['mirror_id']}",
            border_style="green",
            box=box.ASCII,
            title="TeleVault Connected",
        ))
        return 0
    else:
        console.print(f"[bold red]Authentication failed:[/bold red] {res.get('message')}")
        return 1


async def handle_logout(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    auth_service = TelegramAuthService(config)
    auth_service.clear_credentials()
    console.print("[green]TeleVault credentials and session cleared. Switched to simulation mode.[/green]")
    return 0


async def handle_backup(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    event_bus = SimpleEventBus()
    gateway = await get_gateway(use_fake=args.fake, config=config)

    use_case = BackupFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)
    target_path = Path(args.path)

    if not target_path.exists():
        console.print(f"[bold red]Error:[/bold red] Path does not exist: {target_path}", file=sys.stderr)
        return 1

    files_to_backup = [target_path] if target_path.is_file() else list(target_path.rglob("*"))
    files_to_backup = [f for f in files_to_backup if f.is_file()]

    if not files_to_backup:
        console.print("[yellow]No files found to back up.[/yellow]")
        return 0

    mode_label = "[magenta]PRIVATE[/magenta]" if args.private else "[cyan]ORIGINAL[/cyan]"
    console.print(f"\nBacking up {len(files_to_backup)} file(s)... (mode: {mode_label}, dry-run: {args.dry_run})")

    for file_path in files_to_backup:
        file_size = file_path.stat().st_size
        size_str = f"{file_size / (1024 * 1024):.2f} MB" if file_size > 1024 * 1024 else f"{file_size / 1024:.1f} KB"

        result = await use_case.execute(
            file_path=file_path,
            mode=VaultMode.PRIVATE if args.private else VaultMode.ORIGINAL,
            dry_run=args.dry_run,
        )

        if result.is_duplicate:
            console.print(f"   [ALREADY BACKED UP] {result.message}")
        elif result.dry_run:
            console.print(f"   {result.message}")
        else:
            state_label = "[HEALTHY]" if result.record.state == HealthState.HEALTHY else "[DEGRADED]"
            state_color = "green" if result.record.state == HealthState.HEALTHY else "yellow"
            console.print(f"   [DONE] {result.record.id} | [{state_color}]{state_label}[/{state_color}] | Size: {result.record.size} bytes | {file_path.name}")

    return 0


async def handle_restore(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    event_bus = SimpleEventBus()
    gateway = await get_gateway(use_fake=args.fake, config=config)

    use_case = RestoreFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)
    dest_dir = Path(args.dest) if args.dest else Path.cwd() / "restored"

    console.print(f"Restoring record {args.id} to {dest_dir}... (dry-run: {args.dry_run})")
    try:
        result = await use_case.execute(
            record_id=args.id,
            dest_dir=dest_dir,
            dry_run=args.dry_run,
        )
    except KeyError:
        console.print(f"Error: Record with ID '{args.id}' not found in index.", file=sys.stderr)
        return 1

    if result.success:
        console.print(f"[RESTORE OK] Restored to: {result.restored_path}")
        console.print(f"             SHA-256 Verified: {result.sha256_matched}")
        return 0
    else:
        console.print(f"[RESTORE FAILED] {result.message}", file=sys.stderr)
        return 1


async def handle_dashboard(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    auth_service = TelegramAuthService(config)

    is_auth = await auth_service.is_authorized()
    user_info = await auth_service.get_current_user() if is_auth else None
    creds = auth_service.get_stored_credentials()
    records = repo.list_all()

    total_size = sum(r.size for r in records)
    total_size_str = f"{total_size / (1024 * 1024):.2f} MB" if total_size > 1024 * 1024 else f"{total_size / 1024:.1f} KB"
    healthy_cnt = sum(1 for r in records if r.state == HealthState.HEALTHY)
    degraded_cnt = sum(1 for r in records if r.state == HealthState.DEGRADED)
    lost_cnt = sum(1 for r in records if r.state == HealthState.LOST)

    header = (
        "[bold cyan]TELEVAULT SECURE DASHBOARD[/bold cyan]  "
        "[dim]Manual-Only, Append-Only Telegram Backup Vault[/dim]\n"
    )
    if is_auth and user_info:
        header += f"[green][ONLINE] Live MTProto Account:[/green] [bold white]{user_info['first_name']}[/bold white] (@{user_info.get('username') or 'N/A'}) | {user_info.get('phone')}"
    else:
        header += "[yellow][SIMULATION] Fake Gateway Active[/yellow] [dim](Run 'televault login' to connect your Telegram account)[/dim]"

    console.print(Panel(header, border_style="cyan", box=box.ASCII))

    summary_table = Table(title="Vault Summary", border_style="dim", box=box.ASCII, show_header=True)
    summary_table.add_column("Indexed Files", style="bold white")
    summary_table.add_column("Total Size", style="cyan")
    summary_table.add_column("Healthy", style="bold green")
    summary_table.add_column("Degraded", style="bold yellow")
    summary_table.add_column("Lost", style="bold red")
    summary_table.add_column("Cloud Channels", style="magenta")

    chan_info = f"P: {creds.get('primary_channel_id') or 'N/A'} | M: {creds.get('mirror_channel_id') or 'N/A'}"
    summary_table.add_row(
        str(len(records)),
        total_size_str,
        str(healthy_cnt),
        str(degraded_cnt),
        str(lost_cnt),
        chan_info,
    )
    console.print(summary_table)

    if not records:
        console.print("Vault is empty. Use 'televault backup <path>' to back up your first file.")
        return 0

    table = Table(title="Protected Files", border_style="blue", box=box.ASCII, show_lines=False)
    table.add_column("Name", style="bold white")
    table.add_column("ID (Short)", style="dim cyan")
    table.add_column("Size", justify="right")
    table.add_column("Health", justify="center")
    table.add_column("Mode", justify="center")
    table.add_column("Local Status", style="dim")

    for r in records:
        size_str = f"{r.size / (1024 * 1024):.2f} MB" if r.size > 1024 * 1024 else f"{r.size / 1024:.1f} KB"

        if r.state == HealthState.HEALTHY:
            health_text = "[bold green][HEALTHY][/bold green]"
        elif r.state == HealthState.DEGRADED:
            health_text = "[bold yellow][DEGRADED][/bold yellow]"
        else:
            health_text = "[bold red][LOST][/bold red]"

        mode_text = "[magenta]PRIVATE[/magenta]" if r.mode == VaultMode.PRIVATE else "[dim]ORIGINAL[/dim]"

        table.add_row(
            r.name,
            r.id[:8],
            size_str,
            health_text,
            mode_text,
            r.local_status.value,
        )

    console.print(table)
    return 0


async def handle_ls(args: argparse.Namespace) -> int:
    return await handle_dashboard(args)


async def handle_verify(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    recovery_cache = RecoveryCache(config.cache_dir)
    gateway = await get_gateway(use_fake=args.fake, config=config)

    from televault.application.heal import HealVaultUseCase
    from televault.application.verify import VerifyVaultUseCase

    verify_uc = VerifyVaultUseCase(gateway=gateway, repo=repo, recovery_cache=recovery_cache)
    summary = await verify_uc.execute(dry_run=args.dry_run)

    console.print(f"Verification Complete: Checked {summary.total_checked} item(s)")
    console.print(f"  [HEALTHY]:  {summary.healthy_count}")
    console.print(f"  [DEGRADED]: {summary.degraded_count}")
    console.print(f"  [LOST]:     {summary.lost_count}")

    for r in summary.results:
        state_tag = f"[{r.new_state.value}]"
        console.print(f"  -> {r.name:<25} {state_tag:<12} {r.diagnosis}")

    if args.heal and summary.degraded_count > 0:
        console.print("\nStarting auto-healing for DEGRADED records...")
        heal_uc = HealVaultUseCase(gateway=gateway, repo=repo, recovery_cache=recovery_cache)
        heal_summary = await heal_uc.execute(dry_run=args.dry_run)
        console.print(f"Healing Complete: {heal_summary.healed_count} healed, {heal_summary.failed_count} failed.")
        for item in heal_summary.results:
            console.print(f"  -> {item.name}: {item.action_taken}")

    return 0


async def handle_rebuild(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    gateway = await get_gateway(use_fake=args.fake, config=config)

    from televault.application.rebuild import RebuildIndexUseCase

    rebuild_uc = RebuildIndexUseCase(gateway=gateway, repo=repo)
    console.print("Scanning Telegram Primary and Mirror channels for tv1 captions...")
    summary = await rebuild_uc.execute(dry_run=args.dry_run)

    console.print(f"Rebuild Complete: (dry-run: {args.dry_run})")
    console.print(f"  Primary messages scanned: {summary.primary_messages_scanned}")
    console.print(f"  Mirror messages scanned:  {summary.mirror_messages_scanned}")
    console.print(f"  Records reconstructed:    {summary.records_reconstructed}")
    console.print(f"    - [HEALTHY]:            {summary.records_healthy}")
    console.print(f"    - [DEGRADED]:           {summary.records_degraded}")
    return 0


async def handle_snapshot(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    gateway = await get_gateway(use_fake=args.fake, config=config)

    from televault.application.snapshot import SnapshotUseCase

    snapshot_uc = SnapshotUseCase(gateway=gateway, repo=repo, temp_dir=config.cache_dir)
    console.print("Creating SQLite index snapshot and uploading to Primary vault...")
    dest = await snapshot_uc.execute(dry_run=args.dry_run)
    console.print(f"Snapshot exported: {dest.name}")
    return 0


async def handle_doctor(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    gateway = await get_gateway(use_fake=args.fake, config=config)

    from televault.application.doctor import VaultDoctorUseCase
    from televault.domain.states import DoctorHealthState

    doctor = VaultDoctorUseCase(
        gateway=gateway,
        repo=repo,
        journal=repo,
        manifest_repo=repo,
        audit_ledger=repo,
    )

    console.print(Panel.fit(
        "[bold cyan]TeleVault Doctor: Diagnostic Health Scan[/bold cyan]",
        border_style="cyan",
        box=box.ASCII,
    ))

    with console.status("[bold green]Running comprehensive vault diagnostics...[/bold green]"):
        findings = await doctor.diagnose()

    table = Table(box=box.ROUNDED, show_header=True, header_style="bold cyan")
    table.add_column("Category", style="bold")
    table.add_column("Title")
    table.add_column("State")
    table.add_column("Details")
    table.add_column("Recommendation")

    has_critical = False
    for f in findings:
        color = "green"
        if f.state == DoctorHealthState.ATTENTION:
            color = "yellow"
        elif f.state == DoctorHealthState.DEGRADED:
            color = "magenta"
        elif f.state in (DoctorHealthState.RECOVERABLE, DoctorHealthState.LOST):
            color = "bold red"
            has_critical = True

        table.add_row(
            f.category,
            f.title,
            f"[{color}]{f.state.value}[/{color}]",
            f.what_happened,
            f.recommended_action or "-",
        )

    console.print(table)

    if args.repair:
        console.print("\n[bold yellow]Attempting automated repairs...[/bold yellow]")
        actions = await doctor.repair()
        if actions:
            for action in actions:
                console.print(f"  [green]✓[/green] {action}")
        else:
            console.print("  [dim]No repairs needed.[/dim]")

    return 1 if has_critical else 0


async def handle_recovery_test(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)
    gateway = await get_gateway(use_fake=args.fake, config=config)

    from televault.application.recovery_drill import RecoveryDrillUseCase

    drill = RecoveryDrillUseCase(
        gateway=gateway,
        repo=repo,
        scratch_dir=config.recovery_dir,
        audit_ledger=repo,
    )

    console.print(Panel.fit(
        "[bold cyan]TeleVault Recovery Drill (Non-Destructive Test)[/bold cyan]\n"
        "[dim]Downloads a file from Telegram cloud, verifies SHA-256 integrity, cleans up temp files.[/dim]",
        border_style="cyan",
        box=box.ASCII,
    ))

    with console.status("[bold green]Executing non-destructive recovery drill...[/bold green]"):
        try:
            report = await drill.execute(record_id=args.id, passphrase=args.passphrase)
        except Exception as e:
            console.print(f"[bold red]Drill Failed:[/bold red] {e}")
            return 1

    status_color = "green" if report.passed else "bold red"
    status_text = "PASSED" if report.passed else "FAILED"

    summary_text = (
        f"[bold]Target File:[/bold]        {report.file_name}\n"
        f"[bold]Target Record ID:[/bold]   {report.record_id}\n"
        f"[bold]Drill Mode:[/bold]         {report.mode.value}\n"
        f"[bold]File Size:[/bold]          {report.file_size} bytes\n"
        f"[bold]Duration:[/bold]           {report.duration_seconds:.2f}s\n"
        f"[bold]SHA-256 Matched:[/bold]    [{status_color}]{report.sha256_matched}[/{status_color}]\n"
        f"[bold]Size Matched:[/bold]      [{status_color}]{report.size_matched}[/{status_color}]\n"
        f"[bold]Scratch Cleaned:[/bold]   [green]{report.scratch_cleaned}[/green]\n"
        f"[bold]Result:[/bold]             [{status_color}][bold]{status_text}[/bold][/{status_color}]"
    )
    console.print(Panel(summary_text, title="Recovery Drill Report", border_style=status_color, box=box.ROUNDED))
    return 0 if report.passed else 1


async def handle_manifest(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)

    from televault.application.manifest import ManifestService

    service = ManifestService(
        repo=repo,
        manifest_repo=repo,
        vault_id="v_default",
        audit_ledger=repo,
    )

    if args.verify:
        is_valid, msg = service.verify_chain()
        style = "green" if is_valid else "bold red"
        console.print(Panel(
            f"[{style}]{msg}[/{style}]",
            title="Vault Manifest Chain Verification",
            border_style=style,
            box=box.ROUNDED,
        ))
        return 0 if is_valid else 1

    latest = service.get_latest()
    if not args.generate and latest:
        console.print(Panel(
            f"[bold]Generation:[/bold]       {latest.generation}\n"
            f"[bold]Vault ID:[/bold]         {latest.vault_id}\n"
            f"[bold]Total Files:[/bold]      {latest.total_files}\n"
            f"[bold]Total Bytes:[/bold]      {latest.total_bytes}\n"
            f"[bold]Manifest Hash:[/bold]    {latest.manifest_hash}\n"
            f"[bold]Previous Hash:[/bold]    {latest.previous_hash}\n"
            f"[bold]Timestamp:[/bold]        {latest.timestamp.isoformat()}",
            title="Latest Vault Manifest",
            border_style="cyan",
            box=box.ROUNDED,
        ))
        return 0

    m = service.generate_manifest()
    console.print(Panel(
        f"[green]New manifest generation {m.generation} generated successfully.[/green]\n\n"
        f"[bold]Total Files:[/bold]   {m.total_files}\n"
        f"[bold]Total Bytes:[/bold]   {m.total_bytes}\n"
        f"[bold]Manifest Hash:[/bold] {m.manifest_hash}\n"
        f"[bold]Previous Hash:[/bold] {m.previous_hash}",
        title="Manifest Generated",
        border_style="green",
        box=box.ROUNDED,
    ))
    return 0


async def handle_audit(args: argparse.Namespace) -> int:
    config = TeleVaultConfig()
    config.ensure_directories()
    repo = SQLiteVaultRepository(config.db_path)

    is_valid, msg = repo.verify_integrity()
    color = "green" if is_valid else "bold red"
    console.print(f"[{color}]Audit Ledger Integrity: {msg}[/{color}]\n")

    events = repo.list_events(limit=args.limit)
    if not events:
        console.print("[dim]No audit events recorded yet.[/dim]")
        return 0

    table = Table(title="TeleVault Tamper-Evident Audit Ledger", box=box.ROUNDED)
    table.add_column("Event ID", style="dim", max_width=12)
    table.add_column("Timestamp", style="cyan")
    table.add_column("Action", style="bold")
    table.add_column("Entity ID", max_width=14)
    table.add_column("Result")
    table.add_column("Details")

    for e in reversed(events):
        res_color = "green" if e.result == "SUCCESS" else "red"
        table.add_row(
            e.event_id[:8],
            e.timestamp.strftime("%Y-%m-%d %H:%M:%S"),
            e.action,
            e.entity_id or "-",
            f"[{res_color}]{e.result}[/{res_color}]",
            e.details or "-",
        )

    console.print(table)
    return 0


def handle_web(args: argparse.Namespace) -> int:
    import uvicorn
    url = f"http://{args.host}:{args.port}"
    console.print(Panel.fit(
        f"[bold cyan]TeleVault Web Server Running[/bold cyan]\n"
        f"[bold white]URL:[/bold white] [link={url}]{url}[/link]\n"
        f"[dim]Press Ctrl+C to terminate the web server.[/dim]",
        border_style="cyan",
        box=box.ASCII,
    ))
    if not args.no_browser:
        webbrowser.open(url)
    uvicorn.run("televault.presentation.web.app:app", host=args.host, port=args.port, reload=False)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="televault",
        description="TeleVault: Manual-only, append-only resilient desktop backup vault.",
    )
    subparsers = parser.add_subparsers(dest="command")

    # gui
    subparsers.add_parser("gui", help="Launch the native Windows 11 Desktop GUI Application")

    # login
    subparsers.add_parser("login", help="Log in to your Telegram account via MTProto")

    # logout
    subparsers.add_parser("logout", help="Clear Telegram credentials and session")

    # web
    p_web = subparsers.add_parser("web", help="Launch the TeleVault Web Dashboard")
    p_web.add_argument("--host", default="127.0.0.1", help="Host address to bind to")
    p_web.add_argument("--port", type=int, default=8000, help="Port to listen on")
    p_web.add_argument("--no-browser", action="store_true", help="Do not open browser automatically")

    # dashboard
    subparsers.add_parser("dashboard", help="Display rich colored TeleVault dashboard")

    # backup
    p_backup = subparsers.add_parser("backup", help="Back up a file or directory")
    p_backup.add_argument("path", help="Path of the file or folder to back up")
    p_backup.add_argument("--dry-run", action="store_true", help="Print actions without modifying state")
    p_backup.add_argument("--private", action="store_true", help="Encrypt with Private Mode (AES-256-GCM)")
    p_backup.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # restore
    p_restore = subparsers.add_parser("restore", help="Restore a file from the vault")
    p_restore.add_argument("id", help="Record ID of the file to restore")
    p_restore.add_argument("--dest", help="Destination directory to restore to")
    p_restore.add_argument("--dry-run", action="store_true", help="Simulate restore without downloading")
    p_restore.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # ls
    p_ls = subparsers.add_parser("ls", help="List all files in the vault")
    p_ls.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # verify
    p_verify = subparsers.add_parser("verify", help="Verify presence and health of vault files")
    p_verify.add_argument("--heal", action="store_true", help="Automatically heal degraded records")
    p_verify.add_argument("--dry-run", action="store_true", help="Report status without modifying state")
    p_verify.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # rebuild
    p_rebuild = subparsers.add_parser("rebuild", help="Reconstruct local index from Telegram channels")
    p_rebuild.add_argument("--dry-run", action="store_true", help="Scan channels without modifying local DB")
    p_rebuild.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # snapshot
    p_snapshot = subparsers.add_parser("snapshot", help="Export and upload an index snapshot to Primary vault")
    p_snapshot.add_argument("--dry-run", action="store_true", help="Export snapshot locally without uploading")
    p_snapshot.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # doctor
    p_doctor = subparsers.add_parser("doctor", help="Run diagnostic health scan and self-healing")
    p_doctor.add_argument("--repair", action="store_true", help="Automatically attempt safe repairs")
    p_doctor.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # recovery-test
    p_rectest = subparsers.add_parser("recovery-test", help="Execute non-destructive recovery drill")
    p_rectest.add_argument("--id", help="Optional specific FileRecord ID to drill")
    p_rectest.add_argument("--passphrase", help="Passphrase if drilling a Private Mode record")
    p_rectest.add_argument("--fake", action="store_true", help="Force in-memory fake gateway")

    # manifest
    p_manifest = subparsers.add_parser("manifest", help="Manage vault tamper-evident hash-chained manifests")
    p_manifest.add_argument("--generate", action="store_true", help="Generate a new manifest generation")
    p_manifest.add_argument("--verify", action="store_true", help="Verify cryptographic chain integrity")

    # audit
    p_audit = subparsers.add_parser("audit", help="Inspect tamper-evident append-only audit ledger")
    p_audit.add_argument("--limit", type=int, default=20, help="Number of recent events to display")

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if not args.command:
        sys.exit(asyncio.run(handle_dashboard(args)))

    if args.command == "gui":
        from televault.presentation.gui.app import run_gui
        sys.exit(run_gui())
    elif args.command == "login":
        sys.exit(asyncio.run(handle_login(args)))
    elif args.command == "logout":
        sys.exit(asyncio.run(handle_logout(args)))
    elif args.command == "web":
        sys.exit(handle_web(args))
    elif args.command == "dashboard":
        sys.exit(asyncio.run(handle_dashboard(args)))
    elif args.command == "backup":
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
    elif args.command == "doctor":
        sys.exit(asyncio.run(handle_doctor(args)))
    elif args.command == "recovery-test":
        sys.exit(asyncio.run(handle_recovery_test(args)))
    elif args.command == "manifest":
        sys.exit(asyncio.run(handle_manifest(args)))
    elif args.command == "audit":
        sys.exit(asyncio.run(handle_audit(args)))
    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
