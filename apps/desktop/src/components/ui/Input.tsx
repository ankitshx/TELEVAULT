import React, { forwardRef } from "react";

export interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  icon?: React.ReactNode;
  iconRight?: React.ReactNode;
  label?: string;
  error?: string;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ icon, iconRight, label, error, style, ...props }, ref) => {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: "6px", width: "100%" }}>
        {label && (
          <label style={{ fontSize: "12px", fontWeight: 500, color: "var(--text-secondary)" }}>
            {label}
          </label>
        )}
        <div
          style={{
            position: "relative",
            display: "flex",
            alignItems: "center",
            width: "100%",
          }}
        >
          {icon && (
            <div
              style={{
                position: "absolute",
                left: "12px",
                display: "flex",
                alignItems: "center",
                color: "var(--text-muted)",
                pointerEvents: "none",
              }}
            >
              {icon}
            </div>
          )}
          <input
            ref={ref}
            style={{
              width: "100%",
              backgroundColor: "var(--bg-surface)",
              color: "var(--text-primary)",
              border: error ? "1px solid var(--status-error)" : "1px solid var(--border-card)",
              borderRadius: "var(--radius-md)",
              padding: `9px 14px 9px ${icon ? "38px" : "14px"}`,
              paddingRight: iconRight ? "38px" : "14px",
              fontSize: "13px",
              outline: "none",
              transition: "border-color var(--transition-fast), box-shadow var(--transition-fast)",
              ...style,
            }}
            onFocus={(e) => {
              e.currentTarget.style.borderColor = "var(--border-focus)";
              e.currentTarget.style.boxShadow = "0 0 0 2px var(--accent-subtle)";
              props.onFocus?.(e);
            }}
            onBlur={(e) => {
              e.currentTarget.style.borderColor = error ? "var(--status-error)" : "var(--border-card)";
              e.currentTarget.style.boxShadow = "none";
              props.onBlur?.(e);
            }}
            {...props}
          />
          {iconRight && (
            <div
              style={{
                position: "absolute",
                right: "12px",
                display: "flex",
                alignItems: "center",
                color: "var(--text-muted)",
              }}
            >
              {iconRight}
            </div>
          )}
        </div>
        {error && (
          <span style={{ fontSize: "11px", color: "var(--status-error)" }}>
            {error}
          </span>
        )}
      </div>
    );
  }
);

Input.displayName = "Input";
