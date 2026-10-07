# KNOWN FAILURES & RISK MITIGATIONS

## KF-001: Interrupted Multi-Chunk Upload Leading to Incomplete Manifest
- Risk: A multi-chunk upload (e.g. 5.2 GB file across 3 chunks) fails or is cancelled after chunk 0 and 1 upload, leaving chunk 2 unuploaded.
- Failure Mode: If a restore engine attempts to reconstruct the file from the incomplete manifest, it could write a partial/truncated file.
- Protection in Phase 9: Manifest pre-validation (`validate_manifest_for_restore`) strictly checks that NO chunk has `StorageReference::Pending`, verifies expected chunk counts against logical size, and asserts all chunk references are present and verified before any download commences. Incomplete manifests are rejected upfront with `RestoreError::PendingChunk` or `RestoreError::IncompleteBackup`.

## KF-002: Destination Collision Silently Overwriting User Files
- Risk: Restoring to an existing file path could overwrite newer local modifications if not explicitly confirmed.
- Failure Mode: Data loss of current local files.
- Protection in Phase 9: Explicit `CollisionPolicy` (`Overwrite`, `Skip`, `KeepBoth`). By default, collision resolution provides deterministic alternate file names (`file (1).ext`, `file (2).ext`) or skips restoration without modifying the target. `Overwrite` is only permitted when explicitly selected by the user.
