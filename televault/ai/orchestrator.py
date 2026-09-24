import logging
from typing import Any

from televault.ai.agents import (
    ChannelHealthAgent,
    DisasterRecoveryCopilotAgent,
    IncidentForensicAgent,
    OnboardingCoachAgent,
    RecoveryDrillAgent,
    RedundancyAuditorAgent,
    RetentionVersioningAgent,
    SecurityReviewerAgent,
    StorageOptimizerAgent,
    VaultDoctorAgent,
)
from televault.ai.base_agent import AgentResponse, BaseAgent
from televault.ai.provider import LocalHeuristicProvider
from televault.domain.entities import ActionProposal
from televault.domain.ports import AIProvider, AuditLedger, ManifestRepository, TelegramGateway, VaultRepository

logger = logging.getLogger("televault.ai.orchestrator")


class AIOrchestrator:
    """Orchestrator for TeleVault's 10 Specialized Advisory Agents.
    Enforces Deterministic Core Supremacy:
    - Queries gather read-only domain context from ports.
    - Routes requests to specialized agent personas.
    - Holds all state-changing recommendations in ActionProposals requiring human confirmation.
    """

    def __init__(
        self,
        repo: VaultRepository,
        gateway: TelegramGateway,
        manifest_repo: ManifestRepository | None = None,
        audit_ledger: AuditLedger | None = None,
        provider: AIProvider | None = None,
    ):
        self.repo = repo
        self.gateway = gateway
        self.manifest_repo = manifest_repo
        self.audit_ledger = audit_ledger
        self.provider = provider or LocalHeuristicProvider()

        # Initialize the 10 Specialized Agents
        self.agents: dict[str, BaseAgent] = {
            "doctor": VaultDoctorAgent(self.provider),
            "redundancy": RedundancyAuditorAgent(self.provider),
            "recovery": RecoveryDrillAgent(self.provider),
            "versioning": RetentionVersioningAgent(self.provider),
            "forensics": IncidentForensicAgent(self.provider),
            "channels": ChannelHealthAgent(self.provider),
            "storage": StorageOptimizerAgent(self.provider),
            "security": SecurityReviewerAgent(self.provider),
            "onboarding": OnboardingCoachAgent(self.provider),
            "copilot": DisasterRecoveryCopilotAgent(self.provider),
        }

        # Pending proposals awaiting user confirmation
        self._proposals: dict[str, ActionProposal] = {}

    def get_agent(self, agent_id_or_role: str) -> BaseAgent:
        if agent_id_or_role in self.agents:
            return self.agents[agent_id_or_role]
        for agent in self.agents.values():
            if agent.agent_id == agent_id_or_role:
                return agent
        return self.agents["doctor"]

    async def build_context(self) -> dict[str, Any]:
        """Collect read-only sanitized telemetry from domain ports."""
        records = self.repo.list_all()
        files_data = [
            {
                "id": r.id,
                "name": r.name,
                "size": r.size,
                "state": r.state.value,
                "mode": r.mode.value,
                "version": r.version,
            }
            for r in records
        ]

        total_files = len(records)
        healthy = sum(1 for r in records if r.state.value == "HEALTHY")
        degraded = sum(1 for r in records if r.state.value == "DEGRADED")
        lost = sum(1 for r in records if r.state.value == "LOST")

        readiness_score = 100
        if total_files > 0:
            readiness_score = int((healthy / total_files) * 100)
            if lost > 0:
                readiness_score = max(0, readiness_score - 30)

        channels_info = {}
        try:
            vaults = await self.gateway.ensure_vaults()
            channels_info = {
                "primary_id": vaults.primary_id,
                "mirror_id": vaults.mirror_id,
            }
        except Exception:
            pass

        audit_events = []
        if self.audit_ledger:
            try:
                events = self.audit_ledger.list_events(limit=10)
                audit_events = [
                    {
                        "id": e.event_id,
                        "action": e.action,
                        "result": e.result,
                        "time": e.timestamp.isoformat(),
                    }
                    for e in events
                ]
            except Exception:
                pass

        return {
            "files": files_data,
            "total_files": total_files,
            "healthy": healthy,
            "degraded": degraded,
            "lost": lost,
            "readiness_score": readiness_score,
            "channels": channels_info,
            "audit_events": audit_events,
        }

    async def query_agent(self, role_or_id: str, prompt: str) -> AgentResponse:
        """Route a targeted query to a specialized agent."""
        agent = self.get_agent(role_or_id)
        context = await self.build_context()
        response = await agent.query(user_prompt=prompt, context=context)

        for prop in response.proposals:
            self._proposals[prop.proposal_id] = prop

        return response

    async def ask(self, prompt: str) -> AgentResponse:
        """Auto-route a natural language prompt to the most relevant agent."""
        p_lower = prompt.lower()
        if any(w in p_lower for w in ("broken", "degraded", "doctor", "health", "fix", "heal")):
            return await self.query_agent("doctor", prompt)
        elif any(w in p_lower for w in ("redundancy", "mirror", "spof", "channel")):
            return await self.query_agent("redundancy", prompt)
        elif any(w in p_lower for w in ("recovery", "drill", "mttr", "test", "restore")):
            return await self.query_agent("recovery", prompt)
        elif any(w in p_lower for w in ("version", "history", "tree", "retention")):
            return await self.query_agent("versioning", prompt)
        elif any(w in p_lower for w in ("forensic", "audit", "tamper", "log", "incident")):
            return await self.query_agent("forensics", prompt)
        elif any(w in p_lower for w in ("storage", "size", "bytes", "dedup", "compress")):
            return await self.query_agent("storage", prompt)
        elif any(w in p_lower for w in ("security", "crypto", "argon", "cipher", "password")):
            return await self.query_agent("security", prompt)
        elif any(w in p_lower for w in ("disaster", "rebuild", "wipe", "lost")):
            return await self.query_agent("copilot", prompt)
        elif any(w in p_lower for w in ("how", "start", "setup", "rules", "invariants")):
            return await self.query_agent("onboarding", prompt)
        else:
            return await self.query_agent("doctor", prompt)

    async def run_full_audit(self) -> dict[str, AgentResponse]:
        """Execute comprehensive audit across all 10 specialized agents."""
        context = await self.build_context()
        results = {}
        for role, agent in self.agents.items():
            resp = await agent.query(user_prompt="Provide status review.", context=context)
            results[role] = resp
            for prop in resp.proposals:
                self._proposals[prop.proposal_id] = prop
        return results

    def get_pending_proposals(self) -> list[ActionProposal]:
        return list(self._proposals.values())

    def approve_proposal(self, proposal_id: str) -> ActionProposal | None:
        return self._proposals.pop(proposal_id, None)

    def reject_proposal(self, proposal_id: str) -> None:
        self._proposals.pop(proposal_id, None)
