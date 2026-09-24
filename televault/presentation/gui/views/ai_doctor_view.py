from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QButtonGroup,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QPushButton,
    QScrollArea,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from televault.domain.entities import ActionProposal


class ProposalCard(QFrame):
    approved = pyqtSignal(str)
    rejected = pyqtSignal(str)

    def __init__(self, proposal: ActionProposal):
        super().__init__()
        self.proposal_id = proposal.proposal_id
        self.setProperty("class", "fluentCard")
        self.setStyleSheet("border-left: 3px solid #38BDF8; background-color: #262626;")
        layout = QVBoxLayout(self)
        layout.setContentsMargins(14, 12, 14, 12)
        layout.setSpacing(6)

        title = QLabel(f"PROPOSED ACTION: {proposal.action_title}")
        title.setStyleSheet("font-weight: 700; color: #38BDF8; font-size: 13px;")
        layout.addWidget(title)

        why_lbl = QLabel(f"Why: {proposal.why}")
        why_lbl.setStyleSheet("color: #E5E7EB; font-size: 12px;")
        why_lbl.setWordWrap(True)
        layout.addWidget(why_lbl)

        change_lbl = QLabel(f"What changes: {proposal.what_will_change}")
        change_lbl.setStyleSheet("color: #9CA3AF; font-size: 11px;")
        change_lbl.setWordWrap(True)
        layout.addWidget(change_lbl)

        safe_lbl = QLabel(f"Safety: {proposal.what_will_not_change}")
        safe_lbl.setStyleSheet("color: #10B981; font-size: 11px;")
        safe_lbl.setWordWrap(True)
        layout.addWidget(safe_lbl)

        btn_row = QHBoxLayout()
        btn_row.addStretch()

        btn_approve = QPushButton("Approve & Execute")
        btn_approve.setProperty("class", "btnSuccess")
        btn_approve.clicked.connect(lambda: self.approved.emit(self.proposal_id))
        btn_row.addWidget(btn_approve)

        btn_reject = QPushButton("Dismiss")
        btn_reject.clicked.connect(lambda: self.rejected.emit(self.proposal_id))
        btn_row.addWidget(btn_reject)

        layout.addLayout(btn_row)


class AIDoctorView(QWidget):
    """Conversational interface with 10 Specialized Advisory Agents & Action Proposals."""

    query_requested = pyqtSignal(str, str)  # role, prompt
    proposal_approved = pyqtSignal(str)
    proposal_rejected = pyqtSignal(str)

    def __init__(self):
        super().__init__()
        self._active_role = "doctor"
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(14)

        # Header
        top = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("AI Doctor & Advisory Team")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("10 specialized advisory personas operating under strict Deterministic Core Supremacy.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top.addLayout(vbox)
        top.addStretch()

        p_pill = QLabel("PROPOSE-ONLY GATEWAY ACTIVE")
        p_pill.setProperty("class", "pillHealthy")
        top.addWidget(p_pill, alignment=Qt.AlignmentFlag.AlignVCenter)
        layout.addLayout(top)

        # Agent Selector Ribbon (Horizontal Scroll/Wrap)
        ribbon = QHBoxLayout()
        ribbon.setSpacing(6)
        self.btn_group = QButtonGroup(self)

        agents_spec = [
            ("doctor", "Vault Doctor"),
            ("redundancy", "Redundancy"),
            ("recovery", "Recovery"),
            ("versioning", "Versioning"),
            ("forensics", "Forensics"),
            ("channels", "Channels"),
            ("storage", "Storage"),
            ("security", "Security"),
            ("onboarding", "Onboarding"),
            ("copilot", "DR Co-Pilot"),
        ]

        for idx, (role, name) in enumerate(agents_spec):
            btn = QPushButton(name)
            btn.setCheckable(True)
            if role == "doctor":
                btn.setChecked(True)
            btn.clicked.connect(lambda _, r=role: self._select_agent(r))
            self.btn_group.addButton(btn)
            ribbon.addWidget(btn)

        ribbon.addStretch()
        layout.addLayout(ribbon)

        # Proposals Container
        self.proposals_container = QVBoxLayout()
        self.proposals_container.setSpacing(8)
        layout.addLayout(self.proposals_container)

        # Chat Stream Area
        self.chat_area = QTextEdit()
        self.chat_area.setReadOnly(True)
        self.chat_area.setPlaceholderText("Select an agent above or enter a question below to consult your vault advisors...")
        layout.addWidget(self.chat_area)

        # Input Box Row
        in_row = QHBoxLayout()
        self.prompt_input = QLineEdit()
        self.prompt_input.setPlaceholderText("Ask the agent anything about vault health, recovery, or security...")
        self.prompt_input.returnPressed.connect(self._send_query)
        in_row.addWidget(self.prompt_input)

        self.btn_send = QPushButton("Ask Agent")
        self.btn_send.setProperty("class", "btnAccent")
        self.btn_send.clicked.connect(self._send_query)
        in_row.addWidget(self.btn_send)

        layout.addLayout(in_row)

    def _select_agent(self, role: str):
        self._active_role = role
        self.prompt_input.setPlaceholderText(f"Ask the {role.title()} Agent a question...")

    def _send_query(self):
        txt = self.prompt_input.text().strip()
        if not txt:
            txt = "Provide current advisory review."
        self.chat_area.append(f"\n<b>[YOU]</b>: {txt}\n")
        self.prompt_input.clear()
        self.query_requested.emit(self._active_role, txt)

    def append_response(self, agent_name: str, message: str):
        self.chat_area.append(f"<b>[{agent_name}]</b>:\n{message}\n")

    def display_proposals(self, proposals: list[ActionProposal]):
        # Clear existing
        while self.proposals_container.count():
            item = self.proposals_container.takeAt(0)
            if item.widget():
                item.widget().deleteLater()

        for prop in proposals:
            card = ProposalCard(prop)
            card.approved.connect(self.proposal_approved.emit)
            card.rejected.connect(self.proposal_rejected.emit)
            self.proposals_container.addWidget(card)
