//! TELEVAULT Desktop application entry point.

use std::path::PathBuf;
use televault_desktop::builder::{create_ipc_builder, export_typescript_bindings};
use televault_desktop::state::DesktopAppState;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check if invoked with --export-types flag to generate TypeScript contract
    if args.iter().any(|a| a == "--export-types") {
        let out_path = PathBuf::from("ui/src/bindings.ts");
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

    let base_dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let state = DesktopAppState::new(base_dir, None).expect("Failed to initialize DesktopAppState");

    let specta_builder = create_ipc_builder();

    let mut tauri_builder = tauri::Builder::default();
    tauri_builder = tauri_builder.manage(state);

    // Register all commands through specta handler
    let _tauri_builder = tauri_builder.invoke_handler(specta_builder.invoke_handler());

    println!(
        "TELEVAULT Desktop v{} initialized successfully.",
        env!("CARGO_PKG_VERSION")
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_desktop_version_defined() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
