import { NavTab } from "../types";

interface SidebarProps {
  currentTab: NavTab;
  onTabChange: (tab: NavTab) => void;
  appVersion?: string;
  schedulerStatus?: string;
  telegramUser?: string;
  channelTitle?: string;
}

interface NavItemDef {
  id: NavTab;
  label: string;
  icon: string;
}

const navItems: NavItemDef[] = [
  { id: "dashboard", label: "Dashboard", icon: "📊" },
  { id: "backups", label: "Backups", icon: "💾" },
  { id: "restore", label: "Restore", icon: "🔄" },
  { id: "schedules", label: "Schedules", icon: "⏱️" },
  { id: "verification", label: "Verification", icon: "🛡️" },
  { id: "repair", label: "Remote Repair", icon: "🔧" },
  { id: "activity", label: "Activity", icon: "⚡" },
  { id: "profiles", label: "Profiles", icon: "📁" },
  { id: "retention", label: "Retention", icon: "⏳" },
  { id: "settings", label: "Settings", icon: "⚙️" },
];

export function Sidebar({
  currentTab,
  onTabChange,
  appVersion = "0.1.0",
  schedulerStatus = "Active",
  telegramUser,
  channelTitle,
}: SidebarProps) {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="brand-badge">
          <div className="brand-logo">TV</div>
          <div>
            <div className="brand-title">TELEVAULT</div>
            <div className="brand-subtitle">Secure Cloud Backup</div>
          </div>
        </div>
      </div>

      <nav className="nav-list">
        {navItems.map((item) => (
          <button
            type="button"
            key={item.id}
            className={`nav-item ${currentTab === item.id ? "active" : ""}`}
            onClick={() => onTabChange(item.id)}
          >
            <span className="nav-item-icon">{item.icon}</span>
            <span>{item.label}</span>
          </button>
        ))}
      </nav>

      {/* Account Mini Badge */}
      {telegramUser && (
        <div
          className="sidebar-account-badge"
          onClick={() => onTabChange("settings")}
          role="button"
          tabIndex={0}
          title="Manage connected Telegram account"
        >
          <div className="account-dot-online" />
          <div className="account-badge-info">
            <div className="account-badge-name">{telegramUser}</div>
            <div className="account-badge-channel">{channelTitle || "Vault Connected"}</div>
          </div>
        </div>
      )}

      <div className="sidebar-footer">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span>TELEVAULT Core</span>
          <span className="mono" style={{ color: "var(--primary)" }}>
            v{appVersion}
          </span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
          <span
            style={{
              width: "6px",
              height: "6px",
              borderRadius: "50%",
              backgroundColor:
                schedulerStatus === "running" || schedulerStatus === "Active"
                  ? "var(--success)"
                  : "var(--warning)",
            }}
          />
          <span style={{ fontSize: "0.7rem", color: "var(--text-dim)" }}>
            Scheduler: {schedulerStatus}
          </span>
        </div>
        <div className="sidebar-creator-attribution">
          Created by Ankit Sharma
        </div>
      </div>
    </aside>
  );
}
