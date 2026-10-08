import { StatusBadge } from "./StatusBadge";

interface HeaderProps {
  title: string;
  subtitle: string;
  schedulerStatus: string;
  activeTransfers: number;
  onRefresh: () => void;
  isRefreshing: boolean;
}

export function Header({
  title,
  subtitle,
  schedulerStatus,
  activeTransfers,
  onRefresh,
  isRefreshing,
}: HeaderProps) {
  return (
    <header className="header">
      <div className="header-title-group">
        <h2>{title}</h2>
        <p>{subtitle}</p>
      </div>

      <div className="header-status-group">
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <span style={{ fontSize: "0.75rem", color: "var(--text-dim)" }}>Scheduler:</span>
          <StatusBadge status={schedulerStatus} />
        </div>

        {activeTransfers > 0 && (
          <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
            <span style={{ fontSize: "0.75rem", color: "var(--text-dim)" }}>Transfers:</span>
            <span className="badge badge-warning">
              {activeTransfers} Active
            </span>
          </div>
        )}

        <button
          className="btn btn-secondary btn-sm"
          onClick={onRefresh}
          disabled={isRefreshing}
          title="Refresh telemetry and records from in-process core"
        >
          {isRefreshing ? "Refreshing..." : "↻ Refresh"}
        </button>
      </div>
    </header>
  );
}
