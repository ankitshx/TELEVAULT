import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  RetentionPolicyDto,
  RetentionEvaluationDto,
  RetentionHistoryDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { Modal } from "../components/Modal";

interface RetentionViewProps {
  initialProfileId?: string;
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function RetentionView({
  initialProfileId,
  onNotify,
}: RetentionViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>(initialProfileId || "");
  const [policy, setPolicy] = useState<RetentionPolicyDto | null>(null);
  const [history, setHistory] = useState<RetentionHistoryDto[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Policy Form State
  const [keepLatestN, setKeepLatestN] = useState<number>(10);
  const [maxAgeDays, setMaxAgeDays] = useState<number>(30);
  const [keepLatestSuccessful, setKeepLatestSuccessful] = useState<boolean>(true);
  const [enabled, setEnabled] = useState<boolean>(true);
  const [isSavingPolicy, setIsSavingPolicy] = useState<boolean>(false);

  // Dry-Run Preview State
  const [previewModalOpen, setPreviewModalOpen] = useState<boolean>(false);
  const [previewEvaluation, setPreviewEvaluation] = useState<RetentionEvaluationDto | null>(null);
  const [loadingPreview, setLoadingPreview] = useState<boolean>(false);

  // Execution State
  const [isPruning, setIsPruning] = useState<boolean>(false);

  const loadData = useCallback(async (profileId: string) => {
    if (!profileId) return;
    try {
      setLoading(true);
      setError(null);

      const [polRes, histRes] = await Promise.all([
        commands.getRetentionPolicy(profileId),
        commands.getRetentionHistory(profileId, 20),
      ]);

      if (polRes.status === "ok") {
        setPolicy(polRes.data);
        setKeepLatestN(polRes.data.keep_latest_n ?? 10);
        setMaxAgeDays(
          polRes.data.keep_newer_than_secs
            ? Math.round(polRes.data.keep_newer_than_secs / 86400)
            : 30
        );
        setKeepLatestSuccessful(polRes.data.keep_latest_successful);
        setEnabled(polRes.data.enabled);
      } else {
        setError(polRes.error.message);
      }

      if (histRes.status === "ok") {
        setHistory(histRes.data);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      const res = await commands.listBackupProfiles();
      if (res.status === "ok") {
        setProfiles(res.data);
        if (res.data.length > 0 && !selectedProfileId) {
          setSelectedProfileId(res.data[0].profile_id);
        }
      }
    } catch (err: unknown) {
      setError(String(err));
    }
  }, [selectedProfileId]);

  useEffect(() => {
    loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    if (selectedProfileId) {
      loadData(selectedProfileId);
    }
  }, [selectedProfileId, loadData]);

  const handleSavePolicy = async () => {
    if (!selectedProfileId) return;
    try {
      setIsSavingPolicy(true);
      setError(null);

      const res = await commands.setRetentionPolicy({
        profile_id: selectedProfileId,
        keep_latest_n: keepLatestN > 0 ? keepLatestN : null,
        keep_newer_than_secs: maxAgeDays > 0 ? maxAgeDays * 86400 : null,
        keep_latest_successful: keepLatestSuccessful,
        keep_latest_always: true,
        prune_failed: false,
        prune_empty: false,
        enabled,
      });

      if (res.status === "ok") {
        setPolicy(res.data);
        onNotify("success", "Retention Policy Saved", "Policy rules persisted to SQLite database.");
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsSavingPolicy(false);
    }
  };

  const handlePreviewDryRun = async () => {
    if (!selectedProfileId) return;
    try {
      setLoadingPreview(true);
      setPreviewModalOpen(true);
      setError(null);

      const res = await commands.previewRetention(selectedProfileId);
      if (res.status === "ok") {
        setPreviewEvaluation(res.data);
      } else {
        setError(res.error.message);
        setPreviewModalOpen(false);
      }
    } catch (err: unknown) {
      setError(String(err));
      setPreviewModalOpen(false);
    } finally {
      setLoadingPreview(false);
    }
  };

  const handleExecutePruning = async () => {
    if (!selectedProfileId) return;
    if (!confirm("Execute retention pruning on local SQLite catalog? This deletes expired local metadata inside an atomic transaction. (Remote Telegram objects remain permanently intact).")) {
      return;
    }

    try {
      setIsPruning(true);
      setError(null);

      const res = await commands.executeRetention(selectedProfileId);
      if (res.status === "ok") {
        onNotify(
          "success",
          "Retention Pruned",
          `Pruned ${res.data.pruned_snapshots.length} local snapshot records (${res.data.pruned_versions_count} versions unlinked).`
        );
        loadData(selectedProfileId);
        setPreviewModalOpen(false);
      } else {
        setError(res.error.message);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setIsPruning(false);
    }
  };

  if (loading && profiles.length === 0) {
    return <LoadingSpinner message="Loading retention policy configuration..." />;
  }

  const keptDecisions = previewEvaluation?.decisions.filter((d) => d.action === "KEEP") || [];
  const prunedDecisions = previewEvaluation?.decisions.filter((d) => d.action === "PRUNE") || [];

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Retention Engine Error" onDismiss={() => setError(null)} />}

      {/* Safety Notice Banner */}
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
        <span style={{ fontSize: "1.5rem" }}>🔒</span>
        <div style={{ fontSize: "0.82rem", color: "var(--text-muted)" }}>
          <strong style={{ color: "var(--text-main)" }}>Remote Cloud Immutability Invariant:</strong> Retention
          pruning operates strictly on embedded SQLite metadata. Remote Telegram Cloud objects, chunks, and manifests
          remain completely immutable and are never deleted by retention.
        </div>
      </div>

      {/* Profile Selector & Policy Config */}
      <div className="card">
        <div className="card-header">
          <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
            <label style={{ fontWeight: 700, color: "var(--text-main)" }}>Select Profile:</label>
            <select
              value={selectedProfileId}
              onChange={(e) => setSelectedProfileId(e.target.value)}
              style={{ minWidth: "200px" }}
            >
              {profiles.map((p) => (
                <option key={p.profile_id} value={p.profile_id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
          <div>
            <StatusBadge
              status={policy?.enabled ? "Active" : "Disabled"}
              label={policy?.enabled ? "Policy Active" : "Policy Inactive"}
            />
          </div>
        </div>

        <div className="grid-3" style={{ marginTop: "0.5rem" }}>
          <div className="form-group">
            <label>Keep Latest N Snapshots:</label>
            <input
              type="number"
              min={1}
              max={100}
              value={keepLatestN}
              onChange={(e) => setKeepLatestN(parseInt(e.target.value) || 0)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Always preserves the N most recent completed snapshots.
            </span>
          </div>

          <div className="form-group">
            <label>Max Age (Days):</label>
            <input
              type="number"
              min={0}
              max={3650}
              value={maxAgeDays}
              onChange={(e) => setMaxAgeDays(parseInt(e.target.value) || 0)}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>
              Snapshots within this age window are kept (0 = infinite).
            </span>
          </div>

          <div className="form-group">
            <label>Safety Rules:</label>
            <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem", marginTop: "0.2rem" }}>
              <label className="checkbox-label">
                <input
                  type="checkbox"
                  checked={keepLatestSuccessful}
                  onChange={(e) => setKeepLatestSuccessful(e.target.checked)}
                />
                <span>Preserve Latest Successful (Protected)</span>
              </label>
              <label className="checkbox-label">
                <input type="checkbox" checked={enabled} onChange={(e) => setEnabled(e.target.checked)} />
                <span>Enable Policy Evaluation</span>
              </label>
            </div>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.75rem", marginTop: "1rem" }}>
          <button
            className="btn btn-secondary"
            onClick={handlePreviewDryRun}
            disabled={!selectedProfileId || loadingPreview}
          >
            {loadingPreview ? "Evaluating..." : "🔍 Preview Dry-Run"}
          </button>
          <button
            className="btn btn-primary"
            onClick={handleSavePolicy}
            disabled={!selectedProfileId || isSavingPolicy}
          >
            {isSavingPolicy ? "Saving..." : "💾 Save Policy"}
          </button>
        </div>
      </div>

      {/* Retention History Table */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Retention Audit History</div>
            <div className="card-subtitle">Historical pruning executions for profile {selectedProfileId}</div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={() => loadData(selectedProfileId)}>
            ↻ Refresh History
          </button>
        </div>

        {history.length === 0 ? (
          <EmptyState
            title="No Pruning History"
            description="No retention pruning operations have been executed for this profile yet."
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Pruned At</th>
                  <th>Snapshots Pruned</th>
                  <th>Snapshots Kept</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {history.map((h) => (
                  <tr key={h.history_id}>
                    <td style={{ fontSize: "0.75rem" }}>{new Date(h.executed_at).toLocaleString()}</td>
                    <td style={{ fontWeight: 600 }}>{h.snapshots_pruned}</td>
                    <td>{h.snapshots_kept}</td>
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

      {/* Dry-Run Preview Modal */}
      <Modal
        isOpen={previewModalOpen}
        onClose={() => setPreviewModalOpen(false)}
        title="Retention Evaluation (Dry-Run Preview)"
        large={true}
        footer={
          <div style={{ display: "flex", gap: "0.75rem" }}>
            <button className="btn btn-secondary" onClick={() => setPreviewModalOpen(false)}>
              Close
            </button>
            <button
              className="btn btn-danger"
              onClick={handleExecutePruning}
              disabled={isPruning || !previewEvaluation || prunedDecisions.length === 0}
            >
              {isPruning ? "Pruning..." : "🗑️ Execute Pruning"}
            </button>
          </div>
        }
      >
        {previewEvaluation && (
          <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
            <div
              style={{
                display: "flex",
                gap: "2rem",
                background: "var(--bg-card)",
                padding: "0.75rem 1rem",
                borderRadius: "var(--radius-md)",
              }}
            >
              <div>
                Total Kept: <strong style={{ color: "var(--success)" }}>{previewEvaluation.snapshots_kept}</strong>
              </div>
              <div>
                Total Prunable: <strong style={{ color: "var(--danger)" }}>{previewEvaluation.snapshots_pruned}</strong>
              </div>
            </div>

            <div>
              <h4 style={{ marginBottom: "0.5rem" }}>Snapshots to Keep:</h4>
              {keptDecisions.length === 0 ? (
                <div style={{ fontSize: "0.8rem", color: "var(--text-dim)" }}>None</div>
              ) : (
                <div className="table-container" style={{ maxHeight: "200px", overflowY: "auto" }}>
                  <table>
                    <thead>
                      <tr>
                        <th>Snapshot ID</th>
                        <th>Preservation Reason</th>
                      </tr>
                    </thead>
                    <tbody>
                      {keptDecisions.map((k) => (
                        <tr key={k.snapshot_id}>
                          <td className="mono" style={{ color: "var(--primary)" }}>
                            {k.snapshot_id}
                          </td>
                          <td>
                            <StatusBadge status="Healthy" label={k.reason} />
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>

            <div>
              <h4 style={{ marginBottom: "0.5rem" }}>Expired Snapshots to Prune:</h4>
              {prunedDecisions.length === 0 ? (
                <div style={{ fontSize: "0.8rem", color: "var(--text-dim)" }}>No prunable snapshots identified.</div>
              ) : (
                <div className="table-container" style={{ maxHeight: "200px", overflowY: "auto" }}>
                  <table>
                    <thead>
                      <tr>
                        <th>Snapshot ID</th>
                        <th>Expiration Reason</th>
                      </tr>
                    </thead>
                    <tbody>
                      {prunedDecisions.map((p) => (
                        <tr key={p.snapshot_id}>
                          <td className="mono" style={{ color: "var(--danger)" }}>
                            {p.snapshot_id}
                          </td>
                          <td>{p.description || p.reason}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </div>
        )}
      </Modal>
    </div>
  );
}
