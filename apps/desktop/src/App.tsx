import React, { useState, useEffect, useCallback } from "react";
import {
  NavigationTab,
  LogicalFile,
  FolderItem,
  BreadcrumbItem,
  TransferItem,
  StorageStatus,
  ToastMessage,
} from "./types";
import { tauriApi, isTauri } from "./services/tauri";
import { Sidebar } from "./components/layout/Sidebar";
import { TopBar } from "./components/layout/TopBar";
import { HomeDashboard } from "./components/dashboard/HomeDashboard";
import { FileManager } from "./components/files/FileManager";
import { FilePreviewModal } from "./components/files/FilePreviewModal";
import { TransferCenter } from "./components/transfers/TransferCenter";
import { TransferDrawer } from "./components/transfers/TransferDrawer";
import { CommandPalette } from "./components/search/CommandPalette";
import { SettingsView } from "./components/settings/SettingsView";
import { UploadModal } from "./components/modals/UploadModal";
import { CreateFolderModal } from "./components/modals/CreateFolderModal";
import { ConfirmModal } from "./components/modals/ConfirmModal";
import { TelegramLoginModal } from "./components/modals/TelegramLoginModal";
import { ContextMenu, ContextMenuItem } from "./components/ui/ContextMenu";
import { ToastContainer } from "./components/ui/Toast";
import {
  Download,
  Star,
  Trash2,
  Folder,
  Info,
} from "lucide-react";

export default function App() {
  const [activeTab, setActiveTab] = useState<NavigationTab>("home");
  const [currentFolderId, setCurrentFolderId] = useState<string | null>(null);

  const [files, setFiles] = useState<LogicalFile[]>([]);
  const [folders, setFolders] = useState<FolderItem[]>([]);
  const [breadcrumbs, setBreadcrumbs] = useState<BreadcrumbItem[]>([
    { id: null, name: "My Cloud" },
  ]);
  const [favorites, setFavorites] = useState<LogicalFile[]>([]);
  const [transfers, setTransfers] = useState<TransferItem[]>([]);
  const [storageStatus, setStorageStatus] = useState<StorageStatus>({
    is_connected: false,
    provider_name: "Telegram MTProto",
    total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
    used_bytes: 0,
  });

  // Modals & Drawers state
  const [selectedPreviewFile, setSelectedPreviewFile] = useState<LogicalFile | null>(null);
  const [showUploadModal, setShowUploadModal] = useState(false);
  const [uploadInitialPath, setUploadInitialPath] = useState<string>("");
  const [showFolderModal, setShowFolderModal] = useState(false);
  const [showLoginModal, setShowLoginModal] = useState(false);
  const [showCommandPalette, setShowCommandPalette] = useState(false);
  const [isDragOver, setIsDragOver] = useState(false);

  // Custom Confirm Dialog state
  const [confirmDialog, setConfirmDialog] = useState<{
    isOpen: boolean;
    title: string;
    message: string;
    onConfirm: () => void;
  }>({
    isOpen: false,
    title: "",
    message: "",
    onConfirm: () => {},
  });

  // Custom Context Menu state
  const [contextMenu, setContextMenu] = useState<{
    isOpen: boolean;
    x: number;
    y: number;
    items: ContextMenuItem[];
  }>({
    isOpen: false,
    x: 0,
    y: 0,
    items: [],
  });

  // Toasts
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  const addToast = (type: "success" | "error" | "info" | "warning", title: string, message?: string) => {
    const id = `toast-${Date.now()}-${Math.random()}`;
    setToasts((prev) => [...prev, { id, type, title, message }]);
  };

  const dismissToast = (id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  };

  // Keyboard shortcut listener for Ctrl+K
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setShowCommandPalette((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Drag and Drop listeners over the application
  useEffect(() => {
    let unlistenTauriDrop: (() => void) | undefined;

    if (isTauri()) {
      import("@tauri-apps/api/webviewWindow")
        .then(({ getCurrentWebviewWindow }) => {
          return getCurrentWebviewWindow().onDragDropEvent((event: any) => {
            if (event.payload.type === "drop" && event.payload.paths?.length > 0) {
              const dropped = event.payload.paths[0];
              setUploadInitialPath(dropped);
              setShowUploadModal(true);
            }
          });
        })
        .then((unsub) => {
          unlistenTauriDrop = unsub;
        })
        .catch(() => {});
    }

    const handleDragOver = (e: DragEvent) => {
      e.preventDefault();
      setIsDragOver(true);
    };
    const handleDragLeave = (e: DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);
    };
    const handleDrop = (e: DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);
      if (!storageStatus.is_connected) {
        addToast("warning", "Login Required", "Please connect your Telegram account before uploading files.");
        setShowLoginModal(true);
        return;
      }
      setShowUploadModal(true);
    };

    window.addEventListener("dragover", handleDragOver);
    window.addEventListener("dragleave", handleDragLeave);
    window.addEventListener("drop", handleDrop);

    return () => {
      window.removeEventListener("dragover", handleDragOver);
      window.removeEventListener("dragleave", handleDragLeave);
      window.removeEventListener("drop", handleDrop);
      if (unlistenTauriDrop) unlistenTauriDrop();
    };
  }, [storageStatus.is_connected]);

  // Fetch initial storage status and enforce login
  const loadStorageStatus = useCallback(async () => {
    try {
      const status = await tauriApi.getStorageStatus();
      setStorageStatus(status);
      if (!status.is_connected) {
        setShowLoginModal(true);
      }
    } catch (err) {
      console.error(err);
      setShowLoginModal(true);
    }
  }, []);

  useEffect(() => {
    loadStorageStatus();
  }, [loadStorageStatus]);

  // Fetch directory contents
  const loadDirectory = useCallback(async () => {
    try {
      const [fFolders, fFiles, fCrumbs] = await Promise.all([
        tauriApi.listFolders(currentFolderId),
        tauriApi.listFiles(currentFolderId),
        tauriApi.getBreadcrumbs(currentFolderId),
      ]);
      setFolders(fFolders);
      setFiles(fFiles);
      setBreadcrumbs(fCrumbs);
    } catch (err) {
      console.error("Failed to load directory:", err);
    }
  }, [currentFolderId]);

  // Fetch transfers
  const loadTransfers = useCallback(async () => {
    try {
      const data = await tauriApi.getTransfers();
      setTransfers(data);
    } catch (err) {
      console.error("Failed to load transfers:", err);
    }
  }, []);

  // Fetch favorites
  const loadFavorites = useCallback(async () => {
    try {
      const favs = await tauriApi.listFavorites();
      setFavorites(favs);
    } catch (err) {
      console.error("Failed to load favorites:", err);
    }
  }, []);

  useEffect(() => {
    loadDirectory();
  }, [loadDirectory]);

  useEffect(() => {
    loadFavorites();
  }, [loadFavorites]);

  // Polling transfers in background every 1.5s
  useEffect(() => {
    loadTransfers();
    const interval = setInterval(loadTransfers, 1500);
    return () => clearInterval(interval);
  }, [loadTransfers]);

  // Folder and file operations
  const handleCreateFolder = async (name: string) => {
    try {
      await tauriApi.createFolder(name, currentFolderId);
      addToast("success", "Folder created", `"${name}" added successfully.`);
      loadDirectory();
    } catch (err) {
      addToast("error", "Failed to create folder", String(err));
    }
  };

  const handleUpload = async (filePath: string, isEncrypted: boolean, passphrase?: string) => {
    if (!storageStatus.is_connected) {
      addToast("warning", "Login Required", "Please connect your Telegram account before uploading files.");
      setShowLoginModal(true);
      return;
    }
    try {
      const res = await tauriApi.enqueueUpload(filePath, currentFolderId, passphrase, isEncrypted);
      if (res.isDuplicate) {
        addToast(
          "warning",
          "Already in Telegram Vault",
          `"${res.name}" is already safely stored in your Telegram channel.`
        );
      } else {
        addToast(
          "success",
          "Uploaded to Telegram",
          `"${res.name}" verified and uploaded to Primary & Mirror channels.`
        );
      }
      await loadDirectory();
      await loadStorageStatus();
      loadTransfers();
    } catch (err) {
      addToast("error", "Upload failed", String(err));
      throw err;
    }
  };

  const handleDownload = async (file: LogicalFile) => {
    if (!storageStatus.is_connected) {
      addToast("warning", "Login Required", "Please connect your Telegram account before downloading files.");
      setShowLoginModal(true);
      return;
    }
    try {
      addToast("info", "Download Started", `Retrieving "${file.name}" from Telegram MTProto...`);
      const res = await tauriApi.enqueueDownload(file.id, file.name);
      addToast("success", "Download Complete", `File saved to: ${res.restored_path}`);
      loadTransfers();
    } catch (err) {
      addToast("error", "Download failed", String(err));
    }
  };

  const handleToggleFavorite = async (fileId: string) => {
    try {
      await tauriApi.toggleFavorite(fileId);
      loadDirectory();
      loadFavorites();
    } catch (err) {
      addToast("error", "Error toggling favorite", String(err));
    }
  };

  const handleLogout = () => {
    setConfirmDialog({
      isOpen: true,
      title: "Log Out of Telegram",
      message:
        "Are you sure you want to disconnect this Telegram account? You will be prompted to log in with another Telegram account or phone number to access your cloud vault.",
      onConfirm: async () => {
        try {
          await tauriApi.logout();
          setStorageStatus({
            is_connected: false,
            provider_name: "Telegram MTProto",
            is_premium: false,
            max_single_upload_bytes: 2000 * 1024 * 1024,
            total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
            used_bytes: 0,
          });
          setFiles([]);
          addToast("info", "Logged out", "Telegram MTProto session cleared. Please log in with an account.");
          setShowLoginModal(true);
        } catch (err) {
          addToast("error", "Logout failed", String(err));
        }
      },
    });
  };

  const handleDeleteFile = (fileId: string) => {
    setConfirmDialog({
      isOpen: true,
      title: "Delete File from Cloud",
      message: "Are you sure you want to permanently delete this file from Telegram and local index?",
      onConfirm: async () => {
        try {
          await tauriApi.deleteFile(fileId);
          addToast("success", "File deleted", "Removed from Telegram channels and local index.");
          await loadDirectory();
          await loadStorageStatus();
          loadFavorites();
        } catch (err) {
          addToast("error", "Delete failed", String(err));
        }
      },
    });
  };

  const handleDeleteFolder = (folderId: string) => {
    setConfirmDialog({
      isOpen: true,
      title: "Delete Folder",
      message: "Are you sure you want to delete this folder and all its contents?",
      onConfirm: async () => {
        try {
          await tauriApi.deleteFolder(folderId);
          addToast("success", "Folder deleted", "Folder removed.");
          loadDirectory();
        } catch (err) {
          addToast("error", "Delete failed", String(err));
        }
      },
    });
  };

  // Context Menu Helpers
  const handleFileContextMenu = (file: LogicalFile, e: React.MouseEvent) => {
    e.stopPropagation();
    setContextMenu({
      isOpen: true,
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          id: "open",
          label: "Preview File",
          icon: <Info size={14} />,
          onClick: () => setSelectedPreviewFile(file),
        },
        {
          id: "download",
          label: "Download",
          icon: <Download size={14} />,
          onClick: () => handleDownload(file),
        },
        {
          id: "favorite",
          label: file.is_favorite ? "Remove Favorite" : "Add to Favorites",
          icon: <Star size={14} />,
          onClick: () => handleToggleFavorite(file.id),
        },
        {
          id: "delete",
          label: "Delete File",
          icon: <Trash2 size={14} />,
          danger: true,
          divider: true,
          onClick: () => handleDeleteFile(file.id),
        },
      ],
    });
  };

  const handleFolderContextMenu = (folder: FolderItem, e: React.MouseEvent) => {
    e.stopPropagation();
    setContextMenu({
      isOpen: true,
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          id: "open",
          label: "Open Folder",
          icon: <Folder size={14} />,
          onClick: () => setCurrentFolderId(folder.id),
        },
        {
          id: "delete",
          label: "Delete Folder",
          icon: <Trash2 size={14} />,
          danger: true,
          divider: true,
          onClick: () => handleDeleteFolder(folder.id),
        },
      ],
    });
  };

  // Active Transfer Drawer calculations
  const activeTransfers = transfers.filter(
    (t) =>
      t.state === "UPLOADING" ||
      t.state === "DOWNLOADING" ||
      t.state === "PREPARING" ||
      t.state === "QUEUED" ||
      t.state === "PAUSED"
  );
  const primaryActiveTransfer = activeTransfers[0];

  return (
    <div
      style={{
        display: "flex",
        width: "100%",
        height: "100vh",
        backgroundColor: "var(--bg-app)",
        color: "var(--text-primary)",
        overflow: "hidden",
      }}
    >
      {/* Sidebar */}
      <Sidebar
        activeTab={activeTab}
        onSelectTab={(tab) => {
          setActiveTab(tab);
          if (tab === "drive") setCurrentFolderId(null);
        }}
        storageStatus={storageStatus}
        activeTransferCount={activeTransfers.length}
      />

      {/* Main Container */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", height: "100vh", overflow: "hidden" }}>
        {/* Top Bar */}
        <TopBar
          onOpenSearch={() => setShowCommandPalette(true)}
          storageStatus={storageStatus}
          onOpenSettings={() => setActiveTab("settings")}
          onOpenConnect={() => setShowLoginModal(true)}
          onLogout={handleLogout}
        />

        {/* View Content Area */}
        <main style={{ flex: 1, overflow: "hidden", position: "relative" }}>
          {activeTab === "home" && (
            <HomeDashboard
              files={files}
              folders={folders}
              favorites={favorites}
              storageStatus={storageStatus}
              onOpenUpload={() => setShowUploadModal(true)}
              onOpenNewFolder={() => setShowFolderModal(true)}
              onOpenSearch={() => setShowCommandPalette(true)}
              onOpenTransfers={() => setActiveTab("transfers")}
              onSelectFile={(f) => setSelectedPreviewFile(f)}
              onNavigateToDrive={() => {
                setActiveTab("drive");
                setCurrentFolderId(null);
              }}
              onOpenConnect={() => setShowLoginModal(true)}
            />
          )}

          {activeTab === "drive" && (
            <FileManager
              files={files}
              folders={folders}
              breadcrumbs={breadcrumbs}
              currentFolderId={currentFolderId}
              onNavigateFolder={(id) => setCurrentFolderId(id)}
              onSelectFile={(f) => setSelectedPreviewFile(f)}
              onToggleFavorite={(id) => handleToggleFavorite(id)}
              onFileContextMenu={handleFileContextMenu}
              onFolderContextMenu={handleFolderContextMenu}
              onOpenUpload={() => setShowUploadModal(true)}
              onOpenNewFolder={() => setShowFolderModal(true)}
              isDragOver={isDragOver}
            />
          )}

          {activeTab === "favorites" && (
            <FileManager
              files={favorites}
              folders={[]}
              breadcrumbs={[{ id: null, name: "Favorites" }]}
              currentFolderId={null}
              onNavigateFolder={() => {}}
              onSelectFile={(f) => setSelectedPreviewFile(f)}
              onToggleFavorite={(id) => handleToggleFavorite(id)}
              onFileContextMenu={handleFileContextMenu}
              onFolderContextMenu={() => {}}
              onOpenUpload={() => setShowUploadModal(true)}
              onOpenNewFolder={() => setShowFolderModal(true)}
            />
          )}

          {activeTab === "recent" && (
            <FileManager
              files={files.slice(0, 20)}
              folders={[]}
              breadcrumbs={[{ id: null, name: "Recent Files" }]}
              currentFolderId={null}
              onNavigateFolder={() => {}}
              onSelectFile={(f) => setSelectedPreviewFile(f)}
              onToggleFavorite={(id) => handleToggleFavorite(id)}
              onFileContextMenu={handleFileContextMenu}
              onFolderContextMenu={() => {}}
              onOpenUpload={() => setShowUploadModal(true)}
              onOpenNewFolder={() => setShowFolderModal(true)}
            />
          )}

          {activeTab === "transfers" && (
            <TransferCenter
              transfers={transfers}
              onPauseTransfer={(id) => tauriApi.pauseTransfer(id).then(loadTransfers)}
              onResumeTransfer={(id) => tauriApi.resumeTransfer(id).then(loadTransfers)}
              onCancelTransfer={(id) => tauriApi.cancelTransfer(id).then(loadTransfers)}
            />
          )}

          {activeTab === "settings" && (
            <SettingsView storageStatus={storageStatus} onLogout={handleLogout} />
          )}
        </main>

        {/* Bottom Active Transfer Drawer (when not on transfers tab) */}
        {activeTab !== "transfers" && (
          <TransferDrawer
            activeTransfer={primaryActiveTransfer}
            totalActiveCount={activeTransfers.length}
            onOpenTransferCenter={() => setActiveTab("transfers")}
            onPause={(id) => tauriApi.pauseTransfer(id).then(loadTransfers)}
            onResume={(id) => tauriApi.resumeTransfer(id).then(loadTransfers)}
          />
        )}
      </div>

      {/* Global Modals & Overlays */}
      <UploadModal
        isOpen={showUploadModal}
        initialFilePath={uploadInitialPath}
        onClose={() => {
          setShowUploadModal(false);
          setUploadInitialPath("");
        }}
        onUpload={handleUpload}
      />

      <CreateFolderModal
        isOpen={showFolderModal}
        onClose={() => setShowFolderModal(false)}
        onCreate={handleCreateFolder}
      />

      <FilePreviewModal
        file={selectedPreviewFile}
        isOpen={!!selectedPreviewFile}
        onClose={() => setSelectedPreviewFile(null)}
        onDownload={handleDownload}
        onToggleFavorite={handleToggleFavorite}
        onDelete={handleDeleteFile}
      />

      <CommandPalette
        isOpen={showCommandPalette}
        onClose={() => setShowCommandPalette(false)}
        files={files}
        folders={folders}
        onNavigateTab={(tab) => {
          setActiveTab(tab);
          if (tab === "drive") setCurrentFolderId(null);
        }}
        onSelectFile={(f) => setSelectedPreviewFile(f)}
        onOpenFolder={(fId) => {
          setActiveTab("drive");
          setCurrentFolderId(fId);
        }}
        onOpenUpload={() => setShowUploadModal(true)}
        onOpenNewFolder={() => setShowFolderModal(true)}
      />

      <ConfirmModal
        isOpen={confirmDialog.isOpen}
        onClose={() => setConfirmDialog((prev) => ({ ...prev, isOpen: false }))}
        onConfirm={confirmDialog.onConfirm}
        title={confirmDialog.title}
        message={confirmDialog.message}
      />

      {contextMenu.isOpen && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenu.items}
          onClose={() => setContextMenu((prev) => ({ ...prev, isOpen: false }))}
        />
      )}

      <TelegramLoginModal
        isOpen={showLoginModal}
        onClose={() => setShowLoginModal(false)}
        storageStatus={storageStatus}
        onConnect={(newStatus) => {
          setStorageStatus(newStatus);
          addToast("success", "Storage connected", `${newStatus.provider_name} ready.`);
        }}
      />

      {/* Toast Notification Stack */}
      <ToastContainer toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
