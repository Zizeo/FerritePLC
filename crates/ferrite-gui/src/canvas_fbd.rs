use std::collections::HashMap;

use iced::border::Radius;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, event};
use iced::widget::{button, column, container, mouse_area, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme, Vector, mouse};

use ferrite_core::{IecValue, StandardFbKind};
use ferrite_lang_fbd::{CompositeFbDef, FbdBlock, FbdBlockKind, FbdInputSource, FbdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub fn delete_fbd_block(program: &mut FbdProgram, idx: usize) {
    if idx >= program.blocks.len() {
        return;
    }
    let removed = program.blocks.remove(idx);
    let removed_id = removed.id;

    for blk in &mut program.blocks {
        for src in blk.inputs.values_mut() {
            if let FbdInputSource::Link { from_block, .. } = src
                && *from_block == removed_id
            {
                *src = FbdInputSource::Value {
                    expr: "FALSE".to_string(),
                };
            }
        }
    }
}

fn is_input_energized(rt: &PlcRuntime, src: &FbdInputSource) -> bool {
    match src {
        FbdInputSource::Value { expr } => {
            if let Ok(IecValue::Bool(b)) = rt.read_value(expr) {
                b
            } else {
                expr.trim().eq_ignore_ascii_case("true")
            }
        }
        FbdInputSource::Link {
            from_block,
            from_pin,
        } => {
            let sym = format!("{from_block}.{from_pin}");
            matches!(rt.read_value(&sym), Ok(IecValue::Bool(true)))
        }
    }
}

fn is_output_energized(rt: &PlcRuntime, target: &str) -> bool {
    matches!(rt.read_value(target), Ok(IecValue::Bool(true)))
}

fn read_input_live_value(rt: &PlcRuntime, src: &FbdInputSource) -> Option<String> {
    match src {
        FbdInputSource::Value { expr } => rt
            .read_value(expr.trim())
            .ok()
            .or_else(|| IecValue::parse_literal(expr.trim(), None).ok())
            .map(format_live_iec_value),
        FbdInputSource::Link {
            from_block,
            from_pin,
        } => {
            let sym = format!("{from_block}.{from_pin}");
            rt.read_value(&sym).ok().map(format_live_iec_value)
        }
    }
}

fn read_output_live_value(rt: &PlcRuntime, target: &str) -> Option<String> {
    rt.read_value(target.trim()).ok().map(format_live_iec_value)
}

fn format_live_iec_value(val: IecValue) -> String {
    match val {
        IecValue::Bool(b) => if b { "TRUE".to_string() } else { "FALSE".to_string() },
        IecValue::Sint(i) => format!("{i}"),
        IecValue::Int(i) => format!("{i}"),
        IecValue::Dint(d) => format!("{d}"),
        IecValue::Lint(l) => format!("{l}"),
        IecValue::Usint(u) => format!("{u}"),
        IecValue::Uint(u) => format!("{u}"),
        IecValue::Udint(u) => format!("{u}"),
        IecValue::Ulint(u) => format!("{u}"),
        IecValue::Byte(b) => format!("16#{b:02X}"),
        IecValue::Word(w) => format!("16#{w:04X}"),
        IecValue::Dword(d) => format!("16#{d:08X}"),
        IecValue::Lword(l) => format!("16#{l:016X}"),
        IecValue::Real(r) => format!("{r:.1}"),
        IecValue::Lreal(r) => format!("{r:.1}"),
        IecValue::Time(ms) => format!("{ms}ms"),
        IecValue::String(s) => s,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FbdCategory {
    Logic,
    Arithmetic,
    Comparator,
    Timer,
    Counter,
    Edge,
    Bistable,
    Conditioning,
    Corrector,
    Composite,
}

impl FbdCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Logic => "LOGIC:GATE",
            Self::Arithmetic => "MATH:OP",
            Self::Comparator => "COMPARE",
            Self::Timer => "TIME:DELAY",
            Self::Counter => "COUNT:ACC",
            Self::Edge => "EDGE:TRIG",
            Self::Bistable => "LATCH:FF",
            Self::Conditioning => "SIGNAL:COND",
            Self::Corrector => "CTRL:PID",
            Self::Composite => "CUSTOM:MOD",
        }
    }

    pub fn accent_color(self) -> iced::Color {
        match self {
            Self::Logic => iced::Color::from_rgb(0.388, 0.400, 0.945),        // #6366F1 Electric Indigo
            Self::Arithmetic => iced::Color::from_rgb(0.961, 0.620, 0.043),   // #F59E0B Amber Tech
            Self::Comparator => iced::Color::from_rgb(0.918, 0.702, 0.031),   // #EAB308 Gold Compare
            Self::Timer => iced::Color::from_rgb(0.024, 0.714, 0.831),        // #06B6D4 Neon Cyan
            Self::Counter => iced::Color::from_rgb(0.659, 0.333, 0.969),      // #A855F7 Plasma Purple
            Self::Edge => iced::Color::from_rgb(0.925, 0.282, 0.600),         // #EC4899 Laser Fuchsia
            Self::Bistable => iced::Color::from_rgb(0.231, 0.510, 0.965),     // #3B82F6 Electric Blue
            Self::Conditioning => iced::Color::from_rgb(0.078, 0.722, 0.651), // #14B8A6 Signal Teal
            Self::Corrector => iced::Color::from_rgb(0.063, 0.725, 0.506),    // #10B981 Emerald Control
            Self::Composite => iced::Color::from_rgb(0.957, 0.247, 0.369),    // #F43F5E Rose Tech
        }
    }

    pub fn bg_subtle(self) -> iced::Color {
        let mut c = self.accent_color();
        c.a = 0.06;
        c
    }

    pub fn badge_bg(self) -> iced::Color {
        let mut c = self.accent_color();
        c.a = 0.14;
        c
    }
}

pub fn block_category(kind: &FbdBlockKind) -> FbdCategory {
    match kind {
        FbdBlockKind::And | FbdBlockKind::Or | FbdBlockKind::Xor | FbdBlockKind::Not => {
            FbdCategory::Logic
        }
        FbdBlockKind::Add | FbdBlockKind::Sub | FbdBlockKind::Mul | FbdBlockKind::Div => {
            FbdCategory::Arithmetic
        }
        FbdBlockKind::Gt
        | FbdBlockKind::Ge
        | FbdBlockKind::Lt
        | FbdBlockKind::Le
        | FbdBlockKind::Eq
        | FbdBlockKind::Ne => FbdCategory::Comparator,
        FbdBlockKind::StandardFb {
            fb_type: Some(fb), ..
        } => match fb {
            StandardFbKind::Ton
            | StandardFbKind::Tof
            | StandardFbKind::Tp
            | StandardFbKind::Blink => FbdCategory::Timer,
            StandardFbKind::Ctu | StandardFbKind::Ctd | StandardFbKind::Ctud => {
                FbdCategory::Counter
            }
            StandardFbKind::RTrig | StandardFbKind::FTrig => FbdCategory::Edge,
            StandardFbKind::Sr | StandardFbKind::Rs => FbdCategory::Bistable,
            StandardFbKind::Hysteresis
            | StandardFbKind::RampReal
            | StandardFbKind::ScaleLin => FbdCategory::Conditioning,
            StandardFbKind::P
            | StandardFbKind::Pi
            | StandardFbKind::PidParallel
            | StandardFbKind::PidSeries
            | StandardFbKind::PidMixed => FbdCategory::Corrector,
        },
        FbdBlockKind::StandardFb { .. } => FbdCategory::Logic,
        FbdBlockKind::Composite { .. } => FbdCategory::Composite,
    }
}

fn operator_symbol(kind: &FbdBlockKind) -> (&str, &'static str) {
    match kind {
        FbdBlockKind::And => ("&", "AND GATE"),
        FbdBlockKind::Or => ("\u{2265}1", "OR GATE"),
        FbdBlockKind::Xor => ("=1", "XOR GATE"),
        FbdBlockKind::Not => ("1", "NOT INVERT"),
        FbdBlockKind::Add => ("+", "ADDITION"),
        FbdBlockKind::Sub => ("\u{2212}", "SUBTRACT"),
        FbdBlockKind::Mul => ("\u{00D7}", "MULTIPLY"),
        FbdBlockKind::Div => ("\u{00F7}", "DIVIDE"),
        FbdBlockKind::Gt => (">", "GREATER THAN"),
        FbdBlockKind::Ge => ("\u{2265}", "GREATER OR EQ"),
        FbdBlockKind::Lt => ("<", "LESS THAN"),
        FbdBlockKind::Le => ("\u{2264}", "LESS OR EQ"),
        FbdBlockKind::Eq => ("==", "EQUAL"),
        FbdBlockKind::Ne => ("\u{2260}", "NOT EQUAL"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ton),
            ..
        } => ("TON", "TIMER ON-DELAY"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Tof),
            ..
        } => ("TOF", "TIMER OFF-DELAY"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Tp),
            ..
        } => ("TP", "PULSE TIMER"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctu),
            ..
        } => ("CTU", "COUNT UP"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctd),
            ..
        } => ("CTD", "COUNT DOWN"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctud),
            ..
        } => ("CTUD", "UP/DOWN CNT"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Blink),
            ..
        } => ("BLINK", "CLOCK GEN"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::RTrig),
            ..
        } => ("R_TRIG", "RISING EDGE"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::FTrig),
            ..
        } => ("F_TRIG", "FALLING EDGE"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Sr),
            ..
        } => ("SR", "SET LATCH"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Rs),
            ..
        } => ("RS", "RESET LATCH"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Hysteresis),
            ..
        } => ("HYST", "DEADBAND"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::RampReal),
            ..
        } => ("RAMP", "SLEW LIMIT"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::ScaleLin),
            ..
        } => ("SCALE", "LIN SCALE"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidParallel),
            ..
        } => ("PID_PAR", "PARALLEL PID"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidSeries),
            ..
        } => ("PID_SER", "SERIES PID"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidMixed),
            ..
        } => ("PID_MIX", "MIXED (ISA)"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Pi),
            ..
        } => ("PI", "PI REGULATOR"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::P),
            ..
        } => ("P", "PROPORTIONAL"),
        FbdBlockKind::StandardFb { instance, .. } => (instance.as_str(), "FB INSTANCE"),
        FbdBlockKind::Composite { type_name } => (type_name.as_str(), "COMPOSITE"),
    }
}

fn pin_display_label(pin: &str, is_corrector: bool, lang: UiLanguage) -> String {
    if is_corrector {
        match pin.to_ascii_uppercase().as_str() {
            "SP" => lang.tr("SP (Set)", "SP (Cons)").to_string(),
            "PV" => lang.tr("PV (Proc)", "PV (Mes)").to_string(),
            "KP" => "Kp".to_string(),
            "TI" => "Ti (s)".to_string(),
            "TD" => "Td (s)".to_string(),
            "OUT" => lang.tr("OUT (Cmd)", "OUT (Cmd)").to_string(),
            other => other.to_string(),
        }
    } else {
        pin.to_string()
    }
}

fn tech_terminal_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let is_dark = StudioTheme::is_dark(theme);
    let (border_color, bg) = match status {
        text_input::Status::Focused => (
            StudioTheme::ACCENT_CYAN,
            if is_dark {
                iced::Color::from_rgb(0.02, 0.04, 0.07)
            } else {
                iced::Color::WHITE
            },
        ),
        text_input::Status::Hovered => (
            iced::Color::from_rgba(0.220, 0.741, 0.973, 0.6),
            if is_dark {
                iced::Color::from_rgb(0.04, 0.06, 0.09)
            } else {
                iced::Color::from_rgb(0.97, 0.98, 0.99)
            },
        ),
        text_input::Status::Disabled => (
            if is_dark {
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            } else {
                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.04)
            },
            if is_dark {
                iced::Color::from_rgb(0.03, 0.04, 0.06)
            } else {
                iced::Color::from_rgb(0.94, 0.95, 0.96)
            },
        ),
        text_input::Status::Active => (
            StudioTheme::border_card(theme),
            if is_dark {
                iced::Color::from_rgb(0.04, 0.05, 0.08)
            } else {
                iced::Color::WHITE
            },
        ),
    };
    text_input::Style {
        background: Background::Color(bg),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius::from(4.0),
        },
        icon: StudioTheme::text_secondary(theme),
        placeholder: StudioTheme::text_muted(theme),
        value: StudioTheme::text_primary(theme),
        selection: iced::Color::from_rgba(0.220, 0.741, 0.973, 0.3),
    }
}

fn tech_id_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let is_dark = StudioTheme::is_dark(theme);
    let (border_color, bg) = match status {
        text_input::Status::Focused => (
            StudioTheme::ACCENT_CYAN,
            if is_dark {
                iced::Color::from_rgb(0.03, 0.05, 0.08)
            } else {
                iced::Color::WHITE
            },
        ),
        text_input::Status::Hovered => (
            if is_dark {
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.3)
            } else {
                iced::Color::from_rgba(0.0, 0.0, 0.0, 0.25)
            },
            if is_dark {
                iced::Color::from_rgb(0.05, 0.07, 0.10)
            } else {
                iced::Color::from_rgb(0.98, 0.99, 1.0)
            },
        ),
        _ => (
            StudioTheme::border_card(theme),
            if is_dark {
                iced::Color::from_rgb(0.05, 0.07, 0.10)
            } else {
                iced::Color::WHITE
            },
        ),
    };
    text_input::Style {
        background: Background::Color(bg),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius {
                top_left: 0.0,
                bottom_left: 0.0,
                top_right: 4.0,
                bottom_right: 4.0,
            },
        },
        icon: StudioTheme::text_secondary(theme),
        placeholder: StudioTheme::text_muted(theme),
        value: StudioTheme::text_primary(theme),
        selection: iced::Color::from_rgba(0.220, 0.741, 0.973, 0.3),
    }
}

fn tech_remove_button(_theme: &Theme, status: button::Status) -> button::Style {
    let (bg, text_color, border_color) = match status {
        button::Status::Hovered => (
            iced::Color::from_rgba(0.957, 0.247, 0.369, 0.25),
            StudioTheme::ACCENT_ROSE,
            StudioTheme::ACCENT_ROSE,
        ),
        button::Status::Pressed => (
            iced::Color::from_rgba(0.957, 0.247, 0.369, 0.40),
            iced::Color::WHITE,
            StudioTheme::ACCENT_ROSE,
        ),
        button::Status::Disabled => (
            iced::Color::TRANSPARENT,
            StudioTheme::TEXT_MUTED,
            iced::Color::TRANSPARENT,
        ),
        button::Status::Active => (
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04),
            StudioTheme::TEXT_MUTED,
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
        ),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius::from(4.0),
        },
        ..Default::default()
    }
}

fn render_single_block_card<'a>(
    program_name: &str,
    b_idx: usize,
    blk: &'a FbdBlock,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
    selected_block: Option<usize>,
    drag_hover_block: Option<usize>,
) -> Element<'a, Message> {
    let p_blk = program_name.to_string();
    let category = block_category(&blk.kind);
    let (op_symbol, op_desc) = operator_symbol(&blk.kind);

    let is_corrector = match &blk.kind {
        FbdBlockKind::StandardFb {
            fb_type: Some(k), ..
        } => k.is_corrector(),
        _ => false,
    };
    let current_corrector_kind = match &blk.kind {
        FbdBlockKind::StandardFb {
            fb_type: Some(k), ..
        } if k.is_corrector() => Some(*k),
        _ => None,
    };

    let is_selected = selected_block == Some(b_idx);
    let is_hovered = drag_hover_block == Some(b_idx);
    let blk_id = blk.id.clone();
    let p_blk_id = blk.id.clone();

    let any_input_energized = runtime.is_some_and(|rt| {
        blk.inputs.values().any(|src| is_input_energized(rt, src))
    });
    let any_output_energized = runtime.is_some_and(|rt| {
        blk.outputs.values().any(|target| is_output_energized(rt, target))
    });
    let is_block_active = any_input_energized || any_output_energized;

    // Top Category Accent Rail
    let top_rail = container(iced::widget::Space::with_width(Length::Fill))
        .height(Length::Fixed(3.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(category.accent_color())),
            border: Border {
                radius: Radius {
                    top_left: 8.0,
                    top_right: 8.0,
                    bottom_right: 0.0,
                    bottom_left: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        });

    let drag_handle = mouse_area(
        container(
            text(crate::icons::icon_grip())
                .size(12)
                .shaping(iced::widget::text::Shaping::Advanced)
                .color(if is_selected {
                    StudioTheme::ACCENT_CYAN
                } else {
                    StudioTheme::TEXT_MUTED
                }),
        )
        .padding([4, 6])
        .style(|_| container::Style {
            background: Some(Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                width: 1.0,
                radius: Radius::from(4.0),
            },
            ..Default::default()
        }),
    )
    .on_press(Message::FbdBlockDragStarted(p_blk.clone(), b_idx))
    .on_enter(Message::FbdBlockDragHovered(p_blk.clone(), b_idx))
    .on_release(Message::FbdBlockDragDropped);

    // Block ID with hardware badge prefix
    let id_box = row![
        container(
            text("ID")
                .size(8.5)
                .font(iced::Font::MONOSPACE)
                .color(StudioTheme::TEXT_MUTED),
        )
        .padding([4, 6])
        .style(|_| container::Style {
            background: Some(Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
            border: Border {
                color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                width: 1.0,
                radius: Radius {
                    top_left: 4.0,
                    bottom_left: 4.0,
                    top_right: 0.0,
                    bottom_right: 0.0,
                },
            },
            ..Default::default()
        }),
        text_input("Block ID", &blk.id)
            .on_input({
                let p = p_blk.clone();
                let old_id = blk_id.clone();
                move |s| Message::FbdBlockIdChanged(p.clone(), old_id.clone(), s)
            })
            .font(iced::Font::MONOSPACE)
            .size(11.0)
            .width(Length::FillPortion(1))
            .padding([3, 6])
            .style(tech_id_input),
    ]
    .align_y(Alignment::Center);

    let cat_badge = container(
        text(category.label())
            .size(8.5)
            .font(iced::Font::MONOSPACE)
            .color(category.accent_color()),
    )
    .padding([3, 7])
    .style(move |_| container::Style {
        background: Some(Background::Color(category.badge_bg())),
        border: Border {
            color: iced::Color::from_rgba(
                category.accent_color().r,
                category.accent_color().g,
                category.accent_color().b,
                0.35,
            ),
            width: 0.75,
            radius: Radius::from(4.0),
        },
        ..Default::default()
    });

    let live_status_badge: Element<'a, Message> = if runtime.is_some() {
        container(
            row![
                container(iced::widget::Space::with_width(Length::Fixed(5.0)))
                    .height(Length::Fixed(5.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(if is_block_active {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.2)
                        })),
                        border: Border {
                            radius: Radius::from(2.5),
                            color: if is_block_active {
                                iced::Color::from_rgb(0.2, 0.9, 0.5)
                            } else {
                                iced::Color::TRANSPARENT
                            },
                            width: if is_block_active { 1.0 } else { 0.0 },
                        },
                        ..Default::default()
                    }),
                text(if is_block_active { "ACTIVE" } else { "IDLE" })
                    .size(8.0)
                    .font(iced::Font::MONOSPACE)
                    .color(if is_block_active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::TEXT_MUTED
                    }),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding([2, 5])
        .style(move |_| container::Style {
            background: Some(Background::Color(if is_block_active {
                iced::Color::from_rgba(0.063, 0.725, 0.506, 0.12)
            } else {
                iced::Color::from_rgba(1.0, 1.0, 1.0, 0.03)
            })),
            border: Border {
                color: if is_block_active {
                    iced::Color::from_rgba(0.063, 0.725, 0.506, 0.3)
                } else {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05)
                },
                width: 0.5,
                radius: Radius::from(3.0),
            },
            ..Default::default()
        })
        .into()
    } else {
        iced::widget::Space::with_width(Length::Shrink).into()
    };

    let remove_btn = button(
        text(crate::icons::icon_close())
            .size(11.0)
            .shaping(iced::widget::text::Shaping::Advanced),
    )
    .on_press(Message::FbdRemoveBlock(p_blk.clone(), p_blk_id.clone()))
    .style(tech_remove_button)
    .padding([2, 6]);

    // Block Header with Drag Handle, Editable ID, Category Badge, and Remove Button
    let blk_header = row![
        drag_handle,
        id_box,
        cat_badge,
        live_status_badge,
        iced::widget::Space::with_width(Length::Fill),
        remove_btn,
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // Optional Corrector Architecture Switcher Row
    let arch_row = current_corrector_kind.map(|cur_k| {
        let corrector_kinds = vec![
            StandardFbKind::PidParallel,
            StandardFbKind::PidSeries,
            StandardFbKind::PidMixed,
            StandardFbKind::Pi,
            StandardFbKind::P,
        ];
        container(
            column![
                row![
                    text(lang.tr("CONTROL LOOP TUNING", "BOUCLE DE RÉGULATION"))
                        .size(8.5)
                        .font(iced::Font::MONOSPACE)
                        .color(category.accent_color()),
                    iced::widget::Space::with_width(Length::Fill),
                    text("IEC 61131-3")
                        .size(8.0)
                        .font(iced::Font::MONOSPACE)
                        .color(StudioTheme::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
                row![
                    text(lang.tr("Architecture:", "Architecture :"))
                        .size(9.5)
                        .color(StudioTheme::TEXT_SECONDARY),
                    pick_list(corrector_kinds, Some(cur_k), {
                        let p = p_blk.clone();
                        move |k| Message::FbdChangeCorrectorKind(p.clone(), b_idx, k)
                    })
                    .text_size(10.0)
                    .padding([3, 6])
                    .width(Length::Fill),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
                container(
                    row![
                        text("ƒ(e):")
                            .size(9.0)
                            .font(iced::Font::MONOSPACE)
                            .color(category.accent_color()),
                        text(cur_k.formula_hint())
                            .size(9.0)
                            .font(iced::Font::MONOSPACE)
                            .color(StudioTheme::ACCENT_CYAN)
                            .wrapping(iced::widget::text::Wrapping::Word),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .padding([4, 7])
                .style(|_| container::Style {
                    background: Some(Background::Color(iced::Color::from_rgb(0.03, 0.05, 0.08))),
                    border: Border {
                        color: iced::Color::from_rgba(0.063, 0.725, 0.506, 0.25),
                        width: 0.5,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                }),
            ]
            .spacing(5),
        )
        .padding([6, 8])
        .style(|_| container::Style {
            background: Some(Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
            border: Border {
                color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05),
                width: 1.0,
                radius: Radius::from(5.0),
            },
            ..Default::default()
        })
    });

    // Inputs Column
    let mut sorted_inputs: Vec<_> = blk.inputs.iter().collect();
    sorted_inputs.sort_by_key(|&(k, _)| k);

    let mut inputs_col = column![].spacing(6);
    if sorted_inputs.is_empty() {
        inputs_col = inputs_col.push(
            text(lang.tr("(No inputs)", "(Sans entrée)"))
                .size(9.5)
                .font(iced::Font::MONOSPACE)
                .color(StudioTheme::TEXT_MUTED),
        );
    } else {
        for (pin_name, src) in sorted_inputs {
            let pin_n = pin_name.clone();
            let expr_str = match src {
                FbdInputSource::Value { expr } => expr.clone(),
                FbdInputSource::Link {
                    from_block,
                    from_pin,
                } => format!("{from_block}.{from_pin}"),
            };

            let is_energized = runtime.is_some_and(|rt| is_input_energized(rt, src));
            let terminal_led = container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                .height(Length::Fixed(8.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(if is_energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        iced::Color::from_rgb(0.12, 0.15, 0.20)
                    })),
                    border: Border {
                        color: if is_energized {
                            iced::Color::from_rgb(0.3, 0.9, 0.6)
                        } else {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                        },
                        width: 1.0,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                });

            let display_pin = pin_display_label(pin_name, is_corrector, lang);
            let pin_badge = container(
                text(display_pin)
                    .size(9.5)
                    .font(iced::Font::MONOSPACE)
                    .color(if is_energized {
                        StudioTheme::TEXT_PRIMARY
                    } else {
                        StudioTheme::TEXT_SECONDARY
                    }),
            )
            .padding([2, 5])
            .style(move |_| container::Style {
                background: Some(Background::Color(if is_energized {
                    iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)
                } else {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                })),
                border: Border {
                    color: if is_energized {
                        iced::Color::from_rgba(0.063, 0.725, 0.506, 0.4)
                    } else {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                    },
                    width: 0.5,
                    radius: Radius::from(3.0),
                },
                ..Default::default()
            });

            let live_badge: Element<'a, Message> = if let Some(rt) = runtime
                && let Some(val_str) = read_input_live_value(rt, src)
            {
                container(
                    text(val_str)
                        .size(9.0)
                        .font(iced::Font::MONOSPACE)
                        .color(if is_energized {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            StudioTheme::ACCENT_CYAN
                        }),
                )
                .padding([2, 5])
                .style(move |_| container::Style {
                    background: Some(Background::Color(iced::Color::from_rgb(0.03, 0.05, 0.08))),
                    border: Border {
                        color: if is_energized {
                            iced::Color::from_rgba(0.063, 0.725, 0.506, 0.3)
                        } else {
                            iced::Color::from_rgba(0.220, 0.741, 0.973, 0.25)
                        },
                        width: 1.0,
                        radius: Radius::from(3.0),
                    },
                    ..Default::default()
                })
                .into()
            } else {
                iced::widget::Space::with_width(Length::Shrink).into()
            };

            let wire_trace = container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                .height(Length::Fixed(2.0))
                .style(move |_| container::Style {
                    background: Some(Background::Color(if is_energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                    })),
                    ..Default::default()
                });

            let in_row = row![
                terminal_led,
                pin_badge,
                text_input("Source", &expr_str)
                    .on_input({
                    let p = p_blk.clone();
 let bid = p_blk_id.clone();
 let pn = pin_n.clone();
 move |s| Message::FbdInputSourceChanged(p.clone(), bid.clone(), pn.clone(), s)
                    })
                    .font(iced::Font::MONOSPACE)
                    .size(11.0)
                    .width(Length::Fill)
                    .padding([3, 6])
                    .style(tech_terminal_input),
                live_badge,
                wire_trace,
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            inputs_col = inputs_col.push(in_row);
        }
    }

    // Central Operator Core ("Processing Cell")
    let center_box = container(
        column![
            text(op_symbol)
                .size(20)
                .font(iced::Font::MONOSPACE)
                .color(if is_block_active {
                    iced::Color::WHITE
                } else {
                    StudioTheme::TEXT_PRIMARY
                }),
            container(
                text(op_desc)
                    .size(8.5)
                    .font(iced::Font::MONOSPACE)
                    .color(category.accent_color())
                    .wrapping(iced::widget::text::Wrapping::Word),
            )
            .padding([2, 5])
            .style(move |_| container::Style {
                background: Some(Background::Color(category.bg_subtle())),
                border: Border {
                    radius: Radius::from(3.0),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]
        .spacing(4)
        .align_x(Alignment::Center),
    )
    .padding([8, 12])
    .style(move |theme: &Theme| {
        let is_dark = StudioTheme::is_dark(theme);
        let center_bg = if is_dark {
            iced::Color::from_rgb(0.038, 0.048, 0.068)
        } else {
            iced::Color::WHITE
        };
        container::Style {
            background: Some(Background::Color(center_bg)),
            border: Border {
                color: if is_block_active {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    iced::Color::from_rgba(
                        category.accent_color().r,
                        category.accent_color().g,
                        category.accent_color().b,
                        if is_dark { 0.35 } else { 0.50 },
                    )
                },
                width: if is_block_active { 1.5 } else { 1.0 },
                radius: Radius::from(6.0),
            },
            ..Default::default()
        }
    });

    // Outputs Column
    let mut sorted_outputs: Vec<_> = blk.outputs.iter().collect();
    sorted_outputs.sort_by_key(|&(k, _)| k);

    let mut outputs_col = column![].spacing(6);
    if sorted_outputs.is_empty() {
        let fb_live_summary = if let (
            Some(rt),
            FbdBlockKind::StandardFb { instance, .. },
        ) = (runtime, &blk.kind)
        {
            rt.tags.get_fb(instance).map(|fb| {
                format!(
                    "Q={} OUT={:.1} CV={}",
                    if fb.q { "1" } else { "0" },
                    fb.out_real,
                    fb.cv
                )
            })
        } else {
            None
        };

        if let Some(summary) = fb_live_summary {
            outputs_col = outputs_col.push(
                container(
                    text(summary)
                        .size(9.0)
                        .font(iced::Font::MONOSPACE)
                        .color(StudioTheme::ACCENT_EMERALD),
                )
                .padding([3, 7])
                .style(|_| container::Style {
                    background: Some(Background::Color(iced::Color::from_rgb(0.03, 0.05, 0.08))),
                    border: Border {
                        color: iced::Color::from_rgba(0.063, 0.725, 0.506, 0.3),
                        width: 0.5,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                }),
            );
        } else {
            outputs_col = outputs_col.push(
                text(lang.tr("(Internal)", "(Interne)"))
                    .size(9.5)
                    .font(iced::Font::MONOSPACE)
                    .color(StudioTheme::TEXT_MUTED),
            );
        }
    } else {
        for (pin_name, target) in sorted_outputs {
            let pin_n = pin_name.clone();
            let is_energized = runtime.is_some_and(|rt| is_output_energized(rt, target));
            let terminal_led = container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                .height(Length::Fixed(8.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(if is_energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        iced::Color::from_rgb(0.12, 0.15, 0.20)
                    })),
                    border: Border {
                        color: if is_energized {
                            iced::Color::from_rgb(0.3, 0.9, 0.6)
                        } else {
                            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                        },
                        width: 1.0,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                });

            let display_pin = pin_display_label(pin_name, is_corrector, lang);
            let pin_badge = container(
                text(display_pin)
                    .size(9.5)
                    .font(iced::Font::MONOSPACE)
                    .color(if is_energized {
                        StudioTheme::TEXT_PRIMARY
                    } else {
                        StudioTheme::TEXT_SECONDARY
                    }),
            )
            .padding([2, 5])
            .style(move |_| container::Style {
                background: Some(Background::Color(if is_energized {
                    iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)
                } else {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                })),
                border: Border {
                    color: if is_energized {
                        iced::Color::from_rgba(0.063, 0.725, 0.506, 0.4)
                    } else {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                    },
                    width: 0.5,
                    radius: Radius::from(3.0),
                },
                ..Default::default()
            });

            let out_live_badge: Element<'a, Message> = if let Some(rt) = runtime
                && let Some(val_str) = read_output_live_value(rt, target)
            {
                container(
                    text(val_str)
                        .size(9.0)
                        .font(iced::Font::MONOSPACE)
                        .color(if is_energized {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            StudioTheme::ACCENT_AMBER
                        }),
                )
                .padding([2, 5])
                .style(move |_| container::Style {
                    background: Some(Background::Color(iced::Color::from_rgb(0.03, 0.05, 0.08))),
                    border: Border {
                        color: if is_energized {
                            iced::Color::from_rgba(0.063, 0.725, 0.506, 0.3)
                        } else {
                            iced::Color::from_rgba(0.96, 0.62, 0.04, 0.25)
                        },
                        width: 1.0,
                        radius: Radius::from(3.0),
                    },
                    ..Default::default()
                })
                .into()
            } else {
                iced::widget::Space::with_width(Length::Shrink).into()
            };

            let wire_trace = container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                .height(Length::Fixed(2.0))
                .style(move |_| container::Style {
                    background: Some(Background::Color(if is_energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                    })),
                    ..Default::default()
                });

            let out_row = row![
                wire_trace,
                pin_badge,
                text_input("Target", target)
                    .on_input({
                    let p = p_blk.clone();
 let bid = p_blk_id.clone();
 let pn = pin_n.clone();
 move |s| Message::FbdOutputTargetChanged(p.clone(), bid.clone(), pn.clone(), s)
                    })
                    .font(iced::Font::MONOSPACE)
                    .size(11.0)
                    .width(Length::Fill)
                    .padding([3, 6])
                    .style(tech_terminal_input),
                out_live_badge,
                terminal_led,
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            outputs_col = outputs_col.push(out_row);
        }
    }

    let schematic_body = row![
        container(inputs_col).width(Length::FillPortion(1)),
        center_box,
        container(outputs_col).width(Length::FillPortion(1)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut card_col = column![top_rail];
    let mut inner_col = column![blk_header];
    if let Some(arch) = arch_row {
        inner_col = inner_col.push(arch);
    }
    inner_col = inner_col.push(
        container(schematic_body)
            .padding([8, 10])
            .style(move |theme: &Theme| {
                let is_dark = StudioTheme::is_dark(theme);
                let body_bg = if is_dark {
                    iced::Color::from_rgb(0.045, 0.055, 0.075)
                } else {
                    iced::Color::from_rgb(0.950, 0.960, 0.975)
                };
                container::Style {
                    background: Some(Background::Color(body_bg)),
                    border: Border {
                        color: StudioTheme::border_hairline(theme),
                        width: 1.0,
                        radius: Radius::from(6.0),
                    },
                    ..Default::default()
                }
            })
            .width(Length::Fill),
    );

    card_col = card_col.push(
        container(inner_col.spacing(8)).padding(Padding {
            top: 6.0,
            right: 10.0,
            bottom: 10.0,
            left: 10.0,
        }),
    );

    let card_style_fn = move |theme: &Theme| {
        let is_dark = StudioTheme::is_dark(theme);
        let border = if is_hovered {
            Border {
                color: iced::Color::from_rgb(0.25, 0.65, 1.0),
                width: 2.0,
                radius: Radius::from(8.0),
            }
        } else if is_selected {
            Border {
                color: StudioTheme::ACCENT_CYAN,
                width: 2.0,
                radius: Radius::from(8.0),
            }
        } else {
            Border {
                color: StudioTheme::border_card(theme),
                width: 1.0,
                radius: Radius::from(8.0),
            }
        };

        let card_bg = if is_dark {
            iced::Color::from_rgb(0.070, 0.082, 0.110)
        } else {
            iced::Color::WHITE
        };

        container::Style {
            background: Some(Background::Color(card_bg)),
            border,
            ..Default::default()
        }
    };

    let card_container = container(card_col.spacing(0))
        .style(card_style_fn)
        .width(Length::FillPortion(1));

    mouse_area(card_container)
        .on_enter(Message::FbdBlockDragHovered(p_blk, b_idx))
        .on_release(Message::FbdBlockDragDropped)
        .into()
}

const FBD_BLOCK_WIDTH: f32 = 176.0;
const FBD_HEADER_HEIGHT: f32 = 26.0;
const FBD_PIN_ROW_HEIGHT: f32 = 20.0;
const FBD_BOTTOM_PAD: f32 = 8.0;
const FBD_GRID_SIZE: f32 = 20.0;
const FBD_PIN_HIT_RADIUS: f32 = 12.0;
const FBD_MIN_ZOOM: f32 = 0.25;
const FBD_MAX_ZOOM: f32 = 3.0;

#[derive(Debug, Clone)]
pub enum FbdDragState {
    None,
    MovingBlock {
        block_id: String,
        grab_offset: Vector,
    },
    Wiring {
        from_block: String,
        from_pin: String,
        start_pt: Point,
        current_pt: Point,
    },
}

#[derive(Debug, Clone)]
pub struct FbdCanvasState {
    pub pan: Vector,
    pub zoom: f32,
    pub is_panning: bool,
    pub pan_start: Option<Point>,
    pub active_pou: String,
    pub per_pou_viewport: HashMap<String, (Vector, f32)>,
    pub block_positions: HashMap<String, Point>,
    pub drag: FbdDragState,
    pub selected_block_id: Option<String>,
}

impl Default for FbdCanvasState {
    fn default() -> Self {
        Self {
            pan: Vector::default(),
            zoom: 1.0,
            is_panning: false,
            pan_start: None,
            active_pou: String::new(),
            per_pou_viewport: HashMap::new(),
            block_positions: HashMap::new(),
            drag: FbdDragState::None,
            selected_block_id: None,
        }
    }
}

impl FbdCanvasState {
    pub fn effective_zoom(&self) -> f32 {
        if self.zoom.is_finite() && self.zoom > 0.0 {
            self.zoom.clamp(FBD_MIN_ZOOM, FBD_MAX_ZOOM)
        } else {
            1.0
        }
    }

    pub fn viewport_for(&self, pou_name: &str) -> (Vector, f32) {
        if self.active_pou == pou_name || self.active_pou.is_empty() {
            (self.pan, self.effective_zoom())
        } else if let Some(&(saved_pan, saved_zoom)) = self.per_pou_viewport.get(pou_name) {
            (saved_pan, saved_zoom.clamp(FBD_MIN_ZOOM, FBD_MAX_ZOOM))
        } else {
            (Vector::default(), 1.0)
        }
    }

    pub fn sync_pou(&mut self, pou_name: &str) {
        if self.active_pou == pou_name {
            return;
        }
        if !self.active_pou.is_empty() {
            self.per_pou_viewport
                .insert(self.active_pou.clone(), (self.pan, self.effective_zoom()));
        }
        let was_uninitialized = self.active_pou.is_empty();
        self.active_pou = pou_name.to_string();
        if let Some(&(saved_pan, saved_zoom)) = self.per_pou_viewport.get(pou_name) {
            self.pan = saved_pan;
            self.zoom = saved_zoom.clamp(FBD_MIN_ZOOM, FBD_MAX_ZOOM);
        } else if !was_uninitialized {
            self.pan = Vector::default();
            self.zoom = 1.0;
        }
        let cur_pan = self.pan;
        let eff_zoom = self.effective_zoom();
        self.per_pou_viewport
            .entry(pou_name.to_string())
            .or_insert((cur_pan, eff_zoom));
        self.is_panning = false;
        self.pan_start = None;
        self.drag = FbdDragState::None;
    }

    fn store_current_viewport(&mut self, pou_name: &str) {
        if !pou_name.is_empty() {
            self.active_pou = pou_name.to_string();
            self.per_pou_viewport
                .insert(pou_name.to_string(), (self.pan, self.effective_zoom()));
        }
    }

    pub fn ensure_layout(&mut self, program: &FbdProgram) {
        let defaults = compute_default_block_positions(program);
        for blk in &program.blocks {
            if !self.block_positions.contains_key(&blk.id)
                && let Some(&pt) = defaults.get(&blk.id)
            {
                self.block_positions.insert(blk.id.clone(), pt);
            }
        }
    }

    pub fn resolved_layout(&self, program: &FbdProgram) -> HashMap<String, Point> {
        let mut layout = compute_default_block_positions(program);
        for blk in &program.blocks {
            if let Some(&pos) = self.block_positions.get(&blk.id) {
                layout.insert(blk.id.clone(), pos);
            }
        }
        layout
    }
}

fn fbd_block_input_pins(blk: &FbdBlock) -> Vec<String> {
    let mut pins: Vec<String> = blk.inputs.keys().cloned().collect();
    pins.sort();
    pins
}

fn fbd_block_output_pins(blk: &FbdBlock, composites: &[CompositeFbDef]) -> Vec<String> {
    let mut pins: Vec<String> = match &blk.kind {
        FbdBlockKind::StandardFb {
            fb_type: Some(kind),
            ..
        } => match kind {
            StandardFbKind::Ton | StandardFbKind::Tof | StandardFbKind::Tp => {
                vec!["Q".to_string(), "ET".to_string()]
            }
            StandardFbKind::Ctu | StandardFbKind::Ctd => {
                vec!["Q".to_string(), "CV".to_string()]
            }
            StandardFbKind::Ctud => {
                vec!["QU".to_string(), "QD".to_string(), "CV".to_string()]
            }
            StandardFbKind::RampReal => vec!["OUT".to_string()],
            StandardFbKind::ScaleLin => vec!["OUT".to_string(), "ERR".to_string()],
            StandardFbKind::P
            | StandardFbKind::Pi
            | StandardFbKind::PidParallel
            | StandardFbKind::PidSeries
            | StandardFbKind::PidMixed => vec!["OUT".to_string(), "ERR".to_string()],
            _ => vec!["Q".to_string()],
        },
        FbdBlockKind::StandardFb { fb_type: None, .. } => vec!["Q".to_string(), "OUT".to_string()],
        FbdBlockKind::Composite { type_name } => composites
            .iter()
            .find(|c| &c.name == type_name)
            .map(|c| c.output_pins.clone())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec!["OUT".to_string()]),
        _ => vec!["OUT".to_string()],
    };

    let mut configured: Vec<String> = blk.outputs.keys().cloned().collect();
    configured.sort();
    for pin in configured {
        if !pins.iter().any(|p| p.eq_ignore_ascii_case(&pin)) {
            pins.push(pin);
        }
    }
    pins
}

fn fbd_block_size(blk: &FbdBlock, composites: &[CompositeFbDef]) -> Size {
    let in_count = fbd_block_input_pins(blk).len();
    let out_count = fbd_block_output_pins(blk, composites).len();
    let rows = in_count.max(out_count).max(1) as f32;
    Size::new(
        FBD_BLOCK_WIDTH,
        FBD_HEADER_HEIGHT + rows * FBD_PIN_ROW_HEIGHT + FBD_BOTTOM_PAD,
    )
}

fn fbd_input_pin_pos(block_pos: Point, row_idx: usize) -> Point {
    Point::new(
        block_pos.x,
        block_pos.y + FBD_HEADER_HEIGHT + 10.0 + row_idx as f32 * FBD_PIN_ROW_HEIGHT,
    )
}

fn fbd_output_pin_pos(block_pos: Point, row_idx: usize) -> Point {
    Point::new(
        block_pos.x + FBD_BLOCK_WIDTH,
        block_pos.y + FBD_HEADER_HEIGHT + 10.0 + row_idx as f32 * FBD_PIN_ROW_HEIGHT,
    )
}

pub fn compute_default_block_positions(program: &FbdProgram) -> HashMap<String, Point> {
    let count = program.blocks.len();
    let mut id_to_idx: HashMap<&str, usize> = HashMap::with_capacity(count);
    for (idx, blk) in program.blocks.iter().enumerate() {
        id_to_idx.insert(blk.id.as_str(), idx);
    }

    let mut depths = vec![0_usize; count];
    for _pass in 0..count {
        let mut changed = false;
        for (idx, blk) in program.blocks.iter().enumerate() {
            for src in blk.inputs.values() {
                if let FbdInputSource::Link { from_block, .. } = src
                    && let Some(&src_idx) = id_to_idx.get(from_block.as_str())
                {
                    let candidate = (depths[src_idx] + 1).min(count);
                    if candidate > depths[idx] {
                        depths[idx] = candidate;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut col_y: HashMap<usize, f32> = HashMap::new();
    let mut positions = HashMap::with_capacity(count);
    for (idx, blk) in program.blocks.iter().enumerate() {
        let col = depths[idx];
        let x = 28.0 + col as f32 * (FBD_BLOCK_WIDTH + 68.0);
        let y = *col_y.get(&col).unwrap_or(&24.0);
        positions.insert(blk.id.clone(), Point::new(x, y));
        let size = fbd_block_size(blk, &program.composites);
        col_y.insert(col, y + size.height + 24.0);
    }
    positions
}

fn point_distance_sq(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn build_bezier_wire_path(from: Point, to: Point) -> Path {
    let ctrl_dx = ((to.x - from.x).abs() * 0.5).max(40.0);
    Path::new(|builder| {
        builder.move_to(from);
        builder.bezier_curve_to(
            Point::new(from.x + ctrl_dx, from.y),
            Point::new(to.x - ctrl_dx, to.y),
            to,
        );
    })
}

pub struct FbdSignalFlowCanvas<'a> {
    pub pou_name: &'a str,
    pub program: &'a FbdProgram,
    pub runtime: Option<&'a PlcRuntime>,
    pub selected_block: Option<usize>,
}

impl<'a> canvas::Program<Message> for FbdSignalFlowCanvas<'a> {
    type State = FbdCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        state.sync_pou(self.pou_name);
        state.ensure_layout(self.program);

        let raw_cursor_pos = match cursor.position_in(bounds) {
            Some(p) => p,
            None => {
                if let canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) =
                    event
                {
                    state.is_panning = false;
                    state.pan_start = None;
                    return (event::Status::Captured, None);
                }
                if let canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) =
                    event
                    && !matches!(state.drag, FbdDragState::None)
                {
                    state.drag = FbdDragState::None;
                    return (event::Status::Captured, None);
                }
                return (event::Status::Ignored, None);
            }
        };

        let zoom = state.effective_zoom();
        let cursor_pos = Point::new(
            (raw_cursor_pos.x - state.pan.x) / zoom,
            (raw_cursor_pos.y - state.pan.y) / zoom,
        );

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                state.is_panning = true;
                state.pan_start = Some(raw_cursor_pos);
                (event::Status::Captured, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                if state.is_panning {
                    state.is_panning = false;
                    state.pan_start = None;
                    state.store_current_viewport(self.pou_name);
                    (event::Status::Captured, None)
                } else {
                    (event::Status::Ignored, None)
                }
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let scroll_y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 30.0,
                };
                if scroll_y.abs() > f32::EPSILON {
                    let old_zoom = state.effective_zoom();
                    let factor = (1.0 + scroll_y * 0.12).clamp(0.5, 2.0);
                    let new_zoom = (old_zoom * factor).clamp(FBD_MIN_ZOOM, FBD_MAX_ZOOM);
                    let world_x = (raw_cursor_pos.x - state.pan.x) / old_zoom;
                    let world_y = (raw_cursor_pos.y - state.pan.y) / old_zoom;
                    state.zoom = new_zoom;
                    state.pan = Vector::new(
                        raw_cursor_pos.x - world_x * new_zoom,
                        raw_cursor_pos.y - world_y * new_zoom,
                    );
                    state.store_current_viewport(self.pou_name);
                    return (event::Status::Captured, None);
                }
                (event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let hit_r_sq = FBD_PIN_HIT_RADIUS * FBD_PIN_HIT_RADIUS;
                // 1. Hit-test output pins first to allow wiring
                for blk in self.program.blocks.iter().rev() {
                    if let Some(&pos) = state.block_positions.get(&blk.id) {
                        let out_pins = fbd_block_output_pins(blk, &self.program.composites);
                        for (row_idx, pin_name) in out_pins.iter().enumerate() {
                            let pin_pt = fbd_output_pin_pos(pos, row_idx);
                            if point_distance_sq(cursor_pos, pin_pt) <= hit_r_sq {
                                state.drag = FbdDragState::Wiring {
                                    from_block: blk.id.clone(),
                                    from_pin: pin_name.clone(),
                                    start_pt: pin_pt,
                                    current_pt: cursor_pos,
                                };
                                return (event::Status::Captured, None);
                            }
                        }
                    }
                }

                // 2. Hit-test block bodies for selection & dragging
                for blk in self.program.blocks.iter().rev() {
                    if let Some(&pos) = state.block_positions.get(&blk.id) {
                        let size = fbd_block_size(blk, &self.program.composites);
                        let rect = Rectangle::new(pos, size);
                        if rect.contains(cursor_pos) {
                            state.selected_block_id = Some(blk.id.clone());
                            state.drag = FbdDragState::MovingBlock {
                                block_id: blk.id.clone(),
                                grab_offset: Vector::new(cursor_pos.x - pos.x, cursor_pos.y - pos.y),
                            };
                            return (event::Status::Captured, None);
                        }
                    }
                }

                state.selected_block_id = None;
                (event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.is_panning
                    && let Some(start) = state.pan_start
                {
                    let dx = raw_cursor_pos.x - start.x;
                    let dy = raw_cursor_pos.y - start.y;
                    state.pan = Vector::new(state.pan.x + dx, state.pan.y + dy);
                    state.pan_start = Some(raw_cursor_pos);
                    state.store_current_viewport(self.pou_name);
                    return (event::Status::Captured, None);
                }

                match &mut state.drag {
                    FbdDragState::MovingBlock {
                        block_id,
                        grab_offset,
                    } => {
                        let snapped_x = ((cursor_pos.x - grab_offset.x) / 10.0).round() * 10.0;
                        let snapped_y = ((cursor_pos.y - grab_offset.y) / 10.0).round() * 10.0;
                        state
                            .block_positions
                            .insert(block_id.clone(), Point::new(snapped_x, snapped_y));
                        (event::Status::Captured, None)
                    }
                    FbdDragState::Wiring { current_pt, .. } => {
                        *current_pt = cursor_pos;
                        (event::Status::Captured, None)
                    }
                    FbdDragState::None => (event::Status::Ignored, None),
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let prev_drag = std::mem::replace(&mut state.drag, FbdDragState::None);
                match prev_drag {
                    FbdDragState::MovingBlock { .. } => (event::Status::Captured, None),
                    FbdDragState::Wiring {
                        from_block,
                        from_pin,
                        ..
                    } => {
                        let hit_r_sq = FBD_PIN_HIT_RADIUS * FBD_PIN_HIT_RADIUS;
                        for blk in self.program.blocks.iter() {
                            if blk.id == from_block {
                                continue;
                            }
                            if let Some(&pos) = state.block_positions.get(&blk.id) {
                                let in_pins = fbd_block_input_pins(blk);
                                for (row_idx, in_pin) in in_pins.iter().enumerate() {
                                    let pin_pt = fbd_input_pin_pos(pos, row_idx);
                                    if point_distance_sq(cursor_pos, pin_pt) <= hit_r_sq {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::FbdInputSourceChanged(
                                                self.pou_name.to_string(),
                                                blk.id.clone(),
                                                in_pin.clone(),
                                                format!("{from_block}.{from_pin}"),
                                            )),
                                        );
                                    }
                                }
                            }
                        }
                        (event::Status::Captured, None)
                    }
                    FbdDragState::None => (event::Status::Ignored, None),
                }
            }
            _ => (event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), StudioTheme::bg_base(theme));

        let (pan, zoom) = state.viewport_for(self.pou_name);
        let is_dark = StudioTheme::is_dark(theme);
        let dot_color = if is_dark {
            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.08)
        };
        let dot_spacing = (FBD_GRID_SIZE * zoom).max(4.0);
        let offset_x = pan.x.rem_euclid(dot_spacing);
        let offset_y = pan.y.rem_euclid(dot_spacing);
        let mut gx = offset_x;
        while gx < bounds.width {
            let mut gy = offset_y;
            while gy < bounds.height {
                frame.fill_rectangle(Point::new(gx, gy), Size::new(1.5, 1.5), dot_color);
                gy += dot_spacing;
            }
            gx += dot_spacing;
        }

        frame.translate(pan);
        frame.scale(zoom);

        let layout = state.resolved_layout(self.program);

        // Pass 1: Signal-flow Bezier wires and live values
        for blk in &self.program.blocks {
            let Some(&dst_pos) = layout.get(&blk.id) else {
                continue;
            };
            let in_pins = fbd_block_input_pins(blk);
            for (in_idx, in_pin) in in_pins.iter().enumerate() {
                let Some(src) = blk.inputs.get(in_pin) else {
                    continue;
                };
                if let FbdInputSource::Link {
                    from_block,
                    from_pin,
                } = src
                    && let Some(&src_pos) = layout.get(from_block)
                    && let Some(src_blk) = self.program.blocks.iter().find(|b| b.id == *from_block)
                {
                    let out_pins = fbd_block_output_pins(src_blk, &self.program.composites);
                    let out_idx = out_pins
                        .iter()
                        .position(|p| p.eq_ignore_ascii_case(from_pin))
                        .unwrap_or(0);
                    let p1 = fbd_output_pin_pos(src_pos, out_idx);
                    let p2 = fbd_input_pin_pos(dst_pos, in_idx);

                    let energized = self.runtime.is_some_and(|rt| is_input_energized(rt, src));
                    let wire_color = if energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::ACCENT_CYAN
                    };
                    let wire_stroke = Stroke::default()
                        .with_color(wire_color)
                        .with_width(if energized { 2.4 } else { 1.6 });
                    frame.stroke(&build_bezier_wire_path(p1, p2), wire_stroke);

                    if let Some(rt) = self.runtime
                        && let Some(live_txt) = read_input_live_value(rt, src)
                    {
                        let mid = Point::new((p1.x + p2.x) * 0.5, (p1.y + p2.y) * 0.5);
                        frame.fill_text(canvas::Text {
                            content: live_txt,
                            position: Point::new(mid.x - 16.0, mid.y - 12.0),
                            color: wire_color,
                            size: iced::Pixels(9.5),
                            font: iced::Font::MONOSPACE,
                            ..Default::default()
                        });
                    }
                }
            }
        }

        if let FbdDragState::Wiring {
            start_pt,
            current_pt,
            ..
        } = state.drag
        {
            frame.stroke(
                &build_bezier_wire_path(start_pt, current_pt),
                Stroke::default()
                    .with_color(StudioTheme::ACCENT_AMBER)
                    .with_width(2.0),
            );
        }

        // Pass 2: 2D Function Blocks with live pin indicators
        let block_bg = if is_dark {
            Color::from_rgb(0.085, 0.100, 0.135)
        } else {
            Color::from_rgb(0.96, 0.97, 0.99)
        };
        let header_text_color = Color::WHITE;
        let pin_text_color = if is_dark {
            Color::from_rgb(0.82, 0.86, 0.92)
        } else {
            Color::from_rgb(0.15, 0.18, 0.24)
        };

        for blk in self.program.blocks.iter() {
            let Some(&pos) = layout.get(&blk.id) else {
                continue;
            };
            let size = fbd_block_size(blk, &self.program.composites);
            let cat = block_category(&blk.kind);
            let accent = cat.accent_color();
            let (op_sym, _) = operator_symbol(&blk.kind);
            let is_selected =
                state.selected_block_id.as_deref() == Some(blk.id.as_str())
                    || self.selected_block.is_some_and(|idx| self.program.blocks.get(idx).is_some_and(|b| b.id == blk.id));

            frame.fill_rectangle(pos, size, block_bg);
            frame.fill_rectangle(
                pos,
                Size::new(size.width, FBD_HEADER_HEIGHT),
                Color::from_rgba(accent.r, accent.g, accent.b, 0.88),
            );
            frame.stroke_rectangle(
                pos,
                size,
                Stroke::default()
                    .with_color(if is_selected {
                        StudioTheme::ACCENT_AMBER
                    } else {
                        accent
                    })
                    .with_width(if is_selected { 2.0 } else { 1.1 }),
            );

            frame.fill_text(canvas::Text {
                content: format!("{}  [{}]", blk.id, op_sym),
                position: Point::new(pos.x + 8.0, pos.y + 6.0),
                color: header_text_color,
                size: iced::Pixels(11.0),
                font: iced::Font::MONOSPACE,
                ..Default::default()
            });

            let in_pins = fbd_block_input_pins(blk);
            for (row_idx, in_pin) in in_pins.iter().enumerate() {
                let p_pt = fbd_input_pin_pos(pos, row_idx);
                let src_opt = blk.inputs.get(in_pin);
                let energized = self.runtime.is_some_and(|rt| {
                    src_opt.is_some_and(|s| is_input_energized(rt, s))
                });
                let pin_color = if energized {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    StudioTheme::ACCENT_CYAN
                };
                frame.fill(&Path::circle(p_pt, 4.5), pin_color);

                let tag_hint = match src_opt {
                    Some(FbdInputSource::Value { expr }) => format!("{in_pin}: {expr}"),
                    _ => in_pin.clone(),
                };
                frame.fill_text(canvas::Text {
                    content: tag_hint,
                    position: Point::new(pos.x + 8.0, p_pt.y - 6.0),
                    color: pin_text_color,
                    size: iced::Pixels(10.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            let out_pins = fbd_block_output_pins(blk, &self.program.composites);
            for (row_idx, out_pin) in out_pins.iter().enumerate() {
                let p_pt = fbd_output_pin_pos(pos, row_idx);
                let target_opt = blk.outputs.get(out_pin);
                let energized = self.runtime.is_some_and(|rt| {
                    target_opt.is_some_and(|t| is_output_energized(rt, t))
                });
                let pin_color = if energized {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    accent
                };
                frame.fill(&Path::circle(p_pt, 4.5), pin_color);

                let label = if let Some(target) = target_opt {
                    format!("{target} :{out_pin}")
                } else {
                    out_pin.clone()
                };
                frame.fill_text(canvas::Text {
                    content: label,
                    position: Point::new(pos.x + size.width - 62.0, p_pt.y - 6.0),
                    color: pin_text_color,
                    size: iced::Pixels(10.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.is_panning || matches!(state.drag, FbdDragState::MovingBlock { .. }) {
            mouse::Interaction::Grabbing
        } else if matches!(state.drag, FbdDragState::Wiring { .. }) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

pub fn render_fbd_editor<'a>(
    program: &'a FbdProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
    selected_block: Option<usize>,
    drag_hover_block: Option<usize>,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Header Toolbar with Quick Standard Blocks & Corrector Suite
    let header_row = row![
        row![
            container(
                text("FBD")
                    .size(10.0)
                    .font(iced::Font::MONOSPACE)
                    .color(StudioTheme::ACCENT_CYAN),
            )
            .padding([3, 7])
            .style(|_| container::Style {
                background: Some(Background::Color(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.12))),
                border: Border {
                    color: iced::Color::from_rgba(0.220, 0.741, 0.973, 0.35),
                    width: 0.5,
                    radius: Radius::from(4.0),
                },
                ..Default::default()
            }),
            text(&program.name)
                .size(15)
                .font(iced::Font::MONOSPACE)
                .color(StudioTheme::TEXT_PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        button(
            row![
                text(crate::icons::icon_book())
                    .size(11)
                    .shaping(iced::widget::text::Shaping::Advanced),
                text(lang.tr("Library...", "Bibliothèque...")).size(11.5),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(Message::OpenFbdLibraryWindow)
        .style(StudioTheme::secondary_button)
        .padding([6, 14]),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([6, 12]),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let scrollable_header = scrollable(
        container(header_row).padding(Padding {
            top: 2.0,
            right: 4.0,
            bottom: 6.0,
            left: 2.0,
        }),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new()
            .width(4)
            .scroller_width(4)
            .margin(1),
    ))
    .width(Length::Fill);

    let header_container = container(scrollable_header)
        .width(Length::Fill)
        .padding([4, 6])
        .style(|theme: &Theme| container::Style {
            background: Some(Background::Color(StudioTheme::bg_panel(theme))),
            border: Border {
                color: StudioTheme::border_hairline(theme),
                width: 1.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        });

    // Info badge replaces the old always-visible tips bar: hover reveals the tip.
    let top_bar = row![
        header_container,
        crate::icons::info_tip(lang.tr(
            "FBD Modular Instrument Rack: Double-column schematic with high-speed signal flow. Indicators glow green when energized in real time.",
            "Rack modulaire FBD : Schéma double colonne avec bus de signaux rapides. Les voyants s'illuminent en vert lors de l'exécution en direct.",
        )),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let signal_flow_canvas = container(
        Canvas::new(FbdSignalFlowCanvas {
            pou_name: &program.name,
            program,
            runtime,
            selected_block,
        })
        .width(Length::Fill)
        .height(Length::Fixed(280.0)),
    )
    .width(Length::Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(StudioTheme::bg_base(theme))),
        border: Border {
            color: StudioTheme::border_hairline(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    });

    let mut blocks_column = column![].spacing(14);

    // Render blocks in 2-column grid layout
    let chunk_size = 2;
    for (row_idx, chunk) in program.blocks.chunks(chunk_size).enumerate() {
        let b0_idx = row_idx * chunk_size;
        let card0 = render_single_block_card(&pou_name, b0_idx, &chunk[0], runtime, lang, selected_block, drag_hover_block);

        let grid_row = if chunk.len() > 1 {
            let b1_idx = b0_idx + 1;
            let card1 = render_single_block_card(&pou_name, b1_idx, &chunk[1], runtime, lang, selected_block, drag_hover_block);
            row![card0, card1].spacing(12).width(Length::Fill)
        } else {
            let empty_placeholder = iced::widget::Space::with_width(Length::FillPortion(1));
            row![card0, empty_placeholder]
                .spacing(12)
                .width(Length::Fill)
        };

        blocks_column = blocks_column.push(grid_row);
    }

    let scrollable_blocks = scrollable(blocks_column.padding(Padding {
        top: 8.0,
        right: 12.0,
        bottom: 20.0,
        left: 12.0,
    }))
    .width(Length::Fill)
    .height(Length::Fill);

    column![top_bar, signal_flow_canvas, scrollable_blocks]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;
    use std::collections::HashMap;

    #[test]
    fn test_delete_fbd_block_rewires_links() {
        let mut program = FbdProgram {
            name: "test_fbd".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![
                FbdBlock {
                    id: "A".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                    grid_pos: None,
                },
                FbdBlock {
                    id: "B".to_string(),
                    kind: FbdBlockKind::Or,
                    inputs: [(
                        "IN1".to_string(),
                        FbdInputSource::Link {
                            from_block: "A".to_string(),
                            from_pin: "OUT".to_string(),
                        },
                    )]
                    .into_iter()
                    .collect(),
                    outputs: HashMap::new(),
                    grid_pos: None,
                },
            ],
        };

        delete_fbd_block(&mut program, 0);
        assert_eq!(program.blocks.len(), 1);
        assert_eq!(program.blocks[0].id, "B");
        assert!(matches!(
            program.blocks[0].inputs.get("IN1"),
            Some(FbdInputSource::Value { expr }) if expr == "FALSE"
        ));
        // Out-of-range index is a safe no-op
        delete_fbd_block(&mut program, 5);
        assert_eq!(program.blocks.len(), 1);
    }

    #[test]
    fn test_fbd_editor_toolbar_renders_with_library_button() {
        let program = FbdProgram {
            name: "test_prog".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: Vec::new(),
        };
        let _view = render_fbd_editor(&program, None, UiLanguage::En, None, None);
    }

    #[test]
    fn test_fbd_block_drag_and_drop_reorder() {
        let block_a = FbdBlock {
            id: "BlockA".to_string(),
            kind: FbdBlockKind::And,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            grid_pos: None,
        };
        let block_b = FbdBlock {
            id: "BlockB".to_string(),
            kind: FbdBlockKind::Or,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            grid_pos: None,
        };
        let program = FbdProgram {
            name: "test_drag_fbd".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![block_a, block_b],
        };

        // Renders with selection and drag-hover correctly
        let _view = render_fbd_editor(&program, None, UiLanguage::En, Some(0), Some(1));
    }

    #[test]
    fn test_fbd_signal_flow_canvas_pan_zoom_drag_and_pin_wiring() {
        let mut blk_a_in = HashMap::new();
        blk_a_in.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "TRUE".to_string(),
            },
        );
        let mut blk_b_in = HashMap::new();
        blk_b_in.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "FALSE".to_string(),
            },
        );
        let mut blk_b_in2 = blk_b_in;
        blk_b_in2.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "TRUE".to_string(),
            },
        );

        let program = FbdProgram {
            name: "flow_pou".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![
                FbdBlock {
                    id: "U1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: blk_a_in,
                    outputs: HashMap::new(),
                    grid_pos: None,
                },
                FbdBlock {
                    id: "U2".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: blk_b_in2,
                    outputs: HashMap::new(),
                    grid_pos: None,
                },
            ],
        };

        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        // 1. Middle-mouse panning works and shifts pan
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 100.0)),
        );
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(140.0, 125.0),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(140.0, 125.0)),
        );
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)),
            bounds,
            mouse::Cursor::Available(Point::new(140.0, 125.0)),
        );
        assert_eq!(state.pan, Vector::new(40.0, 25.0));

        // Reset pan to simplify pin hit-testing
        state.pan = Vector::default();
        state.zoom = 1.0;

        // 2. Output pin of U1 -> input pin IN1 of U2 wiring emits FbdInputSourceChanged
        let u1_pos = state
            .block_positions
            .get("U1")
            .copied()
            .unwrap_or(Point::ORIGIN);
        let u2_pos = state
            .block_positions
            .get("U2")
            .copied()
            .unwrap_or(Point::ORIGIN);
        let u1_out = fbd_output_pin_pos(u1_pos, 0);
        let u2_in1 = fbd_input_pin_pos(u2_pos, 0);

        let (st, _) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(u1_out),
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(state.drag, FbdDragState::Wiring { .. }));

        let (st, msg) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(u2_in1),
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::FbdInputSourceChanged(
                ref p,
                ref target_id,
                ref pin,
                ref val
            )) if p == "flow_pou" && target_id == "U2" && pin == "IN1" && val == "U1.OUT"
        ));

        // 3. Left-dragging U1 block updates its 2D canvas coordinates
        let u1_body = Point::new(u1_pos.x + 20.0, u1_pos.y + 10.0);
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(u1_body),
        );
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(u1_body.x + 50.0, u1_body.y + 30.0),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(u1_body.x + 50.0, u1_body.y + 30.0)),
        );
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(u1_body.x + 50.0, u1_body.y + 30.0)),
        );
        let moved_pos = state
            .block_positions
            .get("U1")
            .copied()
            .unwrap_or(Point::ORIGIN);
        assert!(moved_pos.x > u1_pos.x);
        assert!(moved_pos.y > u1_pos.y);

        // 4. Verify mouse_interaction, topological layout positions, and Bezier wire path builder
        assert_eq!(
            canvas.mouse_interaction(&state, bounds, mouse::Cursor::Unavailable),
            mouse::Interaction::default()
        );
        let default_layout = compute_default_block_positions(&program);
        assert!(default_layout.contains_key("U1"));
        assert!(default_layout.contains_key("U2"));
        let _wire_path = build_bezier_wire_path(u1_out, u2_in1);
    }
}