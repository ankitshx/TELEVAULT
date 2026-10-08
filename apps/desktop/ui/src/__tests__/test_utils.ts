import { vi } from "vitest";
import type {
  AppInfoDto,
  SystemPathsDto,
  AppConfigDto,
  BackupProfileDto,
  SnapshotDto,
  ScheduleDto,
  RetentionPolicyDto,
  TransferJobDto,
  RepairHistoryRecordDto,
  TransferStatusDto,
  SchedulerStatusDto,
} from "../bindings";

export const mockAppInfo: AppInfoDto = {
  app_name: "TELEVAULT",
  version: "0.15.0",
  platform: "windows",
  arch: "x86_64",
  build_mode: "release",
};

export const mockSystemPaths: SystemPathsDto = {
  base_dir: "C:\\ProgramData\\TELEVAULT",
  database_path: "C:\\ProgramData\\TELEVAULT\\televault.db",
  temp_dir: "C:\\ProgramData\\TELEVAULT\\cache",
  logs_dir: "C:\\ProgramData\\TELEVAULT\\logs",
  config_dir: "C:\\ProgramData\\TELEVAULT\\config",
};

export const mockAppConfig: AppConfigDto = {
  general: {
    theme: "dark",
    language: "en-US",
    check_updates: false,
  },
  storage: {
    max_cache_size_mb: 2048,
    temp_retention_hours: 24,
  },
  transfer: {
    chunk_size_kb: 4096,
    max_concurrent_transfers: 4,
    upload_limit_kbps: 0,
  },
  backup: {
    default_compression: "Zstd",
    retention_days: 30,
    verify_after_backup: true,
  },
};

export const mockProfiles: BackupProfileDto[] = [
  {
    profile_id: "prof-001",
    name: "Documents & Databases",
    description: "Daily mission critical documents",
    source_path: "C:\\Users\\User\\Documents",
    enabled: true,
    created_at: "2026-10-01T12:00:00Z",
    updated_at: "2026-10-01T12:00:00Z",
  },
  {
    profile_id: "prof-002",
    name: "Media Archives",
    description: "Archive large media payloads",
    source_path: "D:\\Media",
    enabled: false,
    created_at: "2026-10-02T12:00:00Z",
    updated_at: "2026-10-02T12:00:00Z",
  },
];

export const mockSnapshots: SnapshotDto[] = [
  {
    snapshot_id: "snap-001",
    profile_id: "prof-001",
    status: "completed",
    metadata: JSON.stringify({ total_files: 42, total_bytes: 5583457485 }),
    created_at: "2026-10-08T10:00:00Z",
  },
];

export const mockSchedules: ScheduleDto[] = [
  {
    schedule_id: "sched-001",
    profile_id: "prof-001",
    schedule_type: "daily",
    expression: "0 2 * * *",
    enabled: true,
    timezone: "UTC",
    next_run_at: "2026-10-09T02:00:00Z",
    last_run_at: "2026-10-08T02:00:00Z",
    last_status: "SUCCESS",
    last_error_code: null,
    created_at: "2026-10-01T12:00:00Z",
    updated_at: "2026-10-01T12:00:00Z",
  },
];

export const mockRetentionPolicy: RetentionPolicyDto = {
  profile_id: "prof-001",
  keep_latest_n: 10,
  keep_newer_than_secs: 2592000,
  keep_latest_successful: true,
  keep_latest_always: true,
  prune_failed: true,
  prune_empty: true,
  enabled: true,
};

export const mockTransferJobs: TransferJobDto[] = [
  {
    job_id: "xfer-001",
    file_id: "file-5gb-archive",
    chunk_id: "chunk-0",
    direction: "upload",
    status: "running",
    progress: 45.5,
    retry_count: 0,
    created_at: "2026-10-08T10:00:00Z",
    updated_at: "2026-10-08T10:01:00Z",
    error_message: null,
  },
  {
    job_id: "xfer-002",
    file_id: "file-small-doc",
    chunk_id: "chunk-0",
    direction: "upload",
    status: "completed",
    progress: 100.0,
    retry_count: 0,
    created_at: "2026-10-08T09:00:00Z",
    updated_at: "2026-10-08T09:00:10Z",
    error_message: null,
  },
];

export const mockRepairHistory: RepairHistoryRecordDto[] = [
  {
    repair_id: "rep-001",
    profile_id: "prof-001",
    snapshot_id: "snap-001",
    file_id: "file-5gb-archive",
    manifest_id: "man-001",
    chunk_id: "chunk-0",
    chunk_index: 1,
    repair_type: "MISSING_CHUNK",
    finding_code: "REMOTE_CHUNK_MISSING",
    old_storage_reference: "tg://old-ref-001",
    new_storage_reference: "tg://new-ref-001",
    status: "SUCCESS",
    bytes_processed: 2000000000,
    duration_ms: 120,
    error_message: null,
    repaired_at: "2026-10-08T12:05:00Z",
  },
];

export const mockTransferStatus: TransferStatusDto = {
  active_count: 1,
  queued_count: 0,
  completed_count: 1,
  failed_count: 0,
};

export const mockSchedulerStatus: SchedulerStatusDto = {
  status: "running",
  active_schedules_count: 1,
  running_profiles: [],
};

export function createTauriMock(customHandlers: Record<string, any> = {}) {
  const defaultHandlers: Record<string, any> = {
    get_app_info: mockAppInfo,
    get_system_paths: mockSystemPaths,
    get_app_config: mockAppConfig,
    update_app_config: (args: any) => args.request,
    list_backup_profiles: mockProfiles,
    get_backup_profile: (args: any) =>
      mockProfiles.find((p) => p.profile_id === args.profileId) || mockProfiles[0],
    create_backup_profile: (args: any) => ({
      profile_id: `prof-${Date.now()}`,
      name: args.request.name,
      description: args.request.description,
      source_path: args.request.source_path,
      enabled: args.request.enabled,
      created_at: "2026-10-08T12:00:00Z",
      updated_at: "2026-10-08T12:00:00Z",
    }),
    update_backup_profile: (args: any) => ({
      profile_id: args.request.profile_id,
      name: args.request.name,
      description: args.request.description,
      source_path: args.request.source_path,
      enabled: args.request.enabled,
      created_at: "2026-10-01T12:00:00Z",
      updated_at: "2026-10-08T12:00:00Z",
    }),
    delete_backup_profile: () => true,
    list_snapshots: mockSnapshots,
    get_snapshot_files: () => [],
    list_schedules: mockSchedules,
    get_transfer_status: mockTransferStatus,
    get_scheduler_status: mockSchedulerStatus,
    is_scheduler_running: true,
    enable_schedule: () => ({ ...mockSchedules[0], enabled: true }),
    disable_schedule: () => ({ ...mockSchedules[0], enabled: false }),
    run_schedule_now: () => "snap-001",
    get_schedule_history: () => [],
    delete_schedule: () => true,
    create_schedule: (args: any) => ({
      schedule_id: `sched-${Date.now()}`,
      profile_id: args.request.profile_id,
      schedule_type: args.request.schedule_type,
      expression: args.request.expression,
      enabled: args.request.enabled,
      timezone: args.request.timezone,
      next_run_at: "2026-10-09T00:00:00Z",
      last_run_at: null,
      last_status: null,
    }),
    get_retention_policy: () => mockRetentionPolicy,
    set_retention_policy: () => mockRetentionPolicy,
    preview_retention: () => ({
      profile_id: "prof-001",
      evaluated_at: "2026-10-08T12:00:00Z",
      policy: mockRetentionPolicy,
      decisions: [
        {
          snapshot_id: "snap-001",
          action: "KEEP",
          reason: "KEEP_LATEST",
          snapshot_created_at: "2026-10-08T10:00:00Z",
          age_secs: 7200,
          version_count: 42,
        },
        {
          snapshot_id: "snap-002",
          action: "PRUNE",
          reason: "PRUNE_EXCESS_SNAPSHOT",
          snapshot_created_at: "2026-10-01T10:00:00Z",
          age_secs: 700000,
          version_count: 5,
        },
      ],
      snapshots_evaluated: 2,
      snapshots_kept: 1,
      snapshots_pruned: 1,
    }),
    evaluate_retention: () => [
      {
        snapshot_id: "snap-001",
        action: "KEEP",
        reason: "KEEP_LATEST",
      },
    ],
    execute_retention: () => ({
      profile_id: "prof-001",
      executed_at: "2026-10-08T12:00:00Z",
      dry_run: false,
      evaluation: {
        profile_id: "prof-001",
        evaluated_at: "2026-10-08T12:00:00Z",
        policy: mockRetentionPolicy,
        decisions: [],
        snapshots_evaluated: 2,
        snapshots_kept: 1,
        snapshots_pruned: 1,
      },
      pruned_snapshots: ["snap-002"],
      pruned_versions_count: 5,
      success: true,
      error_message: null,
    }),
    get_retention_history: () => [],
    list_transfer_jobs: mockTransferJobs,
    cancel_transfer_job: () => true,
    get_repair_history: mockRepairHistory,
    get_verification_history: () => [],
    verify_snapshot: () => ({
      target_type: "snapshot",
      target_id: "snap-001",
      profile_id: "prof-001",
      level: "metadata_only",
      status: "healthy",
      is_restore_ready: true,
      findings: [],
      summary: {
        total_files: 42,
        total_manifests: 1,
        total_chunks: 42,
        healthy_chunks: 42,
        missing_chunks: 0,
        corrupted_chunks: 0,
        ownership_violations: 0,
        is_restore_ready: true,
        status: "healthy",
        duration_ms: 50,
      },
      verified_at: "2026-10-08T12:00:00Z",
    }),
    verify_profile: () => ({
      target_type: "profile",
      target_id: "prof-001",
      profile_id: "prof-001",
      level: "metadata_only",
      status: "healthy",
      is_restore_ready: true,
      findings: [],
      summary: {
        total_files: 42,
        total_manifests: 1,
        total_chunks: 42,
        healthy_chunks: 42,
        missing_chunks: 0,
        corrupted_chunks: 0,
        ownership_violations: 0,
        is_restore_ready: true,
        status: "healthy",
        duration_ms: 50,
      },
      verified_at: "2026-10-08T12:00:00Z",
    }),
    preview_repair: () => ({
      profile_id: "prof-001",
      target_type: "snapshot",
      target_id: "snap-001",
      eligible_candidates: [
        {
          chunk_id: "chunk-damaged-01",
          chunk_index: 1,
          total_chunks: 3,
          finding_code: "REMOTE_CHUNK_MISSING",
          old_storage_reference: "tg://old-ref-001",
          plaintext_size: 2000000000,
          stored_size: 1980000000,
          source_path: "C:\\Users\\User\\Documents\\large_file.dat",
        },
      ],
      ineligible_findings_count: 0,
      total_affected_chunks: 1,
      total_estimated_bytes: 2000000000,
      is_repairable: true,
    }),
    repair_snapshot: () => ({
      profile_id: "prof-001",
      target_type: "snapshot",
      target_id: "snap-001",
      total_chunks_evaluated: 1,
      repaired_chunks: 1,
      skipped_healthy_chunks: 0,
      failed_chunks: 0,
      total_bytes_transferred: 2000000000,
      duration_ms: 120,
      chunk_results: [
        {
          chunk_id: "chunk-damaged-01",
          chunk_index: 1,
          status: "SUCCESS",
          old_storage_reference: "tg://old-ref-001",
          new_storage_reference: "tg://new-ref-001",
          bytes_processed: 2000000000,
          duration_ms: 120,
          error: null,
        },
      ],
    }),
    restore_snapshot: () => ({
      snapshot_id: "snap-001",
      target_directory: "C:\\Target\\RestoredData",
      total_files: 42,
      restored_files: 42,
      skipped_files: 0,
      failed_files: 0,
      total_bytes: 5583457485,
      duration_ms: 350,
    }),
    restore_file: () => ({
      file_id: "file-5gb-archive",
      destination_path: "C:\\Restore\\archive.bin",
      bytes_restored: 5583457485,
      duration_ms: 320,
    }),
    start_backup: () => ({
      snapshot_id: "snap-002",
      profile_id: "prof-001",
      status: "completed",
      new_files: 5,
      modified_files: 0,
      unchanged_files: 0,
      deleted_files: 0,
      transferred_chunks: 1,
      transferred_bytes: 1048576,
      reused_bytes: 0,
      elapsed_ms: 120,
      error_message: null,
    }),
    run_backup: () => ({
      profile_id: "prof-001",
      snapshot_id: "snap-002",
      total_files: 10,
      total_bytes: 1048576,
      duration_ms: 50,
      status: "completed",
    }),
    cancel_backup: () => true,
    cancel_operation: () => true,
  };

  const handlers = { ...defaultHandlers, ...customHandlers };

  return vi.fn().mockImplementation(async (cmd: string, args: any) => {
    if (handlers[cmd] !== undefined) {
      const val = handlers[cmd];
      return typeof val === "function" ? val(args) : val;
    }
    throw new Error(`Unhandled mock command: ${cmd}`);
  });
}
