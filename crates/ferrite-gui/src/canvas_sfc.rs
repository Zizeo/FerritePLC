use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};

use ferrite_core::IecValue;
use ferrite_lang_sfc::{ActionQualifier, SfcProgram, StepKind};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfcStepKindWrapper(pub StepKind);

impl std::fmt::Display for SfcStepKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            StepKind::Initial => write!(f, "Initial"),
            StepKind::Normal => write!(f, "Normal"),
            StepKind::Macro => write!(f, "Macro"),
        }
    }
}

pub const ALL_STEP_KINDS: [SfcStepKindWrapper; 3] = [
    SfcStepKindWrapper(StepKind::Initial),
    SfcStepKindWrapper(StepKind::Normal),
    SfcStepKindWrapper(StepKind::Macro),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfcActionQualifierWrapper(pub ActionQualifier);

impl std::fmt::Display for SfcActionQualifierWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ActionQualifier::N => write!(f, "N"),
            ActionQualifier::S => write!(f, "S"),
            ActionQualifier::R => write!(f, "R"),
            ActionQualifier::L => write!(f, "L"),
            ActionQualifier::D => write!(f, "D"),
            ActionQualifier::P => write!(f, "P"),
            ActionQualifier::F => write!(f, "F"),
            ActionQualifier::FZ => write!(f, "FZ"),
        }
    }
}

pub const ALL_ACTION_QUALIFIERS: [SfcActionQualifierWrapper; 8] = [
    SfcActionQualifierWrapper(ActionQualifier::N),
    SfcActionQualifierWrapper(ActionQualifier::S),
    SfcActionQualifierWrapper(ActionQualifier::R),
    SfcActionQualifierWrapper(ActionQualifier::L),
    SfcActionQualifierWrapper(ActionQualifier::D),
    SfcActionQualifierWrapper(ActionQualifier::P),
    SfcActionQualifierWrapper(ActionQualifier::F),
    SfcActionQualifierWrapper(ActionQualifier::FZ),
];

fn is_step_active(rt: &PlcRuntime, pou_name: &str, step_id: &str) -> (bool, u64) {
    let tag1 = format!("{step_id}.state");
    let tag2 = format!("{pou_name}.{step_id}.state");
    let t_tag1 = format!("{step_id}.t");
    let t_tag2 = format!("{pou_name}.{step_id}.t");

    let is_on = match rt.read_value(&tag1).or_else(|_| rt.read_value(&tag2)) {
        Ok(IecValue::Bool(b)) => b,
        _ => false,
    };

    let elapsed_ms = match rt.read_value(&t_tag1).or_else(|_| rt.read_value(&t_tag2)) {
        Ok(IecValue::Time(ms)) => ms,
        _ => 0,
    };

    (is_on, elapsed_ms)
}

fn evaluate_receptivity(rt: &PlcRuntime, pou_name: &str, receptivity: &str) -> Option<bool> {
    let expr = receptivity.trim();
    if expr.is_empty() || expr == "1" || expr.eq_ignore_ascii_case("true") {
        return Some(true);
    }
    if expr == "0" || expr.eq_ignore_ascii_case("false") {
        return Some(false);
    }
    if let Ok(IecValue::Bool(b)) = rt.read_value(expr) {
        return Some(b);
    }
    let scoped = format!("{pou_name}.{expr}");
    if let Ok(IecValue::Bool(b)) = rt.read_value(&scoped) {
        return Some(b);
    }
    None
}

/// Renders the authentic Grafcet step box per IEC 60848:
/// - Initial step: Double square (square inside a square)
/// - Normal step: Single square
/// - Macro step: Square with double vertical side borders
fn render_grafcet_step_box<'a>(
    step_id: &str,
    kind: StepKind,
    active: bool,
) -> Element<'a, Message> {
    let border_color = if active {
        StudioTheme::ACCENT_EMERALD
    } else {
        match kind {
            StepKind::Initial => StudioTheme::ACCENT_CYAN,
            _ => StudioTheme::BORDER_CARD,
        }
    };

    let bg_color = if active {
        iced::Color::from_rgba(0.063, 0.725, 0.506, 0.22)
    } else {
        StudioTheme::BG_BASE
    };

    let text_color = if active {
        StudioTheme::ACCENT_EMERALD
    } else {
        StudioTheme::TEXT_PRIMARY
    };

    let step_label = text(step_id.to_string())
        .size(17)
        .color(text_color);

    match kind {
        StepKind::Initial => {
            // IEC 60848 Initial Step: Authentic Double Square!
            container(
                container(step_label)
                    .width(Length::Fixed(46.0))
                    .height(Length::Fixed(46.0))
                    .center_x(Length::Fixed(46.0))
                    .center_y(Length::Fixed(46.0))
                    .style(move |_theme: &Theme| container::Style {
                        background: Some(Background::Color(bg_color)),
                        border: Border {
                            color: border_color,
                            width: 1.5,
                            radius: Radius::from(2.0),
                        },
                        ..Default::default()
                    }),
            )
            .width(Length::Fixed(58.0))
            .height(Length::Fixed(58.0))
            .center_x(Length::Fixed(58.0))
            .center_y(Length::Fixed(58.0))
            .padding(4)
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::BG_ELEVATED)),
                border: Border {
                    color: border_color,
                    width: 2.0,
                    radius: Radius::from(4.0),
                },
                ..Default::default()
            })
            .into()
        }
        StepKind::Normal => {
            // IEC 60848 Normal Step: Single Square!
            container(step_label)
                .width(Length::Fixed(58.0))
                .height(Length::Fixed(58.0))
                .center_x(Length::Fixed(58.0))
                .center_y(Length::Fixed(58.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: if active { 2.0 } else { 1.5 },
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                })
                .into()
        }
        StepKind::Macro => {
            // IEC 60848 Macro Step: Double vertical side borders!
            container(step_label)
                .width(Length::Fixed(58.0))
                .height(Length::Fixed(58.0))
                .center_x(Length::Fixed(58.0))
                .center_y(Length::Fixed(58.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(bg_color)),
                    border: Border {
                        color: border_color,
                        width: 3.0,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                })
                .into()
        }
    }
}

pub fn render_sfc_editor<'a>(
    program: &'a SfcProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Header Toolbar
    let header = row![
        text(format!(
            "{} — {}",
            lang.tr("Sequential Function Chart (Grafcet)", "Grafcet (IEC 60848)"),
            program.name
        ))
        .size(18),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Step", "+ Ajouter Étape")).size(12))
            .on_press(Message::SfcAddStep(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([6, 12]),
        button(text(lang.tr("+ Add Transition", "+ Ajouter Transition")).size(12))
            .on_press(Message::SfcAddTransition(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([6, 12]),
        button(text(lang.tr("Clear POU", "Vider le POU")).size(12))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([6, 12]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Standard IEC 60848 Grafcet: Initial steps have double squares. Actions are attached on the right. Transitions are horizontal crossbars on the sequence link.",
        "Grafcet standard CEI 60848 : Les étapes initiales ont un double carré. Les actions sont reliées à droite. Les transitions barrent la liaison de séquence.",
    ))
    .size(12)
    .color(StudioTheme::TEXT_MUTED);

    let mut sequence_column = column![header, tip].spacing(16);

    // 2. Render Vertical Grafcet Flow
    let mut grafcet_flow = column![].spacing(0);

    for (s_idx, step) in program.steps.iter().enumerate() {
        let p_step = pou_name.clone();
        let (active, elapsed_ms) = runtime
            .map(|rt| is_step_active(rt, &program.name, &step.id))
            .unwrap_or((false, 0));

        // Active Token Indicator
        let active_indicator: Element<'a, Message> = if active {
            container(
                row![
                    container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                        .height(Length::Fixed(8.0))
                        .style(|_theme: &Theme| container::Style {
                            background: Some(Background::Color(StudioTheme::ACCENT_EMERALD)),
                            border: Border {
                                radius: Radius::from(4.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    text(format!("t = {elapsed_ms}ms"))
                        .size(11)
                        .color(StudioTheme::ACCENT_EMERALD),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([2, 6])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)))
            .into()
        } else {
            container(
                text(format!("X{}.state = 0", step.id))
                    .size(10)
                    .color(StudioTheme::TEXT_MUTED),
            )
            .padding([2, 6])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.03)))
            .into()
        };

        let step_box = render_grafcet_step_box(&step.id, step.kind, active);

        // Step Identification Block
        let step_ident_col = column![
            active_indicator,
            step_box,
            row![
                text_input("Name", &step.name)
                    .on_input({
                        let p = p_step.clone();
                        move |s| Message::SfcStepNameChanged(p.clone(), s_idx, s)
                    })
                    .width(130)
                    .padding([2, 6])
                    .style(StudioTheme::styled_input),
                pick_list(
                    &ALL_STEP_KINDS[..],
                    Some(SfcStepKindWrapper(step.kind)),
                    {
                        let p = p_step.clone();
                        move |w| Message::SfcStepKindChanged(p.clone(), s_idx, w.0)
                    },
                )
                .width(85)
                .style(StudioTheme::styled_pick_list),
                button(text("X").size(11))
                    .on_press(Message::SfcRemoveStep(p_step.clone(), s_idx))
                    .style(StudioTheme::ghost_button)
                    .padding([2, 5]),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        ]
        .spacing(4)
        .align_x(Alignment::Center);

        // Right-attached Actions (IEC 60848)
        let step_to_actions_wire = container(iced::widget::Space::with_width(Length::Fixed(24.0)))
            .height(Length::Fixed(2.5))
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(if active {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    StudioTheme::BORDER_CARD
                })),
                ..Default::default()
            });

        let mut actions_col = column![].spacing(6);
        for (a_idx, act) in step.actions.iter().enumerate() {
            let p_act = p_step.clone();
            let action_box = container(
                row![
                    // Left Qualifier Compartment
                    pick_list(
                        &ALL_ACTION_QUALIFIERS[..],
                        Some(SfcActionQualifierWrapper(act.qualifier)),
                        {
                            let p = p_act.clone();
                            move |w| Message::SfcActionQualifierChanged(p.clone(), s_idx, a_idx, w.0)
                        },
                    )
                    .width(55)
                    .style(StudioTheme::styled_pick_list),
                    // Vertical internal separator
                    container(iced::widget::Space::with_width(Length::Fixed(1.5)))
                        .height(Length::Fixed(24.0))
                        .style(|_theme: &Theme| container::Style {
                            background: Some(Background::Color(StudioTheme::BORDER_CARD)),
                            ..Default::default()
                        }),
                    // Right Target Compartment
                    text_input("Target / Statement", &act.target)
                        .on_input({
                            let p = p_act.clone();
                            move |s| Message::SfcActionTargetChanged(p.clone(), s_idx, a_idx, s)
                        })
                        .width(220)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                    button(text("X").size(11))
                        .on_press(Message::SfcRemoveAction(p_act.clone(), s_idx, a_idx))
                        .style(StudioTheme::ghost_button)
                        .padding([2, 5]),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding(4)
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::BG_ELEVATED)),
                border: Border {
                    color: StudioTheme::BORDER_CARD,
                    width: 1.0,
                    radius: Radius::from(4.0),
                },
                ..Default::default()
            });

            actions_col = actions_col.push(action_box);
        }

        let actions_assembly = row![
            actions_col,
            button(text(lang.tr("+ Action", "+ Action")).size(11))
                .on_press(Message::SfcAddAction(p_step.clone(), s_idx))
                .style(StudioTheme::secondary_button)
                .padding([3, 8]),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let step_row = row![
            step_ident_col,
            step_to_actions_wire,
            actions_assembly,
        ]
        .spacing(0)
        .align_y(Alignment::Center);

        let step_card = container(step_row)
            .padding([8, 16])
            .style(StudioTheme::card_style)
            .width(Length::Fill);

        grafcet_flow = grafcet_flow.push(step_card);

        // Find transitions that branch from this step
        let outgoing_transitions: Vec<(usize, &ferrite_lang_sfc::SfcTransition)> = program
            .transitions
            .iter()
            .enumerate()
            .filter(|(_, t)| t.from_steps.iter().any(|s| s == &step.id || s == &step.name))
            .collect();

        for (t_idx, trans) in outgoing_transitions {
            let p_trans = pou_name.clone();
            let is_receptive = runtime.and_then(|rt| evaluate_receptivity(rt, &program.name, &trans.receptivity));

            let from_str = trans.from_steps.join(", ");
            let to_str = trans.to_steps.join(", ");

            // Vertical wire leading into transition crossbar
            let pre_wire = container(iced::widget::Space::with_height(Length::Fixed(16.0)))
                .width(Length::Fixed(2.5))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(if active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::BORDER_CARD
                    })),
                    ..Default::default()
                });

            // The iconic Grafcet Transition Crossbar!
            let crossbar = container(iced::widget::Space::with_width(Length::Fixed(48.0)))
                .height(Length::Fixed(4.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(if is_receptive == Some(true) {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::ACCENT_CYAN
                    })),
                    border: Border {
                        radius: Radius::from(1.0),
                        ..Default::default()
                    },
                    ..Default::default()
                });

            let trans_row = row![
                // Left side: Transition ID + Source Steps + Delete
                row![
                    container(text(&trans.id).size(12).color(StudioTheme::ACCENT_AMBER))
                        .padding([3, 8])
                        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.961, 0.620, 0.043, 0.15))),
                    text(lang.tr("From:", "De :")).size(11).color(StudioTheme::TEXT_MUTED),
                    text_input("0", &from_str)
                        .on_input({
                            let p = p_trans.clone();
                            move |s| Message::SfcTransitionFromChanged(p.clone(), t_idx, s)
                        })
                        .width(70)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                    button(text("X").size(11))
                        .on_press(Message::SfcRemoveTransition(p_trans.clone(), t_idx))
                        .style(StudioTheme::ghost_button)
                        .padding([2, 5]),
                ]
                .spacing(6)
                .align_y(Alignment::Center),

                // Transition Crossbar
                container(crossbar).padding([0, 10]),

                // Right side: Horizontal line to Receptivity Condition & Target
                row![
                    text(lang.tr("Receptivity:", "Réceptivité :")).size(11).color(StudioTheme::TEXT_MUTED),
                    text_input("Condition (e.g. Start_PB)", &trans.receptivity)
                        .on_input({
                            let p = p_trans.clone();
                            move |s| Message::SfcTransitionReceptivityChanged(p.clone(), t_idx, s)
                        })
                        .width(220)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                    // Live Evaluation Badge
                    if let Some(true) = is_receptive {
                        container(text("TRUE").size(10).color(StudioTheme::ACCENT_EMERALD))
                            .padding([2, 6])
                            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.063, 0.725, 0.506, 0.2)))
                    } else if let Some(false) = is_receptive {
                        container(text("FALSE").size(10).color(StudioTheme::TEXT_MUTED))
                            .padding([2, 6])
                            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05)))
                    } else {
                        container(text("EVAL").size(10).color(StudioTheme::ACCENT_CYAN))
                            .padding([2, 6])
                            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.1)))
                    },
                    text("==>").size(12).color(StudioTheme::ACCENT_CYAN),
                    text(lang.tr("To:", "Vers :")).size(11).color(StudioTheme::TEXT_MUTED),
                    text_input("1, 2", &to_str)
                        .on_input({
                            let p = p_trans.clone();
                            move |s| Message::SfcTransitionToChanged(p.clone(), t_idx, s)
                        })
                        .width(70)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(0)
            .align_y(Alignment::Center);

            let trans_card = container(
                column![
                    container(pre_wire).padding([0, 36]),
                    container(trans_row)
                        .padding([6, 12])
                        .style(StudioTheme::canvas_container_style)
                        .width(Length::Fill),
                    container(
                        column![
                            container(iced::widget::Space::with_height(Length::Fixed(12.0)))
                                .width(Length::Fixed(2.5))
                                .style(|_theme: &Theme| container::Style {
                                    background: Some(Background::Color(StudioTheme::BORDER_CARD)),
                                    ..Default::default()
                                }),
                            text("v").size(12).color(StudioTheme::BORDER_CARD),
                        ]
                        .align_x(Alignment::Center),
                    )
                    .padding([0, 36]),
                ]
                .spacing(2),
            )
            .padding([2, 12])
            .width(Length::Fill);

            grafcet_flow = grafcet_flow.push(trans_card);
        }
    }

    // 3. Any transitions not covered by steps (e.g. unlinked or loopback)
    let unlinked_transitions: Vec<(usize, &ferrite_lang_sfc::SfcTransition)> = program
        .transitions
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            !t.from_steps
                .iter()
                .any(|fs| program.steps.iter().any(|s| &s.id == fs || &s.name == fs))
        })
        .collect();

    if !unlinked_transitions.is_empty() {
        let unlinked_header = text(lang.tr("Additional / Free Transitions", "Transitions Libres / Additionnelles")).size(13);
        let mut unlinked_col = column![unlinked_header].spacing(8);

        for (t_idx, trans) in unlinked_transitions {
            let p_trans = pou_name.clone();
            let from_str = trans.from_steps.join(", ");
            let to_str = trans.to_steps.join(", ");

            let row_item = row![
                container(text(&trans.id).size(12).color(StudioTheme::ACCENT_AMBER))
                    .padding([3, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.961, 0.620, 0.043, 0.15))),
                text(lang.tr("From:", "De :")).size(11).color(StudioTheme::TEXT_MUTED),
                text_input("0", &from_str)
                    .on_input({
                        let p = p_trans.clone();
                        move |s| Message::SfcTransitionFromChanged(p.clone(), t_idx, s)
                    })
                    .width(70)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                text("==|==").size(12).color(StudioTheme::ACCENT_CYAN),
                text(lang.tr("Receptivity:", "Réceptivité :")).size(11).color(StudioTheme::TEXT_MUTED),
                text_input("Receptivity", &trans.receptivity)
                    .on_input({
                        let p = p_trans.clone();
                        move |s| Message::SfcTransitionReceptivityChanged(p.clone(), t_idx, s)
                    })
                    .width(220)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                text("==>").size(12).color(StudioTheme::ACCENT_CYAN),
                text(lang.tr("To:", "Vers :")).size(11).color(StudioTheme::TEXT_MUTED),
                text_input("1", &to_str)
                    .on_input({
                        let p = p_trans.clone();
                        move |s| Message::SfcTransitionToChanged(p.clone(), t_idx, s)
                    })
                    .width(70)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                button(text("X").size(11))
                    .on_press(Message::SfcRemoveTransition(p_trans.clone(), t_idx))
                    .style(StudioTheme::ghost_button)
                    .padding([2, 5]),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            unlinked_col = unlinked_col.push(
                container(row_item)
                    .padding(8)
                    .style(StudioTheme::card_style)
                    .width(Length::Fill),
            );
        }

        grafcet_flow = grafcet_flow.push(container(unlinked_col).padding(10));
    }

    sequence_column = sequence_column.push(grafcet_flow);

    scrollable(sequence_column.padding(20))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
