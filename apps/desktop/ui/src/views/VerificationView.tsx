import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  SnapshotDto,
  VerificationResultDto,
  VerificationHistoryRecordDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { NavTab } from "../types";

interface VerificationViewProps {
  initialProfileId?: string;
  initialSnapshotId?: string;
  onNavigate: (tab: NavTab, profileId?: string, snapshotId?: string) => void;
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function VerificationView({
  initialProfileId,
  initialSnapshotId,
  onNavigate,
  onNotify,
}: VerificationViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>(initialProfileId || "");
  const [snapshots, setSnapshots] = useState<SnapshotDto[]>([]);
  const [selectedSnapshotId, setSelectedSnapshotId] = useState<string>(initialSnapshotId || "");

  // Target scope: "profile" or "snapshot"
  const [targetScope, setTargetScope] = useState<"profile" | "snapshot">("profile");
  const [levelNum, setLevelNum] = useState<number>(1); // 1 = MetadataOnly, 2 = Availability, 3 = Integrity, 4 = Readiness

  // Verification Results & History
  const [isVerifying, setIsVerifying] = useState<boolean>(false);
  const [currentResult, setCurrentResult] = useState<VerificationResultDto | null>(null);
  const [history, setHistory] = useState<VerificationHistoryRecordDto[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
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

  const loadHistory = useCallback(async (profileId: string) => {
    if (!profileId) return;
    try {
      const res = await commands.getVerificationHistory(profileId, 20);
      if (res.status === "ok") {
        setHistory(res.data);
      }
    } catch (err: unknown) {
      console.error(err);
    }
  }, []);

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

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    if (selectedProfileId) {
      loadSnapshots(selectedProfileId);
      loadHistory(selectedProfileId);
    }
  }, [selectedProfileId, loadSnapshots, loadHistory]);

  const handleRunVerification = async () => {
    if (!selectedProfileId) return;
    try {
      setIsVerifying(true);
      setError(null);
      setCurrentResult(null);

      const targetId = targetScope === "profile" ? selectedProfileId : selectedSnapshotId;
      if (!targetId) {
        throw new Error("No target selected for verification.");
      }

      const req = {
        profile_id: selectedProfileId,
        target_id: targetId,
        level: levelNum,
        full_hash_check: levelNum >= 3,
        decrypt_check: levelNum >= 4,
        operation_id: `ver-${Date.now()}`,
      };

      const res =
        targetScope === "profile"
          ? await commands.verifyProfile(req)
          : await commands.verifySnapshot(req);

      if (res.status === "ok") {
        setCurrentResult(res.data);
        onNotify(
          res.data.status.toLowerCase() === "healthy" ? "success" : "warning",
          "Verification Completed",
          `Status: ${res.data.status}. Total Chunks: ${res.data.summary.total_chunks} (${res.data.findings.length} findings).`
        );
        loadHistory(selectedProfileId);
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsVerifying(false);
    }
  };

  if (loading && profiles.length === 0) {
    return <LoadingSpinner message="Loading verification subsystem..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Verification Error" onDismiss={() => setError(null)} />}

      {/* Verification Level Explanations */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">1. Select Verification Level & Target</div>
            <div className="card-subtitle">
              Audit remote Telegram Cloud storage integrity without falsifying success
            </div>
          </div>
        </div>

        <div className="grid-2">
          <div className="form-group">
            <label>Verification Level:</label>
            <select value={levelNum} onChange={(e) => setLevelNum(parseInt(e.target.value))} disabled={isVerifying}>
              <option value={1}>Level 1: Metadata Only (Catalog schema & chunk indices)</option>
              <option value={2}>Level 2: Remote Availability (Telegram document existence)</option>
              <option value={3}>Level 3: Remote Integrity (Sizes, headers & hash digests)</option>
              <option value={4}>Level 4: Restore Readiness (Dry-run cryptographic decryption)</option>
            </select>
          </div>

          <div className="form-group">
            <label>Verification Scope:</label>
            <div style={{ display: "flex", gap: "1.5rem", marginTop: "0.4rem" }}>
              <label className="checkbox-label">
                <input
                  type="radio"
                  name="verScope"
                  checked={targetScope === "profile"}
                  onChange={() => setTargetScope("profile")}
                  disabled={isVerifying}
                />
                <span>Entire Profile</span>
              </label>
              <label className="checkbox-label">
                <input
                  type="radio"
                  name="verScope"
                  checked={targetScope === "snapshot"}
                  onChange={() => setTargetScope("snapshot")}
                  disabled={isVerifying}
                />
                <span>Individual Snapshot</span>
              </label>
            </div>
          </div>
        </div>

        <div className="grid-2" style={{ marginTop: "0.5rem" }}>
          <div className="form-group">
            <label>Target Profile:</label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              disabled={isVerifying}
            >
              {profiles.map((p) => (
                <option key={p.profile_id} value={p.profile_id}>
                  {p.name} ({p.profile_id})
                </option>
              ))}
            </select>
          </div>

          {targetScope === "snapshot" && (
            <div className="form-group">
              <label>Target Snapshot:</label>
              <select
                value={selectedSnapshotId}
                onChange={(e) => setSelectedSnapshotId(e.target.value)}
                disabled={isVerifying}
              >
                {snapshots.map((s) => (
                  <option key={s.snapshot_id} value={s.snapshot_id}>
                    {s.snapshot_id.slice(0, 16)}... ({new Date(s.created_at).toLocaleDateString()})
                  </option>
                ))}
              </select>
            </div>
          )}
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "1rem" }}>
          <button className="btn btn-primary" onClick={handleRunVerification} disabled={isVerifying}>
            {isVerifying ? "Verifying Remote State..." : "🛡️ Run Verification Audit"}
          </button>
        </div>
      </div>

      {/* Verification Results Breakdown */}
      {currentResult && (
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">2. Verification Findings Report</div>
              <div className="card-subtitle">
                Level: {currentResult.level} • Verified at {new Date(currentResult.verified_at).toLocaleTimeString()}
              </div>
            </div>
            <div>
              <StatusBadge status={currentResult.status} />
            </div>
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(4, 1fr)",
              gap: "1rem",
              background: "var(--bg-card)",
              padding: "0.75rem 1rem",
              borderRadius: "var(--radius-md)",
            }}
          >
            <div>
              Total Chunks: <strong>{currentResult.summary.total_chunks}</strong>
            </div>
            <div>
              Healthy: <strong style={{ color: "var(--success)" }}>{currentResult.summary.healthy_chunks}</strong>
            </div>
            <div>
              Missing: <strong style={{ color: "var(--warning)" }}>{currentResult.summary.missing_chunks}</strong>
            </div>
            <div>
              Corrupted: <strong style={{ color: "var(--danger)" }}>{currentResult.summary.corrupted_chunks}</strong>
            </div>
          </div>

          {currentResult.findings.length === 0 ? (
            <div style={{ padding: "1rem 0", color: "var(--success)", fontWeight: 600 }}>
              ✅ All manifests, chunks, and storage references verified healthy and uncorrupted.
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontWeight: 600 }}>Detected Findings Breakdown:</span>
                <button
                  className="btn btn-primary btn-sm"
                  onClick={() => onNavigate("repair", selectedProfileId, selectedSnapshotId)}
                >
                  🔧 Open Repair Screen
                </button>
              </div>

              <div className="table-container">
                <table>
                  <thead>
                    <tr>
                      <th>Finding Code</th>
                      <th>Severity</th>
                      <th>Chunk</th>
                      <th>Description</th>
                      <th>Restore Impact</th>
                    </tr>
                  </thead>
                  <tbody>
                    {currentResult.findings.map((f, idx) => (
                      <tr key={idx}>
                        <td className="mono" style={{ color: "var(--primary)" }}>
                          {f.code}
                        </td>
                        <td>
                          <StatusBadge status={f.severity} />
                        </td>
                        <td className="mono">
                          {f.chunk_index !== null ? `Chunk #${f.chunk_index}` : "Manifest"}
                        </td>
                        <td style={{ fontSize: "0.78rem" }}>{f.description}</td>
                        <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{f.restore_impact}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Verification Audit History */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Verification Audit History</div>
            <div className="card-subtitle">Persisted audit results for profile {selectedProfileId}</div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={() => loadHistory(selectedProfileId)}>
            ↻ Refresh
          </button>
        </div>

        {history.length === 0 ? (
          <EmptyState
            title="No Past Audits"
            description="No verification history records exist for this profile."
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Audited At</th>
                  <th>Level</th>
                  <th>Status</th>
                  <th>Total Chunks</th>
                  <th>Healthy</th>
                  <th>Missing / Corrupt</th>
                </tr>
              </thead>
              <tbody>
                {history.map((h) => (
                  <tr key={h.history_id}>
                    <td style={{ fontSize: "0.75rem" }}>{new Date(h.verified_at).toLocaleString()}</td>
                    <td style={{ fontWeight: 600 }}>L{h.level}</td>
                    <td>
                      <StatusBadge status={h.status} />
                    </td>
                    <td>{h.total_chunks}</td>
                    <td style={{ color: "var(--success)" }}>{h.healthy_chunks}</td>
                    <td style={{ color: h.missing_chunks + h.corrupted_chunks > 0 ? "var(--warning)" : "var(--text-dim)" }}>
                      {h.missing_chunks + h.corrupted_chunks}
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
