use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};

use ferrite_core::IecValue;
use ferrite_lang_ld::{CoilKind, ContactKind, LdElement, LdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub const CONTACT_CARD_WIDTH: f32 = 108.0;
pub const COIL_CARD_WIDTH: f32 = 108.0;
pub const WIRE_GAP: f32 = 10.0;
pub const RAIL_HEIGHT: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LdContactKindWrapper(pub ContactKind);

impl std::fmt::Display for LdContactKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ContactKind::NormallyOpen => write!(f, "NO"),
            ContactKind::NormallyClosed => write!(f, "NC"),
            ContactKind::RisingEdge => write!(f, "P"),
            ContactKind::FallingEdge => write!(f, "N"),
        }
    }
}

pub const ALL_CONTACT_KINDS: [LdContactKindWrapper; 4] = [
    LdContactKindWrapper(ContactKind::NormallyOpen),
    LdContactKindWrapper(ContactKind::NormallyClosed),
    LdContactKindWrapper(ContactKind::RisingEdge),
    LdContactKindWrapper(ContactKind::FallingEdge),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LdCoilKindWrapper(pub CoilKind);

impl std::fmt::Display for LdCoilKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            CoilKind::Normal => write!(f, "( )"),
            CoilKind::Set => write!(f, "[S]"),
            CoilKind::Reset => write!(f, "[R]"),
        }
    }
}

pub const ALL_COIL_KINDS: [LdCoilKindWrapper; 3] = [
    LdCoilKindWrapper(CoilKind::Normal),
    LdCoilKindWrapper(CoilKind::Set),
    LdCoilKindWrapper(CoilKind::Reset),
];

fn is_symbol_energized(rt: &PlcRuntime, symbol: &str) -> bool {
    matches!(rt.read_value(symbol), Ok(IecValue::Bool(true)))
}

fn wire_segment<'a>(active: bool, width: f32) -> Element<'a, Message> {
    let color = if active {
        StudioTheme::ACCENT_EMERALD
    } else {
        StudioTheme::BORDER_CARD
    };
    container(iced::widget::Space::with_width(Length::Fixed(width)))
        .height(Length::Fixed(2.5))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

pub fn render_ld_editor<'a>(
    program: &'a LdProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Toolbar Header
    let header = row![
        text(format!(
            "{} — {}",
            lang.tr("Ladder Diagram", "Diagramme à Contacts (Ladder)"),
            program.name
        ))
        .size(17),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Rung", "+ Réseau")).size(11))
            .on_press(Message::LdAddRung(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([5, 10]),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([5, 10]),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Tip: Left power rail is energized. Compact contact and coil cards fit on screen with power flow indicator.",
        "Astuce : Rail d'alimentation actif. Cartes compactes pour s'adapter à l'écran avec flux de puissance direct.",
    ))
    .size(11.5)
    .color(StudioTheme::TEXT_MUTED);

    // 2. Rungs List
    let mut rungs_column = column![header, tip].spacing(14);

    for (r_idx, rung) in program.rungs.iter().enumerate() {
        let p_name = pou_name.clone();

        // Rung Header: Rung ID + Comment + Quick buttons
        let rung_header = row![
            container(text(&rung.id).size(11.5).color(StudioTheme::ACCENT_CYAN))
                .padding([2, 6])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                    0.220, 0.741, 0.973, 0.12
                ))),
            text_input("Comment...", &rung.comment,)
                .on_input({
                    let p = p_name.clone();
                    move |s| Message::LdRungCommentChanged(p.clone(), r_idx, s)
                })
                .width(Length::Fill)
                .padding([3, 7])
                .style(StudioTheme::styled_input),
            button(text("+ NO").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::NormallyOpen
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ NC").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::NormallyClosed
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ |P|").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::RisingEdge
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ |N|").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::FallingEdge
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ ( )").size(10.5))
                .on_press(Message::LdAddCoilWithKind(
                    p_name.clone(),
                    r_idx,
                    CoilKind::Normal
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text(lang.tr("Del", "Suppr")).size(10.5))
                .on_press(Message::LdRemoveRung(p_name.clone(), r_idx))
                .style(StudioTheme::danger_button)
                .padding([3, 6]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        // Power Rails & Circuit
        let left_rail = container(iced::widget::Space::with_width(Length::Fixed(4.0)))
            .height(Length::Fixed(RAIL_HEIGHT))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::ACCENT_EMERALD)),
                border: Border {
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        let right_rail = container(iced::widget::Space::with_width(Length::Fixed(4.0)))
            .height(Length::Fixed(RAIL_HEIGHT))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::BORDER_CARD)),
                border: Border {
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        let mut power_flowing = true;
        let mut circuit_row = row![left_rail, wire_segment(true, WIRE_GAP),]
            .spacing(0)
            .align_y(Alignment::Center);

        // Contacts and parallel branches in this rung
        for (e_idx, elem) in rung.elements.iter().enumerate() {
            let p_elem = p_name.clone();
            match elem {
                LdElement::Contact { symbol, kind } => {
                    let contact_closed = runtime.is_some_and(|rt| {
                        let energized = is_symbol_energized(rt, symbol);
                        match kind {
                            ContactKind::NormallyOpen => energized,
                            ContactKind::NormallyClosed => !energized,
                            ContactKind::RisingEdge | ContactKind::FallingEdge => energized,
                        }
                    });

                    let is_active = power_flowing && contact_closed;
                    power_flowing = is_active;

                    let border_color = if is_active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::BORDER_CARD
                    };
                    let bg_color = if is_active {
                        iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)
                    } else {
                        StudioTheme::BG_BASE
                    };

                    let glyph_str = match kind {
                        ContactKind::NormallyOpen => "--| |--",
                        ContactKind::NormallyClosed => "--|/|--",
                        ContactKind::RisingEdge => "--|P|--",
                        ContactKind::FallingEdge => "--|N|--",
                    };

                    // Compact 2-level stacked contact card
                    let top_row = row![
                        text_input("Tag", symbol)
                            .on_input({
                                let p = p_elem.clone();
                                move |s| Message::LdContactSymbolChanged(p.clone(), r_idx, e_idx, s)
                            })
                            .width(72)
                            .padding([1, 4])
                            .size(10.5)
                            .style(StudioTheme::styled_input),
                        iced::widget::Space::with_width(Length::Fill),
                        button(text("x").size(9.5))
                            .on_press(Message::LdRemoveContact(p_elem.clone(), r_idx, e_idx))
                            .style(StudioTheme::ghost_button)
                            .padding([0, 3]),
                    ]
                    .align_y(Alignment::Center);

                    let bottom_row = row![
                        text(glyph_str)
                            .size(11.5)
                            .font(iced::Font::MONOSPACE)
                            .color(if is_active {
                                StudioTheme::ACCENT_EMERALD
                            } else {
                                StudioTheme::TEXT_MUTED
                            }),
                        iced::widget::Space::with_width(Length::Fill),
                        pick_list(&ALL_CONTACT_KINDS[..], Some(LdContactKindWrapper(*kind)), {
                            let p = p_elem.clone();
                            move |w| Message::LdContactKindChanged(p.clone(), r_idx, e_idx, w.0)
                        },)
                        .width(44)
                        .style(StudioTheme::styled_pick_list),
                    ]
                    .align_y(Alignment::Center);

                    let contact_card = container(column![top_row, bottom_row].spacing(2))
                        .width(CONTACT_CARD_WIDTH)
                        .padding(4)
                        .style(move |_theme: &Theme| container::Style {
                            background: Some(Background::Color(bg_color)),
                            border: Border {
                                color: border_color,
                                width: if is_active { 1.5 } else { 1.0 },
                                radius: Radius::from(4.0),
                            },
                            ..Default::default()
                        });

                    circuit_row = circuit_row.push(contact_card);
                    circuit_row = circuit_row.push(wire_segment(power_flowing, WIRE_GAP));
                }
                LdElement::Branch { paths } => {
                    let branch_closed = runtime.is_some_and(|rt| {
                        paths.iter().any(|path| {
                            path.iter().all(|sub_elem| match sub_elem {
                                LdElement::Contact { symbol, kind } => {
                                    let energized = is_symbol_energized(rt, symbol);
                                    match kind {
                                        ContactKind::NormallyOpen | ContactKind::RisingEdge => {
                                            energized
                                        }
                                        ContactKind::NormallyClosed | ContactKind::FallingEdge => {
                                            !energized
                                        }
                                    }
                                }
                                _ => true,
                            })
                        })
                    });
                    let is_active = power_flowing && branch_closed;
                    power_flowing = is_active;
                    let branch_box = container(text("[OR Branch]").size(11).color(if is_active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::ACCENT_PURPLE
                    }))
                    .padding([4, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                        0.75, 0.5, 0.9, 0.12,
                    )));

                    circuit_row = circuit_row.push(branch_box);
                    circuit_row = circuit_row.push(wire_segment(power_flowing, WIRE_GAP));
                }
                LdElement::InlineFb { instance, .. } => {
                    let fb_q = runtime
                        .is_some_and(|rt| rt.tags.get_fb(instance).map(|fb| fb.q).unwrap_or(false));
                    let is_active = power_flowing && fb_q;
                    power_flowing = is_active;
                    let fb_box = container(text(format!("[FB: {instance}]")).size(11).color(
                        if is_active {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            StudioTheme::ACCENT_CYAN
                        },
                    ))
                    .padding([4, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                        0.22, 0.74, 0.97, 0.12,
                    )));

                    circuit_row = circuit_row.push(fb_box);
                    circuit_row = circuit_row.push(wire_segment(power_flowing, WIRE_GAP));
                }
            }
        }

        // Spanning expanding wire bus across to coils so coils snap to the right power rail
        let span_color = if power_flowing {
            StudioTheme::ACCENT_EMERALD
        } else {
            StudioTheme::BORDER_CARD
        };
        let span_wire = container(iced::widget::Space::with_width(Length::Fill))
            .height(Length::Fixed(2.5))
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(span_color)),
                ..Default::default()
            });
        circuit_row = circuit_row.push(span_wire);

        // Coils in this rung
        for (c_idx, coil) in rung.coils.iter().enumerate() {
            let p_coil = p_name.clone();
            let is_coil_on =
                power_flowing || runtime.is_some_and(|rt| is_symbol_energized(rt, &coil.symbol));

            let coil_border = if is_coil_on {
                StudioTheme::ACCENT_EMERALD
            } else {
                StudioTheme::BORDER_CARD
            };
            let coil_bg = if is_coil_on {
                iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)
            } else {
                StudioTheme::BG_BASE
            };

            let glyph_str = match coil.kind {
                CoilKind::Normal => "--( )--",
                CoilKind::Set => "--[S]--",
                CoilKind::Reset => "--[R]--",
            };

            // Compact 2-level stacked coil card
            let top_row = row![
                text_input("Coil", &coil.symbol)
                    .on_input({
                        let p = p_coil.clone();
                        move |s| Message::LdCoilSymbolChanged(p.clone(), r_idx, c_idx, s)
                    })
                    .width(72)
                    .padding([1, 4])
                    .size(10.5)
                    .style(StudioTheme::styled_input),
                iced::widget::Space::with_width(Length::Fill),
                button(text("x").size(9.5))
                    .on_press(Message::LdRemoveCoil(p_coil.clone(), r_idx, c_idx))
                    .style(StudioTheme::ghost_button)
                    .padding([0, 3]),
            ]
            .align_y(Alignment::Center);

            let bottom_row = row![
                text(glyph_str)
                    .size(11.5)
                    .font(iced::Font::MONOSPACE)
                    .color(if is_coil_on {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::TEXT_MUTED
                    }),
                iced::widget::Space::with_width(Length::Fill),
                pick_list(&ALL_COIL_KINDS[..], Some(LdCoilKindWrapper(coil.kind)), {
                    let p = p_coil.clone();
                    move |w| Message::LdCoilKindChanged(p.clone(), r_idx, c_idx, w.0)
                },)
                .width(44)
                .style(StudioTheme::styled_pick_list),
            ]
            .align_y(Alignment::Center);

            let coil_card = container(column![top_row, bottom_row].spacing(2))
                .width(COIL_CARD_WIDTH)
                .padding(4)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(coil_bg)),
                    border: Border {
                        color: coil_border,
                        width: if is_coil_on { 1.5 } else { 1.0 },
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                });

            circuit_row = circuit_row.push(wire_segment(is_coil_on, WIRE_GAP));
            circuit_row = circuit_row.push(coil_card);
        }

        // Right Rail Return
        circuit_row = circuit_row.push(wire_segment(true, WIRE_GAP));
        circuit_row = circuit_row.push(right_rail);

        let rung_card = container(
            column![
                rung_header,
                container(circuit_row)
                    .padding([8, 12])
                    .style(StudioTheme::canvas_container_style)
                    .width(Length::Fill),
            ]
            .spacing(8),
        )
        .padding(10)
        .style(StudioTheme::card_style)
        .width(Length::Fill);

        rungs_column = rungs_column.push(rung_card);
    }

    scrollable(rungs_column.padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
