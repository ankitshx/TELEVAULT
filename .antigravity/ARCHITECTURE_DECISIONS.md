# ARCHITECTURE DECISIONS

## AD-001: Rust-Only Architecture
- Decision: TELEVAULT uses Rust-only architecture with Tauri 2
- Reason: Eliminates localhost, port, sidecar, and Python dependency issues
- Date: 2026-10-07

## AD-002: Single Executable
- Decision: One .exe file with React frontend inside WebView2 and Rust core in same process
- Reason: No version mismatch, no backend startup issues, no DLL missing
- Date: 2026-10-07

## AD-003: Cargo Workspace Member Structure
- Decision: Centralized root Cargo workspace with 2 applications (`apps/cli`, `apps/desktop`) and 10 library crates under `crates/televault-*`
- Reason: Enforces strict modularity, clean domain boundaries, decoupled testing, and zero cyclic dependencies
- Date: 2026-10-07

## AD-004: Windows Static CRT Target Strategy
- Decision: Target Windows release binaries with static C runtime (`RUSTFLAGS="-C target-feature=+crt-static"`) on MSVC toolchains
- Reason: Ensures self-contained distribution without external Visual C++ Redistributable requirements
- Date: 2026-10-07
