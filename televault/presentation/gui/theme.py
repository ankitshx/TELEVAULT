"""Windows 11 Fluent UI Theme Tokens and Stylesheet for TeleVault."""

THEME_STYLESHEET = """
QMainWindow, QDialog {
    background-color: #1A1A1A;
    color: #F3F4F6;
}

QWidget {
    font-family: 'Segoe UI Variable Text', 'Segoe UI', -apple-system, BlinkMacSystemFont, Roboto, sans-serif;
    font-size: 13px;
    color: #E5E7EB;
}

/* Sidebar Shell */
QFrame#sidebarPanel {
    background-color: #121212;
    border-right: 1px solid #282828;
}

QLabel#sidebarLogo {
    font-size: 17px;
    font-weight: 700;
    color: #38BDF8;
    letter-spacing: 1.5px;
    padding: 6px 0px;
}

QLabel#sidebarSubtitle {
    font-size: 11px;
    color: #9CA3AF;
    letter-spacing: 0.5px;
}

QPushButton.navBtn {
    background-color: transparent;
    color: #9CA3AF;
    border: none;
    border-radius: 6px;
    padding: 10px 14px;
    text-align: left;
    font-weight: 500;
    font-size: 13px;
}

QPushButton.navBtn:hover {
    background-color: #242424;
    color: #FFFFFF;
}

QPushButton.navBtn:checked, QPushButton.navBtnActive {
    background-color: #262626;
    color: #38BDF8;
    font-weight: 600;
    border-left: 3px solid #38BDF8;
    border-top-left-radius: 0px;
    border-bottom-left-radius: 0px;
}

/* Header & Top Bar */
QFrame#headerPanel {
    background-color: #1E1E1E;
    border-bottom: 1px solid #282828;
    padding: 12px 20px;
}

QLabel#brandTitle {
    font-size: 16px;
    font-weight: 600;
    color: #FFFFFF;
}

/* Fluent Cards */
QFrame.fluentCard {
    background-color: #242424;
    border: 1px solid #333333;
    border-radius: 8px;
    padding: 16px;
}

QFrame.statCard {
    background-color: #242424;
    border: 1px solid #333333;
    border-radius: 8px;
    padding: 16px;
}

QLabel.cardValue {
    font-size: 24px;
    font-weight: 700;
    color: #FFFFFF;
}

QLabel.cardLabel {
    font-size: 12px;
    color: #9CA3AF;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.5px;
}

/* Status Pills */
QLabel#badgeHealthy, QLabel.pillHealthy {
    background-color: rgba(16, 185, 129, 0.14);
    color: #34D399;
    border: 1px solid rgba(16, 185, 129, 0.35);
    border-radius: 12px;
    padding: 3px 10px;
    font-weight: 600;
    font-size: 11px;
}

QLabel#badgeDegraded, QLabel.pillDegraded {
    background-color: rgba(245, 158, 11, 0.14);
    color: #FBBF24;
    border: 1px solid rgba(245, 158, 11, 0.35);
    border-radius: 12px;
    padding: 3px 10px;
    font-weight: 600;
    font-size: 11px;
}

QLabel#badgeLost, QLabel.pillLost {
    background-color: rgba(239, 68, 68, 0.14);
    color: #F87171;
    border: 1px solid rgba(239, 68, 68, 0.35);
    border-radius: 12px;
    padding: 3px 10px;
    font-weight: 600;
    font-size: 11px;
}

/* Fluent Action Buttons */
QPushButton {
    background-color: #2E2E2E;
    color: #E5E7EB;
    border: 1px solid #3F3F3F;
    border-radius: 6px;
    padding: 7px 16px;
    font-weight: 500;
}

QPushButton:hover {
    background-color: #383838;
    border-color: #525252;
    color: #FFFFFF;
}

QPushButton:pressed {
    background-color: #222222;
}

QPushButton#btnPrimary, QPushButton.btnAccent {
    background-color: #0284C7;
    color: #FFFFFF;
    border: 1px solid #0284C7;
    font-weight: 600;
}

QPushButton#btnPrimary:hover, QPushButton.btnAccent:hover {
    background-color: #0369A1;
    border-color: #0369A1;
}

QPushButton.btnSuccess {
    background-color: #059669;
    color: #FFFFFF;
    border: 1px solid #059669;
    font-weight: 600;
}

QPushButton.btnSuccess:hover {
    background-color: #047857;
}

/* Inputs & Tables */
QLineEdit, QComboBox, QTextEdit {
    background-color: #1A1A1A;
    border: 1px solid #383838;
    border-radius: 6px;
    color: #FFFFFF;
    padding: 8px 12px;
}

QLineEdit:focus, QTextEdit:focus {
    border: 1px solid #38BDF8;
}

QTableWidget {
    background-color: #1E1E1E;
    border: 1px solid #2E2E2E;
    border-radius: 6px;
    gridline-color: #2A2A2A;
    color: #E5E7EB;
    selection-background-color: #2D3748;
}

QHeaderView::section {
    background-color: #181818;
    color: #9CA3AF;
    padding: 8px;
    border: none;
    border-bottom: 1px solid #2E2E2E;
    font-weight: 600;
    font-size: 11px;
    text-transform: uppercase;
}

QScrollBar:vertical {
    border: none;
    background: #181818;
    width: 8px;
    border-radius: 4px;
}

QScrollBar::handle:vertical {
    background: #383838;
    border-radius: 4px;
}

QScrollBar::handle:vertical:hover {
    background: #525252;
}

QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    height: 0px;
}
"""
