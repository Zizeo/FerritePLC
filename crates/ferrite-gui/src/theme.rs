use iced::border::Radius;
use iced::widget::{button, container, pick_list, text_input, toggler};
use iced::{Background, Border, Color, Theme};

/// Industrial obsidian design tokens and style generators for FerritePLC Studio.
#[derive(Debug, Clone, Copy)]
pub struct StudioTheme;

impl StudioTheme {
    // Backgrounds
    pub const BG_BASE: Color = Color::from_rgb(0.043, 0.051, 0.067);      // #0B0D11 Deep graphite
    pub const BG_PANEL: Color = Color::from_rgb(0.071, 0.086, 0.114);    // #12161D Sidebar / Dock
    pub const BG_ELEVATED: Color = Color::from_rgb(0.102, 0.122, 0.161); // #1A1F29 Card
    pub const BG_SURFACE: Color = Color::from_rgb(0.141, 0.169, 0.224);  // #242B39 Interactive surface
    pub const BG_HOVER: Color = Color::from_rgb(0.180, 0.216, 0.282);    // #2E3748 Hover
    pub const BG_ACTIVE: Color = Color::from_rgb(0.055, 0.455, 0.565);   // #0E7490 Active cyan-blue

    // Borders & Strokes
    pub const BORDER_HAIRLINE: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.07);
    pub const BORDER_CARD: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.10);
    pub const BORDER_ACTIVE: Color = Color::from_rgb(0.220, 0.741, 0.973); // #38BDF8 Sky cyan

    // Accent Colors
    pub const ACCENT_INDIGO: Color = Color::from_rgb(0.388, 0.400, 0.945); // #6366F1 Linear blue
    pub const ACCENT_CYAN: Color = Color::from_rgb(0.220, 0.741, 0.973);   // #38BDF8 Sky cyan
    pub const ACCENT_EMERALD: Color = Color::from_rgb(0.063, 0.725, 0.506);// #10B981 Run / Normal
    pub const ACCENT_AMBER: Color = Color::from_rgb(0.961, 0.620, 0.043);  // #F59E0B Warning / Paused
    pub const ACCENT_ROSE: Color = Color::from_rgb(0.957, 0.247, 0.369);   // #F43F5E Alarm / Error
    pub const ACCENT_PURPLE: Color = Color::from_rgb(0.659, 0.333, 0.969); // #A855F7 Structured Text

    // Typography
    pub const TEXT_PRIMARY: Color = Color::from_rgb(0.953, 0.957, 0.965);   // #F3F4F6
    pub const TEXT_SECONDARY: Color = Color::from_rgb(0.612, 0.639, 0.686); // #9CA3AF
    pub const TEXT_MUTED: Color = Color::from_rgb(0.420, 0.447, 0.502);     // #6B7280

    // Container Styles
    pub fn header_container_style(_theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::BG_PANEL)),
            border: Border {
                color: Self::BORDER_HAIRLINE,
                width: 1.0,
                radius: Radius::default(),
            },
            ..Default::default()
        }
    }

    pub fn sidebar_container_style(_theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::BG_PANEL)),
            border: Border {
                color: Self::BORDER_HAIRLINE,
                width: 1.0,
                radius: Radius::default(),
            },
            ..Default::default()
        }
    }

    pub fn card_style(_theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::BG_ELEVATED)),
            border: Border {
                color: Self::BORDER_CARD,
                width: 1.0,
                radius: Radius::from(10.0),
            },
            ..Default::default()
        }
    }

    pub fn canvas_container_style(_theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::BG_BASE)),
            border: Border {
                color: Self::BORDER_CARD,
                width: 1.0,
                radius: Radius::from(8.0),
            },
            ..Default::default()
        }
    }

    pub fn modal_card_style(_theme: &Theme) -> container::Style {
        container::Style {
            background: Some(Background::Color(Self::BG_ELEVATED)),
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
            text_color: Self::TEXT_PRIMARY,
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
        let bg = match status {
            button::Status::Hovered => Self::BG_HOVER,
            button::Status::Pressed => Self::BG_PANEL,
            button::Status::Disabled => Color::from_rgba(0.141, 0.169, 0.224, 0.3),
            button::Status::Active => Self::BG_SURFACE,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Self::TEXT_PRIMARY,
            border: Border {
                color: Self::BORDER_CARD,
                width: 1.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        }
    }

    pub fn ghost_button(_theme: &Theme, status: button::Status) -> button::Style {
        let bg = match status {
            button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.08),
            button::Status::Pressed => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: Self::TEXT_SECONDARY,
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
            text_color: Self::TEXT_PRIMARY,
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
            text_color: Self::TEXT_PRIMARY,
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn tab_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_theme: &Theme, status: button::Status| {
            let (bg, border_col, txt_col) = if is_active {
                (Self::BG_SURFACE, Self::ACCENT_CYAN, Self::TEXT_PRIMARY)
            } else {
                match status {
                    button::Status::Hovered => (Self::BG_HOVER, Color::TRANSPARENT, Self::TEXT_PRIMARY),
                    _ => (Color::TRANSPARENT, Color::TRANSPARENT, Self::TEXT_SECONDARY),
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
    pub fn styled_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
        let border_color = match status {
            text_input::Status::Focused => Self::ACCENT_INDIGO,
            text_input::Status::Hovered => Self::ACCENT_CYAN,
            text_input::Status::Disabled => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
            text_input::Status::Active => Self::BORDER_CARD,
        };
        text_input::Style {
            background: Background::Color(Self::BG_BASE),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(6.0),
            },
            icon: Self::TEXT_SECONDARY,
            placeholder: Self::TEXT_MUTED,
            value: Self::TEXT_PRIMARY,
            selection: Color::from_rgba(0.388, 0.400, 0.945, 0.3),
        }
    }

    // Pick List Styles
    pub fn styled_pick_list(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
        let border_color = match status {
            pick_list::Status::Hovered | pick_list::Status::Opened => Self::ACCENT_CYAN,
            _ => Self::BORDER_CARD,
        };
        pick_list::Style {
            text_color: Self::TEXT_PRIMARY,
            placeholder_color: Self::TEXT_MUTED,
            handle_color: Self::TEXT_SECONDARY,
            background: Background::Color(Self::BG_BASE),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(6.0),
            },
        }
    }

    // Toggler Styles
    pub fn styled_toggler(_theme: &Theme, status: toggler::Status) -> toggler::Style {
        let (bg, fg) = match status {
            toggler::Status::Active { is_toggled: true }
            | toggler::Status::Hovered { is_toggled: true } => {
                (Self::ACCENT_EMERALD, Color::WHITE)
            }
            toggler::Status::Active { is_toggled: false }
            | toggler::Status::Hovered { is_toggled: false } => {
                (Self::BG_SURFACE, Self::TEXT_MUTED)
            }
            toggler::Status::Disabled => {
                (Color::from_rgba(0.141, 0.169, 0.224, 0.3), Self::TEXT_MUTED)
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
