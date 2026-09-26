import React, { useState } from "react";
import {
  Grid,
  List,
  Upload,
  FolderPlus,
  ChevronRight,
  HardDrive,
  FileQuestion,
} from "lucide-react";
import { BreadcrumbItem, FileCategory, FolderItem, LogicalFile, ViewMode } from "../../types";
import { getFileCategory } from "../../utils/format";
import { Button } from "../ui/Button";
import { FileCard } from "./FileCard";
import { FolderCard } from "./FolderCard";
import { FileRow } from "./FileRow";
import { FolderRow } from "./FolderRow";
import { EmptyState } from "../ui/EmptyState";

export interface FileManagerProps {
  files: LogicalFile[];
  folders: FolderItem[];
  breadcrumbs: BreadcrumbItem[];
  currentFolderId: string | null;
  onNavigateFolder: (folderId: string | null) => void;
  onSelectFile: (file: LogicalFile) => void;
  onToggleFavorite: (fileId: string, e: React.MouseEvent) => void;
  onFileContextMenu: (file: LogicalFile, e: React.MouseEvent) => void;
  onFolderContextMenu: (folder: FolderItem, e: React.MouseEvent) => void;
  onOpenUpload: () => void;
  onOpenNewFolder: () => void;
  isDragOver?: boolean;
}

export const FileManager: React.FC<FileManagerProps> = ({
  files,
  folders,
  breadcrumbs,
  onNavigateFolder,
  onSelectFile,
  onToggleFavorite,
  onFileContextMenu,
  onFolderContextMenu,
  onOpenUpload,
  onOpenNewFolder,
  isDragOver = false,
}) => {
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [activeCategory, setActiveCategory] = useState<FileCategory>("all");

  const filteredFiles = files.filter((f) => {
    if (activeCategory === "all") return true;
    return getFileCategory(f.name, f.mime_type) === activeCategory;
  });

  const categories: { id: FileCategory; label: string }[] = [
    { id: "all", label: "All Items" },
    { id: "documents", label: "Documents" },
    { id: "images", label: "Images" },
    { id: "videos", label: "Videos" },
    { id: "audio", label: "Audio" },
    { id: "archives", label: "Archives" },
    { id: "code", label: "Code" },
  ];

  const isEmpty = folders.length === 0 && filteredFiles.length === 0;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        padding: "24px 32px",
        overflowY: "auto",
        position: "relative",
      }}
    >
      {/* Drag & Drop Visual Overlay */}
      {isDragOver && (
        <div
          style={{
            position: "absolute",
            inset: "16px",
            backgroundColor: "rgba(7, 11, 18, 0.9)",
            border: "2px dashed var(--accent-primary)",
            borderRadius: "var(--radius-xl)",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 100,
            pointerEvents: "none",
            boxShadow: "var(--accent-glow)",
          }}
        >
          <Upload size={48} color="var(--accent-primary)" />
          <h3 style={{ fontSize: "18px", fontWeight: 600, color: "#fff", marginTop: "16px" }}>
            Drop files here to upload to TeleCloud
          </h3>
          <p style={{ fontSize: "13px", color: "var(--text-secondary)", marginTop: "4px" }}>
            Client-side AES-256 chunking and encryption will be applied
          </p>
        </div>
      )}

      {/* Top Controls Toolbar */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "20px",
          flexWrap: "wrap",
          gap: "12px",
        }}
      >
        {/* Breadcrumb Path */}
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          {breadcrumbs.map((crumb, idx) => {
            const isLast = idx === breadcrumbs.length - 1;
            return (
              <React.Fragment key={crumb.id || "root"}>
                {idx > 0 && <ChevronRight size={14} color="var(--text-muted)" />}
                <button
                  onClick={() => onNavigateFolder(crumb.id)}
                  style={{
                    background: "none",
                    border: "none",
                    color: isLast ? "var(--text-primary)" : "var(--text-secondary)",
                    fontSize: "14px",
                    fontWeight: isLast ? 600 : 500,
                    cursor: isLast ? "default" : "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: "6px",
                    padding: "4px 6px",
                    borderRadius: "var(--radius-xs)",
                  }}
                  onMouseEnter={(e) => {
                    if (!isLast) e.currentTarget.style.color = "var(--accent-primary)";
                  }}
                  onMouseLeave={(e) => {
                    if (!isLast) e.currentTarget.style.color = "var(--text-secondary)";
                  }}
                >
                  {idx === 0 && <HardDrive size={15} color="var(--accent-primary)" />}
                  {crumb.name}
                </button>
              </React.Fragment>
            );
          })}
        </div>

        {/* Action Buttons: New Folder, Upload, View Toggle */}
        <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
          <div
            style={{
              display: "flex",
              backgroundColor: "var(--bg-surface)",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--border-card)",
              padding: "2px",
            }}
          >
            <button
              onClick={() => setViewMode("grid")}
              style={{
                background: viewMode === "grid" ? "var(--accent-subtle)" : "transparent",
                color: viewMode === "grid" ? "var(--accent-primary)" : "var(--text-muted)",
                border: "none",
                padding: "6px 8px",
                borderRadius: "var(--radius-sm)",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
              }}
              title="Grid View"
            >
              <Grid size={15} />
            </button>
            <button
              onClick={() => setViewMode("list")}
              style={{
                background: viewMode === "list" ? "var(--accent-subtle)" : "transparent",
                color: viewMode === "list" ? "var(--accent-primary)" : "var(--text-muted)",
                border: "none",
                padding: "6px 8px",
                borderRadius: "var(--radius-sm)",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
              }}
              title="List View"
            >
              <List size={15} />
            </button>
          </div>

          <Button
            variant="secondary"
            size="sm"
            icon={<FolderPlus size={15} />}
            onClick={onOpenNewFolder}
          >
            New Folder
          </Button>

          <Button
            variant="primary"
            size="sm"
            icon={<Upload size={15} />}
            onClick={onOpenUpload}
          >
            Upload File
          </Button>
        </div>
      </div>

      {/* Category Filter Chips */}
      <div
        style={{
          display: "flex",
          gap: "8px",
          overflowX: "auto",
          paddingBottom: "12px",
          marginBottom: "16px",
        }}
      >
        {categories.map((cat) => {
          const isActive = activeCategory === cat.id;
          return (
            <button
              key={cat.id}
              onClick={() => setActiveCategory(cat.id)}
              style={{
                padding: "5px 12px",
                borderRadius: "var(--radius-full)",
                border: `1px solid ${isActive ? "var(--accent-primary)" : "var(--border-subtle)"}`,
                backgroundColor: isActive ? "var(--accent-subtle)" : "transparent",
                color: isActive ? "var(--text-primary)" : "var(--text-secondary)",
                fontSize: "12px",
                fontWeight: isActive ? 600 : 500,
                cursor: "pointer",
                whiteSpace: "nowrap",
                transition: "all var(--transition-fast)",
              }}
            >
              {cat.label}
            </button>
          );
        })}
      </div>

      {/* Main Content Area */}
      {isEmpty ? (
        <EmptyState
          icon={<FileQuestion size={32} />}
          title="This folder is empty"
          description="Upload files or create subfolders to start organizing your cloud storage."
          actionLabel="Upload First File"
          onAction={onOpenUpload}
          style={{ flex: 1 }}
        />
      ) : viewMode === "grid" ? (
        <div style={{ display: "flex", flexDirection: "column", gap: "24px" }}>
          {/* Folders Grid */}
          {folders.length > 0 && (
            <div>
              <h4
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.04em",
                  marginBottom: "12px",
                }}
              >
                Folders ({folders.length})
              </h4>
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
                  gap: "14px",
                }}
              >
                {folders.map((folder) => (
                  <FolderCard
                    key={folder.id}
                    folder={folder}
                    onOpen={onNavigateFolder}
                    onContextMenu={onFolderContextMenu}
                  />
                ))}
              </div>
            </div>
          )}

          {/* Files Grid */}
          {filteredFiles.length > 0 && (
            <div>
              <h4
                style={{
                  fontSize: "12px",
                  fontWeight: 600,
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.04em",
                  marginBottom: "12px",
                }}
              >
                Files ({filteredFiles.length})
              </h4>
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))",
                  gap: "14px",
                }}
              >
                {filteredFiles.map((file) => (
                  <FileCard
                    key={file.id}
                    file={file}
                    onSelect={onSelectFile}
                    onToggleFavorite={onToggleFavorite}
                    onContextMenu={onFileContextMenu}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      ) : (
        /* List View */
        <div
          className="telecloud-card"
          style={{
            display: "flex",
            flexDirection: "column",
            padding: "8px",
            overflow: "hidden",
          }}
        >
          {/* Table Header */}
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "minmax(200px, 3fr) 100px 120px 140px 80px",
              padding: "8px 16px",
              fontSize: "11px",
              fontWeight: 600,
              color: "var(--text-muted)",
              textTransform: "uppercase",
              letterSpacing: "0.04em",
              borderBottom: "1px solid var(--border-subtle)",
            }}
          >
            <span>Name</span>
            <span>Size</span>
            <span>Type</span>
            <span>Modified</span>
            <span style={{ textAlign: "right" }}>Actions</span>
          </div>

          {/* Table Body */}
          <div style={{ display: "flex", flexDirection: "column", gap: "2px", marginTop: "4px" }}>
            {folders.map((folder) => (
              <FolderRow
                key={folder.id}
                folder={folder}
                onOpen={onNavigateFolder}
                onContextMenu={onFolderContextMenu}
              />
            ))}
            {filteredFiles.map((file) => (
              <FileRow
                key={file.id}
                file={file}
                onSelect={onSelectFile}
                onToggleFavorite={onToggleFavorite}
                onContextMenu={onFileContextMenu}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
