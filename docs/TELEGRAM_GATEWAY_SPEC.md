# Telegram Gateway Specification & Limits Research

**Owner:** Agent 1 (Telegram Core)  
**Status:** Approved for Phase 0  
**Target Library:** Telethon 1.34+ (MTProto User Session)

---

## 1. MTProto vs. Bot API Rationale

| Dimension | Telegram Bot API | Telethon MTProto User Client |
| :--- | :--- | :--- |
| **Max Upload Limit** | 50 MB (20 MB for standard downloads) | 2,000 MB (Free) / 4,000 MB (Telegram Premium) |
| **Direct Streaming** | No chunked MTProto streaming | Direct file handle streaming via `upload_file` |
| **Channel Creation** | Cannot create private supergroups/channels | Full channel management (`CreateChannelRequest`) |
| **Server-Side Forwarding** | Restricted | Native `forward_messages` (zero re-upload overhead) |
| **Admin Log Access** | Not available | `iter_admin_log` for deleted message recovery window |

**Conclusion:** Bot API is fundamentally inadequate for a desktop backup tool. Telethon MTProto user client is required.

---

## 2. API Limits & Throttling (FloodWait)

### 2.1 File Size Limits
- **Standard Account:** 2,000 MB (2 GB) strict per file.
- **Telegram Premium:** 4,000 MB (4 GB) strict per file.
- **Limit Detection:** `client.get_me().premium` determines account tier.
- **Handling Strategy:** Files exceeding the account limit trigger the Over-Limit Dialog. The user chooses between **Cancel** (default) or **Split** (`.part001`, `.part002`). Silent splitting is strictly forbidden.

### 2.2 Rate Limiting & FloodWait
- Telegram enforces strict call-frequency thresholds on media uploads and forwarding.
- When `telethon.errors.FloodWaitError` is raised, it provides `error.seconds`.
- **Policy:** The gateway intercepts `FloodWaitError`, emits a `FloodWaitEncounteredEvent(seconds)`, sleeps asynchronously for `error.seconds + 1`, and automatically retries the operation without duplicating messages.
- **Concurrency Default:** Sequential uploads (`max_concurrent = 1`) to preserve account standing and avoid FloodWait bans.

---

## 3. Streaming and Force-Document Guarantee

### 3.1 Streaming Upload
- Files are streamed via Telethon's `client.upload_file()` using file descriptors and chunk callbacks.
- Memory consumption remains low (< 50 MB) even when processing 2 GB files.
- `progress_callback(uploaded_bytes, total_bytes)` drives the live pipeline UI strip.

### 3.2 Single Document Rule
- All uploads call `client.send_file(entity, file, force_document=True)`.
- Telegram photo/video compression is completely bypassed.
- Original filename, extension, and exact byte length are preserved.

---

## 4. Server-Side Dual-Write (Mirroring)

1. **Step 1:** Upload file once to `TeleVault Primary` private channel. Returns `msg_primary`.
2. **Step 2:** Server-side forward `msg_primary` to `TeleVault Mirror` private channel using `client.forward_messages(mirror_channel, msg_primary.id, primary_channel)`.
3. **Step 3:** Telegram copies the document pointer server-side in under 150 ms without any local data transfer.
4. **Step 4:** Verify presence in both channels with `client.get_messages()`. Mark record `HEALTHY`.

---

## 5. Metadata Schema (tv1 Captions)

Every uploaded message includes a machine-parseable JSON payload in the caption:
```text
tv1 {"id":"<uuid>","name":"invoice_2026.pdf","sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","size":489201,"mtime":1774438800.0,"mode":"original","ver":1}
```
- In **Private Mode**, `name` and metadata are omitted from the caption; only an opaque UUID and ciphertext signature are stored.

---

## 6. Admin Log Deletion Window

- When a message is deleted from a channel, Telegram logs it in the channel's recent actions log.
- Telethon queries this via `client.iter_admin_log(channel, delete=True)`.
- Telegram retains admin log events for approximately **48 hours**.
- TeleVault uses this as a secondary recovery clue for degraded messages deleted within this time frame.
