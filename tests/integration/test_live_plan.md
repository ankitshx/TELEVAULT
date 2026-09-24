# Live Account Integration Test Protocol

**Strict Safety Protocol (Rule 0.4 & 0.7)**  
Real-account tests require **explicit user approval** at a phase gate before execution. Live tests are disabled by default (`TV_LIVE_TEST=0`).

---

## 1. Safety Invariants for Live Testing
1. **Approval Gate:** Never run live tests unless user explicitly approves at a gate.
2. **Isolated Throwaway Channels:** Live tests only run in throwaway private channels created specifically for testing (e.g. `TeleVault Test Primary <timestamp>`), NEVER existing channels or chats.
3. **Small Files First:** Initial test uses a small text file (< 100 KB) before testing larger payloads.
4. **Zero Credential Logging:** No phone numbers, login verification codes, 2FA passwords, API hashes, or session tokens are ever printed to logs or terminal.
5. **Clean Verification:** Pre-upload hash must match post-restore hash byte-for-byte.

---

## 2. Opt-In Execution Procedure

When the user grants explicit approval at a gate:
```powershell
# Step 1: Export credentials temporarily in environment (or enter via setup)
$env:TV_LIVE_TEST="1"
$env:TELEVAULT_API_ID="<your_api_id>"
$env:TELEVAULT_API_HASH="<your_api_hash>"
$env:TELEVAULT_PHONE="<your_phone>"

# Step 2: Run dry-run first
python -m televault backup .\sample_test.txt --dry-run

# Step 3: Run live test harness (only after confirmation)
pytest tests/integration/ -m live -v
```
