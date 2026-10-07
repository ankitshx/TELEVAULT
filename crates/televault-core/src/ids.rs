//! Strongly-typed domain identifiers preventing entity ID confusion.

use crate::error::Result;
use crate::validation::validate_identifier_string;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! define_id {
    ($name:ident, $entity_name:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates and validates a new domain identifier.
            pub fn new(id: impl Into<String>) -> Result<Self> {
                let id = id.into();
                validate_identifier_string($entity_name, &id)?;
                Ok(Self(id))
            }

            /// Returns the identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the identifier and returns the inner String.
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

define_id!(
    ProfileId,
    "Profile",
    "Unique identifier for a backup profile."
);
define_id!(
    SnapshotId,
    "Snapshot",
    "Unique identifier for a backup snapshot."
);
define_id!(FileId, "File", "Unique identifier for a tracked file.");
define_id!(
    ChunkId,
    "Chunk",
    "Unique identifier for an encrypted data chunk."
);
define_id!(
    JobId,
    "Job",
    "Unique identifier for an execution or transfer job."
);
define_id!(
    ScheduleId,
    "Schedule",
    "Unique identifier for a recurring backup schedule."
);
define_id!(
    VersionId,
    "Version",
    "Unique identifier for a file or snapshot version."
);
define_id!(
    ManifestId,
    "Manifest",
    "Unique identifier for a chunk manifest."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_id_creation_and_methods() {
        let profile_id = ProfileId::new("profile-work_01").unwrap();
        assert_eq!(profile_id.as_str(), "profile-work_01");
        assert_eq!(&*profile_id, "profile-work_01");
        assert_eq!(profile_id.to_string(), "profile-work_01");
        assert_eq!(profile_id.into_inner(), "profile-work_01");
    }

    #[test]
    fn test_invalid_id_rejection() {
        assert!(ProfileId::new("").is_err());
        assert!(SnapshotId::new("snap/01").is_err());
        assert!(FileId::new("file with spaces").is_err());
        assert!(ChunkId::new("chunk!bad").is_err());
        assert!(JobId::new("job@work").is_err());
        assert!(ScheduleId::new("   ").is_err());
        assert!(VersionId::new("v.1.0").is_err());
    }

    #[test]
    fn test_type_safety_between_ids() {
        let profile_id = ProfileId::new("id-123").unwrap();
        let snapshot_id = SnapshotId::new("id-123").unwrap();

        // While inner strings are identical, types are distinct at compile time.
        assert_eq!(profile_id.as_str(), snapshot_id.as_str());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let id = ProfileId::new("profile-alpha").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"profile-alpha\"");

        let deserialized: ProfileId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, id);
    }
}
