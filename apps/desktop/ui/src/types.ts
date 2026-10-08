export type NavTab =
  | "dashboard"
  | "backups"
  | "restore"
  | "schedules"
  | "verification"
  | "repair"
  | "activity"
  | "profiles"
  | "retention"
  | "settings";

export interface ToastMessage {
  id: string;
  type: "success" | "error" | "warning" | "info";
  title: string;
  message: string;
}

export interface ModalConfig {
  isOpen: boolean;
  type:
    | "create_profile"
    | "edit_profile"
    | "start_backup"
    | "create_schedule"
    | "schedule_history"
    | "snapshot_files"
    | "preview_retention"
    | "preview_repair"
    | "confirm_action"
    | null;
  data?: any;
}
