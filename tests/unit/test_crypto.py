from pathlib import Path
import pytest

from televault.infrastructure.crypto.kdf import derive_key
from televault.infrastructure.crypto.stream_cipher import (
    CryptoError,
    decrypt_file_stream,
    encrypt_file_stream,
)


def test_argon2id_kdf():
    passphrase = "correct horse battery staple"
    key1, salt1 = derive_key(passphrase)
    assert len(key1) == 32
    assert len(salt1) == 16

    # Same salt reproduces identical key
    key2, _ = derive_key(passphrase, salt=salt1)
    assert key1 == key2

    # Different salt produces different key
    key3, _ = derive_key(passphrase)
    assert key1 != key3


def test_aes_gcm_stream_encryption_roundtrip(tmp_path: Path):
    src_file = tmp_path / "secret_document.pdf"
    original_data = b"Confidential business data protected by TeleVault Private Mode." * 1000
    src_file.write_bytes(original_data)

    passphrase = "UltraSecureVaultPassphrase2026!"
    enc_file = tmp_path / "encrypted.tvc"

    # Encrypt
    encrypt_file_stream(src_file, enc_file, passphrase)
    assert enc_file.exists()
    assert enc_file.stat().st_size > 0
    assert enc_file.read_bytes() != original_data  # Ciphertext differs

    # Decrypt
    dest_dir = tmp_path / "decrypted_out"
    restored_file = decrypt_file_stream(enc_file, dest_dir, passphrase)

    assert restored_file.exists()
    assert restored_file.name == "secret_document.pdf"  # Filename restored
    assert restored_file.read_bytes() == original_data  # Content matches perfectly


def test_aes_gcm_wrong_passphrase_fails_cleanly(tmp_path: Path):
    src_file = tmp_path / "vault_key.txt"
    src_file.write_bytes(b"Top secret encryption keys.")

    correct_passphrase = "CorrectPassword123"
    wrong_passphrase = "WrongPassword999"

    enc_file = tmp_path / "test.tvc"
    encrypt_file_stream(src_file, enc_file, correct_passphrase)

    dest_dir = tmp_path / "fail_out"
    with pytest.raises(CryptoError):
        decrypt_file_stream(enc_file, dest_dir, wrong_passphrase)
