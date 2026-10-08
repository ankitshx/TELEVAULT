import { useEffect, useState, useCallback } from "react";
import {
  commands,
  TransferJobDto,
  TransferStatusDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";

interface ActivityViewProps {
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function ActivityView({ onNotify }: ActivityViewProps) {
  const [jobs, setJobs] = useState<TransferJobDto[]>([]);
  const [status, setStatus] = useState<TransferStatusDto | null>(null);
  const [filterStatus, setFilterStatus] = useState<string>("All");
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    try {
      setError(null);
      const [jobsRes, statusRes] = await Promise.all([
        commands.listTransferJobs(50),
        commands.getTransferStatus(),
      ]);

      if (jobsRes.status === "ok") {
        setJobs(jobsRes.data);
      } else {
        setError(jobsRes.error.message);
      }

      if (statusRes.status === "ok") {
        setStatus(statusRes.data);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 3000);
    return () => clearInterval(interval);
  }, [loadData]);

  const handleCancelJob = async (jobId: string) => {
    try {
      const res = await commands.cancelOperation({ operation_id: jobId });
      if (res.status === "ok") {
        onNotify("warning", "Job Cancelled", `Cancelled transfer job ${jobId}.`);
        loadData();
      } else {
        onNotify("error", "Failed to Cancel", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    }
  };

  const filteredJobs = jobs.filter((j) => {
    if (filterStatus === "All") return true;
    return j.status.toLowerCase() === filterStatus.toLowerCase();
  });

  if (loading && jobs.length === 0) {
    return <LoadingSpinner message="Querying transfer workers and queues..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Transfer Engine Error" onDismiss={() => setError(null)} />}

      {/* Queue Status KPIs */}
      {status && (
        <div className="grid-4">
          <div className="kpi-card">
            <div className="kpi-title">ACTIVE WORKERS</div>
            <div className="kpi-value">{status.active_count}</div>
            <div className="kpi-detail">Streaming 64 KiB chunks</div>
          </div>
          <div className="kpi-card">
            <div className="kpi-title">QUEUED JOBS</div>
            <div className="kpi-value">{status.queued_count}</div>
            <div className="kpi-detail">Waiting for workers</div>
          </div>
          <div className="kpi-card">
            <div className="kpi-title">COMPLETED</div>
            <div className="kpi-value" style={{ color: "var(--success)" }}>
              {status.completed_count}
            </div>
            <div className="kpi-detail">Verified upload/download</div>
          </div>
          <div className="kpi-card">
            <div className="kpi-title">FAILED / CANCELLED</div>
            <div className="kpi-value" style={{ color: status.failed_count > 0 ? "var(--danger)" : "var(--text-main)" }}>
              {status.failed_count}
            </div>
            <div className="kpi-detail">Encountered errors</div>
          </div>
        </div>
      )}

      {/* Transfers List */}
      <div className="card">
        <div className="card-header" style={{ flexWrap: "wrap", gap: "1rem" }}>
          <div>
            <div className="card-title">Transfer Operations & Pipeline Activity</div>
            <div className="card-subtitle">
              Bounded metadata streaming. Real-time background workers.
            </div>
          </div>

          <div style={{ display: "flex", alignItems: "center", gap: "0.75rem" }}>
            <label style={{ fontSize: "0.78rem" }}>Filter Status:</label>
            <select
              value={filterStatus}
              onChange={(e) => setFilterStatus(e.target.value)}
              style={{ padding: "0.3rem 0.6rem" }}
            >
              <option value="All">All Jobs</option>
              <option value="Running">Running</option>
              <option value="Queued">Queued</option>
              <option value="Completed">Completed</option>
              <option value="Failed">Failed</option>
              <option value="Cancelled">Cancelled</option>
            </select>
            <button className="btn btn-secondary btn-sm" onClick={loadData}>
              ↻ Refresh
            </button>
          </div>
        </div>

        {filteredJobs.length === 0 ? (
          <EmptyState
            title="No Transfer Jobs Found"
            description="No active or historical transfer jobs matching the selected filter."
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Job ID</th>
                  <th>Direction</th>
                  <th>File / Chunk Reference</th>
                  <th>Status</th>
                  <th>Progress</th>
                  <th>Started At</th>
                  <th>Action</th>
                </tr>
              </thead>
              <tbody>
                {filteredJobs.map((j) => {
                  const progressPct = Math.min(100, Math.round(j.progress));

                  return (
                    <tr key={j.job_id}>
                      <td className="mono" style={{ color: "var(--primary)" }}>
                        {j.job_id.length > 10 ? `${j.job_id.slice(0, 10)}...` : j.job_id}
                      </td>
                      <td>
                        <StatusBadge
                          status={j.direction.toLowerCase() === "upload" ? "Active" : "Info"}
                          label={j.direction}
                        />
                      </td>
                      <td className="mono" style={{ fontSize: "0.78rem", maxWidth: "240px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {j.chunk_id ? `${j.file_id.slice(0, 8)}:chunk#${j.chunk_id}` : j.file_id}
                      </td>
                      <td>
                        <StatusBadge status={j.status} />
                      </td>
                      <td>
                        <div style={{ display: "flex", flexDirection: "column", gap: "0.2rem", width: "120px" }}>
                          <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.72rem" }}>
                            <span>Progress</span>
                            <span>{progressPct}%</span>
                          </div>
                          <div
                            style={{
                              width: "100%",
                              height: "4px",
                              background: "var(--border-card)",
                              borderRadius: "2px",
                              overflow: "hidden",
                            }}
                          >
                            <div
                              style={{
                                width: `${progressPct}%`,
                                height: "100%",
                                background: j.status.toLowerCase() === "failed" ? "var(--danger)" : "var(--primary)",
                                transition: "width 0.2s ease",
                              }}
                            />
                          </div>
                        </div>
                      </td>
                      <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                        {new Date(j.created_at).toLocaleTimeString()}
                      </td>
                      <td>
                        {j.status.toLowerCase() === "running" || j.status.toLowerCase() === "queued" ? (
                          <button
                            className="btn btn-danger btn-sm"
                            onClick={() => handleCancelJob(j.job_id)}
                            title="Cancel active transfer operation"
                          >
                            Cancel
                          </button>
                        ) : j.error_message ? (
                          <span style={{ fontSize: "0.72rem", color: "var(--danger)" }} title={j.error_message}>
                            Error
                          </span>
                        ) : (
                          <span style={{ fontSize: "0.72rem", color: "var(--text-dim)" }}>-</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
