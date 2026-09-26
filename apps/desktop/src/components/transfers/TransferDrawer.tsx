import React from "react";
import { UploadCloud, DownloadCloud, Pause, Play, ChevronUp } from "lucide-react";
import { TransferItem } from "../../types";
import { formatBytes, formatSpeed } from "../../utils/format";
import { ProgressBar } from "../ui/ProgressBar";

export interface TransferDrawerProps {
  activeTransfer?: TransferItem;
  totalActiveCount: number;
  onOpenTransferCenter: () => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
}

export const TransferDrawer: React.FC<TransferDrawerProps> = ({
  activeTransfer,
  totalActiveCount,
  onOpenTransferCenter,
  onPause,
  onResume,
}) => {
  if (!activeTransfer) return null;

  const isUpload = activeTransfer.direction === "UPLOAD";
  const progress =
    activeTransfer.total_bytes > 0
      ? Math.round((activeTransfer.transferred_bytes / activeTransfer.total_bytes) * 100)
      : 0;

  return (
    <div
      className="glass-surface-elevated animate-slide-up"
      style={{
        position: "fixed",
        bottom: 0,
        left: "250px", // Align with sidebar
        right: 0,
        height: "56px",
        backgroundColor: "var(--bg-surface-elevated)",
        borderTop: "1px solid var(--border-card)",
        padding: "0 24px",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "24px",
        zIndex: 500,
        boxShadow: "0 -4px 20px rgba(0, 0, 0, 0.5)",
      }}
    >
      {/* Transfer Information */}
      <div style={{ display: "flex", alignItems: "center", gap: "14px", minWidth: 0, flex: 1 }}>
        <div
          style={{
            width: "32px",
            height: "32px",
            borderRadius: "var(--radius-sm)",
            backgroundColor: isUpload ? "var(--accent-subtle)" : "var(--accent-cyan-subtle)",
            color: isUpload ? "var(--accent-primary)" : "var(--accent-cyan)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            flexShrink: 0,
          }}
        >
          {isUpload ? <UploadCloud size={16} /> : <DownloadCloud size={16} />}
        </div>

        <div style={{ minWidth: 0 }}>
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <span
              style={{
                fontSize: "12px",
                fontWeight: 600,
                color: "var(--text-primary)",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
                maxWidth: "240px",
              }}
              title={activeTransfer.file_name}
            >
              {activeTransfer.file_name}
            </span>
            {totalActiveCount > 1 && (
              <span
                style={{
                  fontSize: "10px",
                  color: "var(--text-muted)",
                  backgroundColor: "rgba(255, 255, 255, 0.06)",
                  padding: "1px 6px",
                  borderRadius: "var(--radius-full)",
                }}
              >
                +{totalActiveCount - 1} more
              </span>
            )}
          </div>
          <span style={{ fontSize: "11px", color: "var(--text-muted)" }}>
            {formatBytes(activeTransfer.transferred_bytes)} / {formatBytes(activeTransfer.total_bytes)}
            {activeTransfer.speed_bytes_per_sec > 0 && ` • ${formatSpeed(activeTransfer.speed_bytes_per_sec)}`}
          </span>
        </div>
      </div>

      {/* Progress Bar & Percentage */}
      <div style={{ display: "flex", alignItems: "center", gap: "12px", width: "300px" }}>
        <ProgressBar progress={progress} height={5} color="var(--accent-primary)" />
        <span style={{ fontSize: "12px", fontWeight: 700, color: "var(--text-primary)", width: "36px" }}>
          {progress}%
        </span>
      </div>

      {/* Controls & Open Transfer Center Button */}
      <div style={{ display: "flex", alignItems: "center", gap: "10px", flexShrink: 0 }}>
        {activeTransfer.state === "UPLOADING" || activeTransfer.state === "DOWNLOADING" ? (
          <button
            onClick={() => onPause(activeTransfer.id)}
            style={{
              background: "transparent",
              border: "1px solid var(--border-card)",
              borderRadius: "var(--radius-sm)",
              color: "var(--text-secondary)",
              padding: "6px",
              cursor: "pointer",
              display: "flex",
            }}
            title="Pause"
          >
            <Pause size={14} />
          </button>
        ) : activeTransfer.state === "PAUSED" ? (
          <button
            onClick={() => onResume(activeTransfer.id)}
            style={{
              background: "var(--accent-subtle)",
              border: "1px solid rgba(22, 131, 255, 0.3)",
              borderRadius: "var(--radius-sm)",
              color: "var(--accent-primary)",
              padding: "6px",
              cursor: "pointer",
              display: "flex",
            }}
            title="Resume"
          >
            <Play size={14} />
          </button>
        ) : null}

        <button
          onClick={onOpenTransferCenter}
          style={{
            background: "transparent",
            border: "1px solid var(--border-card)",
            borderRadius: "var(--radius-sm)",
            color: "var(--text-secondary)",
            padding: "5px 10px",
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: "6px",
            fontSize: "12px",
          }}
        >
          <span>View All</span>
          <ChevronUp size={14} />
        </button>
      </div>
    </div>
  );
};
