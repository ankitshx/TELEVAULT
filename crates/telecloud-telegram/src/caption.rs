use serde::{Deserialize, Serialize};
use telecloud_core::{Result, TeleCloudError};
use uuid::Uuid;

pub const TC1_PREFIX: &str = "tc1 ";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tc1CaptionMetadata {
    #[serde(rename = "f")]
    pub file_id: Uuid,
    #[serde(rename = "p")]
    pub chunk_index: u32,
    #[serde(rename = "t")]
    pub total_chunks: u32,
    #[serde(rename = "s")]
    pub sha256: String,
    #[serde(rename = "n", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

pub fn format_tc1_caption(metadata: &Tc1CaptionMetadata) -> Result<String> {
    let json = serde_json::to_string(metadata).map_err(|e| TeleCloudError::Storage {
        operation: "format_caption".to_string(),
        message: e.to_string(),
        recoverable: false,
    })?;
    Ok(format!("{}{}", TC1_PREFIX, json))
}

pub fn parse_tc1_caption(caption: &str) -> Option<Tc1CaptionMetadata> {
    if !caption.starts_with(TC1_PREFIX) {
        return None;
    }
    let json_part = &caption[TC1_PREFIX.len()..];
    serde_json::from_str(json_part).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_caption_round_trip() {
        let file_id = Uuid::new_v4();
        let meta = Tc1CaptionMetadata {
            file_id,
            chunk_index: 2,
            total_chunks: 5,
            sha256: "abc123456789".to_string(),
            name: Some("test.pdf".to_string()),
        };

        let formatted = format_tc1_caption(&meta).unwrap();
        assert!(formatted.starts_with(TC1_PREFIX));

        let parsed = parse_tc1_caption(&formatted).unwrap();
        assert_eq!(parsed, meta);
    }
}
