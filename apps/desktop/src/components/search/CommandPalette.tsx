import React, { useState, useEffect, useRef } from "react";
import {
  Search,
  Upload,
  FolderPlus,
  Home,
  HardDrive,
  ArrowUpDown,
  Settings,
  FileText,
  Folder,
  X,
} from "lucide-react";
import { FolderItem, LogicalFile, NavigationTab } from "../../types";
import { formatBytes } from "../../utils/format";

export interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  files: LogicalFile[];
  folders: FolderItem[];
  onNavigateTab: (tab: NavigationTab) => void;
  onSelectFile: (file: LogicalFile) => void;
  onOpenFolder: (folderId: string) => void;
  onOpenUpload: () => void;
  onOpenNewFolder: () => void;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  isOpen,
  onClose,
  files,
  folders,
  onNavigateTab,
  onSelectFile,
  onOpenFolder,
  onOpenUpload,
  onOpenNewFolder,
}) => {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  // Static Quick Actions
  const quickActions = [
    {
      id: "action-upload",
      title: "Upload New File",
      category: "Action",
      icon: <Upload size={16} color="var(--accent-primary)" />,
      run: () => {
        onClose();
        onOpenUpload();
      },
    },
    {
      id: "action-new-folder",
      title: "Create New Folder",
      category: "Action",
      icon: <FolderPlus size={16} color="var(--accent-cyan)" />,
      run: () => {
        onClose();
        onOpenNewFolder();
      },
    },
    {
      id: "nav-home",
      title: "Go to Dashboard",
      category: "Navigation",
      icon: <Home size={16} color="var(--text-secondary)" />,
      run: () => {
        onClose();
        onNavigateTab("home");
      },
    },
    {
      id: "nav-drive",
      title: "Go to My Cloud",
      category: "Navigation",
      icon: <HardDrive size={16} color="var(--text-secondary)" />,
      run: () => {
        onClose();
        onNavigateTab("drive");
      },
    },
    {
      id: "nav-transfers",
      title: "Open Transfer Center",
      category: "Navigation",
      icon: <ArrowUpDown size={16} color="var(--text-secondary)" />,
      run: () => {
        onClose();
        onNavigateTab("transfers");
      },
    },
    {
      id: "nav-settings",
      title: "Open Settings",
      category: "Navigation",
      icon: <Settings size={16} color="var(--text-secondary)" />,
      run: () => {
        onClose();
        onNavigateTab("settings");
      },
    },
  ];

  // Dynamic file and folder matches
  const matchedFolders = query.trim()
    ? folders
        .filter((f) => f.name.toLowerCase().includes(query.toLowerCase()))
        .slice(0, 4)
        .map((f) => ({
          id: `folder-${f.id}`,
          title: f.name,
          category: "Folder",
          icon: <Folder size={16} color="var(--accent-primary)" />,
          run: () => {
            onClose();
            onOpenFolder(f.id);
          },
        }))
    : [];

  const matchedFiles = query.trim()
    ? files
        .filter((f) => f.name.toLowerCase().includes(query.toLowerCase()))
        .slice(0, 6)
        .map((f) => ({
          id: `file-${f.id}`,
          title: f.name,
          subtitle: formatBytes(f.size),
          category: "File",
          icon: <FileText size={16} color="var(--accent-cyan)" />,
          run: () => {
            onClose();
            onSelectFile(f);
          },
        }))
    : [];

  const allItems = query.trim()
    ? [...matchedFolders, ...matchedFiles, ...quickActions.filter((a) => a.title.toLowerCase().includes(query.toLowerCase()))]
    : quickActions;

  // Keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev + 1) % allItems.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSelectedIndex((prev) => (prev - 1 + allItems.length) % allItems.length);
    } else if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      allItems[selectedIndex].run();
    } else if (e.key === "Escape") {
      onClose();
    }
  };

  if (!isOpen) return null;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "var(--bg-overlay)",
        backdropFilter: "blur(10px)",
        WebkitBackdropFilter: "blur(10px)",
        display: "flex",
        alignItems: "flex-start",
        justifyContent: "center",
        paddingTop: "12vh",
        zIndex: 2000,
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="glass-surface-elevated animate-fade-in"
        style={{
          width: "100%",
          maxWidth: "580px",
          borderRadius: "var(--radius-xl)",
          overflow: "hidden",
          border: "1px solid var(--border-card)",
          display: "flex",
          flexDirection: "column",
          boxShadow: "var(--shadow-elevated)",
        }}
      >
        {/* Input Bar */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            padding: "16px 20px",
            borderBottom: "1px solid var(--border-subtle)",
          }}
        >
          <Search size={18} color="var(--accent-primary)" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
            placeholder="Type a command or search files..."
            style={{
              flex: 1,
              background: "transparent",
              border: "none",
              color: "var(--text-primary)",
              fontSize: "15px",
              outline: "none",
            }}
          />
          <button
            onClick={onClose}
            style={{
              background: "transparent",
              border: "none",
              color: "var(--text-muted)",
              cursor: "pointer",
            }}
          >
            <X size={18} />
          </button>
        </div>

        {/* Results List */}
        <div style={{ maxHeight: "360px", overflowY: "auto", padding: "10px" }}>
          {allItems.length === 0 ? (
            <div style={{ padding: "30px", textAlign: "center", color: "var(--text-muted)", fontSize: "13px" }}>
              No matches found for "{query}"
            </div>
          ) : (
            allItems.map((item, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={item.id}
                  onClick={item.run}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "space-between",
                    padding: "10px 14px",
                    borderRadius: "var(--radius-md)",
                    cursor: "pointer",
                    backgroundColor: isSelected ? "var(--accent-subtle)" : "transparent",
                    transition: "background var(--transition-fast)",
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: "12px", minWidth: 0 }}>
                    <div style={{ display: "flex", alignItems: "center" }}>{item.icon}</div>
                    <div style={{ minWidth: 0 }}>
                      <p
                        style={{
                          fontSize: "13px",
                          fontWeight: 500,
                          color: isSelected ? "var(--text-primary)" : "var(--text-secondary)",
                          whiteSpace: "nowrap",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                        }}
                      >
                        {item.title}
                      </p>
                      {"subtitle" in item && (
                        <p style={{ fontSize: "11px", color: "var(--text-muted)" }}>
                          {(item as any).subtitle}
                        </p>
                      )}
                    </div>
                  </div>

                  <span
                    style={{
                      fontSize: "11px",
                      color: "var(--text-muted)",
                      backgroundColor: "rgba(255, 255, 255, 0.05)",
                      padding: "2px 8px",
                      borderRadius: "var(--radius-xs)",
                    }}
                  >
                    {item.category}
                  </span>
                </div>
              );
            })
          )}
        </div>

        {/* Footer Shortcut Hints */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "10px 18px",
            backgroundColor: "rgba(0, 0, 0, 0.2)",
            borderTop: "1px solid var(--border-subtle)",
            fontSize: "11px",
            color: "var(--text-muted)",
          }}
        >
          <div style={{ display: "flex", gap: "14px" }}>
            <span>↑↓ Navigate</span>
            <span>↵ Select</span>
            <span>Esc Dismiss</span>
          </div>
          <span>TeleCloud Command Palette</span>
        </div>
      </div>
    </div>
  );
};
