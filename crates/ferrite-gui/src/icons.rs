//! Icon helpers supporting both Nerd Fonts (e.g. FiraCode Nerd Font)
//! and standard Unicode / Emoji fallback symbols with advanced shaping.

/// Returns the lock icon (Nerd Font nf-fa-lock or Unicode padlock emoji).
pub fn icon_lock() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f023}" // nf-fa-lock
    } else {
        "🔒"
    }
}

/// Returns the documentation/book icon.
pub fn icon_book() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f02d}" // nf-fa-book
    } else {
        "📖"
    }
}

/// Returns the warning icon.
pub fn icon_warning() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f071}" // nf-fa-warning
    } else {
        "⚠"
    }
}

/// Returns the close/times icon.
pub fn icon_close() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f00d}" // nf-fa-times
    } else {
        "✕"
    }
}

/// Returns the settings/gear icon.
pub fn icon_gear() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f013}" // nf-fa-cog
    } else {
        "⚙"
    }
}

/// Returns the edit/pencil icon.
pub fn icon_pencil() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f040}" // nf-fa-pencil
    } else {
        "✏"
    }
}

/// Returns the checkmark icon.
pub fn icon_check() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f00c}" // nf-fa-check
    } else {
        "✓"
    }
}
