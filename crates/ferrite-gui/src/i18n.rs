use serde::{Deserialize, Serialize};

/// Supported UI display languages in FerritePLC Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UiLanguage {
    #[default]
    En,
    Fr,
}

impl UiLanguage {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::Fr => "FR",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Fr => "Français",
        }
    }

    /// Selects between an English string and a French string based on `self`.
    pub fn tr<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Self::En => en,
            Self::Fr => fr,
        }
    }
}
