from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QVBoxLayout,
    QWidget,
)


class SecurityView(QWidget):
    """Security & Cryptographic Audit View: Argon2id, AES-GCM, and manifest chain verification."""

    verify_manifest_requested = pyqtSignal()
    generate_manifest_requested = pyqtSignal()

    def __init__(self):
        super().__init__()
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setContentsMargins(28, 20, 28, 20)
        layout.setSpacing(16)

        # Header
        top = QHBoxLayout()
        vbox = QVBoxLayout()
        lbl = QLabel("Security & Cryptographic Center")
        lbl.setStyleSheet("font-size: 20px; font-weight: 700; color: #FFFFFF;")
        vbox.addWidget(lbl)
        sub = QLabel("State-of-the-art key derivation, authenticated stream ciphers, and hash-chained manifests.")
        sub.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        vbox.addWidget(sub)
        top.addLayout(vbox)
        top.addStretch()

        self.sec_pill = QLabel("AUTHENTICATED ENCRYPTION READY")
        self.sec_pill.setProperty("class", "pillHealthy")
        top.addWidget(self.sec_pill, alignment=Qt.AlignmentFlag.AlignVCenter)
        layout.addLayout(top)

        # Cryptographic Standards Card
        crypto_card = QFrame()
        crypto_card.setProperty("class", "fluentCard")
        c_layout = QVBoxLayout(crypto_card)
        c_layout.setContentsMargins(18, 16, 18, 16)
        c_layout.setSpacing(10)

        c_title = QLabel("Cryptographic Architecture")
        c_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        c_layout.addWidget(c_title)

        c_info = (
            "• <b>Master Key Derivation:</b> Argon2id (RFC 9106) with m=64MB, t=3 passes, p=4 lanes.<br>"
            "• <b>Stream Encryption:</b> AES-256-GCM (128-bit authentication tag, chunked streaming).<br>"
            "• <b>Session Isolation:</b> Telegram MTProto sessions are isolated locally to <code>%LOCALAPPDATA%\\TeleVault\\sessions\\</code>.<br>"
            "• <b>Zero-Knowledge Guarantee:</b> Passphrases are never saved or sent to Telegram servers."
        )
        c_lbl = QLabel(c_info)
        c_lbl.setStyleSheet("color: #E5E7EB; font-size: 12px; line-height: 1.6;")
        c_layout.addWidget(c_lbl)
        layout.addWidget(crypto_card)

        # Tamper-Evident Manifest Chain Card
        man_card = QFrame()
        man_card.setProperty("class", "fluentCard")
        m_layout = QVBoxLayout(man_card)
        m_layout.setContentsMargins(18, 16, 18, 16)
        m_layout.setSpacing(12)

        m_title = QLabel("Tamper-Evident Vault Manifest Chain")
        m_title.setStyleSheet("font-size: 15px; font-weight: 600; color: #FFFFFF;")
        m_layout.addWidget(m_title)

        m_desc = QLabel(
            "Every vault generation produces a cryptographically sealed manifest chaining to previous "
            "hashes. Any local database tampering or unauthorized record injection is instantly identified."
        )
        m_desc.setStyleSheet("color: #9CA3AF; font-size: 12px;")
        m_desc.setWordWrap(True)
        m_layout.addWidget(m_desc)

        self.lbl_manifest_status = QLabel("Manifest Chain Status: Intact and verified.")
        self.lbl_manifest_status.setStyleSheet("color: #10B981; font-weight: 600; font-size: 13px;")
        m_layout.addWidget(self.lbl_manifest_status)

        m_btn_row = QHBoxLayout()
        self.btn_verify_chain = QPushButton("Verify Manifest Integrity")
        self.btn_verify_chain.clicked.connect(self.verify_manifest_requested.emit)
        m_btn_row.addWidget(self.btn_verify_chain)

        self.btn_generate_chain = QPushButton("Generate New Manifest Generation")
        self.btn_generate_chain.setProperty("class", "btnAccent")
        self.btn_generate_chain.clicked.connect(self.generate_manifest_requested.emit)
        m_btn_row.addWidget(self.btn_generate_chain)

        m_btn_row.addStretch()
        m_layout.addLayout(m_btn_row)

        layout.addWidget(man_card)
        layout.addStretch()

    def set_manifest_status(self, is_valid: bool, message: str):
        color = "#10B981" if is_valid else "#EF4444"
        self.lbl_manifest_status.setText(f"Manifest Chain Status: {message}")
        self.lbl_manifest_status.setStyleSheet(f"color: {color}; font-weight: 600; font-size: 13px;")
