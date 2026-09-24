# TeleVault Risk Register

**Last Updated:** Phase 0 Gate  
**Owner:** Agent 5 (QA & Security)  

| ID | Risk Description | Likelihood | Impact | Mitigation Strategy | Owner | Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **R-01** | Account FloodWait or temporary rate-limiting by Telegram | Medium | High | Concurrency locked to 1 by default; intercept `FloodWaitError` with automated sleep and resume countdown; no background scraping. | Agent 1 | Open |
| **R-02** | Accidental code path deleting Telegram messages | Low | Critical | Invariant 2 AST static test runs in CI and blocks gates if any delete method is introduced. Append-only policy enforced. | Agent 5 | Controlled |
| **R-03** | Local SQLite index loss or corruption | Medium | Medium | Self-describing `tv1` captions allow 100% rebuild from Telegram channels; index snapshot uploaded after each batch. | Agent 2 | Open |
| **R-04** | Plaintext credential exposure (API ID/Hash/Session) | Low | High | Credentials stored exclusively in Windows Credential Manager (keyring); zero logging of auth tokens; gitignore rules. | Agent 4 | Open |
| **R-05** | Files exceeding Telegram single file limit (2 GB / 4 GB) | Medium | Medium | Over-limit detection dialog with explicit user choice (Cancel default, Split opt-in); never split silently. | Agent 1 | Open |
| **R-06** | Accidental live account disruption during automated tests | Low | Critical | Tests use `FakeTelegramGateway` by default; live tests require explicit user gate approval and `TV_LIVE_TEST=1` in throwaway channel. | Agent 5 | Controlled |
| **R-07** | Data tampering or silent transmission corruption | Low | High | Pre-upload SHA-256 calculation; post-restore SHA-256 verification (Invariant 4); any mismatch fails restore. | Agent 2 | Controlled |
| **R-08** | PyInstaller single-exe packaging bloat or missing DLLs | Medium | Low | Minimal dependencies, excluded test packages, early CI smoke testing of executable build on Windows runner. | Agent 4 | Open |
