use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};

use ferrite_core::IecValue;
use ferrite_lang_ld::{CoilKind, ContactKind, LdElement, LdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LdContactKindWrapper(pub ContactKind);

impl std::fmt::Display for LdContactKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ContactKind::NormallyOpen => write!(f, "NO [ ]"),
            ContactKind::NormallyClosed => write!(f, "NC [/]"),
            ContactKind::RisingEdge => write!(f, "P [P]"),
            ContactKind::FallingEdge => write!(f, "N [N]"),
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
            CoilKind::Normal => write!(f, "Coil ( )"),
            CoilKind::Set => write!(f, "Set [S]"),
            CoilKind::Reset => write!(f, "Reset [R]"),
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
        .size(18),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Rung", "+ Ajouter Réseau")).size(12))
            .on_press(Message::LdAddRung(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([6, 12]),
        button(text(lang.tr("Clear POU", "Vider le POU")).size(12))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([6, 12]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Tip: Power rails highlight green when energized in real-time. Edit contacts and coils directly inline.",
        "Astuce : Les rails s'allument en vert lorsqu'ils sont sous tension en direct. Modifiez directement les contacts et bobines.",
    ))
    .size(12)
    .color(StudioTheme::TEXT_MUTED);

    // 2. Rungs List
    let mut rungs_column = column![header, tip].spacing(16);

    for (r_idx, rung) in program.rungs.iter().enumerate() {
        let p_name = pou_name.clone();

        // Rung Header: Rung ID + Comment + "+ Contact" + "+ Coil" + "Delete Rung"
        let rung_header = row![
            container(
                text(&rung.id).size(12).color(StudioTheme::ACCENT_CYAN)
            )
            .padding([3, 8])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.12))),
            text_input(
                "Comment...",
                &rung.comment,
            )
            .on_input({
                let p = p_name.clone();
                move |s| Message::LdRungCommentChanged(p.clone(), r_idx, s)
            })
            .width(Length::Fill)
            .padding([4, 8])
            .style(StudioTheme::styled_input),
            button(text(lang.tr("+ Contact", "+ Contact")).size(11))
                .on_press(Message::LdAddContact(p_name.clone(), r_idx))
                .style(StudioTheme::secondary_button)
                .padding([4, 8]),
            button(text(lang.tr("+ Coil", "+ Bobine")).size(11))
                .on_press(Message::LdAddCoil(p_name.clone(), r_idx))
                .style(StudioTheme::secondary_button)
                .padding([4, 8]),
            button(text(lang.tr("Delete Rung", "Suppr Réseau")).size(11))
                .on_press(Message::LdRemoveRung(p_name.clone(), r_idx))
                .style(StudioTheme::danger_button)
                .padding([4, 8]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Power Flow & Circuit
        let left_rail = container(iced::widget::Space::with_width(Length::Fixed(4.0)))
            .height(Length::Fixed(48.0))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::ACCENT_CYAN)),
                border: Border {
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        let right_rail = container(iced::widget::Space::with_width(Length::Fixed(4.0)))
            .height(Length::Fixed(48.0))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::ACCENT_CYAN)),
                border: Border {
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        let mut power_flowing = true;
        let mut circuit_row = row![
            left_rail,
            wire_segment(true, 16.0),
        ]
        .spacing(0)
        .align_y(Alignment::Center);

        // Contacts in this rung
        for (e_idx, elem) in rung.elements.iter().enumerate() {
            let p_elem = p_name.clone();
            if let LdElement::Contact { symbol, kind } = elem {
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

                let contact_box = container(
                    row![
                        text(match kind {
                            ContactKind::NormallyOpen => "[ ]",
                            ContactKind::NormallyClosed => "[/]",
                            ContactKind::RisingEdge => "[P]",
                            ContactKind::FallingEdge => "[N]",
                        })
                        .size(13)
                        .color(if is_active { StudioTheme::ACCENT_EMERALD } else { StudioTheme::TEXT_MUTED }),
                        text_input("Tag_Name", symbol)
                            .on_input({
                                let p = p_elem.clone();
                                move |s| Message::LdContactSymbolChanged(p.clone(), r_idx, e_idx, s)
                            })
                            .width(110)
                            .padding([3, 6])
                            .style(StudioTheme::styled_input),
                        pick_list(
                            &ALL_CONTACT_KINDS[..],
                            Some(LdContactKindWrapper(*kind)),
                            {
                                let p = p_elem.clone();
                                move |w| Message::LdContactKindChanged(p.clone(), r_idx, e_idx, w.0)
                            },
                        )
                        .width(85)
                        .style(StudioTheme::styled_pick_list),
                        button(text("X").size(11))
                            .on_press(Message::LdRemoveContact(p_elem.clone(), r_idx, e_idx))
                            .style(StudioTheme::ghost_button)
                            .padding([2, 5]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding(6)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: if is_active { 1.5 } else { 1.0 },
                        radius: Radius::from(6.0),
                    },
                    ..Default::default()
                });

                circuit_row = circuit_row.push(contact_box);
                circuit_row = circuit_row.push(wire_segment(power_flowing, 16.0));
            }
        }

        // Spanning wire to right-aligned coils
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
            let is_coil_on = power_flowing || runtime.is_some_and(|rt| is_symbol_energized(rt, &coil.symbol));

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

            let coil_box = container(
                row![
                    text(match coil.kind {
                        CoilKind::Normal => "( )",
                        CoilKind::Set => "[S]",
                        CoilKind::Reset => "[R]",
                    })
                    .size(13)
                    .color(if is_coil_on { StudioTheme::ACCENT_EMERALD } else { StudioTheme::TEXT_MUTED }),
                    text_input("Coil_Tag", &coil.symbol)
                        .on_input({
                            let p = p_coil.clone();
                            move |s| Message::LdCoilSymbolChanged(p.clone(), r_idx, c_idx, s)
                        })
                        .width(110)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                    pick_list(
                        &ALL_COIL_KINDS[..],
                        Some(LdCoilKindWrapper(coil.kind)),
                        {
                            let p = p_coil.clone();
                            move |w| Message::LdCoilKindChanged(p.clone(), r_idx, c_idx, w.0)
                        },
                    )
                    .width(95)
                    .style(StudioTheme::styled_pick_list),
                    button(text("X").size(11))
                        .on_press(Message::LdRemoveCoil(p_coil.clone(), r_idx, c_idx))
                        .style(StudioTheme::ghost_button)
                        .padding([2, 5]),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding(6)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(coil_bg)),
                border: Border {
                    color: coil_border,
                    width: if is_coil_on { 1.5 } else { 1.0 },
                    radius: Radius::from(6.0),
                },
                ..Default::default()
            });

            circuit_row = circuit_row.push(wire_segment(is_coil_on, 16.0));
            circuit_row = circuit_row.push(coil_box);
        }

        // Right Rail
        circuit_row = circuit_row.push(wire_segment(true, 16.0));
        circuit_row = circuit_row.push(right_rail);

        let rung_card = container(
            column![
                rung_header,
                container(circuit_row)
                    .padding([8, 12])
                    .style(StudioTheme::canvas_container_style)
                    .width(Length::Fill),
            ]
            .spacing(10),
        )
        .padding(14)
        .style(StudioTheme::card_style)
        .width(Length::Fill);

        rungs_column = rungs_column.push(rung_card);
    }

    scrollable(rungs_column.padding(20))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
