export interface LogicalFile {
  id: string;
  folder_id: string | null;
  name: string;
  size: number;
  sha256: string;
  mime_type: string | null;
  is_favorite: boolean;
  is_encrypted: boolean;
  manifest_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface FolderItem {
  id: string;
  parent_id: string | null;
  name: string;
  created_at: string;
  updated_at: string;
  item_count?: number;
}

export interface BreadcrumbItem {
  id: string | null;
  name: string;
}

export type TransferDirection = "UPLOAD" | "DOWNLOAD";

export type TransferState =
  | "QUEUED"
  | "PREPARING"
  | "UPLOADING"
  | "DOWNLOADING"
  | "PAUSED"
  | "COMPLETED"
  | "FAILED"
  | "CANCELLED";

export interface TransferItem {
  id: string;
  file_id: string | null;
  direction: TransferDirection;
  local_path: string;
  file_name: string;
  total_bytes: number;
  transferred_bytes: number;
  state: TransferState;
  speed_bytes_per_sec: number;
  eta_seconds: number | null;
  active_chunk_index: number | null;
  total_chunks: number | null;
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface RecentItem {
  file: LogicalFile;
  action: "uploaded" | "downloaded" | "opened" | "modified";
  accessed_at: string;
}

export interface StorageStatus {
  is_connected: boolean;
  provider_name: string;
  user_identifier?: string;
  is_premium?: boolean;
  max_single_upload_bytes?: number;
  total_quota_bytes?: number;
  used_bytes?: number;
}

export type NavigationTab =
  | "home"
  | "drive"
  | "recent"
  | "favorites"
  | "transfers"
  | "trash"
  | "settings";

export type ViewMode = "grid" | "list";

export type FileCategory =
  | "all"
  | "documents"
  | "images"
  | "videos"
  | "audio"
  | "archives"
  | "code";

export interface ToastMessage {
  id: string;
  type: "success" | "error" | "info" | "warning";
  title: string;
  message?: string;
}

export interface SearchFilter {
  query: string;
  folder_id?: string | null;
  extension?: string | null;
  is_favorite?: boolean | null;
  min_size?: number | null;
  max_size?: number | null;
  limit?: number;
}
