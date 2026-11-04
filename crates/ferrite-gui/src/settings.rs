use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::i18n::UiLanguage;

static SETTINGS_PATH_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Overrides the settings file path (e.g. for testing without mutating disk environment or unsafe blocks).
pub fn set_custom_settings_path(path: Option<PathBuf>) {
    if let Ok(mut lock) = SETTINGS_PATH_OVERRIDE.write() {
        *lock = path;
    }
}

/// User preferences and application settings persisted across sessions.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(default)]
    pub language: Option<UiLanguage>,
}

/// Resolves the user settings file path across Windows, macOS, and Linux.
pub fn get_user_settings_path() -> PathBuf {
    if let Ok(guard) = SETTINGS_PATH_OVERRIDE.read()
        && let Some(ref path) = *guard
    {
        return path.clone();
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let mut p = PathBuf::from(app_data);
            p.push("FerritePLC");
            p.push("settings.json");
            return p;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let mut p = PathBuf::from(xdg);
            p.push("ferrite_plc");
            p.push("settings.json");
            return p;
        }
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    let mut p = PathBuf::from(home);
    p.push(".ferrite_plc");
    p.push("settings.json");
    p
}

impl UserSettings {
    /// Loads settings from the default platform settings path, or returns default if not found.
    pub fn load() -> Self {
        let path = get_user_settings_path();
        Self::load_from_path(&path).unwrap_or_default()
    }

    /// Loads settings from a specific file path.
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read '{path:?}': {e}"))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("Failed to parse JSON '{path:?}': {e}"))
    }

    /// Saves settings to the default platform settings path atomically.
    pub fn save(&self) -> Result<(), String> {
        let path = get_user_settings_path();
        self.save_to_path(&path)
    }

    /// Atomically saves settings to a specific file path using a temporary file.
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory '{parent:?}': {e}"))?;
        }
        let json_str = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, json_str)
            .map_err(|e| format!("Failed to write '{tmp_path:?}': {e}"))?;
        fs::rename(&tmp_path, path)
            .map_err(|e| format!("Failed to rename '{tmp_path:?}' to '{path:?}': {e}"))?;
        Ok(())
    }
}

/// Reads the user's saved UI language, if configured.
pub fn load_saved_language() -> Option<UiLanguage> {
    UserSettings::load().language
}

/// Returns the saved UI language if present, or falls back to system OS language detection.
pub fn load_saved_language_or_os() -> UiLanguage {
    load_saved_language().unwrap_or_else(crate::i18n::detect_os_language)
}

/// Persists the selected UI language to the user's settings.
pub fn save_language_preference(lang: UiLanguage) -> Result<(), String> {
    let mut settings = UserSettings::load();
    settings.language = Some(lang);
    settings.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_settings_save_and_load_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_settings_test_{}", std::process::id()));
        let settings_path = temp_dir.join("settings.json");

        // Non-existent path returns default
        let loaded = UserSettings::load_from_path(&settings_path)?;
        assert_eq!(loaded, UserSettings::default());
        assert_eq!(loaded.language, None);

        // Save French preference
        let settings = UserSettings {
            language: Some(UiLanguage::Fr),
        };
        settings.save_to_path(&settings_path)?;

        // Reload
        let reloaded = UserSettings::load_from_path(&settings_path)?;
        assert_eq!(reloaded.language, Some(UiLanguage::Fr));

        // Save German preference
        let updated = UserSettings {
            language: Some(UiLanguage::De),
        };
        updated.save_to_path(&settings_path)?;

        let reloaded2 = UserSettings::load_from_path(&settings_path)?;
        assert_eq!(reloaded2.language, Some(UiLanguage::De));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
