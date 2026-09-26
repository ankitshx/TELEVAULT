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

const API_BASE = "http://127.0.0.1:8000";

export const tauriApi = {
  async selectFile(): Promise<string | null> {
    if (isTauri()) {
      return invoke<string | null>("select_file");
    }
    return null;
  },

  async getStorageStatus(): Promise<StorageStatus> {
    try {
      const res = await fetch(`${API_BASE}/api/status`);
      if (res.ok) {
        const data = await res.json();
        const user = data.user || {};
        return {
          is_connected: Boolean(data.authenticated),
          provider_name: data.mode || "Telegram MTProto",
          user_identifier: user.username ? `@${user.username}` : user.first_name || "Telegram User",
          is_premium: false,
          max_single_upload_bytes: 2000 * 1024 * 1024,
          total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
          used_bytes: data.stats ? data.stats.total_bytes : 0,
        };
      }
    } catch (e) {
      console.warn("Could not reach TeleVault API status:", e);
    }
    return {
      is_connected: false,
      provider_name: "Telegram MTProto",
      is_premium: false,
      max_single_upload_bytes: 2000 * 1024 * 1024,
      total_quota_bytes: 2 * 1024 * 1024 * 1024 * 1024,
      used_bytes: 0,
    };
  },

  async listFolders(parentId: string | null = null): Promise<FolderItem[]> {
    if (isTauri()) {
      return invoke<FolderItem[]>("list_folders", { parentId }).catch(() => []);
    }
    return [];
  },

  async listFiles(folderId: string | null = null): Promise<LogicalFile[]> {
    try {
      const res = await fetch(`${API_BASE}/api/records`);
      if (res.ok) {
        const data = await res.json();
        return (data.records || []).map((r: any) => ({
          id: r.id,
          folder_id: null,
          name: r.name,
          size: r.size,
          sha256: r.sha256,
          mime_type: r.category,
          is_favorite: false,
          is_encrypted: r.mode === "private",
          manifest_id: r.primary_msg_id ? String(r.primary_msg_id) : undefined,
          created_at: r.created_at,
          updated_at: r.created_at,
        }));
      }
    } catch (e) {
      console.warn("Could not reach TeleVault API records:", e);
    }
    if (isTauri()) {
      return invoke<LogicalFile[]>("list_files", { folderId }).catch(() => []);
    }
    return [];
  },

  async getBreadcrumbs(_folderId: string | null): Promise<BreadcrumbItem[]> {
    return [{ id: null, name: "Telegram Vault" }];
  },

  async createFolder(name: string, parentId: string | null): Promise<FolderItem> {
    if (isTauri()) {
      return invoke<FolderItem>("create_folder", { name, parentId });
    }
    return {
      id: `folder-${Date.now()}`,
      parent_id: parentId,
      name,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
  },

  async renameFolder(id: string, name: string): Promise<FolderItem> {
    if (isTauri()) {
      return invoke<FolderItem>("rename_folder", { id, name });
    }
    throw new Error("Folder operations require desktop runtime");
  },

  async deleteFolder(id: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("delete_folder", { id });
    }
  },

  async renameFile(id: string, name: string): Promise<LogicalFile> {
    const files = await this.listFiles();
    const found = files.find((f) => f.id === id);
    if (found) {
      found.name = name;
      return found;
    }
    throw new Error("File not found");
  },

  async deleteFile(id: string): Promise<void> {
    try {
      await fetch(`${API_BASE}/api/records/${id}`, { method: "DELETE" });
    } catch (e) {
      console.warn("Delete endpoint error:", e);
    }
    if (isTauri()) {
      return invoke<void>("delete_file", { id }).catch(() => {});
    }
  },

  async logout(): Promise<void> {
    try {
      await fetch(`${API_BASE}/api/logout`, { method: "POST" });
    } catch (e) {
      console.warn("Logout error:", e);
    }
  },

  async toggleFavorite(_fileId: string): Promise<boolean> {
    return true;
  },

  async listFavorites(): Promise<LogicalFile[]> {
    const files = await this.listFiles();
    return files.filter((f) => f.is_favorite);
  },

  async listRecentItems(limit: number = 30): Promise<RecentItem[]> {
    const files = await this.listFiles();
    return files.slice(0, limit).map((f) => ({
      file: f,
      action: "uploaded" as const,
      accessed_at: f.created_at,
    }));
  },

  async searchFiles(filter: SearchFilter): Promise<LogicalFile[]> {
    const files = await this.listFiles();
    const query = filter.query.toLowerCase().trim();
    if (!query) return files;
    return files.filter(
      (f) => f.name.toLowerCase().includes(query) || f.sha256.toLowerCase().includes(query)
    );
  },

  async enqueueUpload(
    localPath: string,
    _folderId: string | null = null,
    passphrase?: string,
    isEncrypted: boolean = false,
    _fileName?: string
  ): Promise<{
    recordId: string;
    isDuplicate: boolean;
    message: string;
    name: string;
    messageId?: number;
    channelId?: number;
  }> {
    const res = await fetch(`${API_BASE}/api/backup/path`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        path: localPath,
        mode: isEncrypted ? "private" : "original",
        passphrase: passphrase || null,
      }),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({ detail: res.statusText }));
      throw new Error(err.detail || "Upload to Telegram failed");
    }

    const data = await res.json();
    const item = data.results?.[0] || {};
    return {
      recordId: item.record_id || `upload-${Date.now()}`,
      isDuplicate: Boolean(item.is_duplicate),
      message: item.message || "Uploaded to Telegram.",
      name: item.name || localPath.split(/[\\/]/).pop() || "file",
      messageId: item.message_id,
      channelId: item.channel_id,
    };
  },

  async enqueueDownload(
    fileId: string,
    _destPath?: string,
    _passphrase?: string
  ): Promise<{ success: boolean; restored_path: string }> {
    const res = await fetch(`${API_BASE}/api/restore/${fileId}`, {
      method: "POST",
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({ detail: res.statusText }));
      throw new Error(err.detail || "Download/Restore failed");
    }

    const data = await res.json();
    return {
      success: true,
      restored_path: data.restored_path || "restored directory",
    };
  },

  async getTransfers(): Promise<TransferItem[]> {
    if (isTauri()) {
      return invoke<TransferItem[]>("get_transfers").catch(() => []);
    }
    return [];
  },

  async getActiveTransfers(): Promise<TransferItem[]> {
    if (isTauri()) {
      return invoke<TransferItem[]>("get_active_transfers").catch(() => []);
    }
    return [];
  },

  async pauseTransfer(transferId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("pause_transfer", { transferId }).catch(() => {});
    }
  },

  async resumeTransfer(transferId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("resume_transfer", { transferId }).catch(() => {});
    }
  },

  async cancelTransfer(transferId: string): Promise<void> {
    if (isTauri()) {
      return invoke<void>("cancel_transfer", { transferId }).catch(() => {});
    }
  },
};
