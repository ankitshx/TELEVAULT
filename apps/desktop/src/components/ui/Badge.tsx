import React from "react";

export interface BadgeProps {
  children: React.ReactNode;
  variant?: "blue" | "cyan" | "success" | "warning" | "error" | "neutral";
  size?: "sm" | "md";
  icon?: React.ReactNode;
  style?: React.CSSProperties;
}

export const Badge: React.FC<BadgeProps> = ({
  children,
  variant = "blue",
  size = "md",
  icon,
  style,
}) => {
  const getVariantStyles = (): React.CSSProperties => {
    switch (variant) {
      case "cyan":
        return {
          backgroundColor: "var(--accent-cyan-subtle)",
          color: "var(--accent-cyan)",
          border: "1px solid rgba(0, 210, 255, 0.25)",
        };
      case "success":
        return {
          backgroundColor: "var(--status-success-subtle)",
          color: "var(--status-success)",
          border: "1px solid rgba(16, 185, 129, 0.25)",
        };
      case "warning":
        return {
          backgroundColor: "var(--status-warning-subtle)",
          color: "var(--status-warning)",
          border: "1px solid rgba(245, 158, 11, 0.25)",
        };
      case "error":
        return {
          backgroundColor: "var(--status-error-subtle)",
          color: "var(--status-error)",
          border: "1px solid rgba(239, 68, 68, 0.25)",
        };
      case "neutral":
        return {
          backgroundColor: "rgba(255, 255, 255, 0.05)",
          color: "var(--text-secondary)",
          border: "1px solid var(--border-subtle)",
        };
      case "blue":
      default:
        return {
          backgroundColor: "var(--accent-subtle)",
          color: "var(--accent-hover)",
          border: "1px solid rgba(22, 131, 255, 0.25)",
        };
    }
  };

  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        padding: size === "sm" ? "2px 6px" : "3px 8px",
        fontSize: size === "sm" ? "10px" : "11px",
        fontWeight: 600,
        letterSpacing: "0.02em",
        borderRadius: "var(--radius-full)",
        lineHeight: 1.2,
        ...getVariantStyles(),
        ...style,
      }}
    >
      {icon && <span style={{ display: "inline-flex", alignItems: "center" }}>{icon}</span>}
      {children}
    </span>
  );
};
