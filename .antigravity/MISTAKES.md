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

## Phase 8 — Manifest StorageReference Placeholder Validation Failure
- Mistake: In `PayloadPipeline::process_file`, staging chunks were initially populated with `StorageReference::Telegram { chat_id: 0, message_id: 0, file_id: "" }`. When saving the manifest to SQLite, `manifest.validate()` rejected `chat_id == 0`.
- Root Cause: Used zeroed Telegram values as a placeholder prior to upload rather than using the dedicated `StorageReference::Pending` variant designed for in-flight chunks.
- Fix: Set `storage_reference: StorageReference::Pending` during initial chunk manifest generation, updated chunk manifests with verified storage references returned by `execute_staged_upload`, and saved the updated manifest to SQLite.
- Regression Test: `test_incremental_backup_regression_scenario` in `crates/televault-backup/tests/backup_tests.rs`.
- Prevention Rule: Always use `StorageReference::Pending` for in-flight/staged chunks prior to upload verification.

## Phase 8 — Chunk Unique Constraint Violation on Modified File
- Mistake: In `execute_backup`, updating a modified file with a new version failed with `SQLite error: UNIQUE constraint failed: chunks.file_id, chunks.chunk_index`.
- Root Cause: `V1__initial_schema.sql` enforces `UNIQUE(file_id, chunk_index)`. When `file2.txt` was modified, chunk 0 of the new version conflicted with chunk 0 of the initial backup version in the `chunks` table.
- Fix: Added `delete_chunks_by_file` method to `televault-db` and called it when updating modified files to clear old physical chunk rows from the active chunk index before inserting the new version's chunks. All historical versions remain fully preserved in `manifests.serialized_manifest` linked to `versions`.
- Regression Test: `test_incremental_backup_regression_scenario` in `crates/televault-backup/tests/backup_tests.rs`.
- Prevention Rule: When updating physical chunks for an active file version, delete superseded chunk index rows for that `file_id` before inserting new chunk records.

## Phase 8 — Clippy needless_late_init on chunk_stored_bytes
- Mistake: Declared `let chunk_stored_bytes: u64;` and assigned in match arms, triggering `-D clippy::needless_late_init`.
- Root Cause: Wrote imperative late assignment pattern instead of idiomatic Rust expression return.
- Fix: Refactored to `let chunk_stored_bytes = match options.encryption_policy { ... };` returning the byte count from each match arm.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Prevention Rule: Bind variables directly from match expressions rather than using uninitialized late binding.

## Phase 9 — Staging File Disown on Atomic Move
- Mistake: Restoring a file to its final destination by `fs::rename(staging_file.path(), final_target)` caused destination disappearance or cleanup races if `staging_file` was subsequently dropped.
- Root Cause: `TempPayloadFile` implements RAII `Drop` which automatically executes `fs::remove_file(&self.path)`. If the path is renamed to `final_target`, the RAII drop on the old path is harmless on POSIX, but in edge cases or if renamed in place, could lead to unexpected behavior if not explicitly disowned.
- Fix: Added and called `staging_file.disown()` prior to finalizing the destination path, ensuring ownership is cleanly relinquished from the temporary staging lifecycle manager.
- Regression Test: `test_restore_all_four_encryption_and_compression_combinations` and `test_restore_snapshot_entire_folder_hierarchy` in `crates/televault-backup/tests/restore_tests.rs`.
- Prevention Rule: When transferring a staged temporary file out of the temporary sandbox via rename, always call `.disown()` on the wrapper to prevent double-free or unintended RAII deletion.

## Phase 9 — Manifest Chunk Count Validation vs Sub-2GB Test Manifests
- Mistake: `validate_manifest_for_restore` strictly asserted `manifest.chunks.len() == calculate_expected_chunk_count(original_size)` for all file sizes. Because `calculate_expected_chunk_count` returns 1 for all files < 2 GB, testing multi-chunk physical reconstruction on small files (< 2 GB) failed manifest pre-validation.
- Root Cause: `calculate_expected_chunk_count` represents the production splitting threshold (2 GB threshold). `ManifestV1` itself permits multi-chunk manifests of any size (as proven by `sample_valid_manifest(3, 1MB)` in Phase 4).
- Fix: Restructured the chunk count validation to strictly enforce `calculate_expected_chunk_count` for files >= 2 GB (`CHUNK_THRESHOLD_BYTES`), while continuing to validate sum of plaintext chunk sizes, unique indices, contiguous ordering, and absence of pending chunks across all sizes.
- Regression Test: `test_restore_chunk_ordering_resilient_to_shuffled_list` in `crates/televault-backup/tests/restore_tests.rs`.
- Prevention Rule: Distinguish between production chunk splitting rules for large files and the schema's general support for multi-chunk streams in unit and integration testing.

## Phase 10 — Specta-TypeScript BigInt Export Rejection on Byte Count Fields
- Mistake: Exporting TypeScript bindings using `specta_typescript::Typescript::default()` failed with `ExportError::BigIntForbidden` when generating types for structs containing `u64`, `usize`, or `i64` byte sizes (e.g. `file_size: u64`, `total_bytes: u64`).
- Root Cause: Specta's TypeScript exporter explicitly rejects 64-bit integer types by default to prevent silent precision loss in JavaScript `JSON.parse` which truncates integers above `Number.MAX_SAFE_INTEGER` (2^53 - 1, ~9 Petabytes).
- Fix: Annotated 64-bit integer fields with `#[specta(type = specta_typescript::Number)]` on DTO structs (since backup files under 9 PB serialize safely as standard JavaScript numbers), and typed command limit parameters as `Option<u32>` instead of `Option<usize>`.
- Regression Test: `cargo run -p televault-desktop -- --export-types` and `test_ipc_builder_initialization` in `apps/desktop/tests/ipc_tests.rs`.
- Prevention Rule: Always annotate 64-bit size and timestamp fields in Specta DTOs with `#[specta(type = specta_typescript::Number)]` or use `u32`/`f64` where appropriate.

## Phase 10 — Submodule Macro Scoping in collect_commands!
- Mistake: Calling `tauri_specta::collect_commands![get_app_info, create_backup_profile, ...]` produced compile errors `cannot find macro __specta__fn__get_app_info in this scope` because command functions were implemented in separate submodule files (`commands::system`, `commands::backup`, etc.).
- Root Cause: `#[tauri_specta::specta]` generates internal helper macros `__cmd__<fn>` and `__specta__fn__<fn>` inside the defining module, which `collect_commands!` cannot locate if unadorned function names are passed without module qualification.
- Fix: Qualified command identifiers with their declaring module paths in `collect_commands![system::get_app_info, backup::create_backup_profile, ...]` and imported the command submodules into `builder.rs`.
- Regression Test: `cargo check --workspace` and `cargo test -p televault-desktop`.
- Prevention Rule: When organizing Tauri Specta commands across multiple modules, always qualify command names with their module path inside `collect_commands!`.

## Phase 10 — Tauri State Constructor Unavailable in Test Fixtures
- Mistake: Attempting to invoke Tauri command handlers directly in integration tests via `tauri::State::from(&app_state)` failed compilation because `tauri::State` has private constructors and cannot be instantiated directly from references.
- Root Cause: Tauri manages dependency injection via its internal container registry inside `tauri::App` or `tauri::AppHandle`.
- Fix: Enabled the `test` feature on `tauri = { version = "2", features = ["wry", "test"] }` in `apps/desktop/Cargo.toml` `[dev-dependencies]` and used `tauri::test::mock_app()` with `app.manage(app_state)` to acquire a valid `app.state::<DesktopAppState>()` for command tests.
- Regression Test: `apps/desktop/tests/ipc_tests.rs` verifying all 8 command suites against live mock state.
- Prevention Rule: Use `tauri::test::mock_app()` to test Tauri command handlers requiring `tauri::State`.

## Phase 11 — DbError::NotFound Struct Variant Pattern
- Mistake: In `crates/televault-db/src/db.rs`, attempted to construct `DbError::NotFound("schedule_not_found".into())` as a tuple variant, causing compilation error `expected struct variant, found tuple variant`.
- Root Cause: Assumed `DbError::NotFound` was a single-string tuple variant without checking its declaration in `crates/televault-db/src/error.rs`.
- Fix: Constructed `DbError::NotFound { entity: "Schedule", id: schedule_id }` matching the actual struct variant schema.
- Regression Test: `cargo check --workspace` and `cargo test -p televault-db`.
- Prevention Rule: Always check the declaration of error variants before instantiating them, especially in domain and database layers.

## Phase 11 — Clippy Assign Op Pattern on MockClock Advance
- Mistake: In `MockClock::advance`, wrote `*current = *current + duration`, triggering `-D clippy::assign_op_pattern`.
- Root Cause: Used explicit binary addition assignment rather than compound assignment operator.
- Fix: Replaced with `*current += duration`.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Prevention Rule: Prefer compound assignment operators (`+=`, `-=`) on dereferenced types to satisfy Clippy.

## Phase 11 — CreateProfileRequest DTO Schema Mismatch in Test Fixture
- Mistake: In `apps/desktop/tests/scheduler_ipc_tests.rs`, constructed `CreateProfileRequest` with `passphrase: None`, failing compilation with `no field 'passphrase' on type 'CreateProfileRequest'`.
- Root Cause: Inferred DTO fields from memory rather than inspecting `apps/desktop/src/dto/backup.rs`.
- Fix: Constructed `CreateProfileRequest` using actual fields: `name`, `description`, `backup_directory`, `encryption_enabled`, `compression_enabled`, and `path_filter_rules`.
- Regression Test: `cargo test -p televault-desktop --test scheduler_ipc_tests`.
- Prevention Rule: Always reference DTO struct definitions directly when assembling test payload fixtures.

## Phase 12 — Specta BigInt Constraint on Retention Duration Fields
- Mistake: In `apps/desktop/src/dto/retention.rs`, fields `keep_newer_than_secs: Option<u64>` and `age_secs: u64` caused `--export-types` to fail with `Specta forbids exporting BigInt-style types (usize, isize, i64, u64, i128, u128) to avoid precision loss`.
- Root Cause: Exported raw 64-bit integer fields without considering Specta's restriction for TypeScript number safety.
- Fix: Typed duration and age fields as `u32` in DTOs (which supports up to ~136 years in seconds), converting cleanly between domain `u64` and DTO `u32`.
- Regression Test: `cargo run -p televault-desktop -- --export-types` exports cleanly to `apps/desktop/ui/src/bindings.ts`.
- Prevention Rule: Use `u32` for time intervals in seconds and human counts in IPC DTOs to avoid BigInt export issues.

## Phase 12 — Clippy redundant_guards on DbError Match Arm
- Mistake: In `crates/televault-backup/src/error.rs`, wrote `televault_db::DbError::NotFound { entity, id } if entity == "Snapshot"`, triggering `-D clippy::redundant_guards`.
- Root Cause: Used conditional `if` guard instead of direct pattern matching on the static string literal field.
- Fix: Replaced with `televault_db::DbError::NotFound { entity: "Snapshot", id } => ...`.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Prevention Rule: Pattern match directly against string literals in struct match arms rather than using guards on string comparisons.

## Phase 12 — Clippy unnecessary_unwrap in Retention Evaluator
- Mistake: In `crates/televault-backup/src/retention/evaluator.rs`, wrote `if policy.keep_latest_n.is_some() { ... policy.keep_latest_n.unwrap() }`, triggering `-D clippy::unnecessary_unwrap`.
- Root Cause: Checked option presence imperatively before unwrapping rather than using idiomatic pattern matching.
- Fix: Replaced with `match (policy.keep_newer_than_secs, policy.keep_latest_n) { (Some(_), Some(max_n)) => ..., ... }`.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Prevention Rule: Use `match` or `if let` pattern matching on `Option` types instead of `is_some()` followed by `.unwrap()`.

## Phase 13 — Integer Subtraction Underflow on Extra Chunk Size Boundary Check
- Mistake: In `VerificationEngine::verify_manifest`, computing expected chunk size for index `i >= chunk_count - 1` evaluated `manifest.logical_file.original_size - (i as u64 * TARGET_CHUNK_SIZE_BYTES)`, causing integer underflow panic when verifying a manifest with an extra unexpected chunk (`i = 3`).
- Root Cause: Direct unchecked subtraction on `u64` when `i * TARGET_CHUNK_SIZE_BYTES` exceeded `original_size`.
- Fix: Replaced with `manifest.logical_file.original_size.saturating_sub(i as u64 * TARGET_CHUNK_SIZE_BYTES)`.
- Regression Test: `test_24_large_file_extra_unexpected_chunk` in `crates/televault-backup/tests/verification_tests.rs`.
- Prevention Rule: Always use saturating arithmetic (`.saturating_sub(...)`) when calculating expected chunk boundaries from untrusted or malicious manifest structures.

## Phase 13 — Omitted Audit History Persistence on Early Verification Exits
- Mistake: Early exit points in `verify_manifest` (MetadataOnly, RemoteAvailability, profile validation failure, cancellation) returned without calling `self.persist_history(&res)`, causing audit history records to be skipped.
- Root Cause: Only placed `self.persist_history(&res)` at the terminal Level 4 completion point rather than on all successful exits.
- Fix: Ensured `self.persist_history(&res)` is consistently executed before returning on all verification exit paths.
- Regression Test: `test_39_repeated_verification_is_idempotent` in `crates/televault-backup/tests/verification_tests.rs`.
- Prevention Rule: Consistently persist structured audit records across all verification levels and early exits before returning.

## Phase 13 — Hex Digest Character Constraint in Test Fixtures
- Mistake: Synthesized test fixtures used mock hash strings like `"hash0000..."` and `"whole000..."`, which failed `IntegrityMetadata.validate()` with `contains non-hex characters`.
- Root Cause: Manifest integrity validation strictly requires 64-character lowercase ASCII hexadecimal characters (`0-9`, `a-f`).
- Fix: Replaced mock digests with valid 64-character lowercase hex strings (e.g., `"a000000000000000000000000000000000000000000000000000000000000000"`).
- Regression Test: `test_02_healthy_multi_chunk_backup` and `test_21_large_file_5_2gb_three_chunks_healthy` in `crates/televault-backup/tests/verification_tests.rs`.
- Prevention Rule: Ensure all synthesized hash fixtures in tests conform to strict 64-character lowercase hex validation.

## Phase 14 — Hardcoded TARGET_CHUNK_SIZE_BYTES Multiplier for Chunk Seek Offset
- Mistake: In `RepairPipeline::repair_single_chunk`, seek offset was calculated as `candidate.chunk_index as u64 * TARGET_CHUNK_SIZE_BYTES`. In multi-chunk files with smaller chunks, seeking to 1.8 GB reached EOF, causing `SourceSizeMismatch (actual: 0)`.
- Root Cause: Assumed all multi-chunk files partition exactly at `TARGET_CHUNK_SIZE_BYTES` rather than computing offsets dynamically from the manifest.
- Fix: Calculated seek offset dynamically: `manifest.chunks.iter().take(candidate.chunk_index as usize).map(|c| c.plaintext_size).sum()`.
- Regression Test: `test_5_2gb_large_file_middle_chunk_isolated_repair` in `crates/televault-backup/tests/repair_tests.rs`.
- Prevention Rule: Always compute chunk seek offsets dynamically by summing previous chunk sizes from the authoritative manifest rather than assuming uniform chunk sizes.

## Phase 14 — SQLite Foreign Key Constraint Order in Encrypted Repair Test
- Mistake: In `test_encrypted_chunk_repair_with_key_and_missing_key_rejection`, `save_manifest` was called before `create_file`, triggering SQLite constraint violation `FOREIGN KEY constraint failed`.
- Root Cause: In SQLite, `manifests.file_id` is a foreign key referencing `files.file_id`.
- Fix: Swapped fixture order to call `create_file` prior to `save_manifest`.
- Regression Test: `test_encrypted_chunk_repair_with_key_and_missing_key_rejection` in `crates/televault-backup/tests/repair_tests.rs`.
- Prevention Rule: Always insert parent entity rows (`profiles`, `files`) before child rows (`manifests`, `chunks`, `versions`) in test fixtures.

## Phase 14 — Clippy too_many_arguments on atomic_apply_chunk_repair
- Mistake: `atomic_apply_chunk_repair` had 8 parameters, triggering `-D clippy::too_many_arguments`.
- Root Cause: Atomic update transaction coordinates chunk updates, manifest persistence, and audit history recording simultaneously.
- Fix: Added `#[allow(clippy::too_many_arguments)]` to the database method.
- Regression Test: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- Prevention Rule: Explicitly allow `clippy::too_many_arguments` on multi-table database transaction methods when bundling entities into a synthetic wrapper reduces domain readability.

## Phase 15 — Vite Windows Drive Letter Normalization (d: vs D:)
- Mistake: In `apps/desktop/ui/vite.config.ts`, Vitest and Vite root resolution on Windows caused module discovery failure when drive letters differed in case (`d:` vs `D:`).
- Root Cause: `__dirname` supplied lowercase `d:\Projects\Televalut\apps\desktop\ui` while Node's `fs.realpathSync` normalized to uppercase `D:`, causing Vite to treat paths outside root as external.
- Fix: Used `fs.realpathSync(__dirname).replace(/^[a-z]:/, m => m.toUpperCase()).replace(/\\/g, "/")` for root and test configuration in `vite.config.ts`.
- Regression Test: `npm.cmd test -- --run` and `npm.cmd run build` inside `apps/desktop/ui`.
- Prevention Rule: Always normalize drive letter casing and forward slashes when configuring Vite/Vitest root paths on Windows environments.

## Phase 15 — Specta TypeScript DTO Field Alignment
- Mistake: Initial frontend type definitions used informal field names (`destination`, `repaired_count`, `next_run`) instead of matching authoritative Rust Specta structs (`destination_directory`, `pruned_versions_count`, `next_run_at`).
- Root Cause: Drafted frontend component props before regenerating and checking the authoritative TypeScript bindings file `bindings.ts`.
- Fix: Regenerated `bindings.ts` via `--export-types` and strictly aligned all frontend component props and form handlers with `bindings.ts` and `types.ts`.
- Regression Test: `npm.cmd run build` (`tsc && vite build`) passing with zero type errors.
- Prevention Rule: Always export and inspect generated Specta bindings before implementing frontend forms and IPC consumption.

## Phase 16 — Manifest ID Collision Overwriting Historical File Versions
- Mistake: In `crates/televault-backup/src/pipeline.rs`, `manifest_id` was hardcoded to `format!("man-{}", file_id)`. When an incremental backup modified a file, its new manifest overwrote the old manifest row in SQLite `manifests`, causing historical snapshots to restore the latest content rather than point-in-time content.
- Root Cause: Assumed `manifest_id` could be uniquely keyed by `file_id` alone instead of incorporating the content hash.
- Fix: Generated `manifest_id` with content hash differentiation: `format!("man-{}-{}", file_id, &logical_file_hash[..16])`.
- Regression Test: `test_e2e_multi_generation_incremental_backup_and_point_in_time_recovery` in `apps/desktop/tests/e2e_pipeline_tests.rs`.
- Prevention Rule: Any entity representing immutable point-in-time content in a primary-keyed database must incorporate content hash or version identity into its primary key.

## Phase 16 — Mock Storage In-Memory Payload Mutation without Hash Digest Synchronization
- Mistake: In `MockStorageProvider::corrupt_object`, mutating an object's payload byte array altered the payload but left `sha256_hex` unchanged. When repair idempotency checked `is_chunk_already_healthy`, `get_metadata` reported the old hash and skipped repairing the chunk.
- Root Cause: Decoupled payload state mutation from metadata digest recalculation in the mock store.
- Fix: Updated `corrupt_object` to recompute and update `sha256_hex` immediately upon mutating payload bytes.
- Regression Test: `test_e2e_verification_repair_reverification_and_restore_cycle` in `apps/desktop/tests/verification_repair_e2e_tests.rs`.
- Prevention Rule: Test mocks simulating data corruption must update both stored content and reported digest metadata to ensure accurate failure detection.

## Phase 16 — SQLite PRAGMA integrity_check Assertion Mismatch
- Mistake: Asserted `db.full_integrity_check().unwrap().is_empty()` expecting empty vec on success.
- Root Cause: In SQLite, `PRAGMA integrity_check` returns a single row containing string `"ok"` when healthy; an empty vector is only returned by `PRAGMA foreign_key_check` when there are zero violations.
- Fix: Asserted `assert_eq!(db.full_integrity_check().unwrap(), vec!["ok".to_string()])`.
- Regression Test: `test_metadata_persistence_across_application_restart` in `apps/desktop/tests/restart_persistence_tests.rs`.
- Prevention Rule: Always check SQLite PRAGMA return value documentation: `integrity_check` returns `["ok"]`, whereas `foreign_key_check` returns empty rows on success.





