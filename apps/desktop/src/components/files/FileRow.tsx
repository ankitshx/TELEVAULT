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
import { formatBytes, formatDate, getFileCategory, getCategoryAccent } from "../../utils/format";
import { Badge } from "../ui/Badge";

export interface FileRowProps {
  file: LogicalFile;
  isSelected?: boolean;
  onSelect: (file: LogicalFile) => void;
  onToggleFavorite: (fileId: string, e: React.MouseEvent) => void;
  onContextMenu: (file: LogicalFile, e: React.MouseEvent) => void;
}

export const FileRow: React.FC<FileRowProps> = ({
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
        return <FileImage size={18} color={accentColor} />;
      case "videos":
        return <FileVideo size={18} color={accentColor} />;
      case "audio":
        return <Music size={18} color={accentColor} />;
      case "code":
        return <FileCode size={18} color={accentColor} />;
      case "archives":
        return <Archive size={18} color={accentColor} />;
      default:
        return <FileText size={18} color={accentColor} />;
    }
  };

  return (
    <div
      onClick={() => onSelect(file)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(file, e);
      }}
      style={{
        display: "grid",
        gridTemplateColumns: "minmax(200px, 3fr) 100px 120px 140px 80px",
        alignItems: "center",
        padding: "10px 16px",
        borderRadius: "var(--radius-md)",
        fontSize: "13px",
        cursor: "pointer",
        backgroundColor: isSelected ? "var(--accent-subtle)" : "transparent",
        transition: "background var(--transition-fast)",
      }}
      onMouseEnter={(e) => {
        if (!isSelected) e.currentTarget.style.backgroundColor = "rgba(255, 255, 255, 0.03)";
      }}
      onMouseLeave={(e) => {
        if (!isSelected) e.currentTarget.style.backgroundColor = "transparent";
      }}
    >
      {/* File Name & Icon */}
      <div style={{ display: "flex", alignItems: "center", gap: "12px", minWidth: 0 }}>
        <div style={{ flexShrink: 0 }}>{getIcon()}</div>
        <span
          style={{
            fontWeight: 500,
            color: "var(--text-primary)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          title={file.name}
        >
          {file.name}
        </span>
        {file.is_encrypted && (
          <Badge variant="success" size="sm" icon={<Lock size={10} />}>
            AES
          </Badge>
        )}
      </div>

      {/* Size */}
      <div style={{ color: "var(--text-secondary)", fontSize: "12px" }}>
        {formatBytes(file.size)}
      </div>

      {/* Category */}
      <div style={{ color: "var(--text-muted)", fontSize: "12px", textTransform: "capitalize" }}>
        {category}
      </div>

      {/* Date */}
      <div style={{ color: "var(--text-muted)", fontSize: "12px" }}>
        {formatDate(file.updated_at || file.created_at)}
      </div>

      {/* Actions */}
      <div style={{ display: "flex", justifyContent: "flex-end", alignItems: "center", gap: "6px" }}>
        <button
          onClick={(e) => onToggleFavorite(file.id, e)}
          style={{
            background: "transparent",
            border: "none",
            cursor: "pointer",
            padding: "4px",
            color: file.is_favorite ? "var(--status-warning)" : "var(--text-muted)",
          }}
        >
          <Star size={14} fill={file.is_favorite ? "var(--status-warning)" : "none"} />
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
            color: "var(--text-muted)",
          }}
        >
          <MoreVertical size={14} />
        </button>
      </div>
    </div>
  );
};
