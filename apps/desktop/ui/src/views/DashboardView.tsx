import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  SnapshotDto,
  TransferStatusDto,
  SchedulerStatusDto,
  ScheduleDto,
  VerificationHistoryRecordDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { NavTab } from "../types";

interface DashboardViewProps {
  onNavigate: (tab: NavTab, profileId?: string) => void;
  onStartBackup: (profile: BackupProfileDto) => void;
}

export function DashboardView({ onNavigate, onStartBackup }: DashboardViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [snapshots, setSnapshots] = useState<{ profileName: string; snapshot: SnapshotDto }[]>([]);
  const [transferStatus, setTransferStatus] = useState<TransferStatusDto | null>(null);
  const [schedulerStatus, setSchedulerStatus] = useState<SchedulerStatusDto | null>(null);
  const [schedules, setSchedules] = useState<ScheduleDto[]>([]);
  const [recentVerifications, setRecentVerifications] = useState<VerificationHistoryRecordDto[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const loadDashboardData = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const [profRes, transferRes, schedStatusRes, schedListRes] = await Promise.all([
        commands.listBackupProfiles(),
        commands.getTransferStatus(),
        commands.getSchedulerStatus(),
        commands.listSchedules(),
      ]);

      if (profRes.status !== "ok") throw new Error(profRes.error.message);
      if (transferRes.status !== "ok") throw new Error(transferRes.error.message);
      if (schedStatusRes.status !== "ok") throw new Error(schedStatusRes.error.message);
      if (schedListRes.status !== "ok") throw new Error(schedListRes.error.message);

      setProfiles(profRes.data);
      setTransferStatus(transferRes.data);
      setSchedulerStatus(schedStatusRes.data);
      setSchedules(schedListRes.data);

      const snapshotList: { profileName: string; snapshot: SnapshotDto }[] = [];
      const verificationList: VerificationHistoryRecordDto[] = [];

      for (const p of profRes.data.slice(0, 5)) {
        const snapRes = await commands.listSnapshots(p.profile_id);
        if (snapRes.status === "ok") {
          for (const s of snapRes.data) {
            snapshotList.push({ profileName: p.name, snapshot: s });
          }
        }
        const verRes = await commands.getVerificationHistory(p.profile_id, 3);
        if (verRes.status === "ok") {
          verificationList.push(...verRes.data);
        }
      }

      snapshotList.sort((a, b) => b.snapshot.created_at.localeCompare(a.snapshot.created_at));
      setSnapshots(snapshotList.slice(0, 5));
      setRecentVerifications(verificationList.slice(0, 5));
    } catch (err: unknown) {
      setError(typeof err === "object" && err !== null && "message" in err ? (err as Error).message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadDashboardData();
  }, [loadDashboardData]);

  if (loading && profiles.length === 0) {
    return <LoadingSpinner message="Aggregating telemetry from in-process core..." />;
  }

  const enabledSchedulesCount = schedules.filter((s) => s.enabled).length;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      {error && <ErrorBanner error={error} title="Dashboard Telemetry Error" onRetry={loadDashboardData} />}

      {/* Top KPI Cards */}
      <div className="grid-4">
        <div className="kpi-card">
          <div className="kpi-title">Backup Profiles</div>
          <div className="kpi-value">{profiles.length}</div>
          <div className="kpi-detail">Configured storage targets</div>
        </div>

        <div className="kpi-card">
          <div className="kpi-title">Active Schedules</div>
          <div className="kpi-value">{enabledSchedulesCount}</div>
          <div className="kpi-detail">
            Status: {schedulerStatus?.status || "Idle"} • {schedulerStatus?.running_profiles.length ?? 0} running
          </div>
        </div>

        <div className="kpi-card">
          <div className="kpi-title">Transfer Queue</div>
          <div className="kpi-value">{transferStatus?.active_count ?? 0}</div>
          <div className="kpi-detail">{transferStatus?.queued_count ?? 0} queued in background</div>
        </div>

        <div className="kpi-card">
          <div className="kpi-title">Total Snapshots</div>
          <div className="kpi-value">{snapshots.length}</div>
          <div className="kpi-detail">Recent catalog generations</div>
        </div>
      </div>

      {/* Quick Action Bar */}
      <div className="card" style={{ padding: "1rem 1.25rem" }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "0.75rem" }}>
          <div>
            <div style={{ fontWeight: 600, color: "var(--text-main)" }}>Quick Operations</div>
            <div style={{ fontSize: "0.78rem", color: "var(--text-muted)" }}>
              Directly invoke backup, restore, or verification engines
            </div>
          </div>
          <div style={{ display: "flex", gap: "0.5rem" }}>
            {profiles.length > 0 && (
              <button className="btn btn-primary" onClick={() => onStartBackup(profiles[0])}>
                ▶ Run Backup Now
              </button>
            )}
            <button className="btn btn-secondary" onClick={() => onNavigate("restore")}>
              🔄 Restore Files
            </button>
            <button className="btn btn-secondary" onClick={() => onNavigate("verification")}>
              🛡️ Verify Integrity
            </button>
            <button className="btn btn-secondary" onClick={() => onNavigate("schedules")}>
              ⏱️ Manage Schedules
            </button>
          </div>
        </div>
      </div>

      {/* Recent Snapshots & Verification Grid */}
      <div className="grid-2">
        {/* Recent Snapshots */}
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Recent Snapshots</div>
              <div className="card-subtitle">Point-in-time backup generations</div>
            </div>
            <button className="btn btn-secondary btn-sm" onClick={() => onNavigate("backups")}>
              View All
            </button>
          </div>

          {snapshots.length === 0 ? (
            <EmptyState
              title="No Snapshots Yet"
              description="Create a backup profile and run your first backup to generate snapshots."
              action={
                profiles.length > 0 ? (
                  <button className="btn btn-primary btn-sm" onClick={() => onStartBackup(profiles[0])}>
                    Run First Backup
                  </button>
                ) : (
                  <button className="btn btn-primary btn-sm" onClick={() => onNavigate("profiles")}>
                    Create Profile
                  </button>
                )
              }
            />
          ) : (
            <div className="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Profile</th>
                    <th>Snapshot ID</th>
                    <th>Status</th>
                    <th>Created</th>
                  </tr>
                </thead>
                <tbody>
                  {snapshots.map(({ profileName, snapshot }) => (
                    <tr key={`${profileName}-${snapshot.snapshot_id}`}>
                      <td style={{ fontWeight: 600 }}>{profileName}</td>
                      <td className="mono" style={{ color: "var(--primary)" }}>
                        {snapshot.snapshot_id.slice(0, 16)}...
                      </td>
                      <td>
                        <StatusBadge status={snapshot.status} />
                      </td>
                      <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                        {new Date(snapshot.created_at).toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Verification & Integrity Activity */}
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Integrity Audits</div>
              <div className="card-subtitle">Recent remote & local verification audits</div>
            </div>
            <button className="btn btn-secondary btn-sm" onClick={() => onNavigate("verification")}>
              Run Audit
            </button>
          </div>

          {recentVerifications.length === 0 ? (
            <EmptyState
              title="No Audits Performed"
              description="Run integrity verification to audit remote Telegram storage chunks and hashes."
              action={
                <button className="btn btn-secondary btn-sm" onClick={() => onNavigate("verification")}>
                  Start Verification
                </button>
              }
            />
          ) : (
            <div className="table-container">
              <table>
                <thead>
                  <tr>
                    <th>Level</th>
                    <th>Status</th>
                    <th>Total Chunks</th>
                    <th>Corrupted / Missing</th>
                    <th>Verified At</th>
                  </tr>
                </thead>
                <tbody>
                  {recentVerifications.map((v) => (
                    <tr key={v.history_id}>
                      <td style={{ fontWeight: 600 }}>L{v.level}</td>
                      <td>
                        <StatusBadge status={v.status} />
                      </td>
                      <td>{v.total_chunks}</td>
                      <td>{v.corrupted_chunks + v.missing_chunks}</td>
                      <td style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                        {new Date(v.verified_at).toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
