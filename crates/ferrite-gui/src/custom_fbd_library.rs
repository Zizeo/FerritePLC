use std::fs;
use std::path::{Path, PathBuf};

use ferrite_lang_fbd::CompositeFbDef;
use serde::{Deserialize, Serialize};

/// Global library file format version.
pub const GLOBAL_CUSTOM_FBD_VERSION: u32 = 1;

/// Persisted container for global custom FBD blocks defined by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalCustomFbdLibrary {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub blocks: Vec<CompositeFbDef>,
}

fn default_version() -> u32 {
    GLOBAL_CUSTOM_FBD_VERSION
}

impl Default for GlobalCustomFbdLibrary {
    fn default() -> Self {
        Self {
            version: GLOBAL_CUSTOM_FBD_VERSION,
            blocks: Vec::new(),
        }
    }
}

/// Resolves the global custom FBD library path across Windows, macOS, and Linux.
pub fn get_global_library_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let mut p = PathBuf::from(app_data);
            p.push("FerritePLC");
            p.push("custom_fbd_library.json");
            return p;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let mut p = PathBuf::from(xdg);
            p.push("ferrite_plc");
            p.push("custom_fbd_library.json");
            return p;
        }
    }

    // Fallback using HOME / USERPROFILE
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let mut p = PathBuf::from(home);
    p.push(".ferrite_plc");
    p.push("custom_fbd_library.json");
    p
}

/// Loads the global custom FBD library from a specific path.
pub fn load_from_path(path: &Path) -> Result<GlobalCustomFbdLibrary, String> {
    if !path.exists() {
        return Ok(GlobalCustomFbdLibrary::default());
    }
    let data = fs::read_to_string(path).map_err(|e| format!("Failed to read '{path:?}': {e}"))?;
    serde_json::from_str(&data).map_err(|e| format!("Failed to parse JSON '{path:?}': {e}"))
}

/// Atomically saves the global custom FBD library to a specific path using a temporary file.
pub fn save_to_path(lib: &GlobalCustomFbdLibrary, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory '{parent:?}': {e}"))?;
    }
    let json_str = serde_json::to_string_pretty(lib)
        .map_err(|e| format!("Failed to serialize library: {e}"))?;
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, json_str).map_err(|e| format!("Failed to write '{tmp_path:?}': {e}"))?;
    fs::rename(&tmp_path, path).map_err(|e| format!("Failed to rename '{tmp_path:?}' to '{path:?}': {e}"))?;
    Ok(())
}

/// Loads the global custom FBD library from the default system location.
/// Returns an empty library if the file does not exist or fails to load.
pub fn load_global_library() -> GlobalCustomFbdLibrary {
    let path = get_global_library_path();
    load_from_path(&path).unwrap_or_default()
}

/// Saves the global custom FBD library to the default system location.
pub fn save_global_library(lib: &GlobalCustomFbdLibrary) -> Result<(), String> {
    let path = get_global_library_path();
    save_to_path(lib, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_library_load_nonexistent_file() {
        let temp_dir = std::env::temp_dir().join("ferrite_test_nonexistent");
        let path = temp_dir.join("missing_library.json");
        let lib = load_from_path(&path);
        assert!(lib.is_ok());
        let val = lib.unwrap_or_default();
        assert_eq!(val.blocks.len(), 0);
    }

    #[test]
    fn test_global_library_save_and_load_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ferrite_lib_test_{unique_id}"));
        let path = temp_dir.join("custom_fbd_library.json");

        let mut lib = GlobalCustomFbdLibrary::default();
        lib.blocks.push(CompositeFbDef {
            name: "HighLimitAlarm".to_string(),
            description: "High temperature threshold detector".to_string(),
            input_pins: vec!["IN".to_string(), "LIMIT".to_string()],
            output_pins: vec!["ALARM".to_string()],
            st_code: Some("ALARM := IN > LIMIT;".to_string()),
            blocks: vec![],
        });

        save_to_path(&lib, &path).map_err(|e| format!("Save failed: {e}"))?;
        let loaded = load_from_path(&path).map_err(|e| format!("Load failed: {e}"))?;

        assert_eq!(loaded.blocks.len(), 1);
        assert_eq!(loaded.blocks[0].name, "HighLimitAlarm");
        assert_eq!(loaded.blocks[0].description, "High temperature threshold detector");
        assert_eq!(loaded.blocks[0].input_pins, vec!["IN", "LIMIT"]);
        assert_eq!(loaded.blocks[0].output_pins, vec!["ALARM"]);
        assert_eq!(loaded.blocks[0].st_code.as_deref(), Some("ALARM := IN > LIMIT;"));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
