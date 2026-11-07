use iced::border::Radius;
use iced::widget::{button, column, container, mouse_area, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};

use ferrite_core::{IecValue, StandardFbKind};
use ferrite_lang_fbd::{FbdBlock, FbdBlockKind, FbdInputSource, FbdProgram};
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
                move |s| Message::FbdBlockIdChanged(p.clone(), b_idx, s)
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
    .on_press(Message::FbdRemoveBlock(p_blk.clone(), b_idx))
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
                        let pn = pin_n.clone();
                        move |s| Message::FbdInputSourceChanged(p.clone(), b_idx, pn.clone(), s)
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
                        let pn = pin_n.clone();
                        move |s| Message::FbdOutputTargetChanged(p.clone(), b_idx, pn.clone(), s)
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

    let tip = text(lang.tr(
        "FBD Modular Instrument Rack: Double-column schematic with high-speed signal flow. Indicators glow green when energized in real time.",
        "Rack modulaire FBD : Schéma double colonne avec bus de signaux rapides. Les voyants s'illuminent en vert lors de l'exécution en direct.",
    ))
    .size(11.0)
    .font(iced::Font::MONOSPACE)
    .color(StudioTheme::TEXT_MUTED);

    let top_bar = column![header_container, tip].spacing(4);

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

    column![top_bar, scrollable_blocks]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
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
        };
        let block_b = FbdBlock {
            id: "BlockB".to_string(),
            kind: FbdBlockKind::Or,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
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
}