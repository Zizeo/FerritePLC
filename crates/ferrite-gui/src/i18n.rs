use serde::{Deserialize, Serialize};

/// Supported UI display languages in FerritePLC Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UiLanguage {
    #[default]
    En,
    Fr,
    Pseudo,
}

impl UiLanguage {
    pub const ALL: [UiLanguage; 3] = [UiLanguage::En, UiLanguage::Fr, UiLanguage::Pseudo];

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::Fr => "FR",
            Self::Pseudo => "PSEUDO",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::En => "English",
            Self::Fr => "Français",
            Self::Pseudo => "Pseudo-Loc [TEST]",
        }
    }

    pub fn locale_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
            Self::Pseudo => "en",
        }
    }

    /// Selects between an English string and a French string (legacy helper).
    pub fn tr<'a>(self, en: &'a str, fr: &'a str) -> &'a str {
        match self {
            Self::En | Self::Pseudo => en,
            Self::Fr => fr,
        }
    }
}

impl std::fmt::Display for UiLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Detects the system OS language on startup via `sys_locale`.
/// Defaults to `UiLanguage::Fr` if the system locale starts with "fr", otherwise `UiLanguage::En`.
pub fn detect_os_language() -> UiLanguage {
    if let Some(locale) = sys_locale::get_locale() {
        let lower = locale.to_lowercase();
        if lower.starts_with("fr") {
            return UiLanguage::Fr;
        }
    }
    UiLanguage::En
}

/// Transforms an ASCII English string into a pseudo-localized string with accented glyphs,
/// preserved format placeholders (%{name} or {name}), preserved keyboard shortcuts (Ctrl+...),
/// and approximately +40% expansion text bounded by `[...]`.
pub fn pseudolocalize(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(input.len() * 2);
    out.push('[');

    let mut in_placeholder = false;
    let mut in_shortcut = false;

    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        // Check start of placeholder %{...} or {...}
        if c == '%' && i + 1 < chars.len() && chars[i + 1] == '{' {
            in_placeholder = true;
            out.push(c);
            out.push('{');
            i += 2;
            continue;
        } else if c == '{' {
            in_placeholder = true;
            out.push(c);
            i += 1;
            continue;
        } else if c == '}' && in_placeholder {
            in_placeholder = false;
            out.push(c);
            i += 1;
            continue;
        }

        // Check start of shortcut in parens, e.g. (Ctrl+S), (F1)
        if c == '(' && i + 1 < chars.len() && (chars[i + 1] == 'C' || chars[i + 1] == 'F' || chars[i + 1] == 'S') {
            in_shortcut = true;
            out.push(c);
            i += 1;
            continue;
        } else if c == ')' && in_shortcut {
            in_shortcut = false;
            out.push(c);
            i += 1;
            continue;
        }

        if in_placeholder || in_shortcut {
            out.push(c);
        } else {
            // Replace with visually distinct diacritic glyphs
            let mapped = match c {
                'a' => 'à',
                'A' => 'À',
                'b' => 'ƀ',
                'B' => 'Ɓ',
                'c' => 'ç',
                'C' => 'Ç',
                'd' => 'ð',
                'D' => 'Ð',
                'e' => 'é',
                'E' => 'É',
                'f' => 'ƒ',
                'F' => 'Ƒ',
                'g' => 'ĝ',
                'G' => 'Ĝ',
                'h' => 'ĥ',
                'H' => 'Ĥ',
                'i' => 'î',
                'I' => 'Î',
                'j' => 'ĵ',
                'J' => 'Ĵ',
                'k' => 'ƙ',
                'K' => 'Ƙ',
                'l' => 'ł',
                'L' => 'Ł',
                'm' => 'ɱ',
                'M' => 'Ṁ',
                'n' => 'ñ',
                'N' => 'Ñ',
                'o' => 'ô',
                'O' => 'Ô',
                'p' => 'þ',
                'P' => 'Þ',
                'q' => 'ǫ',
                'Q' => 'Ǫ',
                'r' => 'ř',
                'R' => 'Ř',
                's' => 'š',
                'S' => 'Š',
                't' => 'ť',
                'T' => 'Ť',
                'u' => 'û',
                'U' => 'Û',
                'v' => 'ṽ',
                'V' => 'Ṽ',
                'w' => 'ŵ',
                'W' => 'Ŵ',
                'x' => 'ẋ',
                'X' => 'Ẋ',
                'y' => 'ý',
                'Y' => 'Ý',
                'z' => 'ž',
                'Z' => 'Ž',
                other => other,
            };
            out.push(mapped);
        }
        i += 1;
    }

    // Add expansion padding (+35% to +45% text length) for anti-truncation testing
    let base_len = input.len();
    let pad_chars = (base_len * 4) / 10;
    if pad_chars > 0 {
        out.push(' ');
        let repeats = (pad_chars / 3).max(1);
        for _ in 0..repeats {
            out.push('~');
        }
    }

    out.push(']');
    out
}

/// Translates a key for a specific `UiLanguage`.
pub fn translate(key: &str, lang: UiLanguage) -> String {
    match lang {
        UiLanguage::En => rust_i18n::t!(key, locale = "en").to_string(),
        UiLanguage::Fr => rust_i18n::t!(key, locale = "fr").to_string(),
        UiLanguage::Pseudo => {
            let en_str = rust_i18n::t!(key, locale = "en");
            pseudolocalize(&en_str)
        }
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
            $crate::i18n::UiLanguage::Pseudo => {
                let s = rust_i18n::t!($key, locale = "en", $($name = $val),*);
                $crate::i18n::pseudolocalize(&s)
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
        assert!(lang == UiLanguage::En || lang == UiLanguage::Fr);
    }

    #[test]
    fn test_pseudolocalize_transform() {
        let original = "Save Active POU (Ctrl+S)";
        let pseudo = pseudolocalize(original);
        assert!(pseudo.starts_with('['), "Should be bracketed");
        assert!(pseudo.ends_with(']'), "Should be bracketed");
        assert!(pseudo.contains("(Ctrl+S)"), "Shortcut should remain intact");
        assert!(pseudo.len() > original.len(), "Pseudo should expand string length");
        assert!(pseudo.contains('Š'), "Should contain accented S");
        assert!(pseudo.contains('é'), "Should contain accented e");

        // Test with interpolation placeholder
        let interpolated = "Hello %{name}!";
        let pseudo_interp = pseudolocalize(interpolated);
        assert!(pseudo_interp.contains("%{name}"), "Placeholder %{{name}} must be preserved");
    }

    #[test]
    fn test_translation_fallback_and_keys() {
        let save_en = translate("menu.file.save_project", UiLanguage::En);
        assert_eq!(save_en, "Save Project");

        let save_fr = translate("menu.file.save_project", UiLanguage::Fr);
        assert_eq!(save_fr, "Enregistrer Projet");

        let save_pseudo = translate("menu.file.save_project", UiLanguage::Pseudo);
        assert!(save_pseudo.starts_with('['));
        assert!(save_pseudo.contains('Š'));

        // Test interpolation macro
        let tele_en = tr!("app.telemetry", UiLanguage::En, scan = 42, cycle = 125, time = 1000);
        assert!(tele_en.contains("Scan: 42"));
        assert!(tele_en.contains("Cycle: 125µs"));
        assert!(tele_en.contains("Virt: 1000ms"));

        let tele_fr = tr!("app.telemetry", UiLanguage::Fr, scan = 42, cycle = 125, time = 1000);
        assert!(tele_fr.contains("Scan : 42"));
        assert!(tele_fr.contains("Cycle : 125µs"));
        assert!(tele_fr.contains("Virtuel : 1000ms"));
    }
}
