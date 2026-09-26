import React from "react";
import {
  UploadCloud,
  DownloadCloud,
  Pause,
  Play,
  X,
  CheckCircle2,
  AlertCircle,
} from "lucide-react";
import { TransferItem } from "../../types";
import { formatBytes, formatEta, formatSpeed } from "../../utils/format";
import { ProgressBar } from "../ui/ProgressBar";
import { Badge } from "../ui/Badge";

export interface TransferCardProps {
  transfer: TransferItem;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
  onCancel: (id: string) => void;
}

export const TransferCard: React.FC<TransferCardProps> = ({
  transfer,
  onPause,
  onResume,
  onCancel,
}) => {
  const isUpload = transfer.direction === "UPLOAD";
  const progress =
    transfer.total_bytes > 0
      ? Math.round((transfer.transferred_bytes / transfer.total_bytes) * 100)
      : 0;

  const getStateBadge = () => {
    switch (transfer.state) {
      case "UPLOADING":
      case "DOWNLOADING":
        return (
          <Badge variant="blue" size="sm">
            {isUpload ? "Uploading" : "Downloading"}
          </Badge>
        );
      case "PAUSED":
        return (
          <Badge variant="warning" size="sm">
            Paused
          </Badge>
        );
      case "COMPLETED":
        return (
          <Badge variant="success" size="sm" icon={<CheckCircle2 size={11} />}>
            Completed
          </Badge>
        );
      case "FAILED":
        return (
          <Badge variant="error" size="sm" icon={<AlertCircle size={11} />}>
            Failed
          </Badge>
        );
      case "CANCELLED":
        return (
          <Badge variant="neutral" size="sm">
            Cancelled
          </Badge>
        );
      default:
        return (
          <Badge variant="cyan" size="sm">
            Queued
          </Badge>
        );
    }
  };

  return (
    <div
      className="telecloud-card"
      style={{
        padding: "16px 20px",
        display: "flex",
        flexDirection: "column",
        gap: "12px",
      }}
    >
      {/* Top Row: Icon + Filename + State + Actions */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "12px", minWidth: 0 }}>
          <div
            style={{
              width: "36px",
              height: "36px",
              borderRadius: "var(--radius-md)",
              backgroundColor: isUpload ? "var(--accent-subtle)" : "var(--accent-cyan-subtle)",
              color: isUpload ? "var(--accent-primary)" : "var(--accent-cyan)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            {isUpload ? <UploadCloud size={18} /> : <DownloadCloud size={18} />}
          </div>

          <div style={{ minWidth: 0 }}>
            <h4
              style={{
                fontSize: "13px",
                fontWeight: 600,
                color: "var(--text-primary)",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
              title={transfer.file_name}
            >
              {transfer.file_name}
            </h4>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "8px",
                fontSize: "11px",
                color: "var(--text-muted)",
                marginTop: "2px",
              }}
            >
              <span>{formatBytes(transfer.transferred_bytes)} / {formatBytes(transfer.total_bytes)}</span>
              {transfer.speed_bytes_per_sec > 0 && (
                <>
                  <span>•</span>
                  <span style={{ color: "var(--accent-primary)", fontWeight: 600 }}>
                    {formatSpeed(transfer.speed_bytes_per_sec)}
                  </span>
                </>
              )}
              {transfer.eta_seconds && transfer.eta_seconds > 0 ? (
                <>
                  <span>•</span>
                  <span>{formatEta(transfer.eta_seconds)}</span>
                </>
              ) : null}
            </div>
          </div>
        </div>

        {/* State Badge & Control Buttons */}
        <div style={{ display: "flex", alignItems: "center", gap: "10px", flexShrink: 0 }}>
          {getStateBadge()}

          {transfer.state === "UPLOADING" || transfer.state === "DOWNLOADING" ? (
            <button
              onClick={() => onPause(transfer.id)}
              style={{
                background: "transparent",
                border: "1px solid var(--border-card)",
                borderRadius: "var(--radius-sm)",
                color: "var(--text-secondary)",
                padding: "6px",
                cursor: "pointer",
                display: "flex",
              }}
              title="Pause Transfer"
            >
              <Pause size={14} />
            </button>
          ) : transfer.state === "PAUSED" ? (
            <button
              onClick={() => onResume(transfer.id)}
              style={{
                background: "var(--accent-subtle)",
                border: "1px solid rgba(22, 131, 255, 0.3)",
                borderRadius: "var(--radius-sm)",
                color: "var(--accent-primary)",
                padding: "6px",
                cursor: "pointer",
                display: "flex",
              }}
              title="Resume Transfer"
            >
              <Play size={14} />
            </button>
          ) : null}

          {transfer.state !== "COMPLETED" && transfer.state !== "CANCELLED" && (
            <button
              onClick={() => onCancel(transfer.id)}
              style={{
                background: "transparent",
                border: "1px solid var(--border-card)",
                borderRadius: "var(--radius-sm)",
                color: "var(--text-muted)",
                padding: "6px",
                cursor: "pointer",
                display: "flex",
              }}
              title="Cancel Transfer"
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      {/* Progress Bar */}
      {transfer.state !== "COMPLETED" && (
        <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
          <ProgressBar
            progress={progress}
            height={5}
            color={
              transfer.state === "FAILED"
                ? "var(--status-error)"
                : transfer.state === "PAUSED"
                ? "var(--status-warning)"
                : "var(--accent-primary)"
            }
          />
          <span style={{ fontSize: "11px", fontWeight: 700, color: "var(--text-secondary)", width: "32px" }}>
            {progress}%
          </span>
        </div>
      )}

      {/* Error Message if Failed */}
      {transfer.error_message && (
        <div style={{ fontSize: "11px", color: "var(--status-error)", backgroundColor: "var(--status-error-subtle)", padding: "6px 10px", borderRadius: "var(--radius-xs)" }}>
          {transfer.error_message}
        </div>
      )}
    </div>
  );
};
