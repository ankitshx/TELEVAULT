import React from "react";
import { Folder, MoreVertical } from "lucide-react";
import { FolderItem } from "../../types";
import { formatDate } from "../../utils/format";

export interface FolderRowProps {
  folder: FolderItem;
  itemCount?: number;
  onOpen: (folderId: string) => void;
  onContextMenu: (folder: FolderItem, e: React.MouseEvent) => void;
}

export const FolderRow: React.FC<FolderRowProps> = ({
  folder,
  itemCount,
  onOpen,
  onContextMenu,
}) => {
  return (
    <div
      onClick={() => onOpen(folder.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(folder, e);
      }}
      style={{
        display: "grid",
        gridTemplateColumns: "minmax(200px, 3fr) 100px 120px 140px 80px",
        alignItems: "center",
        padding: "10px 16px",
        borderRadius: "var(--radius-md)",
        fontSize: "13px",
        cursor: "pointer",
        transition: "background var(--transition-fast)",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.backgroundColor = "rgba(255, 255, 255, 0.03)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.backgroundColor = "transparent";
      }}
    >
      {/* Folder Name & Icon */}
      <div style={{ display: "flex", alignItems: "center", gap: "12px", minWidth: 0 }}>
        <Folder size={18} color="var(--accent-primary)" fill="var(--accent-subtle)" />
        <span
          style={{
            fontWeight: 500,
            color: "var(--text-primary)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          title={folder.name}
        >
          {folder.name}
        </span>
      </div>

      {/* Size / Item count */}
      <div style={{ color: "var(--text-muted)", fontSize: "12px" }}>
        {itemCount !== undefined ? `${itemCount} items` : "—"}
      </div>

      {/* Type */}
      <div style={{ color: "var(--text-muted)", fontSize: "12px" }}>Folder</div>

      {/* Date */}
      <div style={{ color: "var(--text-muted)", fontSize: "12px" }}>
        {formatDate(folder.updated_at || folder.created_at)}
      </div>

      {/* Actions */}
      <div style={{ display: "flex", justifyContent: "flex-end", alignItems: "center" }}>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onContextMenu(folder, e);
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
