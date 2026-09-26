import React, { useState } from "react";
import {
  Search,
  Bell,
  CheckCircle2,
  AlertCircle,
  LogOut,
  Settings,
} from "lucide-react";
import { StorageStatus } from "../../types";

export interface TopBarProps {
  onOpenSearch: () => void;
  storageStatus: StorageStatus;
  onOpenSettings: () => void;
  onOpenConnect?: () => void;
  onLogout?: () => void;
}

export const TopBar: React.FC<TopBarProps> = ({
  onOpenSearch,
  storageStatus,
  onOpenSettings,
  onOpenConnect,
  onLogout,
}) => {
  const [showProfileMenu, setShowProfileMenu] = useState(false);

  const userInitial = storageStatus.user_identifier
    ? storageStatus.user_identifier.replace("@", "").charAt(0).toUpperCase()
    : "T";

  return (
    <header
      style={{
        height: "64px",
        backgroundColor: "var(--bg-app)",
        borderBottom: "1px solid var(--border-subtle)",
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "0 24px",
        flexShrink: 0,
        userSelect: "none",
      }}
    >
      {/* Global Search Bar (Trigger for Ctrl+K Palette) */}
      <div
        onClick={onOpenSearch}
        style={{
          display: "flex",
          alignItems: "center",
          gap: "10px",
          backgroundColor: "var(--bg-surface)",
          border: "1px solid var(--border-card)",
          borderRadius: "var(--radius-md)",
          padding: "7px 14px",
          width: "360px",
          cursor: "pointer",
          transition: "all var(--transition-fast)",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.borderColor = "var(--border-focus)";
          e.currentTarget.style.backgroundColor = "var(--bg-surface-elevated)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.borderColor = "var(--border-card)";
          e.currentTarget.style.backgroundColor = "var(--bg-surface)";
        }}
      >
        <Search size={15} color="var(--text-muted)" />
        <span style={{ fontSize: "13px", color: "var(--text-muted)", flex: 1 }}>
          Search files, folders, or commands...
        </span>
        <kbd
          style={{
            backgroundColor: "rgba(255, 255, 255, 0.08)",
            color: "var(--text-secondary)",
            fontSize: "10px",
            fontWeight: 600,
            padding: "2px 6px",
            borderRadius: "var(--radius-xs)",
            border: "1px solid var(--border-subtle)",
          }}
        >
          Ctrl + K
        </kbd>
      </div>

      {/* Right Controls: Cloud Status + Notification + Account */}
      <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
        {/* Storage Provider Status Pill */}
        <div
          onClick={onOpenConnect}
          title="Manage cloud storage connection"
          style={{
            display: "flex",
            alignItems: "center",
            gap: "7px",
            padding: "5px 12px",
            backgroundColor: storageStatus.is_connected
              ? "var(--status-success-subtle)"
              : "var(--status-error-subtle)",
            border: `1px solid ${
              storageStatus.is_connected
                ? "rgba(16, 185, 129, 0.25)"
                : "rgba(239, 68, 68, 0.25)"
            }`,
            borderRadius: "var(--radius-full)",
            fontSize: "12px",
            fontWeight: 500,
            cursor: onOpenConnect ? "pointer" : "default",
            color: storageStatus.is_connected
              ? "var(--status-success)"
              : "var(--status-error)",
          }}
        >
          {storageStatus.is_connected ? (
            <CheckCircle2 size={13} />
          ) : (
            <AlertCircle size={13} />
          )}
          <span>{storageStatus.provider_name}</span>
        </div>

        {/* Notifications Icon Button */}
        <button
          style={{
            width: "36px",
            height: "36px",
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border-card)",
            backgroundColor: "var(--bg-surface)",
            color: "var(--text-secondary)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            cursor: "pointer",
            transition: "all var(--transition-fast)",
            position: "relative",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--text-primary)";
            e.currentTarget.style.borderColor = "var(--border-focus)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--text-secondary)";
            e.currentTarget.style.borderColor = "var(--border-card)";
          }}
        >
          <Bell size={16} />
          <span
            style={{
              position: "absolute",
              top: "7px",
              right: "7px",
              width: "6px",
              height: "6px",
              backgroundColor: "var(--accent-primary)",
              borderRadius: "50%",
              boxShadow: "0 0 6px var(--accent-primary)",
            }}
          />
        </button>

        {/* Profile Avatar / Menu */}
        <div style={{ position: "relative" }}>
          <button
            onClick={() => setShowProfileMenu((prev) => !prev)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              padding: "4px 8px 4px 4px",
              borderRadius: "var(--radius-full)",
              border: "1px solid var(--border-card)",
              backgroundColor: "var(--bg-surface)",
              color: "var(--text-primary)",
              cursor: "pointer",
              transition: "all var(--transition-fast)",
            }}
            onMouseEnter={(e) => (e.currentTarget.style.borderColor = "var(--border-focus)")}
            onMouseLeave={(e) => (e.currentTarget.style.borderColor = "var(--border-card)")}
          >
            <div
              style={{
                width: "28px",
                height: "28px",
                borderRadius: "50%",
                background: "linear-gradient(135deg, var(--accent-primary), var(--accent-cyan))",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: "12px",
                fontWeight: 700,
                color: "#ffffff",
              }}
            >
              {userInitial}
            </div>
            <span style={{ fontSize: "12px", fontWeight: 600, paddingRight: "4px" }}>
              {storageStatus.user_identifier || "Telegram Account"}
            </span>
          </button>

          {/* Profile Dropdown */}
          {showProfileMenu && (
            <div
              className="glass-surface-elevated animate-fade-in"
              style={{
                position: "absolute",
                right: 0,
                top: "42px",
                width: "230px",
                borderRadius: "var(--radius-md)",
                backgroundColor: "var(--bg-surface-elevated)",
                border: "1px solid var(--border-card)",
                padding: "8px",
                zIndex: 2500,
                boxShadow: "var(--shadow-popover)",
              }}
            >
              <div
                style={{
                  padding: "8px 10px 12px 10px",
                  borderBottom: "1px solid var(--border-subtle)",
                  marginBottom: "6px",
                }}
              >
                <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                  {storageStatus.user_identifier || "Telegram Account"}
                </p>
                <p style={{ fontSize: "11px", color: storageStatus.is_connected ? "var(--status-success)" : "var(--text-muted)", marginTop: "2px" }}>
                  {storageStatus.is_connected ? "Connected to MTProto Cloud" : "Disconnected"}
                </p>
              </div>

              <button
                onClick={() => {
                  setShowProfileMenu(false);
                  onOpenSettings();
                }}
                style={{
                  width: "100%",
                  display: "flex",
                  alignItems: "center",
                  gap: "10px",
                  padding: "8px 10px",
                  borderRadius: "var(--radius-sm)",
                  border: "none",
                  backgroundColor: "transparent",
                  color: "var(--text-primary)",
                  fontSize: "12px",
                  cursor: "pointer",
                  textAlign: "left",
                }}
                onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = "var(--accent-subtle)")}
                onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
              >
                <Settings size={15} color="var(--text-muted)" />
                <span>Account Settings</span>
              </button>

              <button
                onClick={() => {
                  setShowProfileMenu(false);
                  if (onLogout) onLogout();
                }}
                style={{
                  width: "100%",
                  display: "flex",
                  alignItems: "center",
                  gap: "10px",
                  padding: "8px 10px",
                  borderRadius: "var(--radius-sm)",
                  border: "none",
                  backgroundColor: "transparent",
                  color: "var(--status-error)",
                  fontSize: "12px",
                  cursor: "pointer",
                  textAlign: "left",
                  marginTop: "4px",
                }}
                onMouseEnter={(e) =>
                  (e.currentTarget.style.backgroundColor = "var(--status-error-subtle)")
                }
                onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = "transparent")}
              >
                <LogOut size={15} color="var(--status-error)" />
                <span>Log Out / Switch Account</span>
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
};
