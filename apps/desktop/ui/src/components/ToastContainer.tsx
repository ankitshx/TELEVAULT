import { ToastMessage } from "../types";

interface ToastContainerProps {
  toasts: ToastMessage[];
  onDismiss: (id: string) => void;
}

export function ToastContainer({ toasts, onDismiss }: ToastContainerProps) {
  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map((toast) => {
        let icon = "ℹ️";
        if (toast.type === "success") icon = "✅";
        if (toast.type === "error") icon = "❌";
        if (toast.type === "warning") icon = "⚠️";

        return (
          <div key={toast.id} className={`toast toast-${toast.type}`}>
            <span className="toast-icon">{icon}</span>
            <div className="toast-content">
              <div className="toast-title">{toast.title}</div>
              <div className="toast-message">{toast.message}</div>
            </div>
            <button
              className="toast-close"
              onClick={() => onDismiss(toast.id)}
              title="Close"
            >
              ✕
            </button>
          </div>
        );
      })}
    </div>
  );
}
