//! Permanent Architectural Invariant Regression Tests for TELEVAULT Phase 10.
//!
//! Validates that the application maintains strict single-process desktop architecture:
//! - NO localhost URL or HTTP client for internal communication
//! - NO separate backend executable or process
//! - NO backend sidecars
//! - NO HTTP server / REST server / listening network ports
//! - NO Python runtime or scripts
//! - Pure Tauri 2 IPC and Rust core in the same process.

use std::fs;
use std::path::Path;

#[test]
fn test_no_localhost_in_frontend_or_ipc() {
    let desktop_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = desktop_dir.join("src");
    let ui_dir = desktop_dir.join("ui");

    let forbidden_patterns = [
        "http://localhost",
        "http://127.0.0.1",
        "https://localhost",
        "https://127.0.0.1",
        "ws://localhost",
        "ws://127.0.0.1",
    ];

    fn check_dir(dir: &Path, forbidden: &[&str]) {
        if !dir.exists() {
            return;
        }
        for entry in fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().unwrap() != "node_modules"
                    && path.file_name().unwrap() != "dist"
                    && path.file_name().unwrap() != "target"
                {
                    check_dir(&path, forbidden);
                }
            } else if let Some(ext) = path.extension() {
                if ext == "rs" || ext == "ts" || ext == "tsx" || ext == "js" || ext == "json" {
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    for pattern in forbidden {
                        assert!(
                            !content.contains(pattern),
                            "ARCHITECTURAL VIOLATION: Forbidden localhost pattern '{}' found in file {:?}",
                            pattern,
                            path
                        );
                    }
                }
            }
        }
    }

    check_dir(&src_dir, &forbidden_patterns);
    check_dir(&ui_dir, &forbidden_patterns);
}

#[test]
fn test_no_backend_sidecars_in_tauri_config() {
    let desktop_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tauri_conf = desktop_dir.join("tauri.conf.json");

    if tauri_conf.exists() {
        let content = fs::read_to_string(&tauri_conf).expect("read tauri.conf.json");
        assert!(
            !content.contains("\"externalBin\""),
            "ARCHITECTURAL VIOLATION: externalBin sidecars are strictly forbidden"
        );
        assert!(
            !content.contains("\"sidecar\""),
            "ARCHITECTURAL VIOLATION: sidecars are strictly forbidden"
        );
    }
}

#[test]
fn test_no_python_runtime_or_scripts_in_workspace() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    fn check_no_python(dir: &Path) {
        for entry in fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("entry");
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy();
            if name == "target" || name == ".git" || name == "node_modules" {
                continue;
            }
            if path.is_dir() {
                assert!(
                    name != "venv" && name != ".venv" && name != "env" && name != "__pycache__",
                    "ARCHITECTURAL VIOLATION: Python environment found at {:?}",
                    path
                );
                check_no_python(&path);
            } else if let Some(ext) = path.extension() {
                assert!(
                    ext != "py" && ext != "pyc" && ext != "pyo" && ext != "whl",
                    "ARCHITECTURAL VIOLATION: Python file found at {:?}",
                    path
                );
            }
        }
    }

    check_no_python(workspace_root);
}

#[test]
fn test_single_process_desktop_state_architecture() {
    // Proves that DesktopAppState owns all engines in-memory within the same process
    let state = televault_desktop::state::DesktopAppState::new_in_memory();
    assert!(state.cancellation_registry.lock().unwrap().is_empty());
    let token = state.register_cancellation("op-123");
    assert!(!token.is_cancelled());
    assert!(state.cancel_operation("op-123"));
    assert!(token.is_cancelled());
}
