import { useState } from "react";
import { IpcError } from "../bindings";

interface ErrorBannerProps {
  error: IpcError | string | null;
  title?: string;
  onRetry?: () => void;
  onDismiss?: () => void;
}

export function ErrorBanner({
  error,
  title = "Operation Failed",
  onRetry,
  onDismiss,
}: ErrorBannerProps) {
  const [showDetails, setShowDetails] = useState(false);

  if (!error) return null;

  const isIpcError = typeof error === "object" && "code" in error;
  const code = isIpcError ? (error as IpcError).code : undefined;
  const message = isIpcError ? (error as IpcError).message : String(error);
  const details = isIpcError ? (error as IpcError).details : undefined;

  return (
    <div className="error-banner">
      <div className="error-banner-header">
        <div className="error-banner-title">
          <span>⚠️</span>
          <span>{title}</span>
          {code && <span className="error-code-badge">{code}</span>}
        </div>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          {onRetry && (
            <button
              className="btn btn-secondary btn-sm"
              onClick={onRetry}
              style={{ fontSize: "0.72rem", padding: "0.2rem 0.5rem" }}
            >
              Retry
            </button>
          )}
          {onDismiss && (
            <button
              onClick={onDismiss}
              style={{
                background: "none",
                border: "none",
                color: "var(--danger)",
                cursor: "pointer",
                fontSize: "1rem",
              }}
              title="Dismiss"
            >
              ×
            </button>
          )}
        </div>
      </div>

      <div className="error-banner-message">{message}</div>

      {details && (
        <div>
          <button
            onClick={() => setShowDetails(!showDetails)}
            style={{
              background: "none",
              border: "none",
              color: "#f87171",
              fontSize: "0.74rem",
              cursor: "pointer",
              textDecoration: "underline",
              padding: 0,
            }}
          >
            {showDetails ? "Hide technical details" : "Show technical details"}
          </button>
          {showDetails && (
            <div className="error-banner-details" style={{ marginTop: "0.4rem" }}>
              {details}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
