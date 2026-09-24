import uuid
from typing import Any

from televault.ai.base_agent import BaseAgent
from televault.domain.entities import ActionProposal
from televault.domain.states import AIPermission


class VaultDoctorAgent(BaseAgent):
    agent_id = "vault_doctor"
    agent_name = "Vault Doctor Agent"
    role = "doctor"
    permissions = [
        AIPermission.READ_HEALTH,
        AIPermission.READ_FILE_METADATA,
        AIPermission.READ_STATUS,
    ]
    system_prompt = (
        "You are the Vault Doctor Agent. Diagnose vault health anomalies, explain "
        "root causes with the 4-part model (What happened, Why, What is safe, Recommended action), "
        "and propose safe self-healing actions."
    )

    def generate_proposals(self, context: dict[str, Any], response_text: str) -> list[ActionProposal]:
        files = context.get("files", [])
        degraded = [f for f in files if f.get("state") == "DEGRADED"]
        if degraded:
            return [
                ActionProposal(
                    proposal_id=str(uuid.uuid4()),
                    agent_name=self.agent_name,
                    action_title=f"Heal {len(degraded)} Degraded File(s)",
                    why="Surviving cloud copies exist in Telegram. Self-healing forwards them to restore dual-write redundancy.",
                    what_will_change=f"Restore dual redundancy for {', '.join(f.get('name', '') for f in degraded[:3])}.",
                    what_will_not_change="Existing healthy files and local disk files remain completely untouched.",
                    reversible=True,
                    safe=True,
                )
            ]
        return []


class RedundancyAuditorAgent(BaseAgent):
    agent_id = "redundancy_auditor"
    agent_name = "Redundancy Auditor Agent"
    role = "redundancy"
    permissions = [AIPermission.READ_STATUS, AIPermission.READ_HEALTH]
    system_prompt = "You are the Redundancy Auditor Agent. Evaluate channel isolation and single-point-of-failure risks."


class RecoveryDrillAgent(BaseAgent):
    agent_id = "recovery_drill"
    agent_name = "Recovery Drill Assistant"
    role = "recovery"
    permissions = [AIPermission.READ_HEALTH, AIPermission.READ_FILE_METADATA, AIPermission.READ_STATUS]
    system_prompt = "You are the Recovery Drill Assistant. Evaluate recovery readiness, simulate MTTR, and suggest non-destructive drills."

    def generate_proposals(self, context: dict[str, Any], response_text: str) -> list[ActionProposal]:
        return [
            ActionProposal(
                proposal_id=str(uuid.uuid4()),
                agent_name=self.agent_name,
                action_title="Run Non-Destructive Recovery Drill",
                why="Periodically verifies that Telegram cloud documents can be downloaded and byte-verified.",
                what_will_change="Downloads a sample file to isolated scratch space, computes SHA-256, then deletes the scratch file.",
                what_will_not_change="Zero changes to Telegram channels, local originals, or vault index.",
                reversible=True,
                safe=True,
            )
        ]


class RetentionVersioningAgent(BaseAgent):
    agent_id = "retention_versioning"
    agent_name = "Retention & Versioning Strategist"
    role = "versioning"
    permissions = [AIPermission.READ_FILE_METADATA, AIPermission.READ_VERSION_HISTORY]
    system_prompt = "You are the Retention & Versioning Strategist. Analyze historical generations, version trees, and retention safety."


class IncidentForensicAgent(BaseAgent):
    agent_id = "incident_forensic"
    agent_name = "Incident Forensic Analyst"
    role = "forensics"
    permissions = [AIPermission.READ_AUDIT, AIPermission.READ_ACTIVITY]
    system_prompt = "You are the Incident Forensic Analyst. Trace audit ledger event chains and detect state discrepancies."


class ChannelHealthAgent(BaseAgent):
    agent_id = "channel_health"
    agent_name = "Telegram Channel Health Inspector"
    role = "channels"
    permissions = [AIPermission.READ_STATUS, AIPermission.READ_HEALTH]
    system_prompt = "You are the Telegram Channel Health Inspector. Monitor MTProto FloodWait states, rate limits, and channel metadata."


class StorageOptimizerAgent(BaseAgent):
    agent_id = "storage_optimizer"
    agent_name = "Storage Optimization Agent"
    role = "storage"
    permissions = [AIPermission.READ_FILE_METADATA]
    system_prompt = "You are the Storage Optimization Agent. Calculate deduplication savings and bandwidth metrics."


class SecurityReviewerAgent(BaseAgent):
    agent_id = "security_reviewer"
    agent_name = "Security & Cryptographic Reviewer"
    role = "security"
    permissions = [AIPermission.READ_STATUS, AIPermission.READ_FILE_METADATA]
    system_prompt = "You are the Security Reviewer. Evaluate Argon2id configuration, AES-GCM coverage, and session isolation."


class OnboardingCoachAgent(BaseAgent):
    agent_id = "onboarding_coach"
    agent_name = "Onboarding & Readiness Coach"
    role = "onboarding"
    permissions = [AIPermission.READ_STATUS]
    system_prompt = "You are the Onboarding Coach. Guide the user through initial Telegram channel creation and invariants."


class DisasterRecoveryCopilotAgent(BaseAgent):
    agent_id = "disaster_recovery"
    agent_name = "Disaster Recovery Co-Pilot"
    role = "copilot"
    permissions = [AIPermission.READ_STATUS, AIPermission.READ_HEALTH, AIPermission.READ_FILE_METADATA]
    system_prompt = "You are the Disaster Recovery Co-Pilot. Coordinate bare-metal vault reconstruction from Telegram captions."

    def generate_proposals(self, context: dict[str, Any], response_text: str) -> list[ActionProposal]:
        files = context.get("files", [])
        if not files:
            return [
                ActionProposal(
                    proposal_id=str(uuid.uuid4()),
                    agent_name=self.agent_name,
                    action_title="Initiate Disaster Recovery Rebuild",
                    why="Local database is empty. Cloud captions (tv1/tv2) can rebuild the entire vault index.",
                    what_will_change="Scans Telegram Primary and Mirror channels and recreates local SQLite records.",
                    what_will_not_change="Telegram cloud messages are strictly read-only; no deletions will occur.",
                    reversible=True,
                    safe=True,
                )
            ]
        return []
