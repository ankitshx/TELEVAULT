from contextlib import asynccontextmanager
import logging
import os
from pathlib import Path
import shutil
import tempfile
from typing import Any

from fastapi import FastAPI, File, Form, HTTPException, UploadFile
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel

from televault.application.backup import BackupFileUseCase
from televault.application.event_bus import SimpleEventBus
from televault.application.heal import HealVaultUseCase
from televault.application.rebuild import RebuildIndexUseCase
from televault.application.restore import RestoreFileUseCase
from televault.application.snapshot import SnapshotUseCase
from televault.application.verify import VerifyVaultUseCase
from televault.domain.ports import TelegramGateway
from televault.domain.states import HealthState, VaultMode
from televault.infrastructure.os.config import TeleVaultConfig
from televault.infrastructure.storage.recovery_cache import RecoveryCache
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from televault.infrastructure.telegram.auth_service import TelegramAuthService
from tests.fakes.fake_gateway import FakeTelegramGateway

logger = logging.getLogger("televault.web")

# Web assets directory
WEB_DIR = Path(__file__).resolve().parent
STATIC_DIR = WEB_DIR / "static"

config = TeleVaultConfig()
config.ensure_directories()
repo = SQLiteVaultRepository(config.db_path)
auth_service = TelegramAuthService(config)
recovery_cache = RecoveryCache(config.cache_dir)
event_bus = SimpleEventBus()

# Shared live gateway or fallback fake gateway
active_gateway: TelegramGateway | None = None
recent_logs: list[dict[str, str]] = []


def add_log(category: str, message: str, level: str = "info") -> None:
    recent_logs.append({
        "timestamp": os.environ.get("LOCAL_TIME", ""),
        "category": category,
        "message": message,
        "level": level,
    })
    if len(recent_logs) > 100:
        recent_logs.pop(0)


async def resolve_gateway(force_fake: bool = False) -> TelegramGateway:
    global active_gateway
    if force_fake:
        return FakeTelegramGateway()

    if active_gateway is not None:
        return active_gateway

    live = await auth_service.get_live_gateway()
    if live:
        active_gateway = live
        return live

    # Real project enforcement: real Telegram account is required
    raise HTTPException(
        status_code=400,
        detail="Telegram MTProto account not connected! Please connect your Telegram account in Settings to perform live cloud vault operations.",
    )


@asynccontextmanager
async def lifespan(app: FastAPI):
    config.ensure_directories()
    add_log("System", "TeleVault Web Application started.", "success")
    yield
    add_log("System", "TeleVault Web Application shutting down.", "info")


app = FastAPI(title="TeleVault Web App", version="1.0.0", lifespan=lifespan)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


# Pydantic Request Models
class SendCodeRequest(BaseModel):
    api_id: str
    api_hash: str
    phone: str


class VerifyCodeRequest(BaseModel):
    code: str
    phone_code_hash: str | None = None
    password: str | None = None


class PathBackupRequest(BaseModel):
    path: str
    mode: str = "original"
    passphrase: str | None = None
    force_fake: bool = False


# Endpoints
@app.get("/")
async def get_index():
    index_path = STATIC_DIR / "index.html"
    if not index_path.exists():
        raise HTTPException(status_code=404, detail="Index file not found")
    return FileResponse(index_path)


@app.get("/api/status")
async def get_status():
    is_auth = await auth_service.is_authorized()
    user_info = await auth_service.get_current_user() if is_auth else None
    creds = auth_service.get_stored_credentials()

    records = repo.list_all()
    total_size = sum(r.size for r in records)
    healthy_cnt = sum(1 for r in records if r.state == HealthState.HEALTHY)
    degraded_cnt = sum(1 for r in records if r.state == HealthState.DEGRADED)
    lost_cnt = sum(1 for r in records if r.state == HealthState.LOST)

    mode_label = "Live MTProto" if is_auth else "Disconnected (Setup Required)"

    return {
        "authenticated": is_auth,
        "mode": mode_label,
        "user": user_info,
        "channels": {
            "primary_id": creds.get("primary_channel_id"),
            "mirror_id": creds.get("mirror_channel_id"),
        },
        "stats": {
            "total_files": len(records),
            "total_bytes": total_size,
            "healthy": healthy_cnt,
            "degraded": degraded_cnt,
            "lost": lost_cnt,
        },
        "logs": recent_logs[-25:],
    }


def categorize_file(name: str) -> tuple[str, str]:
    ext = Path(name).suffix.lower()
    if ext in [".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".bmp", ".ico", ".tiff"]:
        return "media", ext
    if ext in [".mp4", ".mkv", ".mov", ".avi", ".webm"]:
        return "video", ext
    if ext in [".mp3", ".wav", ".ogg", ".flac", ".m4a", ".aac"]:
        return "audio", ext
    if ext in [".pdf", ".docx", ".doc", ".xlsx", ".xls", ".pptx", ".txt", ".rtf", ".odt", ".csv"]:
        return "document", ext
    if ext in [".zip", ".tar", ".gz", ".7z", ".rar", ".bz2", ".xz", ".iso"]:
        return "archive", ext
    if ext in [".py", ".js", ".ts", ".html", ".css", ".json", ".rs", ".go", ".c", ".cpp", ".sql", ".sh", ".bat", ".md", ".yaml", ".yml", ".toml"]:
        return "code", ext
    return "other", ext


@app.get("/api/records")
async def get_records():
    records = repo.list_all()
    out = []
    for r in records:
        cat, ext = categorize_file(r.name)
        out.append({
            "id": r.id,
            "name": r.name,
            "size": r.size,
            "sha256": r.sha256,
            "state": r.state.value,
            "local_status": r.local_status.value,
            "mode": r.mode.value,
            "version": r.version,
            "category": cat,
            "ext": ext,
            "primary_msg_id": r.primary_ref.message_id if r.primary_ref else None,
            "mirror_msg_id": r.mirror_ref.message_id if r.mirror_ref else None,
            "is_multipart": r.is_multipart,
            "parts_count": len(r.parts),
            "created_at": r.created_at.isoformat() if hasattr(r.created_at, "isoformat") else str(r.created_at),
        })
    return {"records": out}


@app.post("/api/login/send-code")
async def login_send_code(req: SendCodeRequest):
    try:
        res = await auth_service.send_login_code(
            api_id=req.api_id.strip(),
            api_hash=req.api_hash.strip(),
            phone=req.phone.strip(),
        )
        add_log("Telegram", f"Login code sent to {req.phone}", "info")
        return res
    except Exception as e:
        add_log("Telegram", f"Failed to send code: {e}", "error")
        raise HTTPException(status_code=400, detail=str(e))


@app.post("/api/login/verify-code")
async def login_verify_code(req: VerifyCodeRequest):
    global active_gateway
    try:
        res = await auth_service.complete_login(
            code=req.code.strip(),
            phone_code_hash=req.phone_code_hash,
            password=req.password,
        )
        if res.get("status") == "success":
            active_gateway = None  # Reload gateway on next call
            add_log("Telegram", f"Successfully authenticated as {res['user'].get('first_name')}", "success")
        elif res.get("status") == "2fa_required":
            add_log("Telegram", "Two-step verification password required.", "warning")
        return res
    except Exception as e:
        add_log("Telegram", f"Authentication failed: {e}", "error")
        raise HTTPException(status_code=400, detail=str(e))


@app.post("/api/logout")
async def logout():
    global active_gateway
    auth_service.clear_credentials()
    active_gateway = None
    add_log("Telegram", "User logged out. MTProto session cleared.", "info")
    return {"status": "logged_out"}


@app.post("/api/backup/upload")
async def backup_uploaded_file(
    file: UploadFile = File(...),
    mode: str = Form("original"),
    passphrase: str = Form(None),
    force_fake: bool = Form(False),
):
    """Save browser-uploaded file to temp folder and run BackupFileUseCase."""
    gateway = await resolve_gateway(force_fake=force_fake)
    use_case = BackupFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)

    temp_dir = Path(tempfile.mkdtemp(prefix="televault_upload_"))
    temp_file = temp_dir / file.filename

    try:
        with open(temp_file, "wb") as buffer:
            shutil.copyfileobj(file.file, buffer)

        vault_mode = VaultMode.PRIVATE if mode.lower() == "private" else VaultMode.ORIGINAL
        result = await use_case.execute(file_path=temp_file, mode=vault_mode)

        if result.is_duplicate:
            add_log("Backup", f"File already backed up: {file.filename}", "warning")
            return {"status": "duplicate", "message": result.message}

        add_log("Backup", f"Successfully backed up {file.filename} (ID: {result.record.id})", "success")
        return {
            "status": "success",
            "record_id": result.record.id,
            "name": result.record.name,
            "size": result.record.size,
            "state": result.record.state.value,
            "mode": result.record.mode.value,
        }
    except Exception as e:
        add_log("Backup", f"Backup failed for {file.filename}: {e}", "error")
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        if temp_dir.exists():
            shutil.rmtree(temp_dir, ignore_errors=True)


@app.post("/api/backup/path")
async def backup_local_path(req: PathBackupRequest):
    """Back up a file located on the local machine directly."""
    target_path = Path(req.path)
    if not target_path.exists():
        raise HTTPException(status_code=400, detail=f"Path not found: {req.path}")

    gateway = await resolve_gateway(force_fake=req.force_fake)
    use_case = BackupFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)

    files = [target_path] if target_path.is_file() else [f for f in target_path.rglob("*") if f.is_file()]
    if not files:
        raise HTTPException(status_code=400, detail="No files found to back up.")

    results = []
    vault_mode = VaultMode.PRIVATE if req.mode.lower() == "private" else VaultMode.ORIGINAL

    for f in files:
        res = await use_case.execute(file_path=f, mode=vault_mode)
        results.append({
            "name": f.name,
            "is_duplicate": res.is_duplicate,
            "message": res.message,
            "record_id": res.record.id if res.record else None,
        })
        add_log("Backup", f"Backed up: {f.name} -> {res.message}", "info")

    return {"status": "complete", "results": results}


@app.post("/api/restore/{record_id}")
async def restore_file(record_id: str, dest_folder: str | None = None, force_fake: bool = False):
    gateway = await resolve_gateway(force_fake=force_fake)
    use_case = RestoreFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)

    dest_dir = Path(dest_folder) if dest_folder else (config.app_dir / "restored")
    dest_dir.mkdir(parents=True, exist_ok=True)

    try:
        result = await use_case.execute(record_id=record_id, dest_dir=dest_dir)
        if result.success:
            add_log("Restore", f"Restored {record_id} to {result.restored_path}", "success")
            return {
                "status": "success",
                "restored_path": str(result.restored_path),
                "sha256_matched": result.sha256_matched,
            }
        else:
            add_log("Restore", f"Failed restoring {record_id}: {result.message}", "error")
            raise HTTPException(status_code=400, detail=result.message)
    except KeyError:
        raise HTTPException(status_code=404, detail=f"Record {record_id} not found in index.")


@app.get("/api/files/{record_id}/download")
async def download_file_endpoint(record_id: str, force_fake: bool = False):
    """Direct stream download for Telegram Drive files."""
    try:
        record = repo.get(record_id)
    except KeyError:
        raise HTTPException(status_code=404, detail="File record not found.")

    # 1. Check recovery cache
    cached = recovery_cache.get(record.sha256)
    if cached and cached.is_file():
        return FileResponse(
            path=cached,
            filename=record.name,
            media_type="application/octet-stream",
            content_disposition_type="attachment",
        )

    # 2. Check original path if matches hash
    if record.original_path and Path(record.original_path).is_file():
        from televault.application.hashing import calculate_sha256
        h, _ = calculate_sha256(Path(record.original_path))
        if h.lower() == record.sha256.lower():
            return FileResponse(
                path=Path(record.original_path),
                filename=record.name,
                media_type="application/octet-stream",
                content_disposition_type="attachment",
            )

    # 3. Restore from Telegram
    gateway = await resolve_gateway(force_fake=force_fake)
    use_case = RestoreFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)
    dest_dir = config.app_dir / "downloads"
    dest_dir.mkdir(parents=True, exist_ok=True)
    res = await use_case.execute(record_id=record.id, dest_dir=dest_dir)
    if not res.success or not res.restored_path:
        raise HTTPException(status_code=400, detail=res.message)

    return FileResponse(
        path=res.restored_path,
        filename=record.name,
        media_type="application/octet-stream",
        content_disposition_type="attachment",
    )


@app.get("/api/files/{record_id}/preview")
async def preview_file_endpoint(record_id: str, force_fake: bool = False):
    """Inline preview endpoint for images, audio, documents, and code."""
    import mimetypes
    try:
        record = repo.get(record_id)
    except KeyError:
        raise HTTPException(status_code=404, detail="File record not found.")

    file_path = None
    cached = recovery_cache.get(record.sha256)
    if cached and cached.is_file():
        file_path = cached
    elif record.original_path and Path(record.original_path).is_file():
        from televault.application.hashing import calculate_sha256
        h, _ = calculate_sha256(Path(record.original_path))
        if h.lower() == record.sha256.lower():
            file_path = Path(record.original_path)

    if not file_path:
        gateway = await resolve_gateway(force_fake=force_fake)
        use_case = RestoreFileUseCase(gateway=gateway, repo=repo, event_bus=event_bus)
        preview_dir = config.app_dir / "preview_cache"
        preview_dir.mkdir(parents=True, exist_ok=True)
        res = await use_case.execute(record_id=record.id, dest_dir=preview_dir)
        if not res.success or not res.restored_path:
            raise HTTPException(status_code=400, detail=res.message)
        file_path = res.restored_path

    mime, _ = mimetypes.guess_type(record.name)
    mime = mime or "application/octet-stream"

    return FileResponse(
        path=file_path,
        media_type=mime,
        content_disposition_type="inline",
    )


@app.post("/api/verify")
async def verify_vault(heal: bool = False, force_fake: bool = False):
    gateway = await resolve_gateway(force_fake=force_fake)
    verify_uc = VerifyVaultUseCase(gateway=gateway, repo=repo, recovery_cache=recovery_cache)
    summary = await verify_uc.execute()

    heal_summary = None
    if heal and summary.degraded_count > 0:
        heal_uc = HealVaultUseCase(gateway=gateway, repo=repo, recovery_cache=recovery_cache)
        heal_summary = await heal_uc.execute()

    add_log(
        "Verify",
        f"Verified {summary.total_checked} items: {summary.healthy_count} healthy, {summary.degraded_count} degraded, {summary.lost_count} lost",
        "info",
    )
    return {
        "status": "complete",
        "total_checked": summary.total_checked,
        "healthy_count": summary.healthy_count,
        "degraded_count": summary.degraded_count,
        "lost_count": summary.lost_count,
        "results": [
            {
                "record_id": r.record_id,
                "name": r.name,
                "old_state": r.old_state.value,
                "new_state": r.new_state.value,
                "diagnosis": r.diagnosis,
            }
            for r in summary.results
        ],
        "healing": {
            "healed_count": heal_summary.healed_count,
            "failed_count": heal_summary.failed_count,
        } if heal_summary else None,
    }


@app.post("/api/rebuild")
async def rebuild_index(force_fake: bool = False):
    gateway = await resolve_gateway(force_fake=force_fake)
    rebuild_uc = RebuildIndexUseCase(gateway=gateway, repo=repo)
    summary = await rebuild_uc.execute()
    add_log("Rebuild", f"Reconstructed {summary.records_reconstructed} records from Telegram.", "success")
    return {
        "status": "complete",
        "records_reconstructed": summary.records_reconstructed,
        "records_healthy": summary.records_healthy,
        "records_degraded": summary.records_degraded,
    }


@app.post("/api/snapshot")
async def create_snapshot(force_fake: bool = False):
    gateway = await resolve_gateway(force_fake=force_fake)
    snapshot_uc = SnapshotUseCase(gateway=gateway, repo=repo, temp_dir=config.cache_dir)
    dest = await snapshot_uc.execute()
    add_log("Snapshot", f"Database snapshot exported to {dest.name}", "info")
    return {"status": "complete", "snapshot_file": dest.name}


# -------------------------------------------------------------
# Doctor, Recovery Drill, Manifest, Audit, and AI Endpoints
# -------------------------------------------------------------
from televault.ai.orchestrator import AIOrchestrator
from televault.application.doctor import VaultDoctorUseCase
from televault.application.manifest import ManifestService
from televault.application.recovery_drill import RecoveryDrillUseCase


class AIQueryRequest(BaseModel):
    role: str = "doctor"
    prompt: str = ""


class ProposalActionRequest(BaseModel):
    proposal_id: str


@app.get("/api/doctor")
async def get_doctor_diagnostics(repair: bool = False, force_fake: bool = False):
    gateway = await resolve_gateway(force_fake=force_fake)
    doctor = VaultDoctorUseCase(gateway, repo, repo, repo, repo)
    findings = await doctor.diagnose()
    repairs = []
    if repair:
        repairs = await doctor.repair()
        add_log("Doctor", f"Repaired {len(repairs)} items automatically.", "success")
    return {
        "status": "complete",
        "findings": [
            {
                "category": f.category,
                "title": f.title,
                "state": f.state.value,
                "what_happened": f.what_happened,
                "why": f.why,
                "what_is_safe": f.what_is_safe,
                "recommended_action": f.recommended_action,
                "action_type": f.action_type,
            }
            for f in findings
        ],
        "repairs": repairs,
    }


@app.post("/api/recovery-drill")
async def run_recovery_drill(record_id: str | None = None, passphrase: str | None = None, force_fake: bool = False):
    gateway = await resolve_gateway(force_fake=force_fake)
    drill = RecoveryDrillUseCase(gateway, repo, config.recovery_dir, repo)
    try:
        report = await drill.execute(record_id=record_id, passphrase=passphrase)
        add_log("RecoveryDrill", f"Recovery drill completed: {report.message}", "success" if report.passed else "error")
        return {
            "status": "complete",
            "passed": report.passed,
            "record_id": report.record_id,
            "file_name": report.file_name,
            "file_size": report.file_size,
            "duration_seconds": report.duration_seconds,
            "sha256_matched": report.sha256_matched,
            "size_matched": report.size_matched,
            "scratch_cleaned": report.scratch_cleaned,
            "message": report.message,
        }
    except Exception as e:
        add_log("RecoveryDrill", f"Drill failed: {e}", "error")
        raise HTTPException(status_code=400, detail=str(e))


@app.get("/api/manifest")
async def get_manifest():
    service = ManifestService(repo, repo, vault_id="v_web", audit_ledger=repo)
    latest = service.get_latest()
    is_valid, msg = service.verify_chain()
    return {
        "is_valid": is_valid,
        "verification_message": msg,
        "latest": {
            "generation": latest.generation,
            "total_files": latest.total_files,
            "total_bytes": latest.total_bytes,
            "manifest_hash": latest.manifest_hash,
            "previous_hash": latest.previous_hash,
            "created_at": latest.created_at.isoformat(),
        } if latest else None,
    }


@app.post("/api/manifest/generate")
async def generate_manifest():
    service = ManifestService(repo, repo, vault_id="v_web", audit_ledger=repo)
    m = service.generate_manifest()
    add_log("Manifest", f"New manifest generation {m.generation} generated.", "success")
    return {
        "status": "complete",
        "generation": m.generation,
        "total_files": m.total_files,
        "total_bytes": m.total_bytes,
        "manifest_hash": m.manifest_hash,
    }


@app.get("/api/audit")
async def get_audit_ledger(limit: int = 50):
    events = repo.list_events(limit=limit)
    is_valid, msg = repo.verify_integrity()
    return {
        "is_valid": is_valid,
        "integrity_message": msg,
        "events": [
            {
                "event_id": e.event_id,
                "timestamp": e.timestamp.isoformat(),
                "action": e.action,
                "entity_id": e.entity_id,
                "result": e.result,
                "details": e.details,
            }
            for e in events
        ],
    }


# Singleton AI Orchestrator instance for web sessions
web_orchestrator: AIOrchestrator | None = None


def get_web_orchestrator() -> AIOrchestrator:
    global web_orchestrator
    if web_orchestrator is None:
        web_orchestrator = AIOrchestrator(
            repo=repo,
            gateway=FakeTelegramGateway(),
            manifest_repo=repo,
            audit_ledger=repo,
        )
    return web_orchestrator


@app.post("/api/ai/query")
async def query_ai(req: AIQueryRequest):
    orch = get_web_orchestrator()
    resp = await orch.query_agent(req.role, req.prompt)
    return {
        "agent_id": resp.agent_id,
        "agent_name": resp.agent_name,
        "message": resp.message,
        "confidence": resp.confidence,
        "proposals": [
            {
                "proposal_id": p.proposal_id,
                "agent_name": p.agent_name,
                "action_title": p.action_title,
                "why": p.why,
                "what_will_change": p.what_will_change,
                "what_will_not_change": p.what_will_not_change,
                "safe": p.safe,
                "reversible": p.reversible,
            }
            for p in resp.proposals
        ],
    }


@app.post("/api/ai/proposals/approve")
async def approve_proposal(req: ProposalActionRequest):
    orch = get_web_orchestrator()
    prop = orch.approve_proposal(req.proposal_id)
    if not prop:
        raise HTTPException(status_code=404, detail="Proposal not found or already executed.")
    add_log("AI_Approval", f"Approved proposal: {prop.action_title}", "info")
    return {"status": "approved", "action_title": prop.action_title}


@app.post("/api/ai/proposals/reject")
async def reject_proposal(req: ProposalActionRequest):
    orch = get_web_orchestrator()
    orch.reject_proposal(req.proposal_id)
    return {"status": "rejected"}


@app.get("/api/ai/agents")
async def get_ai_agents():
    orch = get_web_orchestrator()
    agents_list = []
    for key, ag in orch.agents.items():
        agents_list.append({
            "key": key,
            "id": ag.agent_id,
            "name": ag.agent_name,
            "role": ag.role,
        })
    return {"agents": agents_list}


@app.get("/api/versions")
async def get_versions():
    records = repo.list_all()
    grouped: dict[str, list[dict]] = {}
    for r in records:
        entry = {
            "id": r.id,
            "name": r.name,
            "version": r.version,
            "size": r.size,
            "sha256": r.sha256,
            "state": r.state.value,
            "mode": r.mode.value,
            "local_status": r.local_status.value,
            "created_at": r.created_at.isoformat() if hasattr(r.created_at, "isoformat") else str(r.created_at),
        }
        grouped.setdefault(r.name, []).append(entry)

    # Sort versions descending for each file
    for name, vlist in grouped.items():
        vlist.sort(key=lambda x: x["version"], reverse=True)

    return {"versions": grouped}


# Mount static assets
if STATIC_DIR.exists():
    app.mount("/static", StaticFiles(directory=str(STATIC_DIR)), name="static")

