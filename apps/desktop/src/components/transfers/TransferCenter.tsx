import React, { useState } from "react";
import { ArrowUpDown, CheckCircle2, AlertCircle, Clock } from "lucide-react";
import { TransferItem } from "../../types";
import { TransferCard } from "./TransferCard";
import { EmptyState } from "../ui/EmptyState";
import { formatSpeed } from "../../utils/format";

export interface TransferCenterProps {
  transfers: TransferItem[];
  onPauseTransfer: (id: string) => void;
  onResumeTransfer: (id: string) => void;
  onCancelTransfer: (id: string) => void;
}

export const TransferCenter: React.FC<TransferCenterProps> = ({
  transfers,
  onPauseTransfer,
  onResumeTransfer,
  onCancelTransfer,
}) => {
  const [activeTab, setActiveTab] = useState<"active" | "completed" | "failed">("active");

  const activeTransfers = transfers.filter(
    (t) =>
      t.state === "UPLOADING" ||
      t.state === "DOWNLOADING" ||
      t.state === "PREPARING" ||
      t.state === "QUEUED" ||
      t.state === "PAUSED"
  );

  const completedTransfers = transfers.filter((t) => t.state === "COMPLETED");
  const failedTransfers = transfers.filter((t) => t.state === "FAILED" || t.state === "CANCELLED");

  const totalSpeed = activeTransfers.reduce((acc, t) => acc + (t.speed_bytes_per_sec || 0), 0);

  const getFilteredTransfers = () => {
    switch (activeTab) {
      case "completed":
        return completedTransfers;
      case "failed":
        return failedTransfers;
      case "active":
      default:
        return activeTransfers;
    }
  };

  const currentList = getFilteredTransfers();

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        padding: "28px 32px",
        overflowY: "auto",
        gap: "24px",
      }}
    >
      {/* Header & Speed Metric */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-end" }}>
        <div>
          <h2 style={{ fontSize: "22px", fontWeight: 700, color: "var(--text-primary)" }}>
            Transfer Center
          </h2>
          <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginTop: "4px" }}>
            Monitor and manage asynchronous streaming transfers.
          </p>
        </div>

        {activeTransfers.length > 0 && (
          <div
            style={{
              padding: "8px 16px",
              backgroundColor: "var(--accent-subtle)",
              borderRadius: "var(--radius-md)",
              border: "1px solid rgba(22, 131, 255, 0.25)",
              display: "flex",
              alignItems: "center",
              gap: "8px",
            }}
          >
            <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>Current Speed:</span>
            <span style={{ fontSize: "14px", fontWeight: 700, color: "var(--accent-primary)" }}>
              {formatSpeed(totalSpeed)}
            </span>
          </div>
        )}
      </div>

      {/* Tabs */}
      <div
        style={{
          display: "flex",
          gap: "8px",
          borderBottom: "1px solid var(--border-subtle)",
          paddingBottom: "12px",
        }}
      >
        <button
          onClick={() => setActiveTab("active")}
          style={{
            padding: "8px 16px",
            borderRadius: "var(--radius-md)",
            border: "none",
            backgroundColor: activeTab === "active" ? "var(--accent-subtle)" : "transparent",
            color: activeTab === "active" ? "var(--text-primary)" : "var(--text-secondary)",
            fontSize: "13px",
            fontWeight: activeTab === "active" ? 600 : 500,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: "8px",
          }}
        >
          <Clock size={16} />
          <span>Active Queue ({activeTransfers.length})</span>
        </button>

        <button
          onClick={() => setActiveTab("completed")}
          style={{
            padding: "8px 16px",
            borderRadius: "var(--radius-md)",
            border: "none",
            backgroundColor: activeTab === "completed" ? "var(--accent-subtle)" : "transparent",
            color: activeTab === "completed" ? "var(--text-primary)" : "var(--text-secondary)",
            fontSize: "13px",
            fontWeight: activeTab === "completed" ? 600 : 500,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: "8px",
          }}
        >
          <CheckCircle2 size={16} />
          <span>Completed ({completedTransfers.length})</span>
        </button>

        <button
          onClick={() => setActiveTab("failed")}
          style={{
            padding: "8px 16px",
            borderRadius: "var(--radius-md)",
            border: "none",
            backgroundColor: activeTab === "failed" ? "var(--accent-subtle)" : "transparent",
            color: activeTab === "failed" ? "var(--text-primary)" : "var(--text-secondary)",
            fontSize: "13px",
            fontWeight: activeTab === "failed" ? 600 : 500,
            cursor: "pointer",
            display: "flex",
            alignItems: "center",
            gap: "8px",
          }}
        >
          <AlertCircle size={16} />
          <span>Failed / Cancelled ({failedTransfers.length})</span>
        </button>
      </div>

      {/* Transfers List */}
      {currentList.length === 0 ? (
        <EmptyState
          icon={<ArrowUpDown size={32} />}
          title={`No ${activeTab} transfers`}
          description={
            activeTab === "active"
              ? "All files are synchronized. Start a new upload or download."
              : `Your ${activeTab} transfer history is clear.`
          }
          style={{ flex: 1 }}
        />
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          {currentList.map((t) => (
            <TransferCard
              key={t.id}
              transfer={t}
              onPause={onPauseTransfer}
              onResume={onResumeTransfer}
              onCancel={onCancelTransfer}
            />
          ))}
        </div>
      )}
    </div>
  );
};
