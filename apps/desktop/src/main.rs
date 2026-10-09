// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! TELEVAULT Desktop application entry point.

use std::path::PathBuf;
use televault_desktop::builder::{create_ipc_builder, export_typescript_bindings};
use televault_desktop::state::DesktopAppState;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check if invoked with --export-types flag to generate TypeScript contract
    if args.iter().any(|a| a == "--export-types") {
        let out_path = if std::path::Path::new("apps/desktop/ui/src").exists() {
            PathBuf::from("apps/desktop/ui/src/bindings.ts")
        } else {
            PathBuf::from("ui/src/bindings.ts")
        };
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = export_typescript_bindings(&out_path) {
            eprintln!("Failed to export TypeScript bindings: {e}");
            std::process::exit(1);
        }
        println!(
            "Successfully exported TypeScript bindings to {}",
            out_path.display()
        );
        return;
    }

    let base_dir = std::env::var("TELEVAULT_BASE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            televault_core::paths::PathManager::system_default()
                .base_dir()
                .to_path_buf()
        });
    let state = DesktopAppState::new(base_dir, None).expect("Failed to initialize DesktopAppState");

    // Check if invoked with --check-startup flag for automated headless launch verification
    if args.iter().any(|a| a == "--check-startup") {
        println!(
            "TELEVAULT Desktop v{} initialized successfully.",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }

    let specta_builder = create_ipc_builder();

    let mut tauri_builder = tauri::Builder::default();
    tauri_builder = tauri_builder.manage(state);

    // Register all commands through specta handler
    let tauri_builder = tauri_builder.invoke_handler(specta_builder.invoke_handler());

    tauri_builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_desktop_version_defined() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
