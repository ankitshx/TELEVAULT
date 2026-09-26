import { invoke, isTauri as checkIsTauri } from "@tauri-apps/api/core";
import {
  BreadcrumbItem,
  FolderItem,
  LogicalFile,
  RecentItem,
  SearchFilter,
  StorageStatus,
  TransferItem,
} from "../types";

export const isTauri = (): boolean => {
  if (typeof window === "undefined") return false;
  return checkIsTauri() || "__TAURI_INTERNALS__" in window || Boolean((window as any).isTauri);
};

export const tauriApi = {
  async selectFile(): Promise<string | null> {
    if (!isTauri()) return null;
    return invoke<string | null>("select_file");
  },

  async getStorageStatus(): Promise<StorageStatus> {
    if (!isTauri()) {
      return {
        is_connected: false,
        provider_name: "Telegram MTProto",
        is_premium: false,
        max_single_upload_bytes: 2000 * 1024 * 1024,
        total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024, // 2 TB
        used_bytes: 0,
      };
    }
    return invoke<StorageStatus>("get_storage_status");
  },

  async listFolders(parentId: string | null = null): Promise<FolderItem[]> {
    if (!isTauri()) return [];
    return invoke<FolderItem[]>("list_folders", { parentId });
  },

  async listFiles(folderId: string | null = null): Promise<LogicalFile[]> {
    if (!isTauri()) return [];
    return invoke<LogicalFile[]>("list_files", { folderId });
  },

  async getBreadcrumbs(folderId: string | null): Promise<BreadcrumbItem[]> {
    if (!isTauri()) {
      return [{ id: null, name: "My Cloud" }];
    }
    return invoke<BreadcrumbItem[]>("get_breadcrumbs", { folderId });
  },

  async createFolder(name: string, parentId: string | null): Promise<FolderItem> {
    if (!isTauri()) {
      return {
        id: `mock-folder-${Date.now()}`,
        parent_id: parentId,
        name,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
    }
    return invoke<FolderItem>("create_folder", { name, parentId });
  },

  async renameFolder(id: string, name: string): Promise<FolderItem> {
    if (!isTauri()) throw new Error("Desktop runtime required");
    return invoke<FolderItem>("rename_folder", { id, name });
  },

  async deleteFolder(id: string): Promise<void> {
    if (!isTauri()) return;
    return invoke<void>("delete_folder", { id });
  },

  async renameFile(id: string, name: string): Promise<LogicalFile> {
    if (!isTauri()) throw new Error("Desktop runtime required");
    return invoke<LogicalFile>("rename_file", { id, name });
  },

  async deleteFile(id: string): Promise<void> {
    if (!isTauri()) return;
    return invoke<void>("delete_file", { id });
  },

  async toggleFavorite(fileId: string): Promise<boolean> {
    if (!isTauri()) return true;
    return invoke<boolean>("toggle_favorite", { fileId });
  },

  async listFavorites(): Promise<LogicalFile[]> {
    if (!isTauri()) return [];
    return invoke<LogicalFile[]>("list_favorites");
  },

  async listRecentItems(limit: number = 30): Promise<RecentItem[]> {
    if (!isTauri()) return [];
    return invoke<RecentItem[]>("list_recent_items", { limit });
  },

  async searchFiles(filter: SearchFilter): Promise<LogicalFile[]> {
    if (!isTauri()) return [];
    return invoke<LogicalFile[]>("search_files", { filter });
  },

  async enqueueUpload(
    localPath: string,
    folderId: string | null = null,
    passphrase?: string,
    _isEncrypted: boolean = false,
    fileName?: string
  ): Promise<string> {
    const computedName =
      fileName ||
      localPath.split(/[/\\]/).filter(Boolean).pop() ||
      "uploaded_file";
    if (!isTauri()) {
      return `transfer-${Date.now()}`;
    }
    return invoke<string>("enqueue_upload", {
      localPath,
      fileName: computedName,
      folderId,
      passphrase: passphrase || null,
    });
  },

  async enqueueDownload(
    fileId: string,
    destPath: string,
    passphrase?: string
  ): Promise<string> {
    if (!isTauri()) {
      return `transfer-${Date.now()}`;
    }
    return invoke<string>("enqueue_download", {
      fileId,
      destPath,
      passphrase: passphrase || null,
    });
  },

  async getTransfers(): Promise<TransferItem[]> {
    if (!isTauri()) return [];
    return invoke<TransferItem[]>("get_transfers");
  },

  async getActiveTransfers(): Promise<TransferItem[]> {
    if (!isTauri()) return [];
    return invoke<TransferItem[]>("get_active_transfers");
  },

  async pauseTransfer(transferId: string): Promise<void> {
    if (!isTauri()) return;
    return invoke<void>("pause_transfer", { transferId });
  },

  async resumeTransfer(transferId: string): Promise<void> {
    if (!isTauri()) return;
    return invoke<void>("resume_transfer", { transferId });
  },

  async cancelTransfer(transferId: string): Promise<void> {
    if (!isTauri()) return;
    return invoke<void>("cancel_transfer", { transferId });
  },
};
