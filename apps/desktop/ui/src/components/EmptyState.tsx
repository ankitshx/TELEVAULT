import { ReactNode } from "react";

interface EmptyStateProps {
  icon?: string;
  title: string;
  description: string;
  action?: ReactNode;
}

export function EmptyState({
  icon = "📁",
  title,
  description,
  action,
}: EmptyStateProps) {
  return (
    <div className="empty-state">
      <div className="empty-state-icon">{icon}</div>
      <div className="empty-state-title">{title}</div>
      <div className="empty-state-desc">{description}</div>
      {action && <div style={{ marginTop: "0.5rem" }}>{action}</div>}
    </div>
  );
}
