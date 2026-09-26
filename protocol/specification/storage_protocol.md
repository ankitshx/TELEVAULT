# TeleCloud Storage Protocol & Specification (TCSP-1)

**Specification Version:** 1.0.0  
**Status:** Active Draft  

---

## 1. Abstract
The TeleCloud Storage Protocol (TCSP-1) governs how arbitrary logical files and directory structures are deterministically partitioned, encrypted, stored, and reassembled over object storage backends (primarily Telegram MTProto).

---

## 2. Logical File & Chunking Rules

1. **Logical vs Physical Abstraction:**
   - A `Logical File` is the single entity recognized by the user.
   - A logical file consists of $N \ge 1$ `Chunks`.
   - If a file is smaller than or equal to the configured chunk size $C$, $N = 1$.
   - If a file exceeds $C$, it is deterministically partitioned into $N = \lceil \text{Size} / C \rceil$ chunks.
2. **Chunk Bounds:**
   - Every chunk is defined by:
     - `index`: 0-indexed integer $[0, N-1]$
     - `offset`: byte offset in original stream
     - `size`: byte length of the chunk
     - `sha256`: SHA-256 hash of the chunk data
3. **Manifest Requirement:**
   - Every multi-chunk logical file MUST have a corresponding manifest conforming to `protocol/schemas/manifest_v1.json`.
   - Single-chunk files may embed metadata directly or maintain a manifest for cryptographic consistency.

---

## 3. Cryptographic Specification

### 3.1 Key Derivation
- Algorithm: **Argon2id** (RFC 9106)
- Parameters:
  - Memory cost: $65536 \text{ KiB}$ ($64\text{ MB}$)
  - Time cost (iterations): $3$
  - Parallelism: $2$ lanes
  - Salt: $16$ random bytes generated via CSPRNG
  - Output key length: $32$ bytes ($256$ bits)

### 3.2 Streaming Cipher (AES-256-GCM)
- Authenticated Encryption with Associated Data (AEAD) per chunk.
- Nonce size: $12$ bytes ($96$ bits), randomly generated per chunk.
- Associated Data (AD): $8$-byte big-endian representation of `chunk_index`. This guarantees that chunks cannot be swapped or injected out-of-order.
- Authentication Tag: $16$ bytes appended to chunk ciphertext.
- **Nonce Reuse Prevention:** Because each chunk generates a fresh $96$-bit random nonce from a cryptographically secure RNG and chunk indices are bound in AD, birthday collisions within any single file are vanishingly improbable ($< 2^{-32}$ for tens of millions of chunks).

---

## 4. Verification Hierarchy

Integrity verification MUST strictly follow this 3-tier sequence:

```
[ Tier 1: Chunk Verification ]
For each chunk received:
  Verify SHA-256(chunk_bytes) == chunk.sha256
  If encrypted: Verify AES-256-GCM authentication tag with chunk_index in AD.
             |
             v
[ Tier 2: Manifest Verification ]
Verify manifest schema, chunk offsets, total size == sum(chunk.size).
             |
             v
[ Tier 3: Full-File Verification ]
Stream merged chunks through SHA-256 hasher:
  Verify final SHA-256 == manifest.full_sha256
```

If any tier fails, the transfer MUST NOT be marked complete.

---

## 5. Storage Provider Abstraction

Each chunk is stored as an opaque document inside Telegram. The protocol stores metadata in the Telegram caption using a standardized prefix:

```
tc1 {"f":"<file_uuid>","p":<chunk_index>,"t":<total_chunks>,"s":"<chunk_sha256>"}
```

This ensures that even if local SQLite metadata is destroyed, TeleCloud can scan the user's storage channel and reconstruct the entire directory index and file catalog.
