use iced::border::Radius;
use iced::widget::{button, container, pick_list, text_input, toggler};
use iced::{Background, Border, Color, Theme};

/// Industrial obsidian & technical lab design tokens and style generators for FerritePLC Studio.
#[derive(Debug, Clone, Copy)]
pub struct StudioTheme;

impl StudioTheme {
    // Dark Theme Background Tokens (Default Industrial Obsidian)
    pub const BG_BASE: Color = Color::from_rgb(0.043, 0.051, 0.067); // #0B0D11 Deep graphite
    pub const BG_PANEL: Color = Color::from_rgb(0.071, 0.086, 0.114); // #12161D Sidebar / Dock
    pub const BG_ELEVATED: Color = Color::from_rgb(0.102, 0.122, 0.161); // #1A1F29 Card
    pub const BG_SURFACE: Color = Color::from_rgb(0.141, 0.169, 0.224); // #242B39 Interactive surface
    pub const BG_HOVER: Color = Color::from_rgb(0.180, 0.216, 0.282); // #2E3748 Hover
    pub const BG_ACTIVE: Color = Color::from_rgb(0.055, 0.455, 0.565); // #0E7490 Active cyan-blue

    // Borders & Strokes
    pub const BORDER_HAIRLINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.07);
    pub const BORDER_CARD: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.10);
    pub const BORDER_ACTIVE: Color = Color::from_rgb(0.220, 0.741, 0.973); // #38BDF8 Sky cyan

    // Accent Colors
    pub const ACCENT_INDIGO: Color = Color::from_rgb(0.388, 0.400, 0.945); // #6366F1 Linear blue
    pub const ACCENT_CYAN: Color = Color::from_rgb(0.220, 0.741, 0.973); // #38BDF8 Sky cyan
    pub const ACCENT_EMERALD: Color = Color::from_rgb(0.063, 0.725, 0.506); // #10B981 Run / Normal
    pub const ACCENT_AMBER: Color = Color::from_rgb(0.961, 0.620, 0.043); // #F59E0B Warning / Paused
    pub const ACCENT_ROSE: Color = Color::from_rgb(0.957, 0.247, 0.369); // #F43F5E Alarm / Error
    pub const ACCENT_PURPLE: Color = Color::from_rgb(0.659, 0.333, 0.969); // #A855F7 Structured Text

    // Dark Theme Typography Tokens
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.953, 0.957, 0.965); // #F3F4F6
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.612, 0.639, 0.686); // #9CA3AF
    pub const TEXT_MUTED: Color = Color::from_rgb(0.420, 0.447, 0.502); // #6B7280

    // Theme Mode Inspection
    pub fn is_dark(theme: &Theme) -> bool {
        !matches!(theme, Theme::Light)
    }

    // Dynamic Theme-Aware Color Accessors
    pub fn bg_base(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BG_BASE
        } else {
            Color::from_rgb(0.941, 0.949, 0.961) // #F0F2F5 Clean slate canvas
        }
    }

    pub fn bg_panel(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BG_PANEL
        } else {
            Color::from_rgb(0.894, 0.906, 0.929) // #E4E7ED Header & Dock framing
        }
    }

    pub fn bg_elevated(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BG_ELEVATED
        } else {
            Color::WHITE // Pure white cards & dialogs
        }
    }

    pub fn bg_surface(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BG_SURFACE
        } else {
            Color::from_rgb(0.863, 0.882, 0.910) // #DCE1E8 Interactive surface
        }
    }

    pub fn bg_hover(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BG_HOVER
        } else {
            Color::from_rgb(0.816, 0.835, 0.867) // #D0D5DD Hover
        }
    }

    pub fn border_hairline(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BORDER_HAIRLINE
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.10)
        }
    }

    pub fn border_card(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BORDER_CARD
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.15)
        }
    }

    pub fn border_active(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::BORDER_ACTIVE
        } else {
            Color::from_rgb(0.008, 0.518, 0.780) // #0284C7 Deep Sky
        }
    }

    pub fn text_primary(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::TEXT_PRIMARY
        } else {
            Color::from_rgb(0.059, 0.090, 0.165) // #0F172A Deep ink black
        }
    }

    pub fn text_secondary(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::TEXT_SECONDARY
        } else {
            Color::from_rgb(0.200, 0.255, 0.333) // #334155 Slate
        }
    }

    pub fn text_muted(theme: &Theme) -> Color {
        if Self::is_dark(theme) {
            Self::TEXT_MUTED
        } else {
            Color::from_rgb(0.392, 0.455, 0.545) // #64748B Muted
        }
    }

    // Container Styles
    pub fn header_container_style(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::bg_panel(theme))),
            border: Border {
                color: Self::border_hairline(theme),
                width: 1.0,
                radius: Radius::default(),
            },
            ..Default::default()
        }
    }

    pub fn sidebar_container_style(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::bg_panel(theme))),
            border: Border {
                color: Self::border_hairline(theme),
                width: 1.0,
                radius: Radius::default(),
            },
            ..Default::default()
        }
    }

    pub fn card_style(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::bg_elevated(theme))),
            border: Border {
                color: Self::border_card(theme),
                width: 1.0,
                radius: Radius::from(10.0),
            },
            ..Default::default()
        }
    }

    pub fn canvas_container_style(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::bg_base(theme))),
            border: Border {
                color: Self::border_card(theme),
                width: 1.0,
                radius: Radius::from(8.0),
            },
            ..Default::default()
        }
    }

    pub fn modal_card_style(theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::bg_elevated(theme))),
            border: Border {
                color: Self::ACCENT_INDIGO,
                width: 1.5,
                radius: Radius::from(12.0),
            },
            ..Default::default()
        }
    }

    pub fn subtle_badge_style(bg: Color) -> impl Fn(&Theme) -> container::Style {
        move |_theme: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius::from(9999.0),
            },
            ..Default::default()
        }
    }

    pub fn gutter_style(_theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Color::from_rgba(0.04, 0.05, 0.08, 0.70))),
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                width: 1.0,
                radius: Radius::default(),
            },
            ..Default::default()
        }
    }

    // Button Styles
    pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.310, 0.275, 0.898),
            button::Status::Pressed => Color::from_rgb(0.263, 0.220, 0.792),
            button::Status::Disabled => Color::from_rgba(0.388, 0.400, 0.945, 0.3),
            button::Status::Active => Self::ACCENT_INDIGO,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn secondary_button(theme: &Theme, status: button::Status) -> button::Style {
        let is_dark = Self::is_dark(theme);
        let bg = match status {
            button::Status::Hovered => Self::bg_hover(theme),
            button::Status::Pressed => {
                if is_dark {
                    Self::BG_PANEL
                } else {
                    Color::from_rgb(0.85, 0.87, 0.90)
                }
            }
            button::Status::Disabled => {
                if is_dark {
                    Color::from_rgba(0.141, 0.169, 0.224, 0.3)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.04)
                }
            }
            button::Status::Active => {
                if is_dark {
                    Self::BG_SURFACE
                } else {
                    Color::WHITE
                }
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Self::text_primary(theme),
            border: Border {
                color: Self::border_card(theme),
                width: 1.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        }
    }

    pub fn ghost_button(theme: &Theme, status: button::Status) -> button::Style {
        let is_dark = Self::is_dark(theme);
        let bg = match status {
            button::Status::Hovered => {
                if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.06)
                }
            }
            button::Status::Pressed => {
                if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.04)
                }
            }
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Self::text_secondary(theme),
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn danger_button(_theme: &Theme, status: button::Status) -> button::Style {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.882, 0.114, 0.282),
            button::Status::Pressed => Color::from_rgb(0.745, 0.071, 0.235),
            button::Status::Disabled => Color::from_rgba(0.957, 0.247, 0.369, 0.3),
            button::Status::Active => Self::ACCENT_ROSE,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn success_button(_theme: &Theme, status: button::Status) -> button::Style {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.020, 0.588, 0.412),
            button::Status::Pressed => Color::from_rgb(0.016, 0.471, 0.341),
            button::Status::Disabled => Color::from_rgba(0.063, 0.725, 0.506, 0.3),
            button::Status::Active => Self::ACCENT_EMERALD,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn tab_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |theme: &Theme, status: button::Status| {
            let is_dark = Self::is_dark(theme);
            let (bg, border_col, txt_col) = if is_active {
                (
                    Self::bg_surface(theme),
                    if is_dark {
                        Self::ACCENT_CYAN
                    } else {
                        Color::from_rgb(0.008, 0.518, 0.780)
                    },
                    Self::text_primary(theme),
                )
            } else {
                match status {
                    button::Status::Hovered => {
                        (Self::bg_hover(theme), Color::TRANSPARENT, Self::text_primary(theme))
                    }
                    _ => (Color::TRANSPARENT, Color::TRANSPARENT, Self::text_secondary(theme)),
                }
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: txt_col,
                border: Border {
                    color: border_col,
                    width: if is_active { 1.5 } else { 0.0 },
                    radius: Radius::from(6.0),
                },
                ..Default::default()
            }
        }
    }

    // Input Styles
    pub fn styled_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
        let is_dark = Self::is_dark(theme);
        let border_color = match status {
            text_input::Status::Focused => Self::ACCENT_INDIGO,
            text_input::Status::Hovered => Self::ACCENT_CYAN,
            text_input::Status::Disabled => {
                if is_dark {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                } else {
                    Color::from_rgba(0.0, 0.0, 0.0, 0.06)
                }
            }
            text_input::Status::Active => Self::border_card(theme),
        };
        let bg = if is_dark {
            Self::BG_BASE
        } else {
            Color::WHITE
        };
        text_input::Style {
            background: Background::Color(bg),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(6.0),
            },
            icon: Self::text_secondary(theme),
            placeholder: Self::text_muted(theme),
            value: Self::text_primary(theme),
            selection: Color::from_rgba(0.388, 0.400, 0.945, 0.3),
        }
    }

    // Pick List Styles
    pub fn styled_pick_list(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
        let is_dark = Self::is_dark(theme);
        let border_color = match status {
            pick_list::Status::Hovered | pick_list::Status::Opened => Self::ACCENT_CYAN,
            _ => Self::border_card(theme),
        };
        let bg = if is_dark {
            Self::BG_BASE
        } else {
            Color::WHITE
        };
        pick_list::Style {
            text_color: Self::text_primary(theme),
            placeholder_color: Self::text_muted(theme),
            handle_color: Self::text_secondary(theme),
            background: Background::Color(bg),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(6.0),
            },
        }
    }

    // Toggler Styles
    pub fn styled_toggler(theme: &Theme, status: toggler::Status) -> toggler::Style {
        let (bg, fg) = match status {
            toggler::Status::Active { is_toggled: true }
            | toggler::Status::Hovered { is_toggled: true } => (Self::ACCENT_EMERALD, Color::WHITE),
            toggler::Status::Active { is_toggled: false }
            | toggler::Status::Hovered { is_toggled: false } => {
                (Self::bg_surface(theme), Self::text_muted(theme))
            }
            toggler::Status::Disabled => {
                (
                    if Self::is_dark(theme) {
                        Color::from_rgba(0.141, 0.169, 0.224, 0.3)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.06)
                    },
                    Self::text_muted(theme),
                )
            }
        };
        toggler::Style {
            background: bg,
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: fg,
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
        }
    }
}
