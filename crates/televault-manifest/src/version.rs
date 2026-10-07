//! Manifest specification versioning.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Manifest specification format version.
///
/// Ensures backward compatibility while preventing unvalidated interpretation
/// of future schema versions.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum ManifestVersion {
    /// Format version 1.
    #[serde(rename = "v1")]
    #[default]
    V1,
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V1 => write!(f, "v1"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_display_and_serde() {
        let v = ManifestVersion::V1;
        assert_eq!(v.to_string(), "v1");

        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"v1\"");

        let deserialized: ManifestVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, v);

        // Unknown version fails
        assert!(serde_json::from_str::<ManifestVersion>("\"v99\"").is_err());
    }
}
