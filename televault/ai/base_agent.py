from dataclasses import dataclass, field
from typing import Any

from televault.domain.entities import ActionProposal
from televault.domain.ports import AIProvider
from televault.domain.states import AIPermission


@dataclass
class AgentResponse:
    agent_id: str
    agent_name: str
    message: str
    confidence: float = 1.0
    proposals: list[ActionProposal] = field(default_factory=list)


class BaseAgent:
    """Base class for all 10 TeleVault specialized advisory agents.
    Adheres strictly to Deterministic Core Supremacy:
    - Only reads authorized telemetry through domain ports.
    - Cannot execute state mutations directly.
    - Proposes actions via structured ActionProposal objects requiring user approval.
    """

    agent_id: str = "base"
    agent_name: str = "Base Agent"
    role: str = "general"
    permissions: list[AIPermission] = []
    system_prompt: str = ""

    def __init__(self, provider: AIProvider):
        self.provider = provider

    async def query(self, user_prompt: str, context: dict[str, Any]) -> AgentResponse:
        filtered_context = self._filter_context_by_permissions(context)
        filtered_context["role"] = self.role

        raw_response = await self.provider.analyze(
            system_prompt=self.system_prompt,
            user_prompt=user_prompt,
            context=filtered_context,
        )

        proposals = self.generate_proposals(filtered_context, raw_response)

        return AgentResponse(
            agent_id=self.agent_id,
            agent_name=self.agent_name,
            message=raw_response,
            confidence=1.0,
            proposals=proposals,
        )

    def generate_proposals(self, context: dict[str, Any], response_text: str) -> list[ActionProposal]:
        """Override in subclasses to emit confirmable action proposals."""
        return []

    def _filter_context_by_permissions(self, context: dict[str, Any]) -> dict[str, Any]:
        """Strip telemetry that this agent is not explicitly permitted to inspect."""
        filtered = {}
        for k, v in context.items():
            if k in ("files", "total_files") and AIPermission.READ_FILE_METADATA in self.permissions:
                filtered[k] = v
            elif k in ("states", "readiness_score") and AIPermission.READ_HEALTH in self.permissions:
                filtered[k] = v
            elif k == "channels" and AIPermission.READ_STATUS in self.permissions:
                filtered[k] = v
            elif k == "audit_events" and AIPermission.READ_AUDIT in self.permissions:
                filtered[k] = v
            elif k == "transactions" and AIPermission.READ_TRANSACTION_STATE in self.permissions:
                filtered[k] = v
            elif k in ("readiness_score", "role"):
                filtered[k] = v
        return filtered
