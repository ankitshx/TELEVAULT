import { useEffect, useState, useCallback } from "react";
import {
  commands,
  BackupProfileDto,
  SnapshotDto,
  TransferStatusDto,
  SchedulerStatusDto,
  ScheduleDto,
  TelegramAuthStatusDto,
  VerificationHistoryRecordDto,
} from "../bindings";
import { StatusBadge } from "../components/StatusBadge";
import { LoadingSpinner } from "../components/LoadingSpinner";
import { ErrorBanner } from "../components/ErrorBanner";
import { EmptyState } from "../components/EmptyState";
import { NavTab } from "../types";

interface DashboardViewProps {
  onNavigate: (tab: NavTab, profileId?: string, snapshotId?: string) => void;
  onStartBackup: (profile: BackupProfileDto) => void;
}

export function DashboardView({ onNavigate, onStartBackup }: DashboardViewProps) {
  const [profiles, setProfiles] = useState<BackupProfileDto[]>([]);
  const [snapshots, setSnapshots] = useState<{ profileName: string; snapshot: SnapshotDto }[]>([]);
  const [transferStatus, setTransferStatus] = useState<TransferStatusDto | null>(null);
  const [schedulerStatus, setSchedulerStatus] = useState<SchedulerStatusDto | null>(null);
  const [schedules, setSchedules] = useState<ScheduleDto[]>([]);
  const [authStatus, setAuthStatus] = useState<TelegramAuthStatusDto | null>(null);
  const [recentVerifications, setRecentVerifications] = useState<VerificationHistoryRecordDto[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  const loadDashboardData = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const [profRes, transferRes, schedStatusRes, schedListRes, authRes] = await Promise.all([
        commands.listBackupProfiles(),
        commands.getTransferStatus(),
        commands.getSchedulerStatus(),
        commands.listSchedules(),
        commands.getTelegramAuthStatus(),
      ]);

      if (profRes.status !== "ok") throw new Error(profRes.error.message);
      if (transferRes.status !== "ok") throw new Error(transferRes.error.message);
      if (schedStatusRes.status !== "ok") throw new Error(schedStatusRes.error.message);
      if (schedListRes.status !== "ok") throw new Error(schedListRes.error.message);
      if (authRes.status !== "ok") throw new Error(authRes.error.message);

      setProfiles(profRes.data);
      setTransferStatus(transferRes.data);
      setSchedulerStatus(schedStatusRes.data);
      setSchedules(schedListRes.data);
      setAuthStatus(authRes.data);

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
    return <LoadingSpinner message="Aggregating secure vault telemetry..." />;
  }

  const latestSnapshot = snapshots[0]?.snapshot;
  const totalSnapshotsCount = snapshots.length;

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
  };

  const formatRelativeTime = (dateStr: string) => {
    try {
      const d = new Date(dateStr);
      const diffMs = Date.now() - d.getTime();
      const diffMins = Math.floor(diffMs / (1000 * 60));
      if (diffMins < 1) return "Just now";
      if (diffMins < 60) return `${diffMins}m ago`;
      const diffHours = Math.floor(diffMins / 60);
      if (diffHours < 24) return `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      return `${diffDays}d ago`;
    } catch {
      return dateStr;
    }
  };

  return (
    <div className="dashboard-container">
      {error && <ErrorBanner error={error} title="Dashboard Telemetry Error" onRetry={loadDashboardData} />}

      {/* Friendly Hero Greeting Banner */}
      <div className="dashboard-hero-banner">
        <div className="hero-content">
          <div className="hero-badge">
            <span className="hero-pulse-dot" />
            <span>Vault Protected</span>
          </div>
          <h1 className="hero-title">Your files. Always safe.</h1>
          <p className="hero-subtitle">
            {authStatus?.account ? (
              <>
                Connected as <strong>{authStatus.account.first_name}</strong> • Backing up to dedicated private channel{" "}
                <strong>&quot;{authStatus.channel?.channel_title || "TELEVAULT Backup Vault"}&quot;</strong>
              </>
            ) : (
              "End-to-end encrypted backup repository active across your designated local targets."
            )}
          </p>
        </div>
        <div className="hero-actions">
          {profiles.length > 0 && (
            <button
              type="button"
              className="btn btn-primary btn-lg"
              onClick={() => onStartBackup(profiles[0])}
            >
              ⚡ Run Backup Now
            </button>
          )}
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => onNavigate("restore")}
          >
            🔄 Restore Files
          </button>
        </div>
      </div>

      {/* Summary KPI Cards */}
      <div className="grid-4">
        <div className="kpi-card" onClick={() => onNavigate("profiles")} role="button" tabIndex={0}>
          <div className="kpi-icon-wrap icon-cyan">📁</div>
          <div>
            <div className="kpi-title">Backup Profiles</div>
            <div className="kpi-value">{profiles.length}</div>
            <div className="kpi-detail">My Backups • Active Profiles</div>
          </div>
        </div>

        <div className="kpi-card" onClick={() => onNavigate("schedules")} role="button" tabIndex={0}>
          <div className="kpi-icon-wrap icon-emerald">⏱️</div>
          <div>
            <div className="kpi-title">Active Schedules</div>
            <div className="kpi-value">{schedules.filter((s) => s.enabled).length}</div>
            <div className="kpi-detail">
              {latestSnapshot ? `Last: ${formatRelativeTime(latestSnapshot.created_at)}` : "Backup Schedule"}
            </div>
          </div>
        </div>

        <div className="kpi-card" onClick={() => onNavigate("activity")} role="button" tabIndex={0}>
          <div className="kpi-icon-wrap icon-indigo">⚡</div>
          <div>
            <div className="kpi-title">Transfer Queue</div>
            <div className="kpi-value">{transferStatus?.active_count || 0}</div>
            <div className="kpi-detail">Active transfers in progress</div>
          </div>
        </div>

        <div className="kpi-card" onClick={() => onNavigate("backups")} role="button" tabIndex={0}>
          <div className="kpi-icon-wrap icon-blue">📦</div>
          <div>
            <div className="kpi-title">Total Snapshots</div>
            <div className="kpi-value">{snapshots.length}</div>
            <div className="kpi-detail">Total Backup Versions</div>
          </div>
        </div>
      </div>

      {/* Primary Action Cards Grid */}
      <div className="dashboard-section-header">
        <h2>Quick Actions</h2>
        <span className="section-caption">Common desktop workflow operations</span>
      </div>

      <div className="grid-4">
        <div
          className="action-card"
          onClick={() => {
            if (profiles.length > 0) {
              onStartBackup(profiles[0]);
            } else {
              onNavigate("profiles");
            }
          }}
        >
          <div className="action-icon">🚀</div>
          <div className="action-title">Back Up Now</div>
          <div className="action-desc">Run an immediate incremental snapshot scan on your primary profile.</div>
        </div>

        <div className="action-card" onClick={() => onNavigate("restore")}>
          <div className="action-icon">🔄</div>
          <div className="action-title">Restore Files</div>
          <div className="action-desc">Reconstruct backed-up files or entire folders with verified hashes.</div>
        </div>

        <div className="action-card" onClick={() => onNavigate("schedules")}>
          <div className="action-icon">⏰</div>
          <div className="action-title">Backup Schedule</div>
          <div className="action-desc">
            Configure automated daily, hourly, or cron triggers ({schedulerStatus?.active_schedules_count || 0} active).
          </div>
        </div>

        <div className="action-card" onClick={() => onNavigate("verification")}>
          <div className="action-icon">🛡️</div>
          <div className="action-title">Verify Backups</div>
          <div className="action-desc">Perform cryptographic audits and chunk integrity verification.</div>
        </div>
      </div>

      {/* Cloud & Local Vault Overview Card */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Storage & Cloud Connection Overview</div>
            <div className="card-subtitle">Personal Telegram MTProto integration and dedicated backup vault</div>
          </div>
          <StatusBadge status={authStatus?.state === "ready" ? "ready" : "warning"} label="Vault Active" />
        </div>

        <div className="grid-3" style={{ gap: "1rem" }}>
          <div className="overview-subcard">
            <div className="subcard-label">Personal Telegram Account</div>
            <div className="subcard-main">
              {authStatus?.account ? (
                <>
                  <div className="account-name">
                    {authStatus.account.first_name} {authStatus.account.last_name || ""}
                  </div>
                  <div className="account-meta">
                    {authStatus.account.username ? `@${authStatus.account.username} • ` : ""}
                    {authStatus.account.phone_number}
                  </div>
                </>
              ) : (
                <div className="text-muted">No account authenticated</div>
              )}
            </div>
            <div className="subcard-footer">Direct MTProto protocol</div>
          </div>

          <div className="overview-subcard">
            <div className="subcard-label">Dedicated Backup Channel</div>
            <div className="subcard-main">
              {authStatus?.channel ? (
                <>
                  <div className="channel-title">{authStatus.channel.channel_title}</div>
                  <div className="channel-meta mono">
                    ID: {authStatus.channel.channel_id}
                  </div>
                </>
              ) : (
                <div className="text-muted">No channel configured</div>
              )}
            </div>
            <div className="subcard-footer" style={{ color: "var(--success)" }}>
              ✓ Private & Verified for Backups
            </div>
          </div>

          <div className="overview-subcard">
            <div className="subcard-label">Local Database & Engine</div>
            <div className="subcard-main">
              <div className="engine-title">SQLite WAL + Foreign Keys</div>
              <div className="engine-meta">
                Zero sidecars • Single desktop process
              </div>
            </div>
            <div className="subcard-footer">
              Scheduler: {schedulerStatus?.status || "Running"}
            </div>
          </div>
        </div>
      </div>

      {/* Recent Backups Table */}
      <div className="card">
        <div className="card-header">
          <div>
            <div className="card-title">Recent Backups</div>
            <div className="card-subtitle">Point-in-time snapshots created across backup profiles</div>
          </div>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => onNavigate("backups")}
          >
            View All Backups ➜
          </button>
        </div>

        {snapshots.length === 0 ? (
          <EmptyState
            icon="💾"
            title="No Backups Recorded Yet"
            description="Protect your important files by creating your first backup profile and running a snapshot."
            action={
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => onNavigate("profiles")}
              >
                Create Backup Profile
              </button>
            }
          />
        ) : (
          <div className="table-responsive">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Snapshot ID</th>
                  <th>Profile</th>
                  <th>Created At</th>
                  <th>Status</th>
                  <th>Payload Info</th>
                  <th style={{ textAlign: "right" }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {snapshots.map(({ profileName, snapshot }) => {
                  let parsedMeta: { total_files?: number; total_bytes?: number } = {};
                  try {
                    parsedMeta = JSON.parse(snapshot.metadata || "{}");
                  } catch {
                    // ignore
                  }

                  return (
                    <tr key={`${profileName}-${snapshot.snapshot_id}`}>
                      <td className="mono" style={{ color: "var(--primary)" }}>
                        {snapshot.snapshot_id.slice(0, 16)}...
                      </td>
                      <td>
                        <strong>{profileName}</strong>
                      </td>
                      <td>
                        <div>{new Date(snapshot.created_at).toLocaleString()}</div>
                        <div style={{ fontSize: "0.75rem", color: "var(--text-dim)" }}>
                          {formatRelativeTime(snapshot.created_at)}
                        </div>
                      </td>
                      <td>
                        <StatusBadge status={snapshot.status} />
                      </td>
                      <td style={{ fontSize: "0.85rem" }}>
                        {parsedMeta.total_files !== undefined ? (
                          <span>
                            {parsedMeta.total_files} files • {formatBytes(parsedMeta.total_bytes || 0)}
                          </span>
                        ) : (
                          <span className="text-muted">—</span>
                        )}
                      </td>
                      <td style={{ textAlign: "right" }}>
                        <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end" }}>
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm"
                            onClick={() => onNavigate("restore", snapshot.profile_id, snapshot.snapshot_id)}
                          >
                            Restore
                          </button>
                          <button
                            type="button"
                            className="btn btn-secondary btn-sm"
                            onClick={() => onNavigate("verification", snapshot.profile_id, snapshot.snapshot_id)}
                          >
                            Verify
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

      {/* Recent Verifications Card */}
      {recentVerifications.length > 0 && (
        <div className="card">
          <div className="card-header">
            <div>
              <div className="card-title">Recent Cryptographic Verifications</div>
              <div className="card-subtitle">Integrity audits of remote Telegram storage chunks</div>
            </div>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => onNavigate("verification")}
            >
              Audits View ➜
            </button>
          </div>
          <div className="table-responsive">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Audit Timestamp</th>
                  <th>Target Type</th>
                  <th>Target ID</th>
                  <th>Status</th>
                  <th>Summary</th>
                </tr>
              </thead>
              <tbody>
                {recentVerifications.map((v) => (
                  <tr key={v.history_id}>
                    <td>{new Date(v.verified_at).toLocaleString()}</td>
                    <td><span className="badge badge-info">{v.target_type}</span></td>
                    <td className="mono">{v.target_id.slice(0, 16)}...</td>
                    <td><StatusBadge status={v.status} /></td>
                    <td style={{ fontSize: "0.8rem", color: "var(--text-muted)" }}>
                      Restore ready: {v.is_restore_ready ? "✓ Yes" : "✕ No"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}
