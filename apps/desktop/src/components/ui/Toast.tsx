import React, { useEffect } from "react";
import { CheckCircle2, AlertTriangle, XCircle, Info, X } from "lucide-react";
import { ToastMessage } from "../../types";

export interface ToastProps {
  toasts: ToastMessage[];
  onDismiss: (id: string) => void;
}

export const ToastContainer: React.FC<ToastProps> = ({ toasts, onDismiss }) => {
  return (
    <div
      style={{
        position: "fixed",
        bottom: "24px",
        right: "24px",
        display: "flex",
        flexDirection: "column",
        gap: "10px",
        zIndex: 3000,
        pointerEvents: "none",
        maxWidth: "380px",
        width: "100%",
      }}
    >
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </div>
  );
};

const ToastItem: React.FC<{ toast: ToastMessage; onDismiss: (id: string) => void }> = ({
  toast,
  onDismiss,
}) => {
  useEffect(() => {
    const timer = setTimeout(() => {
      onDismiss(toast.id);
    }, 4500);
    return () => clearTimeout(timer);
  }, [toast.id, onDismiss]);

  const getIcon = () => {
    switch (toast.type) {
      case "success":
        return <CheckCircle2 size={18} color="var(--status-success)" />;
      case "warning":
        return <AlertTriangle size={18} color="var(--status-warning)" />;
      case "error":
        return <XCircle size={18} color="var(--status-error)" />;
      case "info":
      default:
        return <Info size={18} color="var(--accent-cyan)" />;
    }
  };

  return (
    <div
      className="glass-surface-elevated animate-slide-up"
      style={{
        pointerEvents: "auto",
        display: "flex",
        alignItems: "flex-start",
        gap: "12px",
        padding: "14px 16px",
        borderRadius: "var(--radius-lg)",
        backgroundColor: "var(--bg-surface-elevated)",
        border: "1px solid var(--border-card)",
        boxShadow: "var(--shadow-elevated)",
      }}
    >
      <div style={{ marginTop: "1px", flexShrink: 0 }}>{getIcon()}</div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <h4 style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
          {toast.title}
        </h4>
        {toast.message && (
          <p
            style={{
              fontSize: "12px",
              color: "var(--text-secondary)",
              marginTop: "2px",
              lineHeight: 1.4,
            }}
          >
            {toast.message}
          </p>
        )}
      </div>
      <button
        onClick={() => onDismiss(toast.id)}
        style={{
          background: "transparent",
          border: "none",
          color: "var(--text-muted)",
          cursor: "pointer",
          padding: "2px",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          borderRadius: "var(--radius-xs)",
        }}
      >
        <X size={14} />
      </button>
    </div>
  );
};
