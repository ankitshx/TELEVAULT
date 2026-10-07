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

## Phase 5 — Serde Case Mismatch on Enum SQL Deserialization
- Mistake: In `map_snapshot_row`, deserializing `BackupStatus` using `serde_json::from_value` failed because `to_string()` formatted `BackupStatus::BackingUp` as `"backing_up"`, while serde's `#[serde(rename_all = "lowercase")]` expected `"backingup"`.
- Root Cause: Relied on generic serde JSON deserialization for domain enums whose custom `Display` implementation outputs underscored identifiers while serde converts to raw lowercase.
- Fix: Replaced indirect JSON deserialization with explicit `match` blocks handling both `"backing_up"` and `"backingup"`, and applied the same explicit mapping pattern to `TransferDirection` and `TransferStatus`.
- Regression Test: `test_snapshots_versions_and_transfer_jobs_crud` in `crates/televault-db/tests/db_tests.rs`.
- Prevention Rule: When converting between SQL text columns and domain enums, use explicit, exhaustive `match` blocks rather than dynamic JSON serialization wrappers.
