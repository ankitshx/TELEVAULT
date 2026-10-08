import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  SnapshotDto,
  RepairPreviewDto,
  RepairExecutionResultDto,
  RepairHistoryRecordDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";

interface RepairViewProps {
  initialProfileId?: string;
  initialSnapshotId?: string;
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function RepairView({
  initialProfileId,
  initialSnapshotId,
  onNotify,
}: RepairViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>(initialProfileId || "");
  const [snapshots, setSnapshots] = useState<SnapshotDto[]>([]);
  const [selectedSnapshotId, setSelectedSnapshotId] = useState<string>(initialSnapshotId || "");
  const [passphrase, setPassphrase] = useState<string>("");

  // Preview & Execution State
  const [loading, setLoading] = useState<boolean>(true);
  const [loadingPreview, setLoadingPreview] = useState<boolean>(false);
  const [preview, setPreview] = useState<RepairPreviewDto | null>(null);
  const [isRepairing, setIsRepairing] = useState<boolean>(false);
  const [repairResult, setRepairResult] = useState<RepairExecutionResultDto | null>(null);
  const [history, setHistory] = useState<RepairHistoryRecordDto[]>([]);
  const [error, setError] = useState<string | null>(null);

  const loadProfiles = useCallback(async () => {
    try {
      setLoading(true);
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
      setLoading(false);
    }
  }, [selectedProfileId]);

  const loadSnapshots = useCallback(async (profileId: string) => {
    if (!profileId) return;
    try {
      const res = await commands.listSnapshots(profileId);
      if (res.status === "ok") {
        setSnapshots(res.data);
        if (res.data.length > 0 && !selectedSnapshotId) {
          setSelectedSnapshotId(res.data[0].snapshot_id);
        }
      }
    } catch (err: unknown) {
      console.error(err);
    }
  }, [selectedSnapshotId]);

  const loadHistory = useCallback(async (profileId: string) => {
    if (!profileId) return;
    try {
      const res = await commands.getRepairHistory(profileId, 20);
      if (res.status === "ok") {
        setHistory(res.data);
      }
    } catch (err: unknown) {
      console.error(err);
    }
  }, []);

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    if (selectedProfileId) {
      loadSnapshots(selectedProfileId);
      loadHistory(selectedProfileId);
      setPreview(null);
      setRepairResult(null);
    }
  }, [selectedProfileId, loadSnapshots, loadHistory]);

  const handlePreviewRepair = async () => {
    if (!selectedProfileId || !selectedSnapshotId) return;
    try {
      setLoadingPreview(true);
      setError(null);
      setRepairResult(null);

      const res = await commands.previewRepair({
        profile_id: selectedProfileId,
        target_type: "snapshot",
        target_id: selectedSnapshotId,
        passphrase: passphrase.trim() ? passphrase : null,
        operation_id: `repair-prev-${Date.now()}`,
      });

      if (res.status === "ok") {
        setPreview(res.data);
        onNotify(
          res.data.is_repairable ? "info" : "warning",
          "Repair Analysis Complete",
          res.data.is_repairable
            ? `Eligible for repair. Found ${res.data.total_affected_chunks} chunks needing recovery.`
            : `Repair cannot proceed: Ineligible items or missing local sources detected.`
        );
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoadingPreview(false);
    }
  };

  const handleExecuteRepair = async () => {
    if (!selectedProfileId || !selectedSnapshotId) return;
    if (!preview || !preview.is_repairable) return;

    try {
      setIsRepairing(true);
      setError(null);

      const res = await commands.repairSnapshot({
        profile_id: selectedProfileId,
        snapshot_id: selectedSnapshotId,
        dry_run: false,
        passphrase: passphrase.trim() ? passphrase : null,
        operation_id: `repair-exec-${Date.now()}`,
      });

      if (res.status === "ok") {
        setRepairResult(res.data);
        onNotify(
          res.data.failed_chunks === 0 ? "success" : "warning",
          "Remote Repair Completed",
          `Repaired ${res.data.repaired_chunks} chunks (${(res.data.total_bytes_transferred / 1024).toFixed(1)} KB) using bounded 64 KiB streams.`
        );
        loadHistory(selectedProfileId);
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsRepairing(false);
    }
  };

  if (loading && profiles.length === 0) {
    return <LoadingSpinner message="Initializing remote repair engine..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Remote Repair Error" onDismiss={() => setError(null)} />}

      {/* Cloud Non-Deletion Safety Principle */}
      <div
        style={{
          background: "var(--bg-card)",
          border: "1px solid var(--border-subtle)",
          padding: "1rem 1.25rem",
          borderRadius: "var(--radius-lg)",
          display: "flex",
          alignItems: "center",
          gap: "1rem",
        }}
      >
        <span style={{ fontSize: "1.5rem" }}>🛡️</span>
        <div style={{ fontSize: "0.82rem", color: "var(--text-muted)" }}>
          <strong style={{ color: "var(--text-main)" }}>Remote Safety & Non-Deletion Rule:</strong> Repaired
          chunks are reconstructed from local verified sources using bounded 64 KiB streams and uploaded to fresh
          remote Telegram storage references. Damaged remote objects are never deleted.
        </div>
      </div>

      {/* Target Selection & Preview */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">1. Select Target Snapshot for Repair</div>
            <div className="card-subtitle">
              Evaluate damaged or missing chunks against local source files
            </div>
          </div>
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label>Backup Profile:</label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              disabled={isRepairing}
            >
              {profiles.map((p) => (
                <option key={p.profile_id} value={p.profile_id}>
                  {p.name} ({p.profile_id})
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label>Snapshot Target:</label>
            <select
              value={selectedSnapshotId}
              onChange={(e) => setSelectedSnapshotId(e.target.value)}
              disabled={isRepairing}
            >
              {snapshots.map((s) => (
                <option key={s.snapshot_id} value={s.snapshot_id}>
                  {s.snapshot_id.slice(0, 16)}... ({new Date(s.created_at).toLocaleDateString()})
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="form-group" style={{ marginTop: "0.5rem" }}>
          <label>Passphrase (For Encrypted Profile Chunk Reconstruction):</label>
          <input
            type="password"
            placeholder="Passphrase for AES-256-GCM encryption..."
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            disabled={isRepairing}
          />
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "1rem" }}>
          <button
            className="btn btn-secondary"
            onClick={handlePreviewRepair}
            disabled={!selectedSnapshotId || loadingPreview || isRepairing}
          >
            {loadingPreview ? "Evaluating Work..." : "🔍 Preview Repair (Dry-Run)"}
          </button>
        </div>
      </div>

      {/* Preview Analysis Results */}
      {preview && (
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">2. Repair Eligibility & Estimated Work</div>
              <div className="card-subtitle">
                Analysis of affected chunks and local source file availability
              </div>
            </div>
            <div>
              <StatusBadge
                status={preview.is_repairable ? "Repairable" : "Failed"}
                label={preview.is_repairable ? "Repair Allowed" : "Repair Prohibited"}
              />
            </div>
          </div>

          {!preview.is_repairable && preview.ineligible_findings_count > 0 ? (
            <div
              style={{
                background: "var(--danger-bg)",
                border: "1px solid var(--danger-border)",
                padding: "0.85rem 1rem",
                borderRadius: "var(--radius-md)",
                color: "var(--danger)",
                fontSize: "0.85rem",
              }}
            >
              <strong>Rejection Notice:</strong> {preview.ineligible_findings_count} findings ineligible for automatic recovery (e.g. source missing or size mismatch).
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(3, 1fr)",
                  gap: "1rem",
                  background: "var(--bg-card)",
                  padding: "0.75rem 1rem",
                  borderRadius: "var(--radius-md)",
                }}
              >
                <div>
                  Chunks to Reconstruct: <strong>{preview.total_affected_chunks}</strong>
                </div>
                <div>
                  Estimated Bytes: <strong>{(preview.total_estimated_bytes / 1024).toFixed(1)} KB</strong>
                </div>
                <div>
                  Streaming Buffer: <strong>64 KiB Bounded</strong>
                </div>
              </div>

              <div>
                <h4 style={{ marginBottom: "0.5rem" }}>Candidate Items Breakdown:</h4>
                <div className="table-container">
                  <table>
                    <thead>
                      <tr>
                        <th>Chunk ID</th>
                        <th>Index</th>
                        <th>Finding Code</th>
                        <th>Plaintext Size</th>
                        <th>Source Path</th>
                      </tr>
                    </thead>
                    <tbody>
                      {preview.eligible_candidates.map((item, idx) => (
                        <tr key={idx}>
                          <td className="mono" style={{ color: "var(--primary)" }}>
                            {item.chunk_id.slice(0, 12)}...
                          </td>
                          <td className="mono">#{item.chunk_index}</td>
                          <td>
                            <StatusBadge status="Repairable" label={item.finding_code} />
                          </td>
                          <td className="mono">{(item.plaintext_size / 1024).toFixed(1)} KB</td>
                          <td className="mono" style={{ fontSize: "0.75rem", maxWidth: "200px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {item.source_path}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              <div style={{ display: "flex", justifyContent: "flex-end" }}>
                <button
                  className="btn btn-primary"
                  onClick={handleExecuteRepair}
                  disabled={isRepairing || preview.total_affected_chunks === 0}
                >
                  {isRepairing ? "Reconstructing & Uploading..." : "🔧 Execute Remote Repair"}
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Repair Execution Result */}
      {repairResult && (
        <div className="card">
          <div className="card-header">
            <div className="card-title">3. Repair Execution Summary</div>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
              <StatusBadge status={repairResult.failed_chunks === 0 ? "Completed" : "Warning"} />
              <span style={{ fontWeight: 600 }}>
                {repairResult.failed_chunks === 0 ? "Chunks Reconstructed Successfully" : "Partial Repair Result"}
              </span>
            </div>
            <div style={{ fontSize: "0.82rem" }}>
              <div><strong>Repaired Chunks:</strong> {repairResult.repaired_chunks}</div>
              <div><strong>Failed Chunks:</strong> {repairResult.failed_chunks}</div>
              <div><strong>Total Bytes Transferred:</strong> {(repairResult.total_bytes_transferred / 1024).toFixed(1)} KB</div>
              <div><strong>Duration:</strong> {repairResult.duration_ms} ms</div>
            </div>
          </div>
        </div>
      )}

      {/* Repair Audit History */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Remote Repair History</div>
            <div className="card-subtitle">Audited chunk replacements for profile {selectedProfileId}</div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={() => loadHistory(selectedProfileId)}>
            ↻ Refresh
          </button>
        </div>

        {history.length === 0 ? (
          <EmptyState
            title="No Repairs Recorded"
            description="No remote chunk repairs have been executed for this profile."
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Repaired At</th>
                  <th>Chunk</th>
                  <th>Finding Code</th>
                  <th>Bytes</th>
                  <th>Old Reference</th>
                  <th>New Reference</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {history.map((h, idx) => (
                  <tr key={`${h.snapshot_id}-${h.file_id}-${h.chunk_index}-${idx}`}>
                    <td style={{ fontSize: "0.75rem" }}>{new Date(h.repaired_at).toLocaleString()}</td>
                    <td className="mono">#{h.chunk_index}</td>
                    <td className="mono" style={{ color: "var(--primary)" }}>{h.finding_code}</td>
                    <td className="mono">{(h.bytes_processed / 1024).toFixed(1)} KB</td>
                    <td className="mono" style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
                      {h.old_storage_reference.slice(0, 16)}...
                    </td>
                    <td className="mono" style={{ fontSize: "0.72rem", color: "var(--primary)" }}>
                      {h.new_storage_reference.slice(0, 16)}...
                    </td>
                    <td>
                      <StatusBadge status={h.status} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
