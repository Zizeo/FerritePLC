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
        "×"
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

/// Returns the save/floppy icon.
pub fn icon_save() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f0c7}" // nf-fa-floppy_o
    } else {
        "💾"
    }
}

/// Returns the information icon.
pub fn icon_info() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f05a}" // nf-fa-info_circle
    } else {
        "i"
    }
}

/// Compact circular "i" badge that reveals `tip` in a tooltip on hover.
///
/// Used by the POU editors in place of the old always-visible tips bar.
pub fn info_tip<'a, Message: 'a>(tip: &'a str) -> iced::Element<'a, Message> {
    use iced::widget::{container, text};
    use iced::{Background, Border, Length, Theme};

    let badge = container(
        text(icon_info())
            .size(10)
            .shaping(iced::widget::text::Shaping::Advanced)
            .color(crate::theme::StudioTheme::TEXT_SECONDARY),
    )
    .width(Length::Fixed(16.0))
    .height(Length::Fixed(16.0))
    .center_x(Length::Fixed(16.0))
    .center_y(Length::Fixed(16.0))
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(crate::theme::StudioTheme::bg_elevated(
            theme,
        ))),
        border: Border {
            color: crate::theme::StudioTheme::border_card(theme),
            width: 1.0,
            radius: iced::border::Radius::from(8.0),
        },
        ..Default::default()
    });

    let bubble = container(text(tip).size(11))
        .padding([6, 9])
        .max_width(420.0)
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(crate::theme::StudioTheme::bg_elevated(
                theme,
            ))),
            border: Border {
                color: crate::theme::StudioTheme::border_card(theme),
                width: 1.0,
                radius: iced::border::Radius::from(8.0),
            },
            ..Default::default()
        });

    iced::widget::tooltip(badge, bubble, iced::widget::tooltip::Position::Bottom)
        .gap(6)
        .into()
}

/// Returns the left-pointing chevron (collapse panel to the left).
pub fn icon_chevron_left() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f053}" // nf-fa-chevron_left
    } else {
        "◀"
    }
}

/// Returns the right-pointing chevron (expand panel to the right).
pub fn icon_chevron_right() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f054}" // nf-fa-chevron_right
    } else {
        "▶"
    }
}

/// Returns the down-pointing chevron (collapse panel downward).
pub fn icon_chevron_down() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f078}" // nf-fa-chevron_down
    } else {
        "▼"
    }
}

/// Returns the up-pointing chevron (expand panel upward).
pub fn icon_chevron_up() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f077}" // nf-fa-chevron_up
    } else {
        "▲"
    }
}

/// Returns the drag grip handle icon (nf-fa-bars / unicode ⠿ or ≡).
pub fn icon_grip() -> &'static str {
    if crate::is_nerd_font_available() {
        "\u{f0c9}" // nf-fa-bars
    } else {
        "⠿"
    }
}
