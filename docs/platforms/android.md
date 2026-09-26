# TeleCloud — Android Platform Specification (Deferred / Future Scope)

> **IMPORTANT ARCHITECTURAL DIRECTIVE:**  
> Android development is **STRICTLY OUT OF SCOPE** for the current implementation cycle.  
> No Android code or build tooling may be included in the primary repository at this stage.  
> This document serves as the formal specification for future platform parity.

---

## 1. Architectural Vision

When implemented in a future development phase, the Android TeleCloud client will be a first-class native Android application delivering seamless personal cloud storage backed by Telegram, fully interoperable with the TeleCloud Storage Protocol and Manifest Specification.

### Core Principles
1. **Identical Storage & Manifest Protocol:** The Android client will read, verify, write, and repair files using the identical versioned manifest format (`protocol/schemas/manifest_v1.json`) and streaming encryption schemes (`AES-256-GCM` with Argon2id) used by Windows Desktop and Linux CLI.
2. **Native Android Stack:**
   - **Language:** Kotlin
   - **UI Framework:** Jetpack Compose (Material 3)
   - **Architecture:** MVI / Clean Architecture (UI $\rightarrow$ ViewModel $\rightarrow$ UseCase $\rightarrow$ Repository $\rightarrow$ Data Source)
   - **Concurrency:** Kotlin Coroutines + Flow
   - **Background Transfers:** Android Jetpack `WorkManager` (Foreground Service with persistent notification)
   - **Local Cache & Metadata:** Room Database (SQLite)
   - **Key Storage:** Android Keystore System (`MasterKey`, `EncryptedSharedPreferences`)
   - **Telegram MTProto Client:** TDLib (via JNI) or native Kotlin MTProto engine behind the `StorageProvider` abstraction.
3. **No Telegram Internal Exposure:** Like the desktop application, the Android UI will present a pure cloud-drive experience (Folders, Files, Recents, Favorites, Offline, Transfers), never exposing raw Telegram chats or message IDs as primary concepts.

---

## 2. Technical Requirements for Future Implementation

### 2.1 Android Storage Access Framework (SAF) Integration
- The application should provide a `DocumentsProvider` implementation, allowing other Android apps (file managers, media players, office suites) to browse and open files directly from TeleCloud via the system file picker.

### 2.2 Background Transfer Reliability
- File uploads and downloads must survive process death and app suspension using `WorkManager` with `ForegroundInfo` and ongoing status notifications.
- Chunk-level resumability must ensure that network drops on cellular data do not cause multi-gigabyte transfers to restart from byte zero.

### 2.3 Cryptography & Security
- Cryptographic keys derived from the user passphrase (via Argon2id) must be stored in memory only when unlocked and protected using the Android Keystore hardware-backed security (StrongBox / TEE where available).
- Biometric authentication (`BiometricPrompt`) to unlock the local vault index and cache.

---

## 3. Storage Protocol Interoperability

The Android application will implement the following protocol invariants:
- **Chunk Slicing:** Honor manifest chunk offsets and byte counts.
- **Verification Hierarchy:**
  1. Chunk SHA-256 validation.
  2. Manifest generation and signature validation.
  3. Reassembled whole-file SHA-256 validation.
- **Encryption:** Decrypt and stream AES-256-GCM chunk slices using the 12-byte nonce, 16-byte tag, and authenticated 8-byte chunk index.

---

## 4. Current Repository Isolation

- Android code, Gradle files, or Android-specific dependencies MUST NOT be merged into the desktop or CLI crates.
- All shared logic between Windows, Linux, and future Android lives strictly in the language-agnostic **TeleCloud Storage Protocol & Manifest Schemas** located in [`protocol/`](file:///x:/TELEVAULT/protocol).
