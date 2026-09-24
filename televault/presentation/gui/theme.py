"""Dark terminal-inspired schematic theme tokens and stylesheet."""

THEME_STYLESHEET = """
QMainWindow, QDialog {
    background-color: #0B0C0E;
    color: #ECEFF4;
}

QWidget {
    font-family: 'IBM Plex Mono', 'JetBrains Mono', 'Consolas', monospace;
    font-size: 13px;
    color: #ECEFF4;
}

/* Panels */
QFrame#panel {
    background-color: #14171C;
    border: 1px solid #242A35;
}

/* Header */
QFrame#headerPanel {
    background-color: #101318;
    border-bottom: 1px solid #242A35;
    padding: 10px;
}

QLabel#brandTitle {
    font-family: 'JetBrains Mono', 'Consolas', monospace;
    font-size: 16px;
    font-weight: bold;
    color: #FFFFFF;
    letter-spacing: 2px;
}

/* Badges */
QLabel#badgeHealthy {
    background-color: rgba(16, 185, 129, 0.15);
    color: #10B981;
    border: 1px solid #10B981;
    padding: 4px 10px;
    font-weight: bold;
    font-size: 11px;
}

QLabel#badgeDegraded {
    background-color: rgba(245, 158, 11, 0.15);
    color: #F59E0B;
    border: 1px solid #F59E0B;
    padding: 4px 10px;
    font-weight: bold;
    font-size: 11px;
}

QLabel#badgeLost {
    background-color: rgba(239, 68, 68, 0.15);
    color: #EF4444;
    border: 1px solid #EF4444;
    padding: 4px 10px;
    font-weight: bold;
    font-size: 11px;
}

/* Buttons */
QPushButton {
    background-color: #1B2028;
    color: #ECEFF4;
    border: 1px solid #242A35;
    padding: 6px 14px;
    font-weight: 500;
}

QPushButton:hover {
    background-color: #252D3A;
    border-color: #F59E0B;
    color: #FFFFFF;
}

QPushButton:pressed {
    background-color: #0E1116;
}

QPushButton#btnPrimary {
    background-color: #F59E0B;
    color: #0B0C0E;
    border: 1px solid #F59E0B;
    font-weight: bold;
}

QPushButton#btnPrimary:hover {
    background-color: #D97706;
}

/* Inputs */
QLineEdit, QComboBox {
    background-color: #0E1116;
    border: 1px solid #242A35;
    color: #FFFFFF;
    padding: 6px 10px;
}

QLineEdit:focus, QComboBox:focus {
    border-color: #F59E0B;
}

/* Table */
QTableWidget {
    background-color: #14171C;
    border: 1px solid #242A35;
    gridline-color: #1D222A;
    selection-background-color: rgba(245, 158, 11, 0.2);
    selection-color: #FFFFFF;
}

QHeaderView::section {
    background-color: #0E1116;
    color: #7E889B;
    font-size: 11px;
    font-weight: bold;
    text-transform: uppercase;
    border: none;
    border-bottom: 1px solid #242A35;
    padding: 8px;
}

/* Progress Bar */
QProgressBar {
    background-color: #0E1116;
    border: 1px solid #242A35;
    text-align: center;
    color: #ECEFF4;
    font-size: 11px;
    height: 18px;
}

QProgressBar::chunk {
    background-color: #F59E0B;
}
"""
