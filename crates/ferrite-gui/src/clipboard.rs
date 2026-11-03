//! System clipboard data serialization and deserialization for FerritePLC.
//!
//! Provides versioned JSON schemas for copying and pasting Ladder rungs,
//! SFC steps and transitions, and FBD function blocks between POUs,
//! windows, or external tools.

use serde::{Deserialize, Serialize};

use ferrite_lang_fbd::FbdBlock;
use ferrite_lang_ld::LdRung;
use ferrite_lang_sfc::{SfcStep, SfcTransition};

pub const CLIPBOARD_SCHEMA_V1: u32 = 1;

/// Top-level versioned clipboard envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FerriteClipboardPayload {
    pub schema_version: u32,
    pub data: FerriteClipboardData,
}

/// Clipboard payload content variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FerriteClipboardData {
    Ladder {
        rungs: Vec<LdRung>,
    },
    Sfc {
        steps: Vec<SfcStep>,
        transitions: Vec<SfcTransition>,
    },
    Fbd {
        blocks: Vec<FbdBlock>,
    },
}

impl FerriteClipboardPayload {
    pub fn new(data: FerriteClipboardData) -> Self {
        Self {
            schema_version: CLIPBOARD_SCHEMA_V1,
            data,
        }
    }
}

/// Serializes a clipboard payload into formatted JSON.
pub fn serialize_clipboard(data: FerriteClipboardData) -> Result<String, serde_json::Error> {
    let payload = FerriteClipboardPayload::new(data);
    serde_json::to_string_pretty(&payload)
}

/// Attempts to deserialize a JSON string into a valid `FerriteClipboardData`.
/// Returns `None` if the string is empty, invalid JSON, or has an unknown schema version.
pub fn deserialize_clipboard(raw: &str) -> Option<FerriteClipboardData> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match serde_json::from_str::<FerriteClipboardPayload>(trimmed) {
        Ok(payload) if payload.schema_version == CLIPBOARD_SCHEMA_V1 => Some(payload.data),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_lang_fbd::FbdBlockKind;
    use ferrite_lang_sfc::{ConvergenceKind, DivergenceKind, StepKind};
    use std::collections::HashMap;

    #[test]
    fn test_clipboard_fbd_blocks_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let block = FbdBlock {
            id: "TON_1".to_string(),
            kind: FbdBlockKind::And,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        };
        let data = FerriteClipboardData::Fbd {
            blocks: vec![block],
        };

        let json = serialize_clipboard(data.clone())?;
        assert!(json.contains("TON_1"));
        assert!(json.contains("\"schema_version\": 1"));

        let deserialized = deserialize_clipboard(&json).ok_or("Deserialization failed")?;
        assert_eq!(deserialized, data);
        Ok(())
    }

    #[test]
    fn test_clipboard_sfc_elements_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let step = SfcStep {
            id: "Step_1".to_string(),
            name: "Initial step".to_string(),
            kind: StepKind::Initial,
            actions: Vec::new(),
            grid_pos: Some((10, 20)),
        };
        let trans = SfcTransition {
            id: "Trans_1".to_string(),
            receptivity: "TRUE".to_string(),
            from_steps: vec!["Step_1".to_string()],
            to_steps: vec!["Step_2".to_string()],
            convergence: ConvergenceKind::default(),
            divergence: DivergenceKind::default(),
            grid_pos: Some((10, 24)),
        };
        let data = FerriteClipboardData::Sfc {
            steps: vec![step],
            transitions: vec![trans],
        };

        let json = serialize_clipboard(data.clone())?;
        let deserialized = deserialize_clipboard(&json).ok_or("Deserialization failed")?;
        assert_eq!(deserialized, data);
        Ok(())
    }

    #[test]
    fn test_clipboard_ladder_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let rung = LdRung {
            id: "Rung_1".to_string(),
            comment: "Safety interlock".to_string(),
            elements: Vec::new(),
            coils: Vec::new(),
        };
        let data = FerriteClipboardData::Ladder { rungs: vec![rung] };

        let json = serialize_clipboard(data.clone())?;
        let deserialized = deserialize_clipboard(&json).ok_or("Deserialization failed")?;
        assert_eq!(deserialized, data);
        Ok(())
    }

    #[test]
    fn test_clipboard_invalid_json_returns_none() {
        assert_eq!(deserialize_clipboard(""), None);
        assert_eq!(deserialize_clipboard("   "), None);
        assert_eq!(deserialize_clipboard("hello world"), None);
        assert_eq!(deserialize_clipboard(r#"{"random": "json"}"#), None);
        assert_eq!(
            deserialize_clipboard(r#"{"schema_version": 999, "data": {}}"#),
            None
        );
    }
}
