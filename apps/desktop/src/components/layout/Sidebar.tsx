import React from "react";
import {
  Home,
  HardDrive,
  Clock,
  Star,
  ArrowUpDown,
  Trash2,
  Settings,
  ShieldCheck,
  Cloud,
} from "lucide-react";
import { NavigationTab, StorageStatus } from "../../types";
import { ProgressBar } from "../ui/ProgressBar";
import { formatBytes } from "../../utils/format";

export interface SidebarProps {
  activeTab: NavigationTab;
  onSelectTab: (tab: NavigationTab) => void;
  storageStatus: StorageStatus;
  activeTransferCount: number;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  onSelectTab,
  storageStatus,
  activeTransferCount,
}) => {
  const usedBytes = storageStatus.used_bytes || 186.4 * 1024 * 1024 * 1024;
  const totalBytes = storageStatus.total_quota_bytes || 2 * 1024 * 1024 * 1024 * 1024;
  const usagePercentage = Math.round((usedBytes / totalBytes) * 100);

  const navItems: { id: NavigationTab; label: string; icon: React.ReactNode; badge?: number }[] = [
    { id: "home", label: "Dashboard", icon: <Home size={18} /> },
    { id: "drive", label: "My Cloud", icon: <HardDrive size={18} /> },
    { id: "recent", label: "Recent", icon: <Clock size={18} /> },
    { id: "favorites", label: "Favorites", icon: <Star size={18} /> },
    {
      id: "transfers",
      label: "Transfers",
      icon: <ArrowUpDown size={18} />,
      badge: activeTransferCount > 0 ? activeTransferCount : undefined,
    },
    { id: "trash", label: "Trash", icon: <Trash2 size={18} /> },
  ];

  return (
    <aside
      style={{
        width: "250px",
        backgroundColor: "var(--bg-sidebar)",
        borderRight: "1px solid var(--border-subtle)",
        display: "flex",
        flexDirection: "column",
        padding: "20px 14px",
        flexShrink: 0,
        userSelect: "none",
      }}
    >
      {/* Brand Header */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: "12px",
          marginBottom: "26px",
          paddingLeft: "6px",
        }}
      >
        <div
          style={{
            width: "36px",
            height: "36px",
            borderRadius: "var(--radius-md)",
            background: "linear-gradient(135deg, var(--accent-primary) 0%, #0070F3 100%)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            boxShadow: "0 0 18px rgba(22, 131, 255, 0.4)",
            flexShrink: 0,
          }}
        >
          <Cloud size={20} color="#ffffff" />
        </div>
        <div>
          <h1
            style={{
              fontSize: "16px",
              fontWeight: 700,
              letterSpacing: "-0.02em",
              color: "var(--text-primary)",
              lineHeight: 1.2,
            }}
          >
            TeleCloud
          </h1>
          <p
            style={{
              fontSize: "11px",
              color: "var(--accent-cyan)",
              display: "flex",
              alignItems: "center",
              gap: "4px",
              marginTop: "2px",
            }}
          >
            <ShieldCheck size={12} /> Zero-Knowledge
          </p>
        </div>
      </div>

      {/* Main Navigation */}
      <nav style={{ display: "flex", flexDirection: "column", gap: "4px", flex: 1 }}>
        {navItems.map((item) => {
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => onSelectTab(item.id)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "12px",
                padding: "10px 12px",
                borderRadius: "var(--radius-md)",
                border: "none",
                background: isActive ? "var(--accent-subtle)" : "transparent",
                color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
                fontSize: "13px",
                fontWeight: isActive ? 600 : 500,
                cursor: "pointer",
                textAlign: "left",
                transition: "all var(--transition-fast)",
                position: "relative",
              }}
              onMouseEnter={(e) => {
                if (!isActive) e.currentTarget.style.backgroundColor = "rgba(255, 255, 255, 0.04)";
              }}
              onMouseLeave={(e) => {
                if (!isActive) e.currentTarget.style.backgroundColor = "transparent";
              }}
            >
              {isActive && (
                <div
                  style={{
                    position: "absolute",
                    left: 0,
                    top: "6px",
                    bottom: "6px",
                    width: "3px",
                    backgroundColor: "var(--accent-primary)",
                    borderRadius: "0 var(--radius-xs) var(--radius-xs) 0",
                    boxShadow: "0 0 8px var(--accent-primary)",
                  }}
                />
              )}
              <span
                style={{
                  display: "flex",
                  alignItems: "center",
                  color: isActive ? "var(--accent-primary)" : "var(--text-muted)",
                }}
              >
                {item.icon}
              </span>
              <span style={{ flex: 1 }}>{item.label}</span>
              {item.badge !== undefined && (
                <span
                  style={{
                    backgroundColor: "var(--accent-primary)",
                    color: "#ffffff",
                    fontSize: "10px",
                    fontWeight: 700,
                    padding: "2px 6px",
                    borderRadius: "var(--radius-full)",
                    boxShadow: "0 0 8px var(--accent-glow)",
                  }}
                >
                  {item.badge}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Settings Action */}
      <div style={{ paddingBottom: "16px", borderBottom: "1px solid var(--border-subtle)", marginBottom: "16px" }}>
        <button
          onClick={() => onSelectTab("settings")}
          style={{
            width: "100%",
            display: "flex",
            alignItems: "center",
            gap: "12px",
            padding: "10px 12px",
            borderRadius: "var(--radius-md)",
            border: "none",
            background: activeTab === "settings" ? "var(--accent-subtle)" : "transparent",
            color: activeTab === "settings" ? "var(--text-primary)" : "var(--text-secondary)",
            fontSize: "13px",
            fontWeight: activeTab === "settings" ? 600 : 500,
            cursor: "pointer",
            textAlign: "left",
            transition: "all var(--transition-fast)",
          }}
          onMouseEnter={(e) => {
            if (activeTab !== "settings") e.currentTarget.style.backgroundColor = "rgba(255, 255, 255, 0.04)";
          }}
          onMouseLeave={(e) => {
            if (activeTab !== "settings") e.currentTarget.style.backgroundColor = "transparent";
          }}
        >
          <span style={{ color: activeTab === "settings" ? "var(--accent-primary)" : "var(--text-muted)" }}>
            <Settings size={18} />
          </span>
          <span>Settings</span>
        </button>
      </div>

      {/* Storage Meter Card */}
      <div
        className="telecloud-card"
        style={{
          padding: "14px",
          display: "flex",
          flexDirection: "column",
          gap: "8px",
          backgroundColor: "rgba(16, 24, 35, 0.5)",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ fontSize: "11px", fontWeight: 600, color: "var(--text-secondary)" }}>
            CLOUD STORAGE
          </span>
          <span style={{ fontSize: "11px", fontWeight: 700, color: "var(--accent-primary)" }}>
            {usagePercentage}%
          </span>
        </div>
        <ProgressBar progress={usagePercentage} height={5} color="var(--accent-primary)" />
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: "11px", color: "var(--text-muted)" }}>
          <span>{formatBytes(usedBytes)}</span>
          <span>{formatBytes(totalBytes)}</span>
        </div>
      </div>
    </aside>
  );
};
