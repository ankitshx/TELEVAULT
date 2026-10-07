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

## Phase 6 — Mock Provider Whole-File Buffering on 1.8 GB Chunk Test
- Mistake: In `MockStorageProvider::upload`, streamed bytes were collected into an unbounded `Vec<u8>` via `extend_from_slice`, causing `test_large_chunk_streaming_resource_efficiency` with a 1.8 GB virtual stream to attempt allocating 1.8 GB on the heap, violating the bounded-memory architecture rule and stalling execution.
- Root Cause: The test mock provider accumulated raw payload bytes into memory rather than abstracting large virtual payloads.
- Fix: Refactored `MockStoredObject` with `MockPayload::VirtualLarge { fill_byte }` for payloads > 4 MiB, reading and hashing through 64 KiB stack buffers without whole-file allocation, and added `[profile.dev.package.sha2] opt-level = 3` for fast test hashing.
- Regression Test: `test_large_chunk_streaming_resource_efficiency` in `crates/televault-storage/tests/storage_tests.rs` (completes in ~1.88s with 0 multi-gigabyte RAM allocation).
- Prevention Rule: Never collect streamed data into unbounded `Vec<u8>` in storage adapters or mocks; always handle large virtual payloads with streaming generation and bounded buffers.

## Phase 6 — Clippy Warnings on Collapsible If and Complex Transport Type
- Mistake: Clippy triggered `collapsible_if` in `purge_stale_staging_files` and `type_complexity` on `MockTelegramTransport.messages`.
- Root Cause: Nested `if` checks in file cleanup loop and inline raw `Arc<Mutex<HashMap<...>>>` type definition.
- Fix: Collapsed nested `if` into `if age >= max_age && fs::remove_file(&path).is_ok()` and factored complex type into `type MessageStore = ...`.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings` passing with zero warnings.
- Prevention Rule: Combine adjacent conditional filters and define type aliases for complex generic combinations exceeding Clippy thresholds.

## Phase 7 — Clippy too_many_arguments on TransferJob and Progress Constructors
- Mistake: Initial implementations of `TransferJob::new_upload`, `new_download`, `from_db_record`, `ProgressReader::new`, and `ProgressWriter::new` took 8 to 10 parameters, triggering `-D clippy::too_many_arguments`.
- Root Cause: Directly passing all fields as function arguments rather than bundling them into domain parameter structs.
- Fix: Grouped arguments into `UploadJobParams`, `DownloadJobParams`, `DbJobContext`, and `ProgressContext`, providing ergonomic builder patterns with <= 5 constructor parameters.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings` passing with zero warnings across all crates.
- Prevention Rule: Never create constructors with > 6 parameters; introduce typed parameter or builder structs.

## Phase 7 — Cargo Test Log Truncation Misleading Test Count
- Mistake: In the Phase 6 summary report, stdout log truncation sliced the top output lines of `cargo test --workspace` (omitting `televault-backup`, `televault-cli`, `televault-core`), causing an apparent test count difference between Phase 5 report (86 tests) and Phase 6 report (73 tests).
- Root Cause: Relying on the tail portion of terminal logs rather than using `cargo test --workspace -- --list` to enumerate every crate's test binary suite.
- Fix: Audited every test across all 12 crates using `cargo test --workspace -- --list`, proving all Phase 1–5 tests (80 tests) plus Phase 6 tests (19 tests) + placeholder (1 test) were present and 100% active (100 total in Phase 6). With Phase 7's 27 tests replacing the 1 placeholder, workspace test count reached 126 tests.
- Regression Test: `cargo test --workspace -- --list` enumerates all 126 tests.
- Prevention Rule: Always audit test suite inventory with `cargo test --workspace -- --list` to avoid log truncation artifacts.


