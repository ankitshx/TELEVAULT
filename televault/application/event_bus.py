from collections import defaultdict
from collections.abc import Callable
from typing import Any

from televault.domain.events import DomainEvent
from televault.domain.ports import EventBus


class SimpleEventBus(EventBus):
    """In-memory event bus implementation for publishing domain events."""

    def __init__(self) -> None:
        self._handlers: dict[type[DomainEvent], list[Callable[[Any], None]]] = defaultdict(list)

    def publish(self, event: DomainEvent) -> None:
        event_type = type(event)
        for handler in self._handlers[event_type]:
            try:
                handler(event)
            except Exception as e:
                # Log or handle exception in subscriber without crashing publisher
                pass

    def subscribe(self, kind: type[DomainEvent], handler: Callable[[Any], None]) -> None:
        self._handlers[kind].append(handler)
