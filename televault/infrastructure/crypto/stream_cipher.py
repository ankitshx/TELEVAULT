from collections.abc import Callable
import os
from pathlib import Path
import struct
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

from televault.infrastructure.crypto.kdf import derive_key

MAGIC_HEADER = b"TVCR"
CHUNK_SIZE = 1024 * 1024  # 1 MB data chunks
NONCE_SIZE = 12


class CryptoError(Exception):
    """Raised when decryption or passphrase verification fails."""
    pass


def encrypt_file_stream(
    src_path: Path,
    dest_path: Path,
    passphrase: str,
    on_progress: Callable[[int, int], None] | None = None,
) -> Path:
    """Stream encrypt a file with AES-256-GCM and an Argon2id-derived key.

    Encrypts both file payload and filename.
    Never loads the entire file into memory.
    """
    key, salt = derive_key(passphrase)
    aesgcm = AESGCM(key)

    total_bytes = src_path.stat().st_size
    processed_bytes = 0

    dest_path.parent.mkdir(parents=True, exist_ok=True)
    with src_path.open("rb") as f_in, dest_path.open("wb") as f_out:
        # Header: Magic + Salt
        f_out.write(MAGIC_HEADER)
        f_out.write(salt)

        # Encrypt filename
        name_bytes = src_path.name.encode("utf-8")
        name_nonce = os.urandom(NONCE_SIZE)
        enc_name = aesgcm.encrypt(name_nonce, name_bytes, associated_data=MAGIC_HEADER)
        f_out.write(name_nonce)
        f_out.write(struct.pack(">H", len(enc_name)))
        f_out.write(enc_name)

        # Encrypt content in chunks
        chunk_index = 0
        while chunk := f_in.read(CHUNK_SIZE):
            nonce = os.urandom(NONCE_SIZE)
            ad = struct.pack(">Q", chunk_index)
            ciphertext = aesgcm.encrypt(nonce, chunk, associated_data=ad)

            f_out.write(nonce)
            f_out.write(struct.pack(">I", len(ciphertext)))
            f_out.write(ciphertext)

            chunk_index += 1
            processed_bytes += len(chunk)
            if on_progress:
                on_progress(processed_bytes, total_bytes)

        # End of stream marker
        f_out.write(struct.pack(">I", 0))

    return dest_path


def decrypt_file_stream(
    src_path: Path,
    dest_dir: Path,
    passphrase: str,
    on_progress: Callable[[int, int], None] | None = None,
) -> Path:
    """Stream decrypt an AES-256-GCM encrypted file.

    Restores original filename and verifies authentication tag on every chunk.
    Raises CryptoError if passphrase is wrong or file is corrupted.
    """
    with src_path.open("rb") as f_in:
        magic = f_in.read(len(MAGIC_HEADER))
        if magic != MAGIC_HEADER:
            raise CryptoError("Not a valid TeleVault encrypted archive.")

        salt = f_in.read(16)
        key, _ = derive_key(passphrase, salt=salt)
        aesgcm = AESGCM(key)

        # Read encrypted filename
        name_nonce = f_in.read(NONCE_SIZE)
        name_len_bytes = f_in.read(2)
        if len(name_len_bytes) < 2:
            raise CryptoError("Corrupted archive header.")
        (enc_name_len,) = struct.unpack(">H", name_len_bytes)
        enc_name = f_in.read(enc_name_len)

        try:
            filename = aesgcm.decrypt(
                name_nonce, enc_name, associated_data=MAGIC_HEADER
            ).decode("utf-8")
        except Exception as e:
            raise CryptoError(f"Incorrect passphrase or corrupted metadata: {e}") from e

        dest_file = dest_dir / filename
        dest_dir.mkdir(parents=True, exist_ok=True)

        chunk_index = 0
        with dest_file.open("wb") as f_out:
            while True:
                nonce = f_in.read(NONCE_SIZE)
                if not nonce:
                    break

                len_bytes = f_in.read(4)
                if not len_bytes or len(len_bytes) < 4:
                    break
                (chunk_len,) = struct.unpack(">I", len_bytes)
                if chunk_len == 0:
                    # End of stream
                    break

                ciphertext = f_in.read(chunk_len)
                ad = struct.pack(">Q", chunk_index)
                try:
                    plaintext = aesgcm.decrypt(nonce, ciphertext, associated_data=ad)
                except Exception as e:
                    # Clean up partial decrypted file
                    f_out.close()
                    dest_file.unlink(missing_ok=True)
                    raise CryptoError(f"Decryption failed at chunk {chunk_index}: {e}") from e

                f_out.write(plaintext)
                chunk_index += 1

        return dest_file
