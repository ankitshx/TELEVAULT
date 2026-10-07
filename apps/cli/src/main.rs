//! TELEVAULT Command Line Interface (CLI) application entry point.

use televault_core::paths::PathManager;

fn main() {
    let app_name = "TELEVAULT CLI";
    let version = env!("CARGO_PKG_VERSION");
    let base_dir = std::env::current_dir().unwrap_or_else(|_| ".".into());
    let _paths = PathManager::new(base_dir);

    println!("{app_name} v{version} - Cargo Workspace Foundation initialized.");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cli_version_defined() {
        assert!(!env!("CARGO_PKG_VERSION").is_empty());
    }
}
