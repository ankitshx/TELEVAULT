//! TELEVAULT Desktop application entry point foundation.
//!
//! Note: Full Tauri 2 runtime, WebView2 host, and React frontend integration
//! will be connected in subsequent phases according to single-process architecture.

use televault_core::paths::PathManager;

fn main() {
    let app_name = "TELEVAULT Desktop";
    let version = env!("CARGO_PKG_VERSION");
    let base_dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let _paths = PathManager::new(base_dir);

    println!("{app_name} v{version} - Cargo Workspace Foundation initialized.");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_desktop_version_defined() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
