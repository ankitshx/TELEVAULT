import React from "react";
import { Folder, MoreVertical } from "lucide-react";
import { FolderItem } from "../../types";

export interface FolderCardProps {
  folder: FolderItem;
  itemCount?: number;
  onOpen: (folderId: string) => void;
  onContextMenu: (folder: FolderItem, e: React.MouseEvent) => void;
}

export const FolderCard: React.FC<FolderCardProps> = ({
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
      className="telecloud-card"
      style={{
        padding: "16px",
        display: "flex",
        alignItems: "center",
        gap: "14px",
        cursor: "pointer",
      }}
    >
      <div
        style={{
          width: "42px",
          height: "42px",
          borderRadius: "var(--radius-md)",
          backgroundColor: "var(--accent-subtle)",
          color: "var(--accent-primary)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
        }}
      >
        <Folder size={22} fill="var(--accent-subtle)" />
      </div>

      <div style={{ flex: 1, minWidth: 0 }}>
        <h4
          style={{
            fontSize: "13px",
            fontWeight: 600,
            color: "var(--text-primary)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          title={folder.name}
        >
          {folder.name}
        </h4>
        <p style={{ fontSize: "11px", color: "var(--text-muted)", marginTop: "2px" }}>
          {itemCount !== undefined ? `${itemCount} items` : "Folder"}
        </p>
      </div>

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
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--text-muted)",
        }}
      >
        <MoreVertical size={15} />
      </button>
    </div>
  );
};
