from pathlib import Path
from PyQt6.QtCore import Qt, pyqtSignal
from PyQt6.QtGui import QDragEnterEvent, QDropEvent
from PyQt6.QtWidgets import (
    QFileDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QVBoxLayout,
)


class DropZoneWidget(QFrame):
    """Large target drop zone for dragging and dropping files or folders."""

    files_dropped = pyqtSignal(list)  # list[Path]

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAcceptDrops(True)
        self.setObjectName("dropZone")
        self.setStyleSheet("""
            QFrame#dropZone {
                border: 2px dashed #242A35;
                background-color: rgba(14, 17, 22, 0.4);
                padding: 24px;
            }
            QFrame#dropZone:hover {
                border-color: #F59E0B;
                background-color: rgba(245, 158, 11, 0.04);
            }
        """)
        self._init_ui()

    def _init_ui(self):
        layout = QVBoxLayout(self)
        layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

        title = QLabel("DROP FILES HERE TO BACK UP")
        title.setStyleSheet("font-family: 'JetBrains Mono'; font-size: 14px; font-weight: bold; color: #ECEFF4;")
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(title)

        subtitle = QLabel("Accepts individual files or nested folders. Preserves original bytes and names.")
        subtitle.setStyleSheet("color: #7E889B; font-size: 11px;")
        subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(subtitle)

        btn_layout = QHBoxLayout()
        btn_layout.setAlignment(Qt.AlignmentFlag.AlignCenter)

        self.btn_choose_files = QPushButton("CHOOSE FILES")
        self.btn_choose_files.setObjectName("btnPrimary")
        self.btn_choose_files.clicked.connect(self._on_choose_files)
        btn_layout.addWidget(self.btn_choose_files)

        self.btn_choose_folder = QPushButton("CHOOSE FOLDER")
        self.btn_choose_folder.clicked.connect(self._on_choose_folder)
        btn_layout.addWidget(self.btn_choose_folder)

        layout.addLayout(btn_layout)

    def _on_choose_files(self):
        files, _ = QFileDialog.getOpenFileNames(self, "Select Files to Back Up")
        if files:
            self.files_dropped.emit([Path(f) for f in files])

    def _on_choose_folder(self):
        folder = QFileDialog.getExistingDirectory(self, "Select Folder to Back Up")
        if folder:
            self.files_dropped.emit([Path(folder)])

    def dragEnterEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()

    def dropEvent(self, event: QDropEvent):
        urls = event.mimeData().urls()
        paths = [Path(u.toLocalFile()) for u in urls if u.isLocalFile()]
        if paths:
            self.files_dropped.emit(paths)
