//! Tauri Specta IPC builder and TypeScript contract generation.

use std::path::Path;
use tauri_specta::Builder;

use crate::commands::{
    backup, checker, repair, restore, retention, scheduler, system, telegram, transfer,
    verification,
};

/// Configures and returns the central [`Builder`] registering all TELEVAULT IPC commands.
pub fn create_ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        // System
        system::get_app_info,
        system::get_system_paths,
        system::get_app_config,
        system::update_app_config,
        system::get_startup_recovery_report,
        // Backup
        backup::list_backup_profiles,
        backup::get_backup_profile,
        backup::create_backup_profile,
        backup::update_backup_profile,
        backup::delete_backup_profile,
        backup::start_backup,
        backup::list_snapshots,
        backup::get_snapshot_files,
        // Restore
        restore::restore_file,
        restore::restore_manifest,
        restore::restore_snapshot,
        restore::verify_manifest_metadata,
        restore::verify_full_restore,
        // Checker
        checker::check_file_status,
        checker::is_incremental_backup_needed,
        checker::get_file_versions,
        // Transfer
        transfer::list_transfer_jobs,
        transfer::get_transfer_job,
        transfer::get_transfer_status,
        transfer::cancel_operation,
        // Scheduler
        scheduler::list_schedules,
        scheduler::get_schedule,
        scheduler::create_schedule,
        scheduler::update_schedule,
        scheduler::delete_schedule,
        scheduler::enable_schedule,
        scheduler::disable_schedule,
        scheduler::run_schedule_now,
        scheduler::get_scheduler_status,
        scheduler::get_schedule_history,
        // Retention
        retention::get_retention_policy,
        retention::set_retention_policy,
        retention::preview_retention,
        retention::execute_retention,
        retention::get_retention_history,
        // Verification
        verification::verify_file_backup,
        verification::verify_manifest,
        verification::verify_snapshot,
        verification::verify_profile,
        verification::get_verification_history,
        // Repair
        repair::preview_repair,
        repair::repair_file,
        repair::repair_snapshot,
        repair::get_repair_history,
        // Telegram Cloud Storage
        telegram::get_telegram_status,
        telegram::save_telegram_config,
        telegram::test_telegram_connection,
        telegram::disconnect_telegram,
    ])
}

/// Generates deterministic TypeScript type definitions and client bindings.
pub fn export_typescript_bindings(output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let builder = create_ipc_builder();
    builder.export(specta_typescript::Typescript::default(), output_path)?;
    Ok(())
}
