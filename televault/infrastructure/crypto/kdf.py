import os
from argon2.low_level import Type, hash_secret_raw

SALT_SIZE = 16
KEY_SIZE = 32  # 256-bit AES key


def derive_key(passphrase: str, salt: bytes | None = None) -> tuple[bytes, bytes]:
    """Derive a 256-bit encryption key from a user passphrase using Argon2id.

    Returns: (derived_key_bytes, salt_bytes)
    """
    if salt is None:
        salt = os.urandom(SALT_SIZE)

    secret = passphrase.encode("utf-8")
    derived_key = hash_secret_raw(
        secret=secret,
        salt=salt,
        time_cost=3,
        memory_cost=65536,  # 64 MB RAM
        parallelism=2,
        hash_len=KEY_SIZE,
        type=Type.ID,
    )
    return derived_key, salt
