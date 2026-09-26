import React from "react";
import {
  Upload,
  FolderPlus,
  Search,
  ArrowUpDown,
  FileText,
  Folder,
  Star,
  Lock,
  FileVideo,
  FileImage,
  FileCode,
  Archive,
  Music,
  Cloud,
} from "lucide-react";
import { FolderItem, LogicalFile, StorageStatus } from "../../types";
import { formatBytes, getFileCategory, getCategoryAccent } from "../../utils/format";
import { Button } from "../ui/Button";
import { Badge } from "../ui/Badge";

export interface HomeDashboardProps {
  files: LogicalFile[];
  folders: FolderItem[];
  favorites: LogicalFile[];
  storageStatus: StorageStatus;
  onOpenUpload: () => void;
  onOpenNewFolder: () => void;
  onOpenSearch: () => void;
  onOpenTransfers: () => void;
  onSelectFile: (file: LogicalFile) => void;
  onNavigateToDrive: () => void;
  onOpenConnect: () => void;
}

export const HomeDashboard: React.FC<HomeDashboardProps> = ({
  files,
  folders,
  favorites,
  storageStatus,
  onOpenUpload,
  onOpenNewFolder,
  onOpenSearch,
  onOpenTransfers,
  onSelectFile,
  onNavigateToDrive,
  onOpenConnect,
}) => {
  const encryptedCount = files.filter((f) => f.is_encrypted).length;
  const totalBytes = storageStatus.total_quota_bytes || 2 * 1024 * 1024 * 1024 * 1024;
  const usedBytes = files.reduce((acc, f) => acc + f.size, 0) || (storageStatus.used_bytes ?? 0);
  const usagePercentage = Math.round((usedBytes / totalBytes) * 100);

  // Breakdown by category
  const categories = files.reduce(
    (acc, f) => {
      const cat = getFileCategory(f.name, f.mime_type);
      acc[cat] = (acc[cat] || 0) + f.size;
      return acc;
    },
    { documents: 0, images: 0, videos: 0, audio: 0, archives: 0, code: 0, all: 0 } as Record<string, number>
  );

  const recentFiles = files.slice(0, 6);

  const getFileIcon = (file: LogicalFile) => {
    const cat = getFileCategory(file.name, file.mime_type);
    const color = getCategoryAccent(cat);
    switch (cat) {
      case "images":
        return <FileImage size={24} color={color} />;
      case "videos":
        return <FileVideo size={24} color={color} />;
      case "audio":
        return <Music size={24} color={color} />;
      case "code":
        return <FileCode size={24} color={color} />;
      case "archives":
        return <Archive size={24} color={color} />;
      default:
        return <FileText size={24} color={color} />;
    }
  };

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: "28px",
        padding: "28px 32px",
        overflowY: "auto",
        height: "100%",
      }}
    >
      {/* Welcome Banner */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "flex-end",
          paddingBottom: "8px",
        }}
      >
        <div>
          <h2
            style={{
              fontSize: "24px",
              fontWeight: 700,
              letterSpacing: "-0.02em",
              color: "var(--text-primary)",
            }}
          >
            Welcome to TeleCloud
          </h2>
          <p style={{ fontSize: "14px", color: "var(--text-secondary)", marginTop: "4px" }}>
            Personal cloud drive powered by zero-knowledge encryption and Telegram storage.
          </p>
        </div>
        <div style={{ display: "flex", gap: "10px" }}>
          <Button
            variant="ghost"
            icon={<Search size={16} />}
            onClick={onOpenSearch}
          >
            Quick Search
          </Button>
          <Button
            variant="secondary"
            icon={<ArrowUpDown size={16} />}
            onClick={onOpenTransfers}
          >
            Transfers
          </Button>
          <Button
            variant="secondary"
            icon={<FolderPlus size={16} />}
            onClick={onOpenNewFolder}
          >
            New Folder
          </Button>
          <Button
            variant="primary"
            icon={<Upload size={16} />}
            onClick={onOpenUpload}
          >
            Upload File
          </Button>
        </div>
      </div>

      {/* Unconnected / Setup State Banner */}
      {!storageStatus.is_connected && (
        <div
          className="telecloud-card"
          style={{
            padding: "20px 24px",
            backgroundColor: "rgba(59, 130, 246, 0.08)",
            border: "1px solid rgba(59, 130, 246, 0.3)",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            borderRadius: "var(--radius-lg)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
            <div
              style={{
                width: "44px",
                height: "44px",
                borderRadius: "var(--radius-md)",
                backgroundColor: "var(--accent-subtle)",
                color: "var(--accent-primary)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                flexShrink: 0,
              }}
            >
              <Cloud size={24} />
            </div>
            <div>
              <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
                Connect your Telegram account to continue
              </h3>
              <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginTop: "2px" }}>
                Link your Telegram MTProto channel or activate the zero-knowledge local storage engine to begin storing files.
              </p>
            </div>
          </div>
          <Button variant="primary" icon={<Cloud size={16} />} onClick={onOpenConnect}>
            Connect Storage
          </Button>
        </div>
      )}

      {/* Storage Breakdown Card */}
      <div
        className="telecloud-card"
        style={{
          padding: "22px 26px",
          display: "flex",
          flexDirection: "column",
          gap: "18px",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div>
            <span style={{ fontSize: "12px", fontWeight: 600, color: "var(--text-muted)", letterSpacing: "0.04em" }}>
              STORAGE ALLOCATION
            </span>
            <div style={{ display: "flex", alignItems: "baseline", gap: "8px", marginTop: "4px" }}>
              <h3 style={{ fontSize: "22px", fontWeight: 700, color: "var(--text-primary)" }}>
                {formatBytes(usedBytes)}
              </h3>
              <span style={{ fontSize: "14px", color: "var(--text-muted)" }}>
                / {formatBytes(totalBytes)} ({usagePercentage}% used)
              </span>
            </div>
          </div>
          <Badge variant="blue" size="md">
            Telegram MTProto Infinite Engine
          </Badge>
        </div>

        {/* Multi-segment usage bar */}
        <div
          style={{
            height: "10px",
            backgroundColor: "rgba(255, 255, 255, 0.06)",
            borderRadius: "var(--radius-full)",
            overflow: "hidden",
            display: "flex",
            gap: "2px",
          }}
        >
          <div
            style={{
              flex: Math.max(1, categories.documents),
              backgroundColor: "var(--file-doc)",
              borderRadius: "var(--radius-full) 0 0 var(--radius-full)",
            }}
          />
          <div style={{ flex: Math.max(1, categories.videos), backgroundColor: "var(--file-video)" }} />
          <div style={{ flex: Math.max(1, categories.images), backgroundColor: "var(--file-image)" }} />
          <div style={{ flex: Math.max(1, categories.archives), backgroundColor: "var(--file-archive)" }} />
          <div style={{ flex: Math.max(1, categories.code), backgroundColor: "var(--file-code)" }} />
        </div>

        {/* Category Legend */}
        <div style={{ display: "flex", flexWrap: "wrap", gap: "20px", fontSize: "12px" }}>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "var(--file-doc)" }} />
            <span style={{ color: "var(--text-secondary)" }}>Documents</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "var(--file-video)" }} />
            <span style={{ color: "var(--text-secondary)" }}>Videos</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "var(--file-image)" }} />
            <span style={{ color: "var(--text-secondary)" }}>Images</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "var(--file-archive)" }} />
            <span style={{ color: "var(--text-secondary)" }}>Archives</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ width: "8px", height: "8px", borderRadius: "50%", backgroundColor: "var(--file-code)" }} />
            <span style={{ color: "var(--text-secondary)" }}>Code & Other</span>
          </div>
        </div>
      </div>

      {/* Quick Stats Grid */}
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))",
          gap: "16px",
        }}
      >
        <div className="telecloud-card" style={{ padding: "18px 20px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>Total Files</span>
            <FileText size={16} color="var(--accent-primary)" />
          </div>
          <div style={{ fontSize: "24px", fontWeight: 700, marginTop: "8px" }}>
            {files.length}
          </div>
        </div>

        <div className="telecloud-card" style={{ padding: "18px 20px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>Folders</span>
            <Folder size={16} color="var(--accent-cyan)" />
          </div>
          <div style={{ fontSize: "24px", fontWeight: 700, marginTop: "8px" }}>
            {folders.length}
          </div>
        </div>

        <div className="telecloud-card" style={{ padding: "18px 20px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>Favorites</span>
            <Star size={16} color="var(--status-warning)" />
          </div>
          <div style={{ fontSize: "24px", fontWeight: 700, marginTop: "8px" }}>
            {favorites.length}
          </div>
        </div>

        <div className="telecloud-card" style={{ padding: "18px 20px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>Encrypted</span>
            <Lock size={16} color="var(--status-success)" />
          </div>
          <div style={{ fontSize: "24px", fontWeight: 700, marginTop: "8px" }}>
            {encryptedCount}
          </div>
        </div>
      </div>

      {/* Recent Files Section */}
      <div>
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "14px",
          }}
        >
          <h3 style={{ fontSize: "16px", fontWeight: 600, color: "var(--text-primary)" }}>
            Recent Files
          </h3>
          <button
            onClick={onNavigateToDrive}
            style={{
              background: "none",
              border: "none",
              color: "var(--accent-hover)",
              fontSize: "12px",
              fontWeight: 500,
              cursor: "pointer",
            }}
          >
            View All in Cloud →
          </button>
        </div>

        {recentFiles.length === 0 ? (
          <div
            className="telecloud-card"
            style={{
              padding: "40px",
              textAlign: "center",
              color: "var(--text-muted)",
              fontSize: "13px",
            }}
          >
            No files uploaded yet. Click <strong>Upload File</strong> to start your personal drive.
          </div>
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
              gap: "14px",
            }}
          >
            {recentFiles.map((file) => (
              <div
                key={file.id}
                onClick={() => onSelectFile(file)}
                className="telecloud-card"
                style={{
                  padding: "16px",
                  display: "flex",
                  flexDirection: "column",
                  gap: "12px",
                  cursor: "pointer",
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                  <div
                    style={{
                      width: "42px",
                      height: "42px",
                      borderRadius: "var(--radius-md)",
                      backgroundColor: "rgba(255, 255, 255, 0.04)",
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                    }}
                  >
                    {getFileIcon(file)}
                  </div>
                  {file.is_encrypted && (
                    <Badge variant="success" size="sm" icon={<Lock size={10} />}>
                      AES-256
                    </Badge>
                  )}
                </div>
                <div>
                  <h4
                    style={{
                      fontSize: "13px",
                      fontWeight: 600,
                      color: "var(--text-primary)",
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                    title={file.name}
                  >
                    {file.name}
                  </h4>
                  <p style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "2px" }}>
                    {formatBytes(file.size)}
                  </p>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
