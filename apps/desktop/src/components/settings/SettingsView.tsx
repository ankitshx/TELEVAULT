import React, { useState } from "react";
import {
  HardDrive,
  Sliders,
  Moon,
  Cloud,
  Info,
  Lock,
  Cpu,
} from "lucide-react";
import { StorageStatus } from "../../types";
import { Button } from "../ui/Button";
import { Badge } from "../ui/Badge";

export interface SettingsViewProps {
  storageStatus: StorageStatus;
}

export const SettingsView: React.FC<SettingsViewProps> = ({ storageStatus }) => {
  const [activeTab, setActiveTab] = useState<
    "general" | "appearance" | "transfers" | "security" | "storage" | "about"
  >("general");

  const [concurrentTransfers, setConcurrentTransfers] = useState("4");
  const [minimizeToTray, setMinimizeToTray] = useState(true);
  const [autoResume, setAutoResume] = useState(true);

  return (
    <div
      style={{
        display: "flex",
        height: "100%",
        padding: "28px 32px",
        overflowY: "auto",
        gap: "32px",
      }}
    >
      {/* Settings Navigation Sidebar */}
      <div style={{ width: "200px", display: "flex", flexDirection: "column", gap: "4px", flexShrink: 0 }}>
        <h2 style={{ fontSize: "18px", fontWeight: 700, color: "var(--text-primary)", marginBottom: "16px" }}>
          Settings
        </h2>

        {[
          { id: "general", label: "General", icon: <Sliders size={16} /> },
          { id: "appearance", label: "Appearance", icon: <Moon size={16} /> },
          { id: "transfers", label: "Transfers", icon: <Cpu size={16} /> },
          { id: "security", label: "Security & Vault", icon: <Lock size={16} /> },
          { id: "storage", label: "Storage Engine", icon: <HardDrive size={16} /> },
          { id: "about", label: "About TeleCloud", icon: <Info size={16} /> },
        ].map((item) => {
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id as any)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                padding: "9px 12px",
                borderRadius: "var(--radius-md)",
                border: "none",
                backgroundColor: isActive ? "var(--accent-subtle)" : "transparent",
                color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
                fontSize: "13px",
                fontWeight: isActive ? 600 : 500,
                cursor: "pointer",
                textAlign: "left",
                transition: "all var(--transition-fast)",
              }}
            >
              <span style={{ color: isActive ? "var(--accent-primary)" : "var(--text-muted)" }}>
                {item.icon}
              </span>
              <span>{item.label}</span>
            </button>
          );
        })}
      </div>

      {/* Settings Tab Content */}
      <div style={{ flex: 1, maxWidth: "680px", display: "flex", flexDirection: "column", gap: "24px" }}>
        {/* General Settings */}
        {activeTab === "general" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                General Preferences
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                System startup, download targets, and desktop tray behavior.
              </p>
            </div>

            <div className="telecloud-card" style={{ padding: "18px 20px", display: "flex", flexDirection: "column", gap: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Minimize to Windows System Tray
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Keep transfers running in the background when window is closed
                  </p>
                </div>
                <input
                  type="checkbox"
                  checked={minimizeToTray}
                  onChange={(e) => setMinimizeToTray(e.target.checked)}
                  style={{ width: "16px", height: "16px", accentColor: "var(--accent-primary)" }}
                />
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Auto-Resume Interrupted Transfers
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Automatically recover in-flight uploads and downloads upon app start
                  </p>
                </div>
                <input
                  type="checkbox"
                  checked={autoResume}
                  onChange={(e) => setAutoResume(e.target.checked)}
                  style={{ width: "16px", height: "16px", accentColor: "var(--accent-primary)" }}
                />
              </div>
            </div>
          </div>
        )}

        {/* Appearance Settings */}
        {activeTab === "appearance" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                Appearance & Theme
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                TeleCloud visual identity and desktop styling.
              </p>
            </div>

            <div className="telecloud-card" style={{ padding: "18px 20px", display: "flex", flexDirection: "column", gap: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Theme
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Dark navy/black foundation with electric blue accent (Approved standard)
                  </p>
                </div>
                <Badge variant="blue" size="md">
                  Dark Navy (Default)
                </Badge>
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Interface Glassmorphism
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Mild translucent backdrop blur on cards and dialogs
                  </p>
                </div>
                <Badge variant="cyan" size="md">
                  Active
                </Badge>
              </div>
            </div>
          </div>
        )}

        {/* Transfers Settings */}
        {activeTab === "transfers" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                Transfer Pipeline Configuration
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                Tuning parameters for large-file streaming and chunk concurrency.
              </p>
            </div>

            <div className="telecloud-card" style={{ padding: "18px 20px", display: "flex", flexDirection: "column", gap: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Max Single Chunk Size
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Files are sliced into chunks before upload (prevents high RAM usage)
                  </p>
                </div>
                <Badge variant="blue" size="md">
                  20 MB (Standard) / 2 GB (Premium)
                </Badge>
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Concurrent Chunk Pipeline
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Number of parallel chunk upload/download streams
                  </p>
                </div>
                <select
                  value={concurrentTransfers}
                  onChange={(e) => setConcurrentTransfers(e.target.value)}
                  style={{
                    backgroundColor: "var(--bg-surface)",
                    color: "var(--text-primary)",
                    border: "1px solid var(--border-card)",
                    borderRadius: "var(--radius-sm)",
                    padding: "4px 8px",
                    fontSize: "12px",
                    outline: "none",
                  }}
                >
                  <option value="2">2 parallel threads</option>
                  <option value="4">4 parallel threads (Default)</option>
                  <option value="8">8 parallel threads</option>
                </select>
              </div>
            </div>
          </div>
        )}

        {/* Security Settings */}
        {activeTab === "security" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                Cryptographic Security & Vault
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                Zero-knowledge client-side encryption parameters.
              </p>
            </div>

            <div className="telecloud-card" style={{ padding: "18px 20px", display: "flex", flexDirection: "column", gap: "16px" }}>
              <div>
                <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                  Symmetric Encryption Standard
                </p>
                <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                  AES-256-GCM with 96-bit CSPRNG nonces per chunk. Chunk indexes are authenticated in the Associated Data (AAD) to prevent truncation or reordering.
                </p>
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <div>
                <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                  Key Derivation Function (KDF)
                </p>
                <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                  Argon2id (Memory: 64 MB, Iterations: 3, Parallelism: 2, 16-byte random salt).
                </p>
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Windows Credential Manager
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Session tokens stored securely in native OS keyring
                  </p>
                </div>
                <Badge variant="success" size="md">
                  Active
                </Badge>
              </div>
            </div>
          </div>
        )}

        {/* Storage Engine Settings */}
        {activeTab === "storage" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                Storage Backend & Local Database
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                Inspection of Telegram MTProto storage channels and local SQLite WAL database.
              </p>
            </div>

            <div className="telecloud-card" style={{ padding: "18px 20px", display: "flex", flexDirection: "column", gap: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                    Active Storage Provider
                  </p>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    {storageStatus.provider_name}
                  </p>
                </div>
                <Badge variant={storageStatus.is_connected ? "success" : "error"} size="md">
                  {storageStatus.is_connected ? "Connected" : "Offline"}
                </Badge>
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <div>
                <p style={{ fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" }}>
                  Local Database Engine
                </p>
                <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                  SQLite 3 with Write-Ahead Logging (WAL) and FTS5 full-text search index.
                </p>
              </div>
            </div>
          </div>
        )}

        {/* About Settings */}
        {activeTab === "about" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                About TeleCloud
              </h3>
              <p style={{ fontSize: "12px", color: "var(--text-muted)", marginTop: "2px" }}>
                Commercial personal cloud drive powered by Telegram storage.
              </p>
            </div>

            <div className="telecloud-card" style={{ padding: "18px 20px", display: "flex", flexDirection: "column", gap: "14px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
                <div
                  style={{
                    width: "40px",
                    height: "40px",
                    borderRadius: "var(--radius-md)",
                    background: "linear-gradient(135deg, var(--accent-primary), var(--accent-cyan))",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                  }}
                >
                  <Cloud size={22} color="#ffffff" />
                </div>
                <div>
                  <h4 style={{ fontSize: "15px", fontWeight: 700, color: "var(--text-primary)" }}>
                    TeleCloud Desktop
                  </h4>
                  <p style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    Version 0.1.0 • Built with Tauri 2 + Rust
                  </p>
                </div>
              </div>

              <div style={{ height: "1px", backgroundColor: "var(--border-subtle)" }} />

              <p style={{ fontSize: "12px", color: "var(--text-secondary)", lineHeight: 1.6 }}>
                TeleCloud is an independent open-source cloud storage application. It is NOT an official Telegram product and does not claim any affiliation with Telegram FZ-LLC.
              </p>

              <div style={{ display: "flex", gap: "10px", marginTop: "8px" }}>
                <Button variant="secondary" size="sm">
                  View License (MIT)
                </Button>
                <Button variant="ghost" size="sm">
                  Documentation
                </Button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
