//! Embedded database, migrations, and local catalog persistence for TELEVAULT.

#![deny(missing_docs)]

pub use televault_core as core;

/// Returns the database schema version.
pub fn schema_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        assert_eq!(schema_version(), 1);
    }
}
