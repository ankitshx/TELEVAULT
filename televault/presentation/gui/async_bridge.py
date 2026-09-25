import asyncio
from collections.abc import Coroutine
import sys
from typing import Any
from PyQt6.QtCore import QObject, QThread, pyqtSignal


class AsyncBridge(QThread):
    """Bridges asyncio operations to the Qt GUI thread safely via signals."""

    _active_instances: set["AsyncBridge"] = set()

    task_started = pyqtSignal(str)
    progress_updated = pyqtSignal(int, int)  # current, total
    task_completed = pyqtSignal(object)
    task_failed = pyqtSignal(str)

    def __init__(self, coro_func: Any, *args: Any, **kwargs: Any):
        super().__init__()
        self.coro_func = coro_func
        self.args = args
        self.kwargs = kwargs

    def start(self, priority: QThread.Priority = QThread.Priority.InheritPriority) -> None:
        AsyncBridge._active_instances.add(self)
        self.finished.connect(lambda: AsyncBridge._active_instances.discard(self))
        super().start(priority)

    def run(self) -> None:
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # Wire progress callback if requested
            if "on_progress" in self.kwargs:
                pass
            result = loop.run_until_complete(self.coro_func(*self.args, **self.kwargs))
            self.task_completed.emit(result)
        except Exception as e:
            self.task_failed.emit(str(e))
        finally:
            loop.close()
