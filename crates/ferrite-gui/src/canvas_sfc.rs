use std::collections::HashSet;

use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

use ferrite_core::IecValue;
use ferrite_lang_sfc::{ActionQualifier, ConvergenceKind, SfcProgram, SfcTransition, StepKind};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

// Exact color palette matching IEC 60848 industrial Grafcet schematic & target egui design
const COLOR_ACTIVE_STEP: Color = Color::from_rgb(46.0 / 255.0, 213.0 / 255.0, 115.0 / 255.0); // #2ED573 emerald
const COLOR_INACTIVE_STEP: Color = Color::from_rgb(120.0 / 255.0, 135.0 / 255.0, 160.0 / 255.0); // #7887A0 slate
const COLOR_STEP_DROP: Color = Color::from_rgb(72.0 / 255.0, 202.0 / 255.0, 228.0 / 255.0); // #48CAE4 cyan
const COLOR_TRANSITION: Color = Color::from_rgb(1.0, 184.0 / 255.0, 77.0 / 255.0); // #FFB84D amber
const COLOR_OR_BRANCH: Color = Color::from_rgb(198.0 / 255.0, 120.0 / 255.0, 221.0 / 255.0); // #C678DD purple
const COLOR_CANVAS_BG: Color = Color::from_rgb(16.0 / 255.0, 20.0 / 255.0, 26.0 / 255.0); // #10141A
const COLOR_STEP_BG: Color = Color::from_rgb(26.0 / 255.0, 34.0 / 255.0, 46.0 / 255.0); // #1A222E
const COLOR_TRANS_BG: Color = Color::from_rgb(22.0 / 255.0, 28.0 / 255.0, 38.0 / 255.0); // #161C26
const COLOR_ACTION_BG: Color = Color::from_rgb(30.0 / 255.0, 38.0 / 255.0, 52.0 / 255.0); // #1E2634
const COLOR_INPUT_BG: Color = Color::from_rgb(14.0 / 255.0, 20.0 / 255.0, 29.0 / 255.0); // #0E141D

pub const COLUMN_STRIDE: f32 = 560.0;
pub const STEP_BOX_WIDTH: f32 = 188.0;
pub const STEP_BOX_HEIGHT: f32 = 68.0;
pub const TRANSITION_CARD_WIDTH: f32 = 320.0;

/// Returns the exact horizontal center `X` coordinate for a step or transition column
/// on the SFC schematic canvas, given the column index `col_idx`, the total branch columns `cols`,
/// the central sequence spine `base_spine_x`, and the column stride `col_stride`.
pub fn step_center_x(col_idx: usize, cols: usize, base_spine_x: f32, col_stride: f32) -> f32 {
    let row_start_x = base_spine_x - ((cols.saturating_sub(1)) as f32) * col_stride * 0.5;
    row_start_x + (col_idx as f32) * col_stride
}

/// Returns the left margin required to center a step box of width `STEP_BOX_WIDTH`
/// precisely at horizontal coordinate `target_center_x`.
pub fn step_box_left_margin(target_center_x: f32) -> f32 {
    (target_center_x - STEP_BOX_WIDTH * 0.5).max(0.0)
}

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

// Styling helper for dark monospace inputs
fn sfc_input_style(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(COLOR_INPUT_BG),
        border: Border {
            color: Color::from_rgb(44.0 / 255.0, 56.0 / 255.0, 72.0 / 255.0),
            width: 1.0,
            radius: Radius::from(3.0),
        },
        placeholder: StudioTheme::TEXT_MUTED,
        value: Color::from_rgb(226.0 / 255.0, 232.0 / 255.0, 240.0 / 255.0),
        selection: StudioTheme::ACCENT_CYAN,
        icon: StudioTheme::TEXT_SECONDARY,
    }
}

// Subtle button for +Act
fn sfc_subtle_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Color::from_rgb(37.0 / 255.0, 49.0 / 255.0, 65.0 / 255.0),
        button::Status::Pressed => Color::from_rgb(28.0 / 255.0, 38.0 / 255.0, 50.0 / 255.0),
        _ => Color::from_rgb(32.0 / 255.0, 42.0 / 255.0, 56.0 / 255.0),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::from_rgb(200.0 / 255.0, 210.0 / 255.0, 225.0 / 255.0),
        border: Border {
            color: Color::from_rgb(50.0 / 255.0, 65.0 / 255.0, 85.0 / 255.0),
            width: 1.0,
            radius: Radius::from(3.0),
        },
        ..Default::default()
    }
}

/// Renders a vertical line wire of fixed height and color
fn vertical_wire<'a>(height: f32, color: Color) -> Element<'a, Message> {
    container(Space::with_height(Length::Fixed(height)))
        .width(Length::Fixed(2.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

/// Renders the authentic Grafcet step box per IEC 60848:
/// - Fixed dimensions: 188px x 68px
/// - Initial step: Double border (nested inner stroke shrink 3px)
/// - Normal step: Single 2.2px border
/// - Top row: Green dot (if active), centered X<id> in bold white monospace, right +Act button & x delete
/// - Bottom row: Text input for step name with dark background
fn render_step_box<'a>(
    program: &'a SfcProgram,
    s_idx: usize,
    active: bool,
    _elapsed_ms: u64,
    pou_name: &str,
    _lang: UiLanguage,
) -> Element<'a, Message> {
    let step = &program.steps[s_idx];
    let p_name = pou_name.to_string();

    let border_color = if active {
        COLOR_ACTIVE_STEP
    } else {
        COLOR_INACTIVE_STEP
    };

    let active_dot: Element<'a, Message> = if active {
        container(text("●").size(11).color(COLOR_ACTIVE_STEP))
            .padding([0, 4])
            .into()
    } else {
        Space::with_width(Length::Fixed(12.0)).into()
    };

    let step_id_label = text(format!("X{}", step.id))
        .size(13)
        .font(iced::Font::MONOSPACE)
        .color(Color::WHITE);

    let add_act_btn = button(text("+Act").size(10.5))
        .on_press(Message::SfcAddAction(p_name.clone(), s_idx))
        .style(sfc_subtle_button)
        .padding([2, 5]);

    let remove_btn = button(text("x").size(10.5))
        .on_press(Message::SfcRemoveStep(p_name.clone(), s_idx))
        .style(StudioTheme::ghost_button)
        .padding([1, 4]);

    let top_bar = row![
        active_dot,
        Space::with_width(Length::Fill),
        step_id_label,
        Space::with_width(Length::Fill),
        add_act_btn,
        remove_btn,
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let name_input = text_input("Step Name", &step.name)
        .on_input({
            let p = p_name;
            move |s| Message::SfcStepNameChanged(p.clone(), s_idx, s)
        })
        .font(iced::Font::MONOSPACE)
        .size(11)
        .padding([2, 5])
        .style(sfc_input_style)
        .width(Length::Fill);

    let inner_content = column![top_bar, name_input]
        .spacing(4)
        .padding(if step.kind == StepKind::Initial { 3 } else { 6 });

    let step_body: Element<'a, Message> = if step.kind == StepKind::Initial {
        // IEC 60848 Initial Step: Authentic inner border inside the outer card!
        container(inner_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                border: Border {
                    color: border_color,
                    width: 1.2,
                    radius: Radius::from(2.0),
                },
                ..Default::default()
            })
            .into()
    } else {
        inner_content.into()
    };

    container(step_body)
        .width(Length::Fixed(STEP_BOX_WIDTH))
        .height(Length::Fixed(STEP_BOX_HEIGHT))
        .style(move |_| container::Style {
            background: Some(Background::Color(COLOR_STEP_BG)),
            border: Border {
                color: border_color,
                width: 2.2,
                radius: Radius::from(4.0),
            },
            ..Default::default()
        })
        .into()
}

/// Renders the complete Step Node with rigid column-axis anchoring:
/// [ Left Margin / Duration ]  [ Step Box ] === [ Action 1 ]
///                                              [ Action 2 ]
fn render_step_node<'a>(
    program: &'a SfcProgram,
    s_idx: usize,
    runtime: Option<&'a PlcRuntime>,
    pou_name: &str,
    lang: UiLanguage,
    left_margin: f32,
) -> Element<'a, Message> {
    let step = &program.steps[s_idx];
    let (active, elapsed_ms) = runtime
        .map(|rt| is_step_active(rt, &program.name, &step.id))
        .unwrap_or((false, 0));

    let border_color = if active {
        COLOR_ACTIVE_STEP
    } else {
        COLOR_INACTIVE_STEP
    };

    // 1. Elapsed duration on the left, right-aligned within the fixed left margin container
    let elapsed_widget: Element<'a, Message> = if active {
        container(
            text(format!("T#{elapsed_ms}ms"))
                .size(10.5)
                .font(iced::Font::MONOSPACE)
                .color(COLOR_ACTIVE_STEP),
        )
        .padding([0, 8])
        .into()
    } else {
        Space::with_width(Length::Fixed(0.0)).into()
    };

    let left_container = container(elapsed_widget)
        .width(Length::Fixed(left_margin))
        .align_x(iced::alignment::Horizontal::Right);

    // 2. The Step Box (fixed STEP_BOX_WIDTH x STEP_BOX_HEIGHT)
    let step_box = render_step_box(program, s_idx, active, elapsed_ms, pou_name, lang);

    // 3. Right-attached Actions (IEC 60848)
    let (wire_elem, acts_elem): (Element<'a, Message>, Element<'a, Message>) = if step.actions.is_empty() {
        (
            Space::with_width(Length::Fixed(0.0)).into(),
            Space::with_width(Length::Fixed(0.0)).into(),
        )
    } else {
        let wire = container(Space::with_width(Length::Fixed(18.0)))
            .height(Length::Fixed(2.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(border_color)),
                ..Default::default()
            });

        let mut actions_col = column![].spacing(3);
        for (a_idx, act) in step.actions.iter().enumerate() {
            let p_act = pou_name.to_string();

            let qual_picker = pick_list(
                &ALL_ACTION_QUALIFIERS[..],
                Some(SfcActionQualifierWrapper(act.qualifier)),
                {
                    let p = p_act.clone();
                    move |w| Message::SfcActionQualifierChanged(p.clone(), s_idx, a_idx, w.0)
                },
            )
            .width(44)
            .style(StudioTheme::styled_pick_list);

            let duration_input: Option<Element<'a, Message>> =
                if matches!(act.qualifier, ActionQualifier::L | ActionQualifier::D) {
                    let dur_str = act.duration.as_deref().unwrap_or("T#2s");
                    let p = p_act.clone();
                    Some(
                        text_input("T#2s", dur_str)
                            .on_input(move |d| Message::SfcActionDurationChanged(p.clone(), s_idx, a_idx, d))
                            .font(iced::Font::MONOSPACE)
                            .size(10.5)
                            .padding([2, 4])
                            .width(54)
                            .style(sfc_input_style)
                            .into(),
                    )
                } else {
                    None
                };

            let target_input = text_input("Action Target / Statement", &act.target)
                .on_input({
                    let p = p_act.clone();
                    move |t| Message::SfcActionTargetChanged(p.clone(), s_idx, a_idx, t)
                })
                .font(iced::Font::MONOSPACE)
                .size(10.5)
                .padding([2, 5])
                .width(185)
                .style(sfc_input_style);

            let del_btn = button(text("x").size(10.5))
                .on_press(Message::SfcRemoveAction(p_act, s_idx, a_idx))
                .style(StudioTheme::ghost_button)
                .padding([1, 4]);

            let mut act_row = row![qual_picker].spacing(4).align_y(Alignment::Center);
            if let Some(dur) = duration_input {
                act_row = act_row.push(dur);
            }
            act_row = act_row.push(target_input);
            act_row = act_row.push(del_btn);

            let pill = container(act_row)
                .padding([2, 4])
                .style(move |_| container::Style {
                    background: Some(Background::Color(COLOR_ACTION_BG)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                });

            actions_col = actions_col.push(pill);
        }

        (wire.into(), actions_col.into())
    };

    row![
        left_container,
        step_box,
        wire_elem,
        acts_elem,
        Space::with_width(Length::Fill),
    ]
    .spacing(0)
    .align_y(Alignment::Center)
    .into()
}

/// Renders the authentic Grafcet Transition per IEC 60848:
/// - Amber horizontal crossbar (40px x 4px) centered on target_axis_x
/// - Connected on the right via horizontal wire to the Transition Card
/// - Transition Card with amber border, transition header, receptivity condition input & live evaluation
fn render_transition_node<'a>(
    program: &'a SfcProgram,
    t_idx: usize,
    runtime: Option<&'a PlcRuntime>,
    pou_name: &str,
    _lang: UiLanguage,
    target_axis_x: f32,
) -> Element<'a, Message> {
    let trans = &program.transitions[t_idx];
    let p_name = pou_name.to_string();

    let is_receptive = runtime.and_then(|rt| evaluate_receptivity(rt, &program.name, &trans.receptivity));
    let tr_col = if is_receptive == Some(true) {
        COLOR_ACTIVE_STEP
    } else {
        COLOR_TRANSITION
    };

    let crossbar = container(Space::with_width(Length::Fixed(40.0)))
        .height(Length::Fixed(4.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(tr_col)),
            border: Border {
                radius: Radius::from(1.0),
                ..Default::default()
            },
            ..Default::default()
        });

    let wire_to_card = container(Space::with_width(Length::Fixed(12.0)))
        .height(Length::Fixed(2.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(tr_col)),
            ..Default::default()
        });

    let eval_badge: Element<'a, Message> = match is_receptive {
        Some(true) => container(text("TRUE").size(9.5).color(COLOR_ACTIVE_STEP))
            .padding([1, 5])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(0.18, 0.835, 0.45, 0.2)))
            .into(),
        Some(false) => container(text("FALSE").size(9.5).color(StudioTheme::TEXT_MUTED))
            .padding([1, 5])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(1.0, 1.0, 1.0, 0.05)))
            .into(),
        None => container(text("EVAL").size(9.5).color(COLOR_STEP_DROP))
            .padding([1, 5])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(0.28, 0.79, 0.89, 0.15)))
            .into(),
    };

    let from_steps_str = trans.from_steps.join(",");
    let to_steps_str = trans.to_steps.join(",");
    let header_text = format!("{}  (X{} -> X{})", trans.id, from_steps_str, to_steps_str);

    let card_top = row![
        text(header_text)
            .size(10.5)
            .font(iced::Font::MONOSPACE)
            .color(tr_col),
        Space::with_width(Length::Fill),
        eval_badge,
        button(text("x").size(10.5))
            .on_press(Message::SfcRemoveTransition(p_name.clone(), t_idx))
            .style(StudioTheme::ghost_button)
            .padding([1, 4]),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let receptivity_input = text_input("Receptivity (e.g. Start_PB)", &trans.receptivity)
        .on_input({
            let p = p_name;
            move |r| Message::SfcTransitionReceptivityChanged(p.clone(), t_idx, r)
        })
        .font(iced::Font::MONOSPACE)
        .size(10.5)
        .padding([2, 5])
        .style(sfc_input_style)
        .width(Length::Fill);

    let card_content = column![card_top, receptivity_input].spacing(3);

    let transition_card = container(card_content)
        .width(Length::Fixed(TRANSITION_CARD_WIDTH))
        .padding([3, 8])
        .style(move |_| container::Style {
            background: Some(Background::Color(COLOR_TRANS_BG)),
            border: Border {
                color: tr_col,
                width: 1.2,
                radius: Radius::from(4.0),
            },
            ..Default::default()
        });

    let left_pad = (target_axis_x - 20.0).max(0.0);

    row![
        Space::with_width(Length::Fixed(left_pad)),
        crossbar,
        wire_to_card,
        transition_card,
        Space::with_width(Length::Fill),
    ]
    .spacing(0)
    .align_y(Alignment::Center)
    .into()
}

/// Renders the IEC 60848 Double Horizontal Line (`===`) for AND Divergence / Convergence.
fn and_double_bar<'a>(label: &str, left_x: f32, right_x: f32) -> Element<'a, Message> {
    let bar_start_x = (left_x - 15.0).max(0.0);
    let span_width = (right_x - left_x + 30.0).max(40.0);
    let label_width = 110.0;
    let label_left = (bar_start_x - label_width - 8.0).max(0.0);
    let gap_before_bar = (bar_start_x - (label_left + label_width)).max(0.0);

    let line = move || {
        container(Space::with_width(Length::Fixed(span_width)))
            .height(Length::Fixed(2.5))
            .style(|_| container::Style {
                background: Some(Background::Color(COLOR_STEP_DROP)),
                ..Default::default()
            })
    };

    row![
        Space::with_width(Length::Fixed(label_left)),
        container(
            text(label.to_string())
                .size(10.5)
                .font(iced::Font::MONOSPACE)
                .color(COLOR_STEP_DROP),
        )
        .width(Length::Fixed(label_width))
        .align_x(iced::alignment::Horizontal::Right),
        Space::with_width(Length::Fixed(gap_before_bar)),
        column![
            line(),
            Space::with_height(Length::Fixed(3.5)),
            line(),
        ]
        .spacing(0),
        Space::with_width(Length::Fill),
    ]
    .spacing(0)
    .align_y(Alignment::Center)
    .into()
}

/// Renders the IEC 60848 Single Horizontal Line (`---`) for OR Divergence / Convergence.
fn or_single_bar<'a>(label: &str, left_x: f32, right_x: f32) -> Element<'a, Message> {
    let bar_start_x = (left_x - 15.0).max(0.0);
    let span_width = (right_x - left_x + 30.0).max(40.0);
    let label_width = 110.0;
    let label_left = (bar_start_x - label_width - 8.0).max(0.0);
    let gap_before_bar = (bar_start_x - (label_left + label_width)).max(0.0);

    row![
        Space::with_width(Length::Fixed(label_left)),
        container(
            text(label.to_string())
                .size(10.5)
                .font(iced::Font::MONOSPACE)
                .color(COLOR_OR_BRANCH),
        )
        .width(Length::Fixed(label_width))
        .align_x(iced::alignment::Horizontal::Right),
        Space::with_width(Length::Fixed(gap_before_bar)),
        container(Space::with_width(Length::Fixed(span_width)))
            .height(Length::Fixed(2.5))
            .style(|_| container::Style {
                background: Some(Background::Color(COLOR_OR_BRANCH)),
                ..Default::default()
            }),
        Space::with_width(Length::Fill),
    ]
    .spacing(0)
    .align_y(Alignment::Center)
    .into()
}

/// Renders drop wires for `cols` parallel branches
fn render_drop_wires<'a>(cols: usize, height: f32, color: Color) -> Element<'a, Message> {
    let mut r = row![].spacing(0);
    for _ in 0..cols {
        r = r.push(
            container(vertical_wire(height, color))
                .width(Length::Fixed(COLUMN_STRIDE))
                .align_x(Alignment::Center),
        );
    }
    r.into()
}

fn central_vertical_wire<'a>(height: f32, color: Color, total_width: f32) -> Element<'a, Message> {
    container(vertical_wire(height, color))
        .width(Length::Fixed(total_width))
        .align_x(Alignment::Center)
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
        .size(17)
        .font(iced::Font::MONOSPACE),
        Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Step", "+ Étape")).size(11))
            .on_press(Message::SfcAddStep(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([4, 9]),
        button(text(lang.tr("+ Add Transition", "+ Transition")).size(11))
            .on_press(Message::SfcAddTransition(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([4, 9]),
        button(text(lang.tr("+ AND Div (==)", "+ Div ET (==)")).size(11))
            .on_press(Message::SfcAddAndDivergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([4, 9]),
        button(text(lang.tr("+ AND Conv (==)", "+ Conv ET (==)")).size(11))
            .on_press(Message::SfcAddAndConvergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([4, 9]),
        button(text(lang.tr("+ OR Div (--)", "+ Div OU (--)")).size(11))
            .on_press(Message::SfcAddOrDivergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([4, 9]),
        button(text(lang.tr("+ OR Conv (--)", "+ Conv OU (--)")).size(11))
            .on_press(Message::SfcAddOrConvergence(pou_name.clone()))
            .style(StudioTheme::secondary_button)
            .padding([4, 9]),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([4, 9]),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Standard IEC 60848 Grafcet: Double border indicates initial step. Actions attached horizontally on the right. Amber tick transitions connect sequence links. Parallel AND/OR branches align automatically.",
        "Grafcet standard CEI 60848 : Double encadrement pour étape initiale. Actions reliées à droite. Transitions ambre sur liaison orientée. Branches parallèles ET/OU synchronisées.",
    ))
    .size(11.5)
    .color(StudioTheme::TEXT_MUTED);

    // 2. Compute 2D Step Rows (Grouping AND/OR parallel branches)
    let step_rows = compute_sfc_step_rows(program);
    let max_cols = step_rows.iter().map(|r| r.len()).max().unwrap_or(1).max(2);
    let total_canvas_width = ((max_cols as f32) * COLUMN_STRIDE).max(960.0);
    let base_spine_x = total_canvas_width * 0.5;

    let mut grafcet_flow = column![].spacing(0).align_x(Alignment::Start);
    let mut rendered_transitions: HashSet<usize> = HashSet::new();

    for (r_idx, row_indices) in step_rows.iter().enumerate() {
        let cols = row_indices.len();

        // A. Pre-row: Check if incoming transition diverges into steps in this row (AND Divergence)
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
            // 1. Central incoming wire
            grafcet_flow = grafcet_flow.push(central_vertical_wire(16.0, COLOR_INACTIVE_STEP, total_canvas_width));
            // 2. Transition node
            let tr_node = render_transition_node(program, t_idx, runtime, &pou_name, lang, base_spine_x);
            grafcet_flow = grafcet_flow.push(container(tr_node).width(Length::Fixed(total_canvas_width)));
            // 3. Central wire down to double bar
            grafcet_flow = grafcet_flow.push(central_vertical_wire(14.0, COLOR_INACTIVE_STEP, total_canvas_width));
            // 4. AND Divergence double horizontal bar spanning across the branch columns
            let left_x = step_center_x(0, cols, base_spine_x, COLUMN_STRIDE);
            let right_x = step_center_x(cols - 1, cols, base_spine_x, COLUMN_STRIDE);
            let bar = and_double_bar("[AND Div ==]", left_x, right_x);
            grafcet_flow = grafcet_flow.push(container(bar).width(Length::Fixed(total_canvas_width)));
            // 5. Drop wires into each column
            let wires_container = container(render_drop_wires(cols, 16.0, COLOR_STEP_DROP))
                .width(Length::Fixed(total_canvas_width))
                .align_x(Alignment::Center);
            grafcet_flow = grafcet_flow.push(wires_container);
        }

        // B. Render Steps in this Row: Side-by-side if multiple branches!
        if cols == 1 {
            let s_idx = row_indices[0];
            let left_margin = step_box_left_margin(base_spine_x);
            let step_node = render_step_node(program, s_idx, runtime, &pou_name, lang, left_margin);
            grafcet_flow = grafcet_flow.push(
                container(step_node)
                    .width(Length::Fixed(total_canvas_width))
                    .align_x(Alignment::Start),
            );
        } else {
            let mut branch_row = row![].spacing(0).align_y(Alignment::Start);
            let col_axis = COLUMN_STRIDE * 0.5;
            let left_margin = step_box_left_margin(col_axis);
            for &s_idx in row_indices {
                let step_node = render_step_node(program, s_idx, runtime, &pou_name, lang, left_margin);
                branch_row = branch_row.push(
                    container(step_node)
                        .width(Length::Fixed(COLUMN_STRIDE))
                        .align_x(Alignment::Start),
                );
            }
            grafcet_flow = grafcet_flow.push(
                container(branch_row)
                    .width(Length::Fixed(total_canvas_width))
                    .align_x(Alignment::Center),
            );
        }

        // C. Post-row: Check for AND Convergence (transition with multiple from_steps containing all steps in this row)
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

        if let Some((t_idx, tr)) = and_conv_trans {
            if !rendered_transitions.contains(&t_idx) {
                rendered_transitions.insert(t_idx);
                // 1. Drop wires from each branch step into double bar
                let wires_container = container(render_drop_wires(cols, 16.0, COLOR_INACTIVE_STEP))
                    .width(Length::Fixed(total_canvas_width))
                    .align_x(Alignment::Center);
                grafcet_flow = grafcet_flow.push(wires_container);
                // 2. AND Convergence double horizontal bar
                let left_x = step_center_x(0, cols, base_spine_x, COLUMN_STRIDE);
                let right_x = step_center_x(cols - 1, cols, base_spine_x, COLUMN_STRIDE);
                let conv_bar = if tr.convergence == ConvergenceKind::Or {
                    or_single_bar("[OR Conv --]", left_x, right_x)
                } else {
                    and_double_bar("[AND Conv ==]", left_x, right_x)
                };
                grafcet_flow = grafcet_flow.push(container(conv_bar).width(Length::Fixed(total_canvas_width)));
                // 3. Central wire down to transition
                grafcet_flow = grafcet_flow.push(central_vertical_wire(14.0, COLOR_INACTIVE_STEP, total_canvas_width));
                // 4. Centered transition node
                let tr_node = render_transition_node(program, t_idx, runtime, &pou_name, lang, base_spine_x);
                grafcet_flow = grafcet_flow.push(container(tr_node).width(Length::Fixed(total_canvas_width)));
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

        if outgoing.len() > 1 && cols == 1 {
            // OR Divergence!
            let or_cols = outgoing.len();
            // 1. Central wire from step down to single purple bar
            grafcet_flow = grafcet_flow.push(central_vertical_wire(16.0, COLOR_INACTIVE_STEP, total_canvas_width));
            // 2. OR Divergence single horizontal bar
            let left_x = step_center_x(0, or_cols, base_spine_x, COLUMN_STRIDE);
            let right_x = step_center_x(or_cols - 1, or_cols, base_spine_x, COLUMN_STRIDE);
            let or_bar = or_single_bar("[OR Div --]", left_x, right_x);
            grafcet_flow = grafcet_flow.push(container(or_bar).width(Length::Fixed(total_canvas_width)));
            // 3. Drop wires into each alternate transition column
            let wires_container = container(render_drop_wires(or_cols, 14.0, COLOR_OR_BRANCH))
                .width(Length::Fixed(total_canvas_width))
                .align_x(Alignment::Center);
            grafcet_flow = grafcet_flow.push(wires_container);
            // 4. Row of alternate transition columns
            let mut or_div_row = row![].spacing(0).align_y(Alignment::Center);
            for (ti, _) in &outgoing {
                rendered_transitions.insert(*ti);
                let tr_node = render_transition_node(program, *ti, runtime, &pou_name, lang, COLUMN_STRIDE * 0.5);
                or_div_row = or_div_row.push(
                    container(tr_node)
                        .width(Length::Fixed(COLUMN_STRIDE))
                        .align_x(Alignment::Start),
                );
            }
            grafcet_flow = grafcet_flow.push(
                container(or_div_row)
                    .width(Length::Fixed(total_canvas_width))
                    .align_x(Alignment::Center),
            );
            // 5. Drop wires exiting transitions towards next row
            let wires_exit = container(render_drop_wires(or_cols, 16.0, COLOR_INACTIVE_STEP))
                .width(Length::Fixed(total_canvas_width))
                .align_x(Alignment::Center);
            grafcet_flow = grafcet_flow.push(wires_exit);
        } else {
            for (ti, tr) in outgoing {
                rendered_transitions.insert(ti);
                // 1. Wire down from step
                grafcet_flow = grafcet_flow.push(central_vertical_wire(16.0, COLOR_INACTIVE_STEP, total_canvas_width));
                // 2. Centered transition node
                let tr_node = render_transition_node(program, ti, runtime, &pou_name, lang, base_spine_x);
                grafcet_flow = grafcet_flow.push(container(tr_node).width(Length::Fixed(total_canvas_width)));

                // Check if this transition is a Loopback (targets a step in a previous row, e.g. X0)
                let is_loopback = tr.to_steps.iter().any(|target| {
                    step_rows[..r_idx].iter().any(|prev_row| {
                        prev_row.iter().any(|&psi| &program.steps[psi].id == target || &program.steps[psi].name == target)
                    })
                });

                if is_loopback {
                    let loop_targets = tr.to_steps.join(", ");
                    grafcet_flow = grafcet_flow.push(central_vertical_wire(10.0, COLOR_ACTIVE_STEP, total_canvas_width));
                    let loop_badge = container(
                        text(format!("▲ Loopback to X{loop_targets}"))
                            .size(10.5)
                            .font(iced::Font::MONOSPACE)
                            .color(COLOR_ACTIVE_STEP),
                    )
                    .padding([2, 8])
                    .style(StudioTheme::subtle_badge_style(Color::from_rgba(0.18, 0.835, 0.45, 0.15)));
                    grafcet_flow = grafcet_flow.push(
                        container(loop_badge)
                            .width(Length::Fixed(total_canvas_width))
                            .align_x(Alignment::Center),
                    );
                } else if tr.to_steps.len() <= 1 {
                    // Normal directed link into next step
                    grafcet_flow = grafcet_flow.push(central_vertical_wire(16.0, COLOR_INACTIVE_STEP, total_canvas_width));
                }
            }
        }
    }

    // 3. Render any remaining transitions (e.g. unlinked or free transitions)
    let remaining: Vec<(usize, &SfcTransition)> = program
        .transitions
        .iter()
        .enumerate()
        .filter(|(ti, _)| !rendered_transitions.contains(ti))
        .collect();

    if !remaining.is_empty() {
        let unlinked_header = text(lang.tr("Free / Unlinked Transitions", "Transitions Libres")).size(12).color(COLOR_TRANSITION);
        let mut unlinked_col = column![unlinked_header].spacing(6).align_x(Alignment::Center);

        for (ti, _) in remaining {
            let tr_node = render_transition_node(program, ti, runtime, &pou_name, lang, base_spine_x);
            unlinked_col = unlinked_col.push(tr_node);
        }

        grafcet_flow = grafcet_flow.push(
            container(unlinked_col)
                .padding(12)
                .style(StudioTheme::card_style),
        );
    }

    let canvas_container = container(grafcet_flow)
        .padding(24)
        .width(Length::Fixed(total_canvas_width.max(960.0)))
        .style(|_| container::Style {
            background: Some(Background::Color(COLOR_CANVAS_BG)),
            border: Border {
                color: StudioTheme::BORDER_CARD,
                width: 1.0,
                radius: Radius::from(6.0),
            },
            ..Default::default()
        });

    let main_scroll = scrollable(
        column![
            header,
            tip,
            scrollable(canvas_container).direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new()
            )),
        ]
        .spacing(14),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    main_scroll.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_lang_sfc::{ActionQualifier, SfcAction, SfcProgram, SfcStep, SfcTransition, StepKind};

    #[test]
    fn test_sfc_coordinate_grid_and_step_box_anchoring() {
        let base_spine_x = 560.0_f32;
        let col_stride = COLUMN_STRIDE;

        // 1. Single-column row (cols = 1)
        let cx_single = step_center_x(0, 1, base_spine_x, col_stride);
        assert!((cx_single - base_spine_x).abs() < f32::EPSILON);

        let left_margin_single = step_box_left_margin(cx_single);
        let step_center_reconstructed = left_margin_single + STEP_BOX_WIDTH * 0.5;
        assert!((step_center_reconstructed - base_spine_x).abs() < f32::EPSILON);

        // 2. Parallel branches (cols = 2)
        let cx_branch0 = step_center_x(0, 2, base_spine_x, col_stride);
        let cx_branch1 = step_center_x(1, 2, base_spine_x, col_stride);

        assert!((cx_branch0 - (base_spine_x - col_stride * 0.5)).abs() < f32::EPSILON);
        assert!((cx_branch1 - (base_spine_x + col_stride * 0.5)).abs() < f32::EPSILON);

        // Distance between branch centers must strictly equal COLUMN_STRIDE
        let branch_dist = cx_branch1 - cx_branch0;
        assert!((branch_dist - col_stride).abs() < f32::EPSILON);

        // Midpoint of the two branches must strictly align with base_spine_x
        let branch_midpoint = (cx_branch0 + cx_branch1) * 0.5;
        assert!((branch_midpoint - base_spine_x).abs() < f32::EPSILON);

        // Left margins within each column container of width COLUMN_STRIDE
        let col_axis = col_stride * 0.5;
        let left_margin_col = step_box_left_margin(col_axis);
        assert!(((left_margin_col + STEP_BOX_WIDTH * 0.5) - col_axis).abs() < f32::EPSILON);

        // 3. Verify step box left margin is deterministic and independent of actions
        let mut program = SfcProgram {
            name: "test_seq".to_string(),
            comment: "Test sequence".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_Init".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_Busy".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Motor_Run := TRUE".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::L,
                            target: "Valve_Open".to_string(),
                            duration: Some("T#5s".to_string()),
                        },
                    ],
                },
            ],
            transitions: vec![SfcTransition {
                id: "T0".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string()],
                receptivity: "Start_PB".to_string(),
                convergence: ferrite_lang_sfc::ConvergenceKind::And,
                divergence: ferrite_lang_sfc::DivergenceKind::And,
            }],
        };

        // Render editor without panic
        {
            let _elem = render_sfc_editor(&program, None, UiLanguage::En);
        }

        // Adding more actions to step 1 does not change compute_sfc_step_rows or step centering
        program.steps[1].actions.push(SfcAction {
            qualifier: ActionQualifier::R,
            target: "Alarm_Reset".to_string(),
            duration: None,
        });

        let rows = compute_sfc_step_rows(&program);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec![0]);
        assert_eq!(rows[1], vec![1]);

        {
            let _elem2 = render_sfc_editor(&program, None, UiLanguage::Fr);
        }
    }
}
