from pathlib import Path
import pytest

from televault.ai.orchestrator import AIOrchestrator
from televault.ai.provider import LocalHeuristicProvider
from televault.domain.entities import FileRecord
from televault.domain.states import HealthState
from televault.infrastructure.storage.sqlite_repo import SQLiteVaultRepository
from tests.fakes.fake_gateway import FakeTelegramGateway


@pytest.fixture
def repo(tmp_path: Path) -> SQLiteVaultRepository:
    return SQLiteVaultRepository(tmp_path / "test_ai_vault.db")


@pytest.fixture
def gateway() -> FakeTelegramGateway:
    return FakeTelegramGateway()


@pytest.mark.asyncio
async def test_ai_provider_local_heuristics():
    provider = LocalHeuristicProvider()
    context = {
        "role": "doctor",
        "files": [{"name": "file1.pdf", "state": "HEALTHY", "size": 1024}],
        "readiness_score": 100,
    }
    resp = await provider.analyze("system", "what is my status?", context)
    assert "Vault Doctor Diagnosis" in resp
    assert "HEALTHY" in resp


@pytest.mark.asyncio
async def test_ai_orchestrator_initialization_and_10_agents(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway):
    orch = AIOrchestrator(repo=repo, gateway=gateway, manifest_repo=repo, audit_ledger=repo)
    assert len(orch.agents) == 10
    expected_agents = [
        "doctor", "redundancy", "recovery", "versioning", "forensics",
        "channels", "storage", "security", "onboarding", "copilot"
    ]
    for role in expected_agents:
        assert role in orch.agents


@pytest.mark.asyncio
async def test_ai_orchestrator_auto_routing(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway):
    orch = AIOrchestrator(repo=repo, gateway=gateway, manifest_repo=repo, audit_ledger=repo)

    r_doc = await orch.ask("My files are degraded and broken!")
    assert r_doc.agent_id == "vault_doctor"

    r_rec = await orch.ask("How ready is my recovery drill?")
    assert r_rec.agent_id == "recovery_drill"

    r_for = await orch.ask("Check the audit log for incidents")
    assert r_for.agent_id == "incident_forensic"

    r_sec = await orch.ask("Is my crypto password using Argon2id?")
    assert r_sec.agent_id == "security_reviewer"


@pytest.mark.asyncio
async def test_ai_action_proposals_and_approval_gate(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway):
    orch = AIOrchestrator(repo=repo, gateway=gateway, manifest_repo=repo, audit_ledger=repo)

    # Add a degraded file to the repo
    rec = FileRecord(name="report.docx", sha256="abc123", size=5000, state=HealthState.DEGRADED)
    repo.add(rec)

    # Doctor diagnoses degraded files and emits an ActionProposal
    resp = await orch.query_agent("doctor", "Please inspect vault health.")
    assert len(resp.proposals) > 0
    prop = resp.proposals[0]
    assert "Heal 1 Degraded File(s)" in prop.action_title
    assert prop.safe is True

    # Check pending proposals
    pending = orch.get_pending_proposals()
    assert len(pending) == 1
    assert pending[0].proposal_id == prop.proposal_id

    # User approval gate
    approved = orch.approve_proposal(prop.proposal_id)
    assert approved is not None
    assert len(orch.get_pending_proposals()) == 0


@pytest.mark.asyncio
async def test_full_system_multi_agent_audit(repo: SQLiteVaultRepository, gateway: FakeTelegramGateway):
    orch = AIOrchestrator(repo=repo, gateway=gateway, manifest_repo=repo, audit_ledger=repo)
    full_report = await orch.run_full_audit()
    assert len(full_report) == 10
    assert "doctor" in full_report
    assert "storage" in full_report
    assert "security" in full_report
