import { useEffect, useState, useCallback } from "react";
import {
  commands,
  ScheduleDto,
  SchedulerStatusDto,
  ScheduleHistoryDto,
  BackupProfileDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { Modal } from "../components/Modal";

interface SchedulesViewProps {
  onNotify: (type: "success" | "error" | "warning" | "info", title: string, message: string) => void;
}

export function SchedulesView({ onNotify }: SchedulesViewProps) {
  const [schedules, setSchedules] = useState<ScheduleDto[]>([]);
  const [schedulerStatus, setSchedulerStatus] = useState<SchedulerStatusDto | null>(null);
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Create Schedule Modal
  const [createModalOpen, setCreateModalOpen] = useState<boolean>(false);
  const [targetProfileId, setTargetProfileId] = useState<string>("");
  const [scheduleType, setScheduleType] = useState<string>("interval");
  const [expression, setExpression] = useState<string>("3600");
  const [timezone, setTimezone] = useState<string>("utc");

  // History Modal
  const [historyModalOpen, setHistoryModalOpen] = useState<boolean>(false);
  const [selectedScheduleIdForHistory, setSelectedScheduleIdForHistory] = useState<string | null>(null);
  const [historyRecords, setHistoryRecords] = useState<ScheduleHistoryDto[]>([]);
  const [loadingHistory, setLoadingHistory] = useState<boolean>(false);

  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const [schedRes, statusRes, profRes] = await Promise.all([
        commands.listSchedules(),
        commands.getSchedulerStatus(),
        commands.listBackupProfiles(),
      ]);

      if (schedRes.status !== "ok") throw new Error(schedRes.error.message);
      if (statusRes.status !== "ok") throw new Error(statusRes.error.message);
      if (profRes.status !== "ok") throw new Error(profRes.error.message);

      setSchedules(schedRes.data);
      setSchedulerStatus(statusRes.data);
      setProfiles(profRes.data);
      if (profRes.data.length > 0 && !targetProfileId) {
        setTargetProfileId(profRes.data[0].profile_id);
      }
    } catch (err: unknown) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [targetProfileId]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleToggleSchedule = async (schedule: ScheduleDto) => {
    try {
      if (schedule.enabled) {
        const res = await commands.disableSchedule(schedule.schedule_id);
        if (res.status === "ok") {
          onNotify("info", "Schedule Disabled", `Schedule ${schedule.schedule_id} disabled.`);
          loadData();
        } else {
          onNotify("error", "Failed to Disable", res.error.message);
        }
      } else {
        const res = await commands.enableSchedule(schedule.schedule_id);
        if (res.status === "ok") {
          onNotify("success", "Schedule Enabled", `Schedule ${schedule.schedule_id} enabled.`);
          loadData();
        } else {
          onNotify("error", "Failed to Enable", res.error.message);
        }
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    }
  };

  const handleRunNow = async (scheduleId: string) => {
    try {
      onNotify("info", "Triggering Scheduled Backup", `Invoking immediate run for ${scheduleId}...`);
      const res = await commands.runScheduleNow(scheduleId);
      if (res.status === "ok") {
        onNotify("success", "Execution Dispatched", `Scheduled job completed or running. Snapshot: ${res.data || "N/A"}`);
        loadData();
      } else {
        onNotify("error", "Run Failed", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Run Error", String(err));
    }
  };

  const handleDeleteSchedule = async (scheduleId: string) => {
    if (!confirm(`Are you sure you want to delete schedule ${scheduleId}?`)) return;
    try {
      const res = await commands.deleteSchedule(scheduleId);
      if (res.status === "ok") {
        onNotify("success", "Schedule Deleted", `Deleted schedule ${scheduleId}.`);
        loadData();
      } else {
        onNotify("error", "Delete Failed", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    }
  };

  const handleCreateSchedule = async () => {
    if (!targetProfileId) return;
    try {
      const res = await commands.createSchedule({
        profile_id: targetProfileId,
        schedule_type: scheduleType,
        expression: expression.trim(),
        timezone: timezone,
        enabled: true,
      });

      if (res.status === "ok") {
        onNotify("success", "Schedule Created", `Created recurring schedule ${res.data.schedule_id}.`);
        setCreateModalOpen(false);
        loadData();
      } else {
        onNotify("error", "Creation Failed", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    }
  };

  const handleViewHistory = async (scheduleId: string) => {
    try {
      setSelectedScheduleIdForHistory(scheduleId);
      setHistoryModalOpen(true);
      setLoadingHistory(true);
      const res = await commands.getScheduleHistory(scheduleId, 20);
      if (res.status === "ok") {
        setHistoryRecords(res.data);
      } else {
        onNotify("error", "Error Loading History", res.error.message);
      }
    } catch (err: unknown) {
      onNotify("error", "Error", String(err));
    } finally {
      setLoadingHistory(false);
    }
  };

  if (loading && schedules.length === 0) {
    return <LoadingSpinner message="Querying scheduler status from Rust core..." />;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Scheduler Error" onDismiss={() => setError(null)} />}

      {/* Scheduler Engine Overview Card */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">In-Process Scheduler Service</div>
            <div className="card-subtitle">
              Event-driven background backup coordination with zero-busy-wait CPU usage
            </div>
          </div>
          <button
            className="btn btn-primary btn-sm"
            onClick={() => setCreateModalOpen(true)}
            disabled={profiles.length === 0}
          >
            + Create Schedule
          </button>
        </div>

        {schedulerStatus && (
          <div className="grid-4" style={{ marginTop: "0.25rem" }}>
            <div className="kpi-card">
              <div className="kpi-title">STATUS</div>
              <div className="kpi-value">
                <StatusBadge status={schedulerStatus.status} />
              </div>
              <div className="kpi-detail">Loop operational</div>
            </div>
            <div className="kpi-card">
              <div className="kpi-title">ACTIVE WORKERS</div>
              <div className="kpi-value">{schedulerStatus.running_profiles.length}</div>
              <div className="kpi-detail">Profiles executing backups</div>
            </div>
            <div className="kpi-card">
              <div className="kpi-title">ACTIVE SCHEDULES</div>
              <div className="kpi-value">{schedulerStatus.active_schedules_count}</div>
              <div className="kpi-detail">{schedules.filter((s) => s.enabled).length} enabled</div>
            </div>
            <div className="kpi-card">
              <div className="kpi-title">MUTEX GUARD</div>
              <div className="kpi-value" style={{ fontSize: "1.1rem" }}>
                ExecutionGuard
              </div>
              <div className="kpi-detail">Prevents duplicate runs</div>
            </div>
          </div>
        )}
      </div>

      {/* Schedules Table */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Configured Schedules</div>
            <div className="card-subtitle">Automated point-in-time backup frequencies</div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={loadData}>
            ↻ Refresh
          </button>
        </div>

        {schedules.length === 0 ? (
          <EmptyState
            title="No Schedules Configured"
            description="Create a schedule to automate periodic backups without keeping a browser open."
            action={
              profiles.length > 0 ? (
                <button className="btn btn-primary btn-sm" onClick={() => setCreateModalOpen(true)}>
                  Create Schedule
                </button>
              ) : undefined
            }
          />
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Schedule ID</th>
                  <th>Profile</th>
                  <th>Type</th>
                  <th>Expression</th>
                  <th>Timezone</th>
                  <th>Next Run</th>
                  <th>Last Run</th>
                  <th>Status</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {schedules.map((s) => {
                  const prof = profiles.find((p) => p.profile_id === s.profile_id);
                  return (
                    <tr key={s.schedule_id}>
                      <td className="mono" style={{ color: "var(--primary)" }}>
                        {s.schedule_id.slice(0, 14)}...
                      </td>
                      <td style={{ fontWeight: 600 }}>{prof ? prof.name : s.profile_id}</td>
                      <td>{s.schedule_type}</td>
                      <td className="mono">{s.expression}</td>
                      <td>{s.timezone}</td>
                      <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                        {s.next_run_at ? new Date(s.next_run_at).toLocaleString() : "None"}
                      </td>
                      <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                        {s.last_run_at ? new Date(s.last_run_at).toLocaleString() : "Never"}
                      </td>
                      <td>
                        <StatusBadge
                          status={s.enabled ? (s.last_status || "Active") : "Disabled"}
                          label={s.enabled ? (s.last_status || "Active") : "Disabled"}
                        />
                      </td>
                      <td>
                        <div style={{ display: "flex", gap: "0.4rem" }}>
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => handleToggleSchedule(s)}
                            title={s.enabled ? "Disable schedule" : "Enable schedule"}
                          >
                            {s.enabled ? "Disable" : "Enable"}
                          </button>
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => handleRunNow(s.schedule_id)}
                            title="Trigger immediate execution"
                          >
                            Run Now
                          </button>
                          <button
                            className="btn btn-secondary btn-sm"
                            onClick={() => handleViewHistory(s.schedule_id)}
                            title="View past execution history"
                          >
                            History
                          </button>
                          <button
                            className="btn btn-danger btn-sm"
                            onClick={() => handleDeleteSchedule(s.schedule_id)}
                            title="Delete this schedule"
                          >
                            Delete
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Create Schedule Modal */}
      <Modal
        isOpen={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        title="Create Automated Backup Schedule"
        footer={
          <div style={{ display: "flex", gap: "0.75rem" }}>
            <button className="btn btn-secondary" onClick={() => setCreateModalOpen(false)}>
              Cancel
            </button>
            <button className="btn btn-primary" onClick={handleCreateSchedule}>
              Create Schedule
            </button>
          </div>
        }
      >
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div className="form-group">
            <label>Backup Profile:</label>
            <select value={targetProfileId} onChange={(e) => setTargetProfileId(e.target.value)}>
              {profiles.map((p) => (
                <option key={p.profile_id} value={p.profile_id}>
                  {p.name} ({p.profile_id})
                </option>
              ))}
            </select>
          </div>

          <div className="grid-2">
            <div className="form-group">
              <label>Schedule Type:</label>
              <select
                value={scheduleType}
                onChange={(e) => {
                  const val = e.target.value;
                  setScheduleType(val);
                  if (val === "interval") setExpression("3600");
                  if (val === "daily") setExpression("02:00");
                  if (val === "weekly") setExpression("Sun@03:00");
                  if (val === "cron") setExpression("0 0 * * *");
                }}
              >
                <option value="interval">Interval (Seconds)</option>
                <option value="daily">Daily (Time of Day)</option>
                <option value="weekly">Weekly (Day and Time)</option>
                <option value="cron">Cron Expression</option>
              </select>
            </div>

            <div className="form-group">
              <label>Expression / Timing:</label>
              <input
                type="text"
                value={expression}
                onChange={(e) => setExpression(e.target.value)}
                placeholder={
                  scheduleType === "interval"
                    ? "Seconds e.g. 3600"
                    : scheduleType === "daily"
                    ? "HH:MM e.g. 02:00"
                    : scheduleType === "weekly"
                    ? "DAY@HH:MM e.g. Sun@03:00"
                    : "Cron e.g. 0 0 * * *"
                }
              />
            </div>
          </div>

          <div className="form-group">
            <label>Timezone Strategy:</label>
            <select value={timezone} onChange={(e) => setTimezone(e.target.value)}>
              <option value="utc">UTC (Deterministic)</option>
              <option value="local">Local System Time</option>
            </select>
          </div>
        </div>
      </Modal>

      {/* History Modal */}
      <Modal
        isOpen={historyModalOpen}
        onClose={() => setHistoryModalOpen(false)}
        title={`Execution History: ${selectedScheduleIdForHistory || ""}`}
        large={true}
        footer={
          <button className="btn btn-secondary" onClick={() => setHistoryModalOpen(false)}>
            Close
          </button>
        }
      >
        {loadingHistory ? (
          <LoadingSpinner message="Retrieving schedule execution history..." />
        ) : historyRecords.length === 0 ? (
          <EmptyState title="No History Records" description="This schedule has not triggered any runs yet." />
        ) : (
          <div className="table-container" style={{ maxHeight: "400px", overflowY: "auto" }}>
            <table>
              <thead>
                <tr>
                  <th>Started At</th>
                  <th>Status</th>
                  <th>Snapshot ID</th>
                  <th>Files</th>
                  <th>Bytes</th>
                  <th>Error</th>
                </tr>
              </thead>
              <tbody>
                {historyRecords.map((h) => (
                  <tr key={h.history_id}>
                    <td style={{ fontSize: "0.75rem" }}>{new Date(h.started_at).toLocaleString()}</td>
                    <td>
                      <StatusBadge status={h.status} />
                    </td>
                    <td className="mono" style={{ color: "var(--primary)" }}>
                      {h.snapshot_id ? `${h.snapshot_id.slice(0, 10)}...` : "None"}
                    </td>
                    <td>{h.files_processed}</td>
                    <td className="mono">{(h.bytes_transferred / (1024 * 1024)).toFixed(2)} MB</td>
                    <td style={{ fontSize: "0.72rem", color: h.error_message ? "var(--danger)" : "var(--text-dim)" }}>
                      {h.error_message || "None"}
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
