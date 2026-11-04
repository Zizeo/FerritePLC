use serde::{Deserialize, Serialize};

/// Supported UI display languages in FerritePLC Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UiLanguage {
    #[default]
    En,
    Fr,
    De,
    Es,
    It,
}

impl UiLanguage {
    pub const ALL: [UiLanguage; 5] = [
        UiLanguage::En,
        UiLanguage::Fr,
        UiLanguage::De,
        UiLanguage::Es,
        UiLanguage::It,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::Fr => "FR",
            Self::De => "DE",
            Self::Es => "ES",
            Self::It => "IT",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Fr => "Français",
            Self::De => "Deutsch",
            Self::Es => "Español",
            Self::It => "Italiano",
        }
    }

    pub fn locale_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Es => "es",
            Self::It => "it",
        }
    }

    /// Selects between an English string and a French string (legacy helper).
    pub fn tr<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Self::Fr => fr,
            _ => en,
        }
    }
}

impl std::fmt::Display for UiLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Detects the system OS language on startup via `sys_locale`.
pub fn detect_os_language() -> UiLanguage {
    if let Some(locale) = sys_locale::get_locale() {
        let lower = locale.to_lowercase();
        if lower.starts_with("fr") {
            return UiLanguage::Fr;
        } else if lower.starts_with("de") {
            return UiLanguage::De;
        } else if lower.starts_with("es") {
            return UiLanguage::Es;
        } else if lower.starts_with("it") {
            return UiLanguage::It;
        }
    }
    UiLanguage::En
}

/// Translates a key for a specific `UiLanguage`.
pub fn translate(key: &str, lang: UiLanguage) -> String {
    match lang {
        UiLanguage::En => rust_i18n::t!(key, locale = "en").to_string(),
        UiLanguage::Fr => rust_i18n::t!(key, locale = "fr").to_string(),
        UiLanguage::De => rust_i18n::t!(key, locale = "de").to_string(),
        UiLanguage::Es => rust_i18n::t!(key, locale = "es").to_string(),
        UiLanguage::It => rust_i18n::t!(key, locale = "it").to_string(),
    }
}

/// Macro for ergonomic translation retrieval with optional variable interpolation.
#[macro_export]
macro_rules! tr {
    ($key:expr, $lang:expr) => {
        $crate::i18n::translate($key, $lang)
    };
    ($key:expr, $lang:expr, $($name:ident = $val:expr),* $(,)?) => {
        match $lang {
            $crate::i18n::UiLanguage::En => {
                rust_i18n::t!($key, locale = "en", $($name = $val),*).to_string()
            }
            $crate::i18n::UiLanguage::Fr => {
                rust_i18n::t!($key, locale = "fr", $($name = $val),*).to_string()
            }
            $crate::i18n::UiLanguage::De => {
                rust_i18n::t!($key, locale = "de", $($name = $val),*).to_string()
            }
            $crate::i18n::UiLanguage::Es => {
                rust_i18n::t!($key, locale = "es", $($name = $val),*).to_string()
            }
            $crate::i18n::UiLanguage::It => {
                rust_i18n::t!($key, locale = "it", $($name = $val),*).to_string()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_os_language() {
        let lang = detect_os_language();
        assert!(UiLanguage::ALL.contains(&lang));
    }

    #[test]
    fn test_translation_all_languages_and_interpolation() {
        let save_en = translate("menu.file.save_project", UiLanguage::En);
        assert_eq!(save_en, "Save Project");

        let save_fr = translate("menu.file.save_project", UiLanguage::Fr);
        assert_eq!(save_fr, "Enregistrer Projet");

        let save_de = translate("menu.file.save_project", UiLanguage::De);
        assert_eq!(save_de, "Projekt speichern");

        let save_es = translate("menu.file.save_project", UiLanguage::Es);
        assert_eq!(save_es, "Guardar proyecto");

        let save_it = translate("menu.file.save_project", UiLanguage::It);
        assert_eq!(save_it, "Salva progetto");

        // Test interpolation macro across languages
        let tele_en = tr!("app.telemetry", UiLanguage::En, scan = 42, cycle = 125, time = 1000);
        assert!(tele_en.contains("Scan: 42"));

        let tele_de = tr!("app.telemetry", UiLanguage::De, scan = 42, cycle = 125, time = 1000);
        assert!(tele_de.contains("Scan: 42"));
        assert!(tele_de.contains("Zyklus: 125µs"));
        assert!(tele_de.contains("Virtuell: 1000ms"));

        let tele_es = tr!("app.telemetry", UiLanguage::Es, scan = 42, cycle = 125, time = 1000);
        assert!(tele_es.contains("Scan: 42"));
        assert!(tele_es.contains("Ciclo: 125µs"));
        assert!(tele_es.contains("Virtual: 1000ms"));

        let tele_it = tr!("app.telemetry", UiLanguage::It, scan = 42, cycle = 125, time = 1000);
        assert!(tele_it.contains("Scan: 42"));
        assert!(tele_it.contains("Ciclo: 125µs"));
        assert!(tele_it.contains("Virtuale: 1000ms"));
    }
}
