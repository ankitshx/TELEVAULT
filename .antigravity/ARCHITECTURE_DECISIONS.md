# ARCHITECTURE DECISIONS

## AD-001: Rust-Only Architecture
- Decision: TELEVAULT uses Rust-only architecture with Tauri 2
- Reason: Eliminates localhost, port, sidecar, and Python dependency issues
- Date: 2026-10-07

## AD-002: Single Executable
- Decision: One .exe file with React frontend inside WebView2 and Rust core in same process
- Reason: No version mismatch, no backend startup issues, no DLL missing
- Date: 2026-10-07
