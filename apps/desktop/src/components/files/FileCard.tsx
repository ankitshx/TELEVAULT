import React from "react";
import {
  FileText,
  FileImage,
  FileVideo,
  FileCode,
  Archive,
  Music,
  Star,
  Lock,
  MoreVertical,
} from "lucide-react";
import { LogicalFile } from "../../types";
import { formatBytes, getFileCategory, getCategoryAccent } from "../../utils/format";
import { Badge } from "../ui/Badge";

export interface FileCardProps {
  file: LogicalFile;
  isSelected?: boolean;
  onSelect: (file: LogicalFile) => void;
  onToggleFavorite: (fileId: string, e: React.MouseEvent) => void;
  onContextMenu: (file: LogicalFile, e: React.MouseEvent) => void;
}

export const FileCard: React.FC<FileCardProps> = ({
  file,
  isSelected = false,
  onSelect,
  onToggleFavorite,
  onContextMenu,
}) => {
  const category = getFileCategory(file.name, file.mime_type);
  const accentColor = getCategoryAccent(category);

  const getIcon = () => {
    switch (category) {
      case "images":
        return <FileImage size={28} color={accentColor} />;
      case "videos":
        return <FileVideo size={28} color={accentColor} />;
      case "audio":
        return <Music size={28} color={accentColor} />;
      case "code":
        return <FileCode size={28} color={accentColor} />;
      case "archives":
        return <Archive size={28} color={accentColor} />;
      default:
        return <FileText size={28} color={accentColor} />;
    }
  };

  return (
    <div
      onClick={() => onSelect(file)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(file, e);
      }}
      className="telecloud-card"
      style={{
        padding: "16px",
        display: "flex",
        flexDirection: "column",
        gap: "12px",
        cursor: "pointer",
        position: "relative",
        border: isSelected ? "1px solid var(--accent-primary)" : "1px solid var(--border-card)",
        backgroundColor: isSelected ? "var(--bg-card-hover)" : "var(--bg-card)",
      }}
    >
      {/* Top Header: Category Icon + Security Badge + Context Menu */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
        <div
          style={{
            width: "44px",
            height: "44px",
            borderRadius: "var(--radius-md)",
            backgroundColor: "rgba(255, 255, 255, 0.04)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          {getIcon()}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          {file.is_encrypted && (
            <Badge variant="success" size="sm" icon={<Lock size={10} />}>
              AES
            </Badge>
          )}

          <button
            onClick={(e) => onToggleFavorite(file.id, e)}
            style={{
              background: "transparent",
              border: "none",
              cursor: "pointer",
              padding: "4px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: file.is_favorite ? "var(--status-warning)" : "var(--text-muted)",
              transition: "transform var(--transition-fast)",
            }}
            title={file.is_favorite ? "Remove from favorites" : "Add to favorites"}
          >
            <Star size={15} fill={file.is_favorite ? "var(--status-warning)" : "none"} />
          </button>

          <button
            onClick={(e) => {
              e.stopPropagation();
              onContextMenu(file, e);
            }}
            style={{
              background: "transparent",
              border: "none",
              cursor: "pointer",
              padding: "4px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--text-muted)",
            }}
          >
            <MoreVertical size={15} />
          </button>
        </div>
      </div>

      {/* Title & Metadata */}
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
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            fontSize: "11px",
            color: "var(--text-muted)",
            marginTop: "4px",
          }}
        >
          <span>{formatBytes(file.size)}</span>
          <span>•</span>
          <span style={{ textTransform: "uppercase" }}>{category}</span>
        </div>
      </div>
    </div>
  );
};
