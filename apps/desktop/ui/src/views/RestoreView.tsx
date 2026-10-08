import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  SnapshotDto,
  SnapshotFileDto,
  RestoreResultDto,
  SnapshotRestoreResultDto,
  ManifestVerificationReportDto,
  CollisionPolicyDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";

interface RestoreViewProps {
  initialProfileId?: string;
  initialSnapshotId?: string;
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function RestoreView({
  initialProfileId,
  initialSnapshotId,
  onNotify,
}: RestoreViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>(initialProfileId || "");
  const [snapshots, setSnapshots] = useState<SnapshotDto[]>([]);
  const [selectedSnapshotId, setSelectedSnapshotId] = useState<string>(initialSnapshotId || "");
  const [snapshotFiles, setSnapshotFiles] = useState<SnapshotFileDto[]>([]);
  const [selectedFileId, setSelectedFileId] = useState<string>("");

  // Mode: "snapshot" (restore full snapshot) or "file" (restore individual file)
  const [restoreMode, setRestoreMode] = useState<"snapshot" | "file">("snapshot");
  const [destinationPath, setDestinationPath] = useState<string>("");
  const [collisionPolicy, setCollisionPolicy] = useState<CollisionPolicyDto>("keep_both");
  const [passphrase, setPassphrase] = useState<string>("");

  // Execution & Progress State
  const [loadingProfiles, setLoadingProfiles] = useState<boolean>(true);
  const [loadingSnapshots, setLoadingSnapshots] = useState<boolean>(false);
  const [loadingFiles, setLoadingFiles] = useState<boolean>(false);
  const [isRestoring, setIsRestoring] = useState<boolean>(false);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Results
  const [fileRestoreResult, setFileRestoreResult] = useState<RestoreResultDto | null>(null);
  const [snapshotRestoreResult, setSnapshotRestoreResult] = useState<SnapshotRestoreResultDto | null>(null);
  const [manifestReport, setManifestReport] = useState<ManifestVerificationReportDto | null>(null);

  const loadProfiles = useCallback(async () => {
    try {
      setLoadingProfiles(true);
      setError(null);
      const res = await commands.listBackupProfiles();
      if (res.status === "ok") {
        setProfiles(res.data);
        if (res.data.length > 0 && !selectedProfileId) {
          setSelectedProfileId(res.data[0].profile_id);
        }
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoadingProfiles(false);
    }
  }, [selectedProfileId]);

  const loadSnapshots = useCallback(async (profileId: string) => {
    if (!profileId) return;
    try {
      setLoadingSnapshots(true);
      const res = await commands.listSnapshots(profileId);
      if (res.status === "ok") {
        const sorted = [...res.data].sort((a, b) => b.created_at.localeCompare(a.created_at));
        setSnapshots(sorted);
        if (sorted.length > 0 && !selectedSnapshotId) {
          setSelectedSnapshotId(sorted[0].snapshot_id);
        }
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoadingSnapshots(false);
    }
  }, [selectedSnapshotId]);

  const loadSnapshotFiles = useCallback(async (snapshotId: string) => {
    if (!snapshotId) return;
    try {
      setLoadingFiles(true);
      const res = await commands.getSnapshotFiles(snapshotId);
      if (res.status === "ok") {
        setSnapshotFiles(res.data);
        if (res.data.length > 0) {
          setSelectedFileId(res.data[0].file_id);
        }
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoadingFiles(false);
    }
  }, []);

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    if (selectedProfileId) {
      loadSnapshots(selectedProfileId);
    }
  }, [selectedProfileId, loadSnapshots]);

  useEffect(() => {
    if (selectedSnapshotId) {
      loadSnapshotFiles(selectedSnapshotId);
    }
  }, [selectedSnapshotId, loadSnapshotFiles]);

  const handlePrevalidateManifest = async () => {
    if (!selectedFileId && snapshotFiles.length > 0) return;
    const targetFile = snapshotFiles.find((f) => f.file_id === selectedFileId) || snapshotFiles[0];
    if (!targetFile) return;

    try {
      setError(null);
      setManifestReport(null);
      const res = await commands.verifyManifestMetadata({
        file_id: targetFile.file_id,
      });

      if (res.status === "ok") {
        setManifestReport(res.data);
        onNotify(
          res.data.is_restorable ? "success" : "warning",
          "Manifest Pre-validation",
          res.data.is_restorable
            ? `Manifest metadata valid. Total chunks: ${res.data.total_chunks}, logical size: ${(res.data.original_size / 1024).toFixed(1)} KB`
            : `Pre-validation warnings detected: ${res.data.issues.join(", ")}`
        );
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    }
  };

  const handleExecuteRestore = async () => {
    if (!destinationPath.trim()) {
      setError("Please specify a valid local destination directory or path.");
      return;
    }

    try {
      setIsRestoring(true);
      setError(null);
      setFileRestoreResult(null);
      setSnapshotRestoreResult(null);

      const jobId = `restore-${Date.now()}`;
      setActiveJobId(jobId);

      if (restoreMode === "snapshot") {
        if (!selectedSnapshotId) {
          throw new Error("No snapshot selected for full restore.");
        }
        const res = await commands.restoreSnapshot({
          snapshot_id: selectedSnapshotId,
          destination_directory: destinationPath.trim(),
          collision_policy: collisionPolicy,
          passphrase: passphrase.trim() ? passphrase : null,
        });

        if (res.status === "ok") {
          setSnapshotRestoreResult(res.data);
          onNotify(
            "success",
            "Snapshot Restored",
            `Restored ${res.data.restored_files} files (${(res.data.total_bytes / (1024 * 1024)).toFixed(2)} MB) to destination.`
          );
        } else {
          setError(res.error.message);
        }
      } else {
        const fileObj = snapshotFiles.find((f) => f.file_id === selectedFileId);
        if (!fileObj) {
          throw new Error("No file selected for restore.");
        }

        const res = await commands.restoreFile({
          file_id: fileObj.file_id,
          destination_path: destinationPath.trim(),
          collision_policy: collisionPolicy,
          passphrase: passphrase.trim() ? passphrase : null,
        });

        if (res.status === "ok") {
          setFileRestoreResult(res.data);
          onNotify(
            "success",
            "File Restored",
            `File restored to ${res.data.target_path} (${(res.data.bytes_restored / 1024).toFixed(1)} KB). Outcome: ${res.data.outcome}`
          );
        } else {
          setError(res.error.message);
        }
      }
    } catch (err: unknown) {
      setError(typeof err === "object" && err !== null && "message" in err ? (err as Error).message : String(err));
    } finally {
      setIsRestoring(false);
      setActiveJobId(null);
    }
  };

  const handleCancelRestore = async () => {
    if (!activeJobId) return;
    try {
      await commands.cancelOperation({ operation_id: activeJobId });
      onNotify("warning", "Cancellation Sent", "Restore operation cancellation requested.");
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    }
  };

  if (loadingProfiles) {
    return <LoadingSpinner message="Loading profiles and restore catalog..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Restore Engine Error" onDismiss={() => setError(null)} />}

      {/* Target Selection Card */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">1. Select Restore Target</div>
            <div className="card-subtitle">Choose the backup snapshot or file version to reconstruct</div>
          </div>
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label>Backup Profile:</label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              disabled={isRestoring}
            >
              {profiles.map((p) => (
                <option key={p.profile_id} value={p.profile_id}>
                  {p.name} ({p.profile_id})
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label>Historical Snapshot:</label>
            <select
              value={selectedSnapshotId}
              onChange={(e) => setSelectedSnapshotId(e.target.value)}
              disabled={isRestoring || loadingSnapshots}
            >
              {snapshots.map((s) => (
                <option key={s.snapshot_id} value={s.snapshot_id}>
                  {s.snapshot_id.slice(0, 16)}... ({new Date(s.created_at).toLocaleDateString()})
                </option>
              ))}
            </select>
          </div>
        </div>

        <div style={{ display: "flex", gap: "1.5rem", marginTop: "0.5rem" }}>
          <label className="checkbox-label">
            <input
              type="radio"
              name="restoreMode"
              checked={restoreMode === "snapshot"}
              onChange={() => setRestoreMode("snapshot")}
              disabled={isRestoring}
            />
            <span>Restore Entire Snapshot Directory Hierarchy</span>
          </label>
          <label className="checkbox-label">
            <input
              type="radio"
              name="restoreMode"
              checked={restoreMode === "file"}
              onChange={() => setRestoreMode("file")}
              disabled={isRestoring}
            />
            <span>Restore Single File</span>
          </label>
        </div>

        {restoreMode === "file" && (
          <div className="form-group" style={{ marginTop: "0.5rem" }}>
            <label>Select File from Snapshot:</label>
            {loadingFiles ? (
              <span style={{ fontSize: "0.78rem", color: "var(--text-dim)" }}>Loading snapshot files...</span>
            ) : snapshotFiles.length === 0 ? (
              <span style={{ fontSize: "0.78rem", color: "var(--text-dim)" }}>No files in selected snapshot.</span>
            ) : (
              <select
                value={selectedFileId}
                onChange={(e) => setSelectedFileId(e.target.value)}
                disabled={isRestoring}
              >
                {snapshotFiles.map((f) => (
                  <option key={f.file_id} value={f.file_id}>
                    {f.relative_path} ({(f.size_bytes / 1024).toFixed(1)} KB)
                  </option>
                ))}
              </select>
            )}
          </div>
        )}
      </div>

      {/* Destination & Collision Policy */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">2. Destination & Collision Policy</div>
            <div className="card-subtitle">Configure destination folder, collision handling, and decryption</div>
          </div>
          {restoreMode === "file" && snapshotFiles.length > 0 && (
            <button
              className="btn btn-secondary btn-sm"
              onClick={handlePrevalidateManifest}
              disabled={!selectedFileId || isRestoring}
              title="Fast pre-check of manifest headers without downloading payload bytes"
            >
              Pre-validate Manifest
            </button>
          )}
        </div>

        <div className="form-group">
          <label>Local Destination Path:</label>
          <input
            type="text"
            placeholder={
              restoreMode === "snapshot"
                ? "e.g. D:\\RestoredBackups\\Snapshot_01"
                : "e.g. D:\\RestoredBackups\\document.pdf"
            }
            value={destinationPath}
            onChange={(e) => setDestinationPath(e.target.value)}
            disabled={isRestoring}
          />
          <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
            Temporary staging file is verified before atomic commit to destination.
          </span>
        </div>

        <div className="grid-2" style={{ marginTop: "0.5rem" }}>
          <div className="form-group">
            <label>Collision Resolution Policy:</label>
            <select
              value={collisionPolicy}
              onChange={(e) => setCollisionPolicy(e.target.value as any)}
              disabled={isRestoring}
            >
              <option value="keep_both">keep_both — Preserve existing and append safe increment suffix</option>
              <option value="overwrite">overwrite — Atomically overwrite existing destination file</option>
              <option value="skip">skip — Leave existing file unchanged and skip restore</option>
            </select>
          </div>

          <div className="form-group">
            <label>Passphrase (If Encrypted Profile):</label>
            <input
              type="password"
              placeholder="Passphrase for key derivation..."
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              disabled={isRestoring}
            />
          </div>
        </div>

        {/* Pre-validation Report if available */}
        {manifestReport && (
          <div
            style={{
              marginTop: "0.75rem",
              padding: "0.85rem",
              borderRadius: "var(--radius-md)",
              backgroundColor: manifestReport.is_restorable ? "var(--bg-card)" : "var(--warning-bg)",
              border: `1px solid ${manifestReport.is_restorable ? "var(--border-subtle)" : "var(--warning-border)"}`,
            }}
          >
            <div style={{ fontWeight: 600, display: "flex", alignItems: "center", gap: "0.5rem" }}>
              <StatusBadge status={manifestReport.is_restorable ? "Healthy" : "Warning"} />
              <span>Manifest Pre-Validation Report</span>
            </div>
            <div style={{ fontSize: "0.78rem", marginTop: "0.3rem", display: "flex", gap: "1.5rem" }}>
              <span>Total Chunks: <strong>{manifestReport.total_chunks}</strong></span>
              <span>Logical Size: <strong>{(manifestReport.original_size / 1024).toFixed(1)} KB</strong></span>
              <span>Remote Objects Verified: <strong>{manifestReport.remote_objects_verified ? "Yes" : "Pending"}</strong></span>
            </div>
            {manifestReport.issues.length > 0 && (
              <div style={{ fontSize: "0.75rem", color: "var(--warning)", marginTop: "0.3rem" }}>
                Issues: {manifestReport.issues.join("; ")}
              </div>
            )}
          </div>
        )}

        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.75rem", marginTop: "0.5rem" }}>
          {isRestoring ? (
            <button className="btn btn-danger" onClick={handleCancelRestore}>
              ⏹ Cancel Restore
            </button>
          ) : (
            <button
              className="btn btn-primary"
              onClick={handleExecuteRestore}
              disabled={!destinationPath.trim() || isRestoring}
            >
              ▶ Execute Restore
            </button>
          )}
        </div>
      </div>

      {/* Restore Execution Result Card */}
      {(fileRestoreResult || snapshotRestoreResult || isRestoring) && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">3. Restore Execution Result</div>
          </div>

          {isRestoring ? (
            <div style={{ display: "flex", alignItems: "center", gap: "1rem", padding: "1rem" }}>
              <div className="spinner" />
              <div>
                <div style={{ fontWeight: 600 }}>Streaming Chunks from Remote Storage...</div>
                <div style={{ fontSize: "0.78rem", color: "var(--text-muted)" }}>
                  Operating within bounded 64 KiB buffers with zero full-payload RAM loading.
                </div>
              </div>
            </div>
          ) : fileRestoreResult ? (
            <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                <StatusBadge status={fileRestoreResult.outcome === "failed" ? "Failed" : "Completed"} />
                <span style={{ fontWeight: 600 }}>File Restore Completed</span>
              </div>
              <div style={{ fontSize: "0.82rem" }}>
                <div><strong>Destination:</strong> <span className="mono">{fileRestoreResult.target_path}</span></div>
                <div><strong>Bytes Restored:</strong> {(fileRestoreResult.bytes_restored / 1024).toFixed(1)} KB</div>
                <div><strong>Outcome:</strong> {fileRestoreResult.outcome}</div>
                <div><strong>Duration:</strong> {fileRestoreResult.duration_ms} ms</div>
              </div>
            </div>
          ) : snapshotRestoreResult ? (
            <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                <StatusBadge status="Completed" />
                <span style={{ fontWeight: 600 }}>Snapshot Restore Completed</span>
              </div>
              <div style={{ fontSize: "0.82rem" }}>
                <div><strong>Target Directory:</strong> <span className="mono">{snapshotRestoreResult.target_directory}</span></div>
                <div><strong>Restored Files:</strong> {snapshotRestoreResult.restored_files} / {snapshotRestoreResult.total_files}</div>
                <div><strong>Skipped Files:</strong> {snapshotRestoreResult.skipped_files}</div>
                <div><strong>Failed Files:</strong> {snapshotRestoreResult.failed_files}</div>
                <div><strong>Total Bytes:</strong> {(snapshotRestoreResult.total_bytes / (1024 * 1024)).toFixed(2)} MB</div>
                <div><strong>Duration:</strong> {snapshotRestoreResult.duration_ms} ms</div>
              </div>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}
