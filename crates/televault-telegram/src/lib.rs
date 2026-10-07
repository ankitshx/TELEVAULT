//! Telegram communication, session management, and storage adapter for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

/// Returns the telegram module status string.
pub fn telegram_module_status() -> &'static str {
    "initialized"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_module_status() {
        assert_eq!(telegram_module_status(), "initialized");
    }
}
