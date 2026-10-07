# MISTAKES LOG

## Phase 2 — Clippy Derivable Impls Warning
- Mistake: Implemented manual `impl Default` for `AppConfig` where all sub-structs already implemented `Default`, triggering `-D clippy::derivable_impls`.
- Root Cause: Wrote manual constructor boilerplate instead of checking if all inner struct members possessed valid `Default` implementations.
- Fix: Replaced manual `impl Default` with `Default` in `#[derive(...)]` on `AppConfig`.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings` executed as part of standard build validation.
- Prevention Rule: Always derive `Default` on container structs when all constituent fields implement `Default` with desired initial values.

## Phase 3 — Missing thiserror dependency in televault-crypto
- Mistake: `crates/televault-crypto/Cargo.toml` omitted `thiserror`, causing compilation failure in `src/error.rs`.
- Root Cause: Assumed `thiserror` would be transitively accessible from `televault-core` without declaring it in `televault-crypto`.
- Fix: Added `thiserror = { workspace = true }` directly to `crates/televault-crypto/Cargo.toml`.
- Regression Test: `cargo check --workspace` verifies all crate-level imports resolve independently.
- Prevention Rule: Every crate defining error types via `#[derive(Error)]` must explicitly declare `thiserror = { workspace = true }` in its own `Cargo.toml`.

## Phase 4 — Clippy Derivable Impls on ManifestVersion
- Mistake: Implemented manual `impl Default` for `ManifestVersion` instead of deriving `Default` with `#[default]` attribute on the `V1` variant.
- Root Cause: Wrote manual boilerplate `impl Default` on enum instead of using standard Rust derive.
- Fix: Derived `Default` on `ManifestVersion` and marked `#[default]` on variant `V1`.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings` executed as part of standard build verification.
- Prevention Rule: Always use `#[derive(Default)]` with `#[default]` on Rust 2021 enum default variants.
