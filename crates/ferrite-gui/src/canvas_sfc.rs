use std::collections::HashSet;

use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};

use ferrite_core::IecValue;
use ferrite_lang_sfc::{ActionQualifier, SfcProgram, SfcTransition, StepKind};
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

/// Computes visual rows of step indices so that parallel branches created by AND Divergence
/// (`to_steps.len() > 1`) or OR Divergence (multiple transitions sharing `from_steps`) render side-by-side.
pub fn compute_sfc_step_rows(program: &SfcProgram) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut placed: HashSet<usize> = HashSet::new();

    let mut idx = 0;
    while idx < program.steps.len() {
        if placed.contains(&idx) {
            idx += 1;
            continue;
        }
        let step_id = &program.steps[idx].id;
        let step_name = &program.steps[idx].name;
        let mut sibling_row: Vec<usize> = Vec::new();

        // Case A: Single transition with multiple `to_steps` (AND divergence) containing this step
        if let Some(and_tr) = program.transitions.iter().find(|t| {
            t.to_steps.len() > 1
                && (t.to_steps.contains(step_id) || t.to_steps.iter().any(|s| s == step_name))
        }) {
            for tid in &and_tr.to_steps {
                if let Some(s_pos) = program.steps.iter().position(|s| &s.id == tid || &s.name == tid)
                    && !placed.contains(&s_pos)
                    && !sibling_row.contains(&s_pos)
                {
                    sibling_row.push(s_pos);
                }
            }
        }

        // Case B: Multiple transitions sharing the same `from_steps` (OR divergence) targeting this step
        if sibling_row.len() <= 1
            && let Some(parent_tr) = program.transitions.iter().find(|t| {
                (t.to_steps.contains(step_id) || t.to_steps.iter().any(|s| s == step_name))
                    && !t.from_steps.is_empty()
            })
        {
            let shared_from = &parent_tr.from_steps;
            let or_siblings: Vec<&SfcTransition> = program
                .transitions
                .iter()
                .filter(|t| &t.from_steps == shared_from)
                .collect();
            if or_siblings.len() > 1 {
                sibling_row.clear();
                for otr in or_siblings {
                    for tid in &otr.to_steps {
                        if let Some(s_pos) = program.steps.iter().position(|s| &s.id == tid || &s.name == tid)
                            && !placed.contains(&s_pos)
                            && !sibling_row.contains(&s_pos)
                        {
                            sibling_row.push(s_pos);
                        }
                    }
                }
            }
        }

        if sibling_row.is_empty() {
            sibling_row.push(idx);
        }
        for &s_pos in &sibling_row {
            placed.insert(s_pos);
        }
        rows.push(sibling_row);
        idx += 1;
    }

    rows
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

/// Renders the IEC 60848 Double Horizontal Line (`===`) for AND Divergence / Convergence.
fn double_horizontal_bar<'a>(label: &str, is_active: bool) -> Element<'a, Message> {
    let line_color = if is_active {
        StudioTheme::ACCENT_EMERALD
    } else {
        StudioTheme::ACCENT_CYAN
    };

    let bar_line = move || {
        container(iced::widget::Space::with_width(Length::Fill))
            .height(Length::Fixed(2.5))
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(line_color)),
                ..Default::default()
            })
    };

    container(
        column![
            bar_line(),
            row![
                iced::widget::Space::with_width(Length::Fill),
                container(text(label.to_string()).size(10).color(line_color))
                    .padding([2, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.12))),
                iced::widget::Space::with_width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            bar_line(),
        ]
        .spacing(2),
    )
    .padding([2, 16])
    .width(Length::Fill)
    .into()
}

/// Renders the IEC 60848 Single Horizontal Line (`---`) for OR Divergence / Convergence.
fn single_horizontal_bar<'a>(label: &str, is_active: bool) -> Element<'a, Message> {
    let line_color = if is_active {
        StudioTheme::ACCENT_EMERALD
    } else {
        StudioTheme::ACCENT_PURPLE
    };

    container(
        column![
            row![
                iced::widget::Space::with_width(Length::Fill),
                container(text(label.to_string()).size(10).color(line_color))
                    .padding([2, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.75, 0.5, 0.9, 0.12))),
                iced::widget::Space::with_width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            container(iced::widget::Space::with_width(Length::Fill))
                .height(Length::Fixed(2.5))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(line_color)),
                    ..Default::default()
                }),
        ]
        .spacing(2),
    )
    .padding([2, 16])
    .width(Length::Fill)
    .into()
}

/// Renders a single step and its attached actions.
fn render_single_step_card<'a>(
    program: &'a SfcProgram,
    s_idx: usize,
    runtime: Option<&'a PlcRuntime>,
    _lang: UiLanguage,
) -> Element<'a, Message> {
    let step = &program.steps[s_idx];
    let p_step = program.name.clone();

    let (active, elapsed_ms) = runtime
        .map(|rt| is_step_active(rt, &program.name, &step.id))
        .unwrap_or((false, 0));

    // Active Indicator Badge
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

    let step_header_bar = row![
        container(
            text(format!("X{}", step.id))
                .size(11.5)
                .color(if active { StudioTheme::ACCENT_EMERALD } else { StudioTheme::ACCENT_CYAN })
        )
        .padding([2, 5])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.12))),
        active_indicator,
        iced::widget::Space::with_width(Length::Fill),
        button(text("+Act").size(10.5))
            .on_press(Message::SfcAddAction(p_step.clone(), s_idx))
            .style(StudioTheme::secondary_button)
            .padding([2, 5]),
        button(text("x").size(10.5))
            .on_press(Message::SfcRemoveStep(p_step.clone(), s_idx))
            .style(StudioTheme::ghost_button)
            .padding([1, 4]),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let step_bottom_bar = row![
        text_input("Name", &step.name)
            .on_input({
                let p = p_step.clone();
                move |s| Message::SfcStepNameChanged(p.clone(), s_idx, s)
            })
            .width(110)
            .padding([2, 5])
            .style(StudioTheme::styled_input),
        pick_list(
            &ALL_STEP_KINDS[..],
            Some(SfcStepKindWrapper(step.kind)),
            {
                let p = p_step.clone();
                move |w| Message::SfcStepKindChanged(p.clone(), s_idx, w.0)
            },
        )
        .width(72)
        .style(StudioTheme::styled_pick_list),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let step_ident_col = column![
        step_header_bar,
        step_box,
        step_bottom_bar,
    ]
    .spacing(5)
    .align_x(Alignment::Center);

    // Right-attached Actions (IEC 60848)
    let step_to_actions_wire = container(iced::widget::Space::with_width(Length::Fixed(20.0)))
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
                    .width(180)
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

    let (wire_elem, acts_elem): (Element<'a, Message>, Element<'a, Message>) = if step.actions.is_empty() {
        (
            iced::widget::Space::with_width(Length::Fixed(0.0)).into(),
            iced::widget::Space::with_width(Length::Fixed(0.0)).into(),
        )
    } else {
        (
            step_to_actions_wire.into(),
            row![
                actions_col,
                button(text("+").size(11))
                    .on_press(Message::SfcAddAction(p_step, s_idx))
                    .style(StudioTheme::secondary_button)
                    .padding([3, 7]),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .into(),
        )
    };

    container(
        row![
            step_ident_col,
            wire_elem,
            acts_elem,
        ]
        .spacing(0)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .style(StudioTheme::card_style)
    .into()
}

/// Renders a single Grafcet Transition with its crossbar, receptivity, and live evaluation pill.
fn render_transition_row<'a>(
    program: &'a SfcProgram,
    t_idx: usize,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let trans = &program.transitions[t_idx];
    let p_trans = program.name.clone();

    let is_receptive = runtime.and_then(|rt| evaluate_receptivity(rt, &program.name, &trans.receptivity));
    let from_str = trans.from_steps.join(", ");
    let to_str = trans.to_steps.join(", ");

    // Vertical directed link entering crossbar
    let pre_wire = container(iced::widget::Space::with_height(Length::Fixed(14.0)))
        .width(Length::Fixed(2.5))
        .style(|_theme: &Theme| container::Style {
            background: Some(Background::Color(StudioTheme::ACCENT_CYAN)),
            ..Default::default()
        });

    // The iconic Grafcet Transition Crossbar!
    let crossbar = container(iced::widget::Space::with_width(Length::Fixed(56.0)))
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
                .width(65)
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
                .width(200)
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
                    let p = p_trans;
                    move |s| Message::SfcTransitionToChanged(p.clone(), t_idx, s)
                })
                .width(65)
                .padding([3, 6])
                .style(StudioTheme::styled_input),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(0)
    .align_y(Alignment::Center);

    container(
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
    .padding([2, 8])
    .width(Length::Fill)
    .into()
}

pub fn render_sfc_editor<'a>(
    program: &'a SfcProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Header Toolbar with Quick Topology Builders
    let header = row![
        text(format!(
            "{} - {}",
            lang.tr("Sequential Function Chart (Grafcet IEC 60848)", "Grafcet (CEI 60848)"),
            program.name
        ))
        .size(18),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Step", "+ Étape")).size(11))
            .on_press(Message::SfcAddStep(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([5, 10]),
        button(text(lang.tr("+ Add Transition", "+ Transition")).size(11))
            .on_press(Message::SfcAddTransition(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([5, 10]),
        button(text(lang.tr("+ AND Div (==)", "+ Div ET (==)")).size(11))
            .on_press(Message::SfcAddAndDivergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([5, 10]),
        button(text(lang.tr("+ AND Conv (==)", "+ Conv ET (==)")).size(11))
            .on_press(Message::SfcAddAndConvergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([5, 10]),
        button(text(lang.tr("+ OR Div (--)", "+ Div OU (--)")).size(11))
            .on_press(Message::SfcAddOrDivergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([5, 10]),
        button(text(lang.tr("+ OR Conv (--)", "+ Conv OU (--)")).size(11))
            .on_press(Message::SfcAddOrConvergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([5, 10]),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name))
            .style(StudioTheme::danger_button)
            .padding([5, 10]),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Standard IEC 60848 Grafcet: Initial steps have double squares. Actions are attached on the right. Transitions are horizontal crossbars on the sequence link. AND/OR divergences & convergences render parallel branches.",
        "Grafcet standard CEI 60848 : Double carré pour étapes initiales. Actions reliées à droite. Transitions en barres horizontales. Divergences/convergences ET/OU en branches parallèles.",
    ))
    .size(12)
    .color(StudioTheme::TEXT_MUTED);

    let mut sequence_column = column![header, tip].spacing(16);

    // 2. Compute 2D Step Rows (Grouping AND/OR parallel branches)
    let step_rows = compute_sfc_step_rows(program);
    let mut grafcet_flow = column![].spacing(6);

    let mut rendered_transitions: HashSet<usize> = HashSet::new();

    for (r_idx, row_indices) in step_rows.iter().enumerate() {
        // A. Pre-row: Check if any transition converges into ALL steps in this row (AND Divergence leading into this row)
        // If this row has multiple steps coming from an AND Divergence transition:
        let first_step_id = &program.steps[row_indices[0]].id;
        let and_div_trans: Option<(usize, &SfcTransition)> = program
            .transitions
            .iter()
            .enumerate()
            .find(|(_, t)| t.to_steps.len() > 1 && t.to_steps.contains(first_step_id));

        if let Some((t_idx, _tr)) = and_div_trans
            && !rendered_transitions.contains(&t_idx)
        {
            rendered_transitions.insert(t_idx);
            grafcet_flow = grafcet_flow.push(render_transition_row(program, t_idx, runtime, lang));
            grafcet_flow = grafcet_flow.push(double_horizontal_bar("AND DIVERGENCE (ET) [===]", false));
        }

        // B. Render Steps in this Row: Side-by-side if multiple branches!
        if row_indices.len() == 1 {
            let s_idx = row_indices[0];
            let card = render_single_step_card(program, s_idx, runtime, lang);
            grafcet_flow = grafcet_flow.push(card);
        } else {
            let mut branch_row = row![].spacing(16).align_y(Alignment::Start);
            for &s_idx in row_indices {
                let card = render_single_step_card(program, s_idx, runtime, lang);
                branch_row = branch_row.push(container(card).width(Length::FillPortion(1)));
            }
            grafcet_flow = grafcet_flow.push(branch_row);
        }

        // C. Post-row: Check for AND Convergence (transition with from_steps containing all steps in this row)
        let and_conv_trans: Option<(usize, &SfcTransition)> = program
            .transitions
            .iter()
            .enumerate()
            .find(|(_, t)| {
                t.from_steps.len() > 1
                    && row_indices
                        .iter()
                        .all(|&si| t.from_steps.contains(&program.steps[si].id) || t.from_steps.contains(&program.steps[si].name))
            });

        if let Some((t_idx, _tr)) = and_conv_trans {
            if !rendered_transitions.contains(&t_idx) {
                rendered_transitions.insert(t_idx);
                grafcet_flow = grafcet_flow.push(double_horizontal_bar("AND CONVERGENCE (ET) [===]", false));
                grafcet_flow = grafcet_flow.push(render_transition_row(program, t_idx, runtime, lang));
            }
            continue;
        }

        // D. Outgoing transitions from steps in this row
        let outgoing: Vec<(usize, &SfcTransition)> = program
            .transitions
            .iter()
            .enumerate()
            .filter(|(ti, t)| {
                !rendered_transitions.contains(ti)
                    && row_indices.iter().any(|&si| {
                        t.from_steps.contains(&program.steps[si].id)
                            || t.from_steps.contains(&program.steps[si].name)
                    })
            })
            .collect();

        if outgoing.len() > 1 && row_indices.len() == 1 {
            // OR Divergence!
            grafcet_flow = grafcet_flow.push(single_horizontal_bar("OR DIVERGENCE (OU) [---]", false));
            let mut or_div_row = row![].spacing(12).align_y(Alignment::Center);
            for (ti, _) in &outgoing {
                rendered_transitions.insert(*ti);
                or_div_row = or_div_row.push(
                    container(render_transition_row(program, *ti, runtime, lang))
                        .width(Length::FillPortion(1)),
                );
            }
            grafcet_flow = grafcet_flow.push(or_div_row);
        } else {
            for (ti, _) in outgoing {
                rendered_transitions.insert(ti);
                grafcet_flow = grafcet_flow.push(render_transition_row(program, ti, runtime, lang));
            }
        }

        // E. Check if next row is an OR Convergence target (multiple transitions targeted next row)
        if r_idx + 1 < step_rows.len() {
            let next_step_ids: Vec<&String> = step_rows[r_idx + 1]
                .iter()
                .map(|&si| &program.steps[si].id)
                .collect();
            let incoming_count = program
                .transitions
                .iter()
                .filter(|t| t.to_steps.iter().any(|target| next_step_ids.contains(&target)))
                .count();
            if incoming_count > 1 && row_indices.len() > 1 {
                grafcet_flow = grafcet_flow.push(single_horizontal_bar("OR CONVERGENCE (OU) [---]", false));
            }
        }
    }

    // 3. Render any remaining transitions (e.g. unlinked or loopbacks)
    let remaining: Vec<(usize, &SfcTransition)> = program
        .transitions
        .iter()
        .enumerate()
        .filter(|(ti, _)| !rendered_transitions.contains(ti))
        .collect();

    if !remaining.is_empty() {
        let unlinked_header = text(lang.tr("Loopback & Free Transitions", "Transitions de Bouclage & Libres")).size(13);
        let mut unlinked_col = column![unlinked_header].spacing(8);

        for (ti, _) in remaining {
            unlinked_col = unlinked_col.push(render_transition_row(program, ti, runtime, lang));
        }

        grafcet_flow = grafcet_flow.push(container(unlinked_col).padding(10));
    }

    sequence_column = sequence_column.push(grafcet_flow);

    scrollable(sequence_column.padding(20))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
