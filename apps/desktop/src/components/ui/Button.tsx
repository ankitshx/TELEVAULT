import React from "react";

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "cyan";
  size?: "sm" | "md" | "lg";
  icon?: React.ReactNode;
  iconPosition?: "left" | "right";
  loading?: boolean;
}

export const Button: React.FC<ButtonProps> = ({
  children,
  variant = "primary",
  size = "md",
  icon,
  iconPosition = "left",
  loading = false,
  disabled,
  style,
  className = "",
  ...props
}) => {
  const getVariantStyles = (): React.CSSProperties => {
    switch (variant) {
      case "primary":
        return {
          backgroundColor: "var(--accent-primary)",
          color: "#ffffff",
          border: "1px solid rgba(255, 255, 255, 0.15)",
          boxShadow: "0 2px 14px rgba(22, 131, 255, 0.35)",
        };
      case "secondary":
        return {
          backgroundColor: "var(--bg-surface-elevated)",
          color: "var(--text-primary)",
          border: "1px solid var(--border-card)",
        };
      case "cyan":
        return {
          backgroundColor: "var(--accent-cyan)",
          color: "#070B12",
          border: "1px solid rgba(255, 255, 255, 0.2)",
          boxShadow: "0 2px 14px rgba(0, 210, 255, 0.3)",
          fontWeight: 600,
        };
      case "danger":
        return {
          backgroundColor: "var(--status-error-subtle)",
          color: "var(--status-error)",
          border: "1px solid rgba(239, 68, 68, 0.25)",
        };
      case "ghost":
      default:
        return {
          backgroundColor: "transparent",
          color: "var(--text-secondary)",
          border: "1px solid transparent",
        };
    }
  };

  const getSizeStyles = (): React.CSSProperties => {
    switch (size) {
      case "sm":
        return {
          padding: "6px 12px",
          fontSize: "12px",
          gap: "6px",
          borderRadius: "var(--radius-sm)",
        };
      case "lg":
        return {
          padding: "12px 24px",
          fontSize: "15px",
          gap: "10px",
          borderRadius: "var(--radius-lg)",
        };
      case "md":
      default:
        return {
          padding: "8px 16px",
          fontSize: "13px",
          gap: "8px",
          borderRadius: "var(--radius-md)",
        };
    }
  };

  return (
    <button
      disabled={disabled || loading}
      style={{
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        fontWeight: 500,
        cursor: disabled || loading ? "not-allowed" : "pointer",
        opacity: disabled ? 0.5 : 1,
        transition: "all var(--transition-fast)",
        outline: "none",
        whiteSpace: "nowrap",
        ...getSizeStyles(),
        ...getVariantStyles(),
        ...style,
      }}
      className={`telecloud-button ${className}`}
      {...props}
    >
      {loading ? (
        <span
          style={{
            width: "14px",
            height: "14px",
            border: "2px solid currentColor",
            borderTopColor: "transparent",
            borderRadius: "50%",
            display: "inline-block",
            animation: "spin 0.6s linear infinite",
          }}
        />
      ) : (
        <>
          {icon && iconPosition === "left" && icon}
          {children}
          {icon && iconPosition === "right" && icon}
        </>
      )}
    </button>
  );
};
