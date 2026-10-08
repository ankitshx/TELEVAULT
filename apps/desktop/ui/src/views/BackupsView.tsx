import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  SnapshotDto,
  SnapshotFileDto,
  BackupSummaryDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { Modal } from "../components/Modal";
import { NavTab } from "../types";

interface BackupsViewProps {
  initialProfileId?: string;
  onNavigate: (tab: NavTab, profileId?: string, snapshotId?: string) => void;
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function BackupsView({
  initialProfileId,
  onNavigate,
  onNotify,
}: BackupsViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>(initialProfileId || "");
  const [snapshots, setSnapshots] = useState<SnapshotDto[]>([]);
  const [loadingProfiles, setLoadingProfiles] = useState<boolean>(true);
  const [loadingSnapshots, setLoadingSnapshots] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Backup Execution State
  const [isBackupRunning, setIsBackupRunning] = useState<boolean>(false);
  const [backupModalOpen, setBackupModalOpen] = useState<boolean>(false);
  const [passphrase, setPassphrase] = useState<string>("");
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [backupSummary, setBackupSummary] = useState<BackupSummaryDto | null>(null);

  // Incremental status check
  const [incrementalCheckResult, setIncrementalCheckResult] = useState<boolean | null>(null);
  const [checkingIncremental, setCheckingIncremental] = useState<boolean>(false);

  // Snapshot Files Explorer Modal
  const [filesModalOpen, setFilesModalOpen] = useState<boolean>(false);
  const [selectedSnapshotForFiles, setSelectedSnapshotForFiles] = useState<string | null>(null);
  const [snapshotFiles, setSnapshotFiles] = useState<SnapshotFileDto[]>([]);
  const [loadingFiles, setLoadingFiles] = useState<boolean>(false);

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
      setError(null);
      const res = await commands.listSnapshots(profileId);
      if (res.status === "ok") {
        const sorted = [...res.data].sort((a, b) => b.created_at.localeCompare(a.created_at));
        setSnapshots(sorted);
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoadingSnapshots(false);
    }
  }, []);

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    if (selectedProfileId) {
      loadSnapshots(selectedProfileId);
      setIncrementalCheckResult(null);
    }
  }, [selectedProfileId, loadSnapshots]);

  const selectedProfile = profiles.find((p) => p.profile_id === selectedProfileId);

  const handleCheckIncremental = async () => {
    if (!selectedProfileId) return;
    try {
      setCheckingIncremental(true);
      const res = await commands.isIncrementalBackupNeeded(selectedProfileId);
      if (res.status === "ok") {
        setIncrementalCheckResult(res.data);
        onNotify(
          "info",
          "Incremental Audit",
          res.data
            ? "Files have changed or been added since the last snapshot. Backup recommended."
            : "All files match the latest snapshot catalog. No new changes detected."
        );
      } else {
        onNotify("error", "Incremental Check Failed", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    } finally {
      setCheckingIncremental(false);
    }
  };

  const handleStartBackup = async () => {
    if (!selectedProfileId) return;
    try {
      setIsBackupRunning(true);
      setError(null);
      setBackupSummary(null);

      const jobId = `backup-${selectedProfileId}-${Date.now()}`;
      setActiveJobId(jobId);

      const res = await commands.startBackup({
        profile_id: selectedProfileId,
        passphrase: passphrase.trim() ? passphrase : null,
      });

      if (res.status === "ok") {
        setBackupSummary(res.data);
        onNotify(
          "success",
          "Backup Completed",
          `Created snapshot ${res.data.snapshot_id}. Transferred ${res.data.transferred_chunks} chunks (${(res.data.transferred_bytes / 1024).toFixed(0)} KB).`
        );
        loadSnapshots(selectedProfileId);
      } else {
        setError(res.error.message);
        onNotify("error", "Backup Failed", res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
      onNotify("error", "Backup Execution Error", String(err));
    } finally {
      setIsBackupRunning(false);
      setActiveJobId(null);
      setPassphrase("");
    }
  };

  const handleCancelBackup = async () => {
    if (!activeJobId && !selectedProfileId) return;
    try {
      const opId = activeJobId || selectedProfileId;
      await commands.cancelOperation({ operation_id: opId });
      onNotify("warning", "Cancellation Requested", "Sent cancellation signal to in-process backup pipeline.");
    } catch (err: unknown) {
      onNotify("error", "Cancellation Error", String(err));
    }
  };

  const handleExploreFiles = async (snapshotId: string) => {
    try {
      setSelectedSnapshotForFiles(snapshotId);
      setFilesModalOpen(true);
      setLoadingFiles(true);
      const res = await commands.getSnapshotFiles(snapshotId);
      if (res.status === "ok") {
        setSnapshotFiles(res.data);
      } else {
        onNotify("error", "Error Loading Files", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    } finally {
      setLoadingFiles(false);
    }
  };

  if (loadingProfiles) {
    return <LoadingSpinner message="Loading backup profiles..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Backup Management Error" onDismiss={() => setError(null)} />}

      {/* Top Profile Selection and Actions */}
      <div className="card">
        <div className="card-header" style={{ flexWrap: "wrap", gap: "1rem" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
            <label style={{ fontWeight: 700, color: "var(--text-main)" }}>Select Profile:</label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              style={{ minWidth: "220px" }}
            >
              {profiles.map((p) => (
                <option key={p.profile_id} value={p.profile_id}>
                  {p.name} ({p.profile_id})
                </option>
              ))}
            </select>
          </div>

          <div style={{ display: "flex", gap: "0.5rem" }}>
            <button
              className="btn btn-secondary btn-sm"
              onClick={handleCheckIncremental}
              disabled={checkingIncremental || !selectedProfileId}
              title="Detect file modifications by scanning local source against latest snapshot"
            >
              {checkingIncremental ? "Scanning..." : "🔍 Check Incremental Status"}
            </button>
            <button
              className="btn btn-primary btn-sm"
              onClick={() => setBackupModalOpen(true)}
              disabled={!selectedProfileId || isBackupRunning}
            >
              {isBackupRunning ? "Backup in Progress..." : "▶ Run Backup Now"}
            </button>
          </div>
        </div>

        {selectedProfile && (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
              gap: "1rem",
              background: "var(--bg-card)",
              padding: "1rem",
              borderRadius: "var(--radius-md)",
            }}
          >
            <div>
              <div style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>SOURCE DIRECTORY</div>
              <div className="mono" style={{ fontSize: "0.82rem", color: "var(--text-main)" }}>
                {selectedProfile.source_path}
              </div>
            </div>
            <div>
              <div style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>PROFILE STATUS</div>
              <div>
                <StatusBadge status={selectedProfile.enabled ? "Active" : "Disabled"} />
              </div>
            </div>
            <div>
              <div style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>DESCRIPTION</div>
              <div style={{ fontSize: "0.82rem", color: "var(--text-main)" }}>
                {selectedProfile.description || "None provided"}
              </div>
            </div>
            <div>
              <div style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>INCREMENTAL AUDIT</div>
              <div style={{ fontSize: "0.82rem" }}>
                {incrementalCheckResult === null ? (
                  <span style={{ color: "var(--text-dim)" }}>Not scanned</span>
                ) : incrementalCheckResult ? (
                  <span style={{ color: "var(--warning)" }}>⚠️ Modifications Detected</span>
                ) : (
                  <span style={{ color: "var(--success)" }}>✅ Up to Date</span>
                )}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Snapshots Table */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Historical Snapshots</div>
            <div className="card-subtitle">
              Point-in-time immutable catalog records for profile {selectedProfile?.name}
            </div>
          </div>
          <button
            className="btn btn-secondary btn-sm"
            onClick={() => selectedProfileId && loadSnapshots(selectedProfileId)}
            disabled={loadingSnapshots}
          >
            {loadingSnapshots ? "Refreshing..." : "↻ Refresh Snapshots"}
          </button>
        </div>

        {loadingSnapshots ? (
          <LoadingSpinner message="Loading historical snapshot records..." />
        ) : snapshots.length === 0 ? (
          <EmptyState
            title="No Snapshots Found"
            description="This profile has not produced any completed snapshots yet. Click 'Run Backup Now' above to execute the first backup."
            action={
              <button className="btn btn-primary btn-sm" onClick={() => setBackupModalOpen(true)}>
                Run Initial Backup
              </button>
            }
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Snapshot ID</th>
                  <th>Status</th>
                  <th>Created At</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {snapshots.map((s) => (
                  <tr key={s.snapshot_id}>
                    <td className="mono" style={{ color: "var(--primary)", fontWeight: 600 }}>
                      {s.snapshot_id}
                    </td>
                    <td>
                      <StatusBadge status={s.status} />
                    </td>
                    <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                      {new Date(s.created_at).toLocaleString()}
                    </td>
                    <td>
                      <div style={{ display: "flex", gap: "0.4rem" }}>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => handleExploreFiles(s.snapshot_id)}
                          title="Inspect files included in this snapshot"
                        >
                          Files
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => onNavigate("restore", selectedProfileId, s.snapshot_id)}
                          title="Restore files from this snapshot"
                        >
                          Restore
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => onNavigate("verification", selectedProfileId, s.snapshot_id)}
                          title="Verify cryptographic integrity of this snapshot"
                        >
                          Verify
                        </button>
                        <button
                          className="btn btn-secondary btn-sm"
                          onClick={() => onNavigate("repair", selectedProfileId, s.snapshot_id)}
                          title="Inspect and repair damaged chunks in this snapshot"
                        >
                          Repair
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Start Backup Modal */}
      <Modal
        isOpen={backupModalOpen}
        onClose={() => !isBackupRunning && setBackupModalOpen(false)}
        title="Execute Point-In-Time Backup"
        footer={
          <div style={{ display: "flex", gap: "0.75rem" }}>
            {isBackupRunning ? (
              <button className="btn btn-danger" onClick={handleCancelBackup}>
                ⏹ Cancel Backup
              </button>
            ) : (
              <>
                <button className="btn btn-secondary" onClick={() => setBackupModalOpen(false)}>
                  Close
                </button>
                <button className="btn btn-primary" onClick={handleStartBackup}>
                  ▶ Start Backup
                </button>
              </>
            )}
          </div>
        }
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div>
            <strong>Target Profile:</strong> {selectedProfile?.name} ({selectedProfile?.profile_id})
          </div>
          <div>
            <strong>Source Directory:</strong>{" "}
            <span className="mono" style={{ color: "var(--primary)" }}>
              {selectedProfile?.source_path}
            </span>
          </div>

          <div className="form-group">
            <label>Passphrase (Optional):</label>
            <input
              type="password"
              placeholder="Enter passphrase if key derivation is enabled..."
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              disabled={isBackupRunning}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Secrets are zeroized immediately upon drop and never logged.
            </span>
          </div>

          {isBackupRunning && (
            <div style={{ background: "var(--bg-card)", padding: "1rem", borderRadius: "var(--radius-md)" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
                <div className="spinner" style={{ width: "20px", height: "20px" }} />
                <div>
                  <div style={{ fontWeight: 600 }}>Backup Running in Background...</div>
                  <div style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                    Streaming 64 KiB chunks through transfer queue. UI remains responsive.
                  </div>
                </div>
              </div>
            </div>
          )}

          {backupSummary && (
            <div style={{ background: "var(--bg-card)", padding: "1rem", borderRadius: "var(--radius-md)", border: "1px solid var(--success-border)" }}>
              <div style={{ color: "var(--success)", fontWeight: 700, marginBottom: "0.5rem" }}>
                ✅ Backup Completed Successfully
              </div>
              <div style={{ fontSize: "0.8rem", display: "flex", flexDirection: "column", gap: "0.25rem" }}>
                <div><strong>Snapshot ID:</strong> <span className="mono">{backupSummary.snapshot_id}</span></div>
                <div><strong>New Files:</strong> {backupSummary.new_files}</div>
                <div><strong>Modified Files:</strong> {backupSummary.modified_files}</div>
                <div><strong>Unchanged Files:</strong> {backupSummary.unchanged_files}</div>
                <div><strong>Transferred Chunks:</strong> {backupSummary.transferred_chunks}</div>
                <div><strong>Transferred Bytes:</strong> {(backupSummary.transferred_bytes / (1024 * 1024)).toFixed(2)} MB</div>
                <div><strong>Elapsed Time:</strong> {backupSummary.elapsed_ms} ms</div>
              </div>
            </div>
          )}
        </div>
      </Modal>

      {/* Snapshot Files Explorer Modal */}
      <Modal
        isOpen={filesModalOpen}
        onClose={() => setFilesModalOpen(false)}
        title={`Files in Snapshot: ${selectedSnapshotForFiles || ""}`}
        large={true}
        footer={
          <button className="btn btn-secondary" onClick={() => setFilesModalOpen(false)}>
            Close
          </button>
        }
      >
        {loadingFiles ? (
          <LoadingSpinner message="Retrieving snapshot file manifests..." />
        ) : snapshotFiles.length === 0 ? (
          <EmptyState title="No Files" description="No files recorded for this snapshot." />
        ) : (
          <div className="table-container" style={{ maxHeight: "400px", overflowY: "auto" }}>
            <table>
              <thead>
                <tr>
                  <th>Relative Path</th>
                  <th>Size</th>
                  <th>Status</th>
                  <th>MIME Type</th>
                </tr>
              </thead>
              <tbody>
                {snapshotFiles.map((f) => (
                  <tr key={f.file_id}>
                    <td className="mono" style={{ color: "var(--text-main)" }}>
                      {f.relative_path}
                    </td>
                    <td className="mono">{(f.size_bytes / 1024).toFixed(1)} KB</td>
                    <td>
                      <StatusBadge status={f.status} />
                    </td>
                    <td style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
                      {f.mime_type || "application/octet-stream"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Modal>
    </div>
  );
}
