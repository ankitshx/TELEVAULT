interface StatusBadgeProps {
  status: string;
  label?: string;
}

export function StatusBadge({ status, label }: StatusBadgeProps) {
  const norm = status.toLowerCase();

  let badgeClass = "badge-muted";
  let dotColor = "var(--text-dim)";

  if (
    norm === "healthy" ||
    norm === "completed" ||
    norm === "success" ||
    norm === "active" ||
    norm === "enabled" ||
    norm === "ok"
  ) {
    badgeClass = "badge-success";
    dotColor = "var(--success)";
  } else if (
    norm === "warning" ||
    norm === "queued" ||
    norm === "pending" ||
    norm === "running" ||
    norm === "retrying" ||
    norm === "in_progress"
  ) {
    badgeClass = "badge-warning";
    dotColor = "var(--warning)";
  } else if (
    norm === "failed" ||
    norm === "error" ||
    norm === "cancelled" ||
    norm === "aborted" ||
    norm === "disabled"
  ) {
    badgeClass = "badge-danger";
    dotColor = "var(--danger)";
  } else if (
    norm === "repairable" ||
    norm === "info" ||
    norm === "skipped" ||
    norm === "repaired"
  ) {
    badgeClass = "badge-info";
    dotColor = "var(--info)";
  }

  return (
    <span className={`badge ${badgeClass}`}>
      <span
        style={{
          width: "6px",
          height: "6px",
          borderRadius: "50%",
          backgroundColor: dotColor,
          display: "inline-block",
        }}
      />
      {label || status}
    </span>
  );
}
