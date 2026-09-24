import asyncio
from collections.abc import Callable
import logging
from typing import Any, TypeVar

try:
    from telethon.errors import FloodWaitError
except ImportError:
    # Fallback for environments without telethon installed
    class FloodWaitError(Exception):  # type: ignore
        seconds: int = 0

logger = logging.getLogger("televault.telegram.retry")
T = TypeVar("T")


async def with_flood_wait_retry(
    coro_func: Callable[..., Any],
    *args: Any,
    max_retries: int = 3,
    on_flood_wait: Callable[[int], None] | None = None,
    **kwargs: Any,
) -> Any:
    """Executes a coroutine with automatic FloodWaitError interception and sleep."""
    retries = 0
    while True:
        try:
            return await coro_func(*args, **kwargs)
        except FloodWaitError as e:
            retries += 1
            wait_time = int(getattr(e, "seconds", 5)) + 1
            logger.warning(f"Telegram FloodWait encountered: sleeping {wait_time}s (attempt {retries}/{max_retries})")
            if on_flood_wait:
                on_flood_wait(wait_time)
            if retries > max_retries:
                raise
            await asyncio.sleep(wait_time)
        except (ConnectionError, TimeoutError) as e:
            retries += 1
            if retries > max_retries:
                raise
            backoff = 2**retries
            logger.warning(f"Network error ({e}): backing off {backoff}s")
            await asyncio.sleep(backoff)
