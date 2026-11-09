use std::collections::HashSet;

use iced::border::Radius;
use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, event};
use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer,
    Size, Theme,
};

use ferrite_core::IecValue;
pub use ferrite_lang_sfc::compute_sfc_step_rows;
use ferrite_lang_sfc::{
    ActionQualifier, ConvergenceKind, DivergenceKind, SfcProgram, SfcStep, SfcTransition,
    SfcValidationError, SfcValidationReport, StepKind, validate_sfc_topology,
};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

// Exact color palette matching IEC 60848 industrial Grafcet schematic & target egui design
pub const COLOR_ACTIVE_STEP: Color = Color::from_rgb(46.0 / 255.0, 213.0 / 255.0, 115.0 / 255.0); // #2ED573 emerald
pub const COLOR_INACTIVE_STEP: Color = Color::from_rgb(120.0 / 255.0, 135.0 / 255.0, 160.0 / 255.0); // #7887A0 slate
pub const COLOR_STEP_DROP: Color = Color::from_rgb(72.0 / 255.0, 202.0 / 255.0, 228.0 / 255.0); // #48CAE4 cyan
pub const COLOR_TRANSITION: Color = Color::from_rgb(1.0, 184.0 / 255.0, 77.0 / 255.0); // #FFB84D amber
pub const COLOR_OR_BRANCH: Color = Color::from_rgb(198.0 / 255.0, 120.0 / 255.0, 221.0 / 255.0); // #C678DD purple
pub const COLOR_CANVAS_BG: Color = Color::from_rgb(16.0 / 255.0, 20.0 / 255.0, 26.0 / 255.0); // #10141A
pub const COLOR_STEP_BG: Color = Color::from_rgb(26.0 / 255.0, 34.0 / 255.0, 46.0 / 255.0); // #1A222E
pub const COLOR_TRANS_BG: Color = Color::from_rgb(22.0 / 255.0, 28.0 / 255.0, 38.0 / 255.0); // #161C26
pub const COLOR_ACTION_BG: Color = Color::from_rgb(30.0 / 255.0, 38.0 / 255.0, 52.0 / 255.0); // #1E2634
pub const COLOR_INPUT_BG: Color = Color::from_rgb(14.0 / 255.0, 20.0 / 255.0, 29.0 / 255.0); // #0E141D

pub const GRID_SIZE: f32 = 20.0;
pub const STEP_WIDTH: f32 = 180.0;
pub const STEP_HEIGHT: f32 = 60.0;
pub const TRANS_WIDTH: f32 = 80.0;
pub const TRANS_HEIGHT: f32 = 20.0;
pub const PORT_RADIUS: f32 = 4.5;
pub const PORT_HIT_DIST: f32 = 18.0;
pub const PORT_CLICK_RADIUS: f32 = 18.0;
pub const PORT_DROP_RADIUS: f32 = 28.0;

// Legacy constants kept for layout compatibility and unit tests
pub const COLUMN_STRIDE: f32 = 560.0;
pub const STEP_BOX_WIDTH: f32 = 188.0;
pub const STEP_BOX_HEIGHT: f32 = 68.0;
pub const TRANSITION_CARD_WIDTH: f32 = 320.0;

pub fn step_center_x(col_idx: usize, cols: usize, base_spine_x: f32, col_stride: f32) -> f32 {
    let row_start_x = base_spine_x - ((cols.saturating_sub(1)) as f32) * col_stride * 0.5;
    row_start_x + (col_idx as f32) * col_stride
}

pub fn step_box_left_margin(target_center_x: f32) -> f32 {
    (target_center_x - STEP_BOX_WIDTH * 0.5).max(0.0)
}

/// Snaps raw canvas coordinates to the discrete 20px grid.
pub fn snap_to_grid(x: f32, y: f32) -> (i32, i32) {
    let gx = (x / GRID_SIZE).round() as i32 * GRID_SIZE as i32;
    let gy = (y / GRID_SIZE).round() as i32 * GRID_SIZE as i32;
    (gx.max(20), gy.max(20))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SfcElementKey {
    Step(String),
    Transition(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfcPortSource {
    StepOutput(String),
    StepInput(String),
    TransitionOutput(String),
    TransitionInput(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfcPortTarget {
    StepInput(String),
    StepOutput(String),
    TransitionInput(String),
    TransitionOutput(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PortHover {
    StepInput(String, Point),
    StepOutput(String, Point),
    TransitionInput(String, Point),
    TransitionOutput(String, Point),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DragState {
    None,
    MovingElement {
        key: SfcElementKey,
        offset_x: f32,
        offset_y: f32,
        current_x: f32,
        current_y: f32,
    },
    DraggingWire {
        source: SfcPortSource,
        start_pt: Point,
        current_pt: Point,
    },
    BoxSelecting {
        start_pt: Point,
        current_pt: Point,
    },
}

/// Target element field currently being edited directly on the SFC canvas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfcEditTarget {
    StepName { step_id: String },
    ActionTarget { step_id: String, action_idx: usize },
    ActionDuration { step_id: String, action_idx: usize },
    TransitionReceptivity { trans_id: String },
}

/// Active dropdown menu state for picking an action qualifier directly on canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct SfcQualifierDropdown {
    pub step_id: String,
    pub action_idx: usize,
    pub rect: Rectangle,
}

#[derive(Debug, Clone)]
pub struct SfcCanvasState {
    pub drag: DragState,
    pub hover_port: Option<PortHover>,
    pub selected: Option<SfcElementKey>,
    pub selected_elements: Vec<SfcElementKey>,
    pub editing_target: Option<SfcEditTarget>,
    pub edit_buffer: String,
    pub cursor_pos: usize,
    pub last_click: Option<(std::time::Instant, Point)>,
    pub qualifier_dropdown: Option<SfcQualifierDropdown>,
}

impl Default for SfcCanvasState {
    fn default() -> Self {
        Self {
            drag: DragState::None,
            hover_port: None,
            selected: None,
            selected_elements: Vec::new(),
            editing_target: None,
            edit_buffer: String::new(),
            cursor_pos: 0,
            last_click: None,
            qualifier_dropdown: None,
        }
    }
}

impl SfcCanvasState {
    pub fn start_edit(&mut self, target: SfcEditTarget, initial_text: &str) {
        self.qualifier_dropdown = None;
        self.editing_target = Some(target);
        self.edit_buffer = initial_text.to_string();
        self.cursor_pos = self.edit_buffer.len();
    }

    pub fn cancel_edit(&mut self) {
        self.qualifier_dropdown = None;
        self.editing_target = None;
        self.edit_buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        if !c.is_control() {
            if self.cursor_pos >= self.edit_buffer.len() {
                self.edit_buffer.push(c);
            } else {
                self.edit_buffer.insert(self.cursor_pos, c);
            }
            self.cursor_pos += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 && !self.edit_buffer.is_empty() {
            self.cursor_pos -= 1;
            if self.cursor_pos < self.edit_buffer.len() {
                self.edit_buffer.remove(self.cursor_pos);
            }
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.edit_buffer.len() {
            self.edit_buffer.remove(self.cursor_pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.edit_buffer.len() {
            self.cursor_pos += 1;
        }
    }
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

pub const SFC_QUALIFIER_ITEMS: [(ActionQualifier, &str, &str); 8] = [
    (ActionQualifier::N, "N", "N - Non-stored"),
    (ActionQualifier::S, "S", "S - Set (Latch)"),
    (ActionQualifier::R, "R", "R - Reset (Unlatch)"),
    (ActionQualifier::L, "L", "L - Time Limited"),
    (ActionQualifier::D, "D", "D - Time Delayed"),
    (ActionQualifier::P, "P", "P - Pulse"),
    (ActionQualifier::F, "F", "F - Force ON"),
    (ActionQualifier::FZ, "FZ", "FZ - Force OFF"),
];

pub fn qualifier_badge_color(qual: ActionQualifier) -> Color {
    match qual {
        ActionQualifier::N => Color::from_rgb(0.18, 0.80, 0.45),
        ActionQualifier::S | ActionQualifier::R => Color::from_rgb(0.95, 0.65, 0.15),
        ActionQualifier::L | ActionQualifier::D => Color::from_rgb(0.25, 0.75, 0.95),
        ActionQualifier::P => StudioTheme::ACCENT_PURPLE,
        ActionQualifier::F | ActionQualifier::FZ => Color::from_rgb(0.95, 0.35, 0.45),
    }
}

/// Cycles to the next action qualifier in sequence.
#[allow(dead_code)]
pub fn next_action_qualifier(current: ActionQualifier) -> ActionQualifier {
    match current {
        ActionQualifier::N => ActionQualifier::S,
        ActionQualifier::S => ActionQualifier::R,
        ActionQualifier::R => ActionQualifier::L,
        ActionQualifier::L => ActionQualifier::D,
        ActionQualifier::D => ActionQualifier::P,
        ActionQualifier::P => ActionQualifier::F,
        ActionQualifier::F => ActionQualifier::FZ,
        ActionQualifier::FZ => ActionQualifier::N,
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfcConvergenceWrapper(pub ConvergenceKind);

impl std::fmt::Display for SfcConvergenceWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ConvergenceKind::And => write!(f, "AND (===)"),
            ConvergenceKind::Or => write!(f, "OR (—)"),
        }
    }
}

#[allow(dead_code)]
pub const ALL_CONVERGENCE_KINDS: [SfcConvergenceWrapper; 2] = [
    SfcConvergenceWrapper(ConvergenceKind::And),
    SfcConvergenceWrapper(ConvergenceKind::Or),
];

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SfcDivergenceWrapper(pub DivergenceKind);

impl std::fmt::Display for SfcDivergenceWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            DivergenceKind::And => write!(f, "AND (===)"),
            DivergenceKind::Or => write!(f, "OR (—)"),
        }
    }
}

#[allow(dead_code)]
pub const ALL_DIVERGENCE_KINDS: [SfcDivergenceWrapper; 2] = [
    SfcDivergenceWrapper(DivergenceKind::And),
    SfcDivergenceWrapper(DivergenceKind::Or),
];

pub fn is_step_active(rt: &PlcRuntime, pou_name: &str, step_id: &str) -> (bool, u64) {
    is_step_active_with_name(rt, pou_name, step_id, "")
}

pub fn is_step_active_with_name(
    rt: &PlcRuntime,
    pou_name: &str,
    step_id: &str,
    step_name: &str,
) -> (bool, u64) {
    let x_id = ferrite_core::normalize_sfc_step_prefix(step_id);

    let mut state_candidates = Vec::with_capacity(6);
    let mut time_candidates = Vec::with_capacity(6);

    if !pou_name.is_empty() {
        state_candidates.push(format!("{pou_name}.{x_id}.state"));
        state_candidates.push(format!("{pou_name}.{step_id}.state"));
        if !step_name.is_empty() && step_name != step_id {
            state_candidates.push(format!("{pou_name}.{step_name}.state"));
        }
        time_candidates.push(format!("{pou_name}.{x_id}.t"));
        time_candidates.push(format!("{pou_name}.{step_id}.t"));
        if !step_name.is_empty() && step_name != step_id {
            time_candidates.push(format!("{pou_name}.{step_name}.t"));
        }
    }

    state_candidates.push(format!("{x_id}.state"));
    state_candidates.push(format!("{step_id}.state"));
    if !step_name.is_empty() && step_name != step_id {
        state_candidates.push(format!("{step_name}.state"));
    }

    time_candidates.push(format!("{x_id}.t"));
    time_candidates.push(format!("{step_id}.t"));
    if !step_name.is_empty() && step_name != step_id {
        time_candidates.push(format!("{step_name}.t"));
    }

    let is_on = state_candidates
        .iter()
        .find_map(|sym| match rt.read_value(sym) {
            Ok(IecValue::Bool(b)) => Some(b),
            _ => None,
        })
        .unwrap_or(false);

    let elapsed_ms = time_candidates
        .iter()
        .find_map(|sym| match rt.read_value(sym) {
            Ok(IecValue::Time(ms)) => Some(ms),
            _ => None,
        })
        .unwrap_or(0);

    (is_on, elapsed_ms)
}

pub fn evaluate_receptivity(rt: &PlcRuntime, pou_name: &str, receptivity: &str) -> Option<bool> {
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

// Helper to draw downwards directional arrowhead at tip
fn draw_arrowhead(frame: &mut Frame, tip: Point, dir: (f32, f32), color: Color) {
    let len = 8.0;
    let width = 5.0;
    let p1 = Point::new(
        tip.x - dir.0 * len - dir.1 * width,
        tip.y - dir.1 * len + dir.0 * width,
    );
    let p2 = Point::new(
        tip.x - dir.0 * len + dir.1 * width,
        tip.y - dir.1 * len - dir.0 * width,
    );
    let path = Path::new(|b| {
        b.move_to(tip);
        b.line_to(p1);
        b.line_to(p2);
        b.close();
    });
    frame.fill(&path, color);
}

/// The interactive 2D Grid Canvas Program for SFC
pub struct SfcCanvasProgram<'a> {
    pub pou_name: &'a str,
    pub program: &'a SfcProgram,
    pub runtime: Option<&'a PlcRuntime>,
    pub selected: Option<&'a SfcElementKey>,
    pub selected_elements: &'a [SfcElementKey],
    pub validation_report: Option<SfcValidationReport>,
}

impl<'a> SfcCanvasProgram<'a> {
    fn step_pos(&self, step: &SfcStep, state: &SfcCanvasState) -> (f32, f32) {
        if let DragState::MovingElement {
            key: SfcElementKey::Step(ref id),
            current_x,
            current_y,
            ..
        } = state.drag
            && &step.id == id
        {
            let (gx, gy) = snap_to_grid(current_x, current_y);
            return (gx as f32, gy as f32);
        }
        let fallback_idx = self
            .program
            .steps
            .iter()
            .position(|s| s.id == step.id)
            .unwrap_or(0);
        let fallback_pos = (
            60 + (fallback_idx as i32 % 3) * 260,
            40 + (fallback_idx as i32 / 3) * 220,
        );
        let (gx, gy) = step.grid_pos.unwrap_or(fallback_pos);
        (gx as f32, gy as f32)
    }

    fn trans_pos(&self, tr: &SfcTransition, state: &SfcCanvasState) -> (f32, f32) {
        if let DragState::MovingElement {
            key: SfcElementKey::Transition(ref id),
            current_x,
            current_y,
            ..
        } = state.drag
            && &tr.id == id
        {
            let (gx, gy) = snap_to_grid(current_x, current_y);
            return (gx as f32, gy as f32);
        }
        let fallback_idx = self
            .program
            .transitions
            .iter()
            .position(|t| t.id == tr.id)
            .unwrap_or(0);
        let fallback_pos = (
            110 + (fallback_idx as i32 % 3) * 260,
            140 + (fallback_idx as i32 / 3) * 220,
        );
        let (gx, gy) = tr.grid_pos.unwrap_or(fallback_pos);
        (gx as f32, gy as f32)
    }

    pub fn step_width(&self, step: &SfcStep) -> f32 {
        let name_len = step.name.len().max(6);
        (name_len as f32 * 8.2 + 50.0).clamp(170.0, 320.0)
    }

    pub fn step_name_rect(&self, step: &SfcStep, state: &SfcCanvasState) -> Rectangle {
        let (x, y) = self.step_pos(step, state);
        let w = self.step_width(step);
        Rectangle {
            x: x + 8.0,
            y: y + 28.0,
            width: w - 16.0,
            height: 24.0,
        }
    }

    pub fn step_kind_badge_rect(&self, step: &SfcStep, state: &SfcCanvasState) -> Rectangle {
        let (x, y) = self.step_pos(step, state);
        Rectangle {
            x: x + 6.0,
            y: y + 5.0,
            width: 38.0,
            height: 18.0,
        }
    }

    pub fn step_add_action_rect(&self, step: &SfcStep, state: &SfcCanvasState) -> Rectangle {
        let (x, y) = self.step_pos(step, state);
        let w = self.step_width(step);
        Rectangle {
            x: x + w - 46.0,
            y: y + 5.0,
            width: 40.0,
            height: 18.0,
        }
    }

    pub fn action_qualifier_rect(
        &self,
        step: &SfcStep,
        a_idx: usize,
        state: &SfcCanvasState,
    ) -> Rectangle {
        let (x, y) = self.step_pos(step, state);
        let w = self.step_width(step);
        let act_x = x + w + 14.0;
        let act_y = y + 4.0 + (a_idx as f32) * 26.0;
        Rectangle {
            x: act_x,
            y: act_y,
            width: 32.0,
            height: 22.0,
        }
    }

    pub fn action_target_rect(
        &self,
        step: &SfcStep,
        a_idx: usize,
        state: &SfcCanvasState,
    ) -> Rectangle {
        let (x, y) = self.step_pos(step, state);
        let w = self.step_width(step);
        let act_x = x + w + 14.0;
        let act_y = y + 4.0 + (a_idx as f32) * 26.0;
        let act = step.actions.get(a_idx);
        let target_len = act.map(|a| a.target.len()).unwrap_or(8).max(6);
        let target_w = (target_len as f32 * 7.5 + 24.0).clamp(90.0, 240.0);
        Rectangle {
            x: act_x + 34.0,
            y: act_y,
            width: target_w,
            height: 22.0,
        }
    }

    pub fn action_duration_rect(
        &self,
        step: &SfcStep,
        a_idx: usize,
        state: &SfcCanvasState,
    ) -> Option<Rectangle> {
        let act = step.actions.get(a_idx)?;
        if act.qualifier == ActionQualifier::L || act.qualifier == ActionQualifier::D {
            let target_rect = self.action_target_rect(step, a_idx, state);
            Some(Rectangle {
                x: target_rect.x + target_rect.width + 4.0,
                y: target_rect.y,
                width: 60.0,
                height: 22.0,
            })
        } else {
            None
        }
    }

    pub fn action_delete_rect(
        &self,
        step: &SfcStep,
        a_idx: usize,
        state: &SfcCanvasState,
    ) -> Rectangle {
        let target_rect = self.action_target_rect(step, a_idx, state);
        let right_x = if let Some(dur_rect) = self.action_duration_rect(step, a_idx, state) {
            dur_rect.x + dur_rect.width + 4.0
        } else {
            target_rect.x + target_rect.width + 4.0
        };
        Rectangle {
            x: right_x,
            y: target_rect.y,
            width: 20.0,
            height: 22.0,
        }
    }

    pub fn trans_receptivity_rect(&self, tr: &SfcTransition, state: &SfcCanvasState) -> Rectangle {
        let (x, y) = self.trans_pos(tr, state);
        let cy = y + TRANS_HEIGHT * 0.5;
        let is_editing = state.editing_target
            == Some(SfcEditTarget::TransitionReceptivity {
                trans_id: tr.id.clone(),
            });
        let text_len = if is_editing {
            state.edit_buffer.chars().count() + 1
        } else if tr.receptivity.is_empty() {
            4 // "TRUE"
        } else {
            tr.receptivity.chars().count()
        };
        // Monospace 11px font character width is ~6.8px.
        // Horizontal padding: 5.0px left + 5.0px right = 10.0px.
        // Rectangle has the exact length of the text plus a small padding.
        let w = (text_len as f32 * 6.8 + 10.0).max(18.0);
        Rectangle {
            x: x + TRANS_WIDTH + 8.0,
            y: cy - 10.0,
            width: w,
            height: 20.0,
        }
    }

    pub fn commit_current_edit(&self, state: &mut SfcCanvasState) -> Option<Message> {
        let target = state.editing_target.take()?;
        let buffer = std::mem::take(&mut state.edit_buffer);
        state.cursor_pos = 0;
        let p_name = self.pou_name.to_string();

        match target {
            SfcEditTarget::StepName { step_id } => {
                let s_idx = self.program.steps.iter().position(|s| s.id == step_id)?;
                Some(Message::SfcStepNameChanged(p_name, s_idx, buffer))
            }
            SfcEditTarget::ActionTarget {
                step_id,
                action_idx,
            } => {
                let s_idx = self.program.steps.iter().position(|s| s.id == step_id)?;
                Some(Message::SfcActionTargetChanged(
                    p_name, s_idx, action_idx, buffer,
                ))
            }
            SfcEditTarget::ActionDuration {
                step_id,
                action_idx,
            } => {
                let s_idx = self.program.steps.iter().position(|s| s.id == step_id)?;
                Some(Message::SfcActionDurationChanged(
                    p_name, s_idx, action_idx, buffer,
                ))
            }
            SfcEditTarget::TransitionReceptivity { trans_id } => {
                let t_idx = self
                    .program
                    .transitions
                    .iter()
                    .position(|t| t.id == trans_id)?;
                Some(Message::SfcTransitionReceptivityChanged(
                    p_name, t_idx, buffer,
                ))
            }
        }
    }

    fn step_in_port(&self, step: &SfcStep, state: &SfcCanvasState) -> Point {
        let (x, y) = self.step_pos(step, state);
        Point::new(x + self.step_width(step) * 0.5, y)
    }

    fn step_out_port(&self, step: &SfcStep, state: &SfcCanvasState) -> Point {
        let (x, y) = self.step_pos(step, state);
        Point::new(x + self.step_width(step) * 0.5, y + STEP_HEIGHT)
    }

    fn trans_in_port(&self, tr: &SfcTransition, state: &SfcCanvasState) -> Point {
        let (x, y) = self.trans_pos(tr, state);
        Point::new(x + TRANS_WIDTH * 0.5, y)
    }

    fn trans_out_port(&self, tr: &SfcTransition, state: &SfcCanvasState) -> Point {
        let (x, y) = self.trans_pos(tr, state);
        Point::new(x + TRANS_WIDTH * 0.5, y + TRANS_HEIGHT)
    }

    /// Computes the clear X coordinate along the left edge for a loopback return arrow,
    /// ensuring it does not clip through any states or transitions in its vertical span.
    pub fn compute_sfc_loopback_side_x(
        &self,
        sp: Point,
        tr_out: Point,
        tr_id: &str,
        target_step_id: &str,
        state: &SfcCanvasState,
    ) -> f32 {
        let mut min_elem_x = sp.x.min(tr_out.x);

        // Check if we can identify the specific tree for this transition
        let tree_elements: Option<(&Vec<String>, &Vec<String>)> = self
            .validation_report
            .as_ref()
            .and_then(|rep| {
                rep.trees.iter().find(|t| {
                    t.transition_ids.iter().any(|tid| tid == tr_id)
                        || t.step_ids.iter().any(|sid| sid == target_step_id)
                })
            })
            .map(|t| (&t.step_ids, &t.transition_ids));

        for step_elem in &self.program.steps {
            let (sx, sy) = self.step_pos(step_elem, state);
            let in_vertical_span = sy + STEP_HEIGHT >= sp.y - 20.0 && sy <= tr_out.y + 20.0;
            if in_vertical_span {
                let in_same_tree = match tree_elements {
                    Some((s_ids, _)) => s_ids
                        .iter()
                        .any(|id| id == &step_elem.id || id == &step_elem.name),
                    None => true,
                };
                if in_same_tree {
                    min_elem_x = min_elem_x.min(sx);
                }
            }
        }

        for trans_elem in &self.program.transitions {
            let (tx, ty) = self.trans_pos(trans_elem, state);
            let in_vertical_span = ty + TRANS_HEIGHT >= sp.y - 20.0 && ty <= tr_out.y + 20.0;
            if in_vertical_span {
                let in_same_tree = match tree_elements {
                    Some((_, t_ids)) => t_ids.iter().any(|id| id == &trans_elem.id),
                    None => true,
                };
                if in_same_tree {
                    min_elem_x = min_elem_x.min(tx);
                }
            }
        }

        // Check if any element in the program exists to the left of min_elem_x
        // in this vertical span (e.g. another parallel tree)
        let mut obstacle_to_left = false;
        for step_elem in &self.program.steps {
            let (sx, sy) = self.step_pos(step_elem, state);
            if sy + STEP_HEIGHT >= sp.y - 20.0 && sy <= tr_out.y + 20.0 && sx < min_elem_x - 10.0 {
                obstacle_to_left = true;
                break;
            }
        }
        if !obstacle_to_left {
            for trans_elem in &self.program.transitions {
                let (tx, ty) = self.trans_pos(trans_elem, state);
                if ty + TRANS_HEIGHT >= sp.y - 20.0
                    && ty <= tr_out.y + 20.0
                    && tx < min_elem_x - 10.0
                {
                    obstacle_to_left = true;
                    break;
                }
            }
        }

        if !obstacle_to_left {
            // Unobstructed path to the canvas left margin
            if min_elem_x <= 35.0 {
                (min_elem_x - 15.0).max(8.0)
            } else {
                20.0
            }
        } else {
            // Space between this tree and parallel tree to the left
            (min_elem_x - 30.0).max(20.0)
        }
    }
}

impl<'a> canvas::Program<Message> for SfcCanvasProgram<'a> {
    type State = SfcCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, text, .. }) = event {
            if state.qualifier_dropdown.is_some()
                && matches!(key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape))
            {
                state.qualifier_dropdown = None;
                return (event::Status::Captured, None);
            }

            if state.editing_target.is_some() {
                return match key {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => {
                        let msg = self.commit_current_edit(state);
                        (event::Status::Captured, msg)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => {
                        state.cancel_edit();
                        (event::Status::Captured, None)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                        state.backspace();
                        (event::Status::Captured, None)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                        state.delete_forward();
                        (event::Status::Captured, None)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                        state.move_cursor_left();
                        (event::Status::Captured, None)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                        state.move_cursor_right();
                        (event::Status::Captured, None)
                    }
                    _ => {
                        if let Some(txt) = text {
                            for c in txt.chars() {
                                if !c.is_control() {
                                    state.insert_char(c);
                                }
                            }
                            (event::Status::Captured, None)
                        } else if let iced::keyboard::Key::Character(s) = key {
                            for c in s.chars() {
                                if !c.is_control() {
                                    state.insert_char(c);
                                }
                            }
                            (event::Status::Captured, None)
                        } else {
                            (event::Status::Ignored, None)
                        }
                    }
                };
            } else if matches!(
                key,
                iced::keyboard::Key::Named(
                    iced::keyboard::key::Named::Delete | iced::keyboard::key::Named::Backspace
                )
            ) {
                return (
                    event::Status::Captured,
                    Some(Message::SfcRequestDeleteSelected(self.pou_name.to_string())),
                );
            } else {
                return (event::Status::Ignored, None);
            }
        }

        let cursor_pos = match cursor.position_in(bounds) {
            Some(p) => p,
            None => return (event::Status::Ignored, None),
        };

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // 0. Check if an active Qualifier Dropdown Menu is open
                if let Some(dd) = state.qualifier_dropdown.take() {
                    if cursor_pos.x >= dd.rect.x
                        && cursor_pos.x <= dd.rect.x + dd.rect.width
                        && cursor_pos.y >= dd.rect.y
                        && cursor_pos.y <= dd.rect.y + dd.rect.height
                    {
                        let rel_y = cursor_pos.y - (dd.rect.y + 2.0);
                        let item_idx = (rel_y / 22.0) as usize;
                        if item_idx < SFC_QUALIFIER_ITEMS.len() {
                            let chosen_q = SFC_QUALIFIER_ITEMS[item_idx].0;
                            let s_idx = self
                                .program
                                .steps
                                .iter()
                                .position(|s| s.id == dd.step_id)
                                .unwrap_or(0);
                            return (
                                event::Status::Captured,
                                Some(Message::SfcActionQualifierChanged(
                                    self.pou_name.to_string(),
                                    s_idx,
                                    dd.action_idx,
                                    chosen_q,
                                )),
                            );
                        }
                    }

                    // Click was outside dropdown: if clicking the badge that opened it, just toggle closed
                    if let Some(step) = self.program.steps.iter().find(|s| s.id == dd.step_id) {
                        let q_rect = self.action_qualifier_rect(step, dd.action_idx, state);
                        if cursor_pos.x >= q_rect.x
                            && cursor_pos.x <= q_rect.x + q_rect.width
                            && cursor_pos.y >= q_rect.y
                            && cursor_pos.y <= q_rect.y + q_rect.height
                        {
                            return (event::Status::Captured, None);
                        }
                    }
                }

                // 1. Check if clicking ANY Port to begin dragging a wire
                // Step Ports (both Output and Input)
                for step in &self.program.steps {
                    let in_p = self.step_in_port(step, state);
                    let out_p = self.step_out_port(step, state);
                    let d_in = cursor_pos.distance(in_p);
                    let d_out = cursor_pos.distance(out_p);
                    if d_in <= PORT_CLICK_RADIUS || d_out <= PORT_CLICK_RADIUS {
                        if d_in < d_out {
                            state.drag = DragState::DraggingWire {
                                source: SfcPortSource::StepInput(step.id.clone()),
                                start_pt: in_p,
                                current_pt: cursor_pos,
                            };
                            return (event::Status::Captured, None);
                        } else {
                            state.drag = DragState::DraggingWire {
                                source: SfcPortSource::StepOutput(step.id.clone()),
                                start_pt: out_p,
                                current_pt: cursor_pos,
                            };
                            return (event::Status::Captured, None);
                        }
                    }
                }
                // Transition Ports (both Output and Input)
                for tr in &self.program.transitions {
                    let in_p = self.trans_in_port(tr, state);
                    let out_p = self.trans_out_port(tr, state);
                    let d_in = cursor_pos.distance(in_p);
                    let d_out = cursor_pos.distance(out_p);
                    if d_in <= PORT_CLICK_RADIUS || d_out <= PORT_CLICK_RADIUS {
                        if d_in < d_out {
                            state.drag = DragState::DraggingWire {
                                source: SfcPortSource::TransitionInput(tr.id.clone()),
                                start_pt: in_p,
                                current_pt: cursor_pos,
                            };
                            return (event::Status::Captured, None);
                        } else {
                            state.drag = DragState::DraggingWire {
                                source: SfcPortSource::TransitionOutput(tr.id.clone()),
                                start_pt: out_p,
                                current_pt: cursor_pos,
                            };
                            return (event::Status::Captured, None);
                        }
                    }
                }

                // 1.5. Check if clicking on Step Kind badge to cycle: Normal -> Initial -> Macro -> Normal
                for step in self.program.steps.iter().rev() {
                    let k_rect = self.step_kind_badge_rect(step, state);
                    if cursor_pos.x >= k_rect.x
                        && cursor_pos.x <= k_rect.x + k_rect.width
                        && cursor_pos.y >= k_rect.y
                        && cursor_pos.y <= k_rect.y + k_rect.height
                    {
                        let _ = self.commit_current_edit(state);
                        let s_idx = self
                            .program
                            .steps
                            .iter()
                            .position(|s| s.id == step.id)
                            .unwrap_or(0);
                        let next_kind = match step.kind {
                            StepKind::Normal => StepKind::Initial,
                            StepKind::Initial => StepKind::Macro,
                            StepKind::Macro => StepKind::Normal,
                        };
                        return (
                            event::Status::Captured,
                            Some(Message::SfcStepKindChanged(
                                self.pou_name.to_string(),
                                s_idx,
                                next_kind,
                            )),
                        );
                    }
                }

                // 2. Check if clicking [+ Act] button on any step
                for step in self.program.steps.iter().rev() {
                    let add_rect = self.step_add_action_rect(step, state);
                    if cursor_pos.x >= add_rect.x
                        && cursor_pos.x <= add_rect.x + add_rect.width
                        && cursor_pos.y >= add_rect.y
                        && cursor_pos.y <= add_rect.y + add_rect.height
                    {
                        let _ = self.commit_current_edit(state);
                        let s_idx = self
                            .program
                            .steps
                            .iter()
                            .position(|s| s.id == step.id)
                            .unwrap_or(0);
                        return (
                            event::Status::Captured,
                            Some(Message::SfcAddAction(self.pou_name.to_string(), s_idx)),
                        );
                    }
                }

                // 3. Check if clicking on any action row (Delete button, Qualifier badge, Target text, Duration)
                for step in self.program.steps.iter().rev() {
                    let s_idx = self
                        .program
                        .steps
                        .iter()
                        .position(|s| s.id == step.id)
                        .unwrap_or(0);
                    for (a_idx, act) in step.actions.iter().enumerate() {
                        let del_rect = self.action_delete_rect(step, a_idx, state);
                        if cursor_pos.x >= del_rect.x
                            && cursor_pos.x <= del_rect.x + del_rect.width
                            && cursor_pos.y >= del_rect.y
                            && cursor_pos.y <= del_rect.y + del_rect.height
                        {
                            let _ = self.commit_current_edit(state);
                            return (
                                event::Status::Captured,
                                Some(Message::SfcRemoveAction(
                                    self.pou_name.to_string(),
                                    s_idx,
                                    a_idx,
                                )),
                            );
                        }

                        let q_rect = self.action_qualifier_rect(step, a_idx, state);
                        if cursor_pos.x >= q_rect.x
                            && cursor_pos.x <= q_rect.x + q_rect.width
                            && cursor_pos.y >= q_rect.y
                            && cursor_pos.y <= q_rect.y + q_rect.height
                        {
                            let _ = self.commit_current_edit(state);
                            let dd_width = 160.0;
                            let dd_height = 8.0 * 22.0 + 4.0;
                            let dd_y = if q_rect.y + q_rect.height + 2.0 + dd_height > bounds.height
                                && q_rect.y >= dd_height + 4.0
                            {
                                q_rect.y - dd_height - 2.0
                            } else {
                                q_rect.y + q_rect.height + 2.0
                            };
                            let dd_x = if q_rect.x + dd_width > bounds.width {
                                (bounds.width - dd_width - 4.0).max(0.0)
                            } else {
                                q_rect.x
                            };
                            state.qualifier_dropdown = Some(SfcQualifierDropdown {
                                step_id: step.id.clone(),
                                action_idx: a_idx,
                                rect: Rectangle {
                                    x: dd_x,
                                    y: dd_y,
                                    width: dd_width,
                                    height: dd_height,
                                },
                            });
                            return (event::Status::Captured, None);
                        }

                        let t_rect = self.action_target_rect(step, a_idx, state);
                        if cursor_pos.x >= t_rect.x
                            && cursor_pos.x <= t_rect.x + t_rect.width
                            && cursor_pos.y >= t_rect.y
                            && cursor_pos.y <= t_rect.y + t_rect.height
                        {
                            let _ = self.commit_current_edit(state);
                            state.start_edit(
                                SfcEditTarget::ActionTarget {
                                    step_id: step.id.clone(),
                                    action_idx: a_idx,
                                },
                                &act.target,
                            );
                            return (event::Status::Captured, None);
                        }

                        if let Some(dur_rect) = self.action_duration_rect(step, a_idx, state)
                            && cursor_pos.x >= dur_rect.x
                            && cursor_pos.x <= dur_rect.x + dur_rect.width
                            && cursor_pos.y >= dur_rect.y
                            && cursor_pos.y <= dur_rect.y + dur_rect.height
                        {
                            let _ = self.commit_current_edit(state);
                            state.start_edit(
                                SfcEditTarget::ActionDuration {
                                    step_id: step.id.clone(),
                                    action_idx: a_idx,
                                },
                                act.duration.as_deref().unwrap_or("T#5s"),
                            );
                            return (event::Status::Captured, None);
                        }
                    }
                }

                // 4. Check if clicking on Step Name area to edit inline
                for step in self.program.steps.iter().rev() {
                    let name_rect = self.step_name_rect(step, state);
                    if cursor_pos.x >= name_rect.x
                        && cursor_pos.x <= name_rect.x + name_rect.width
                        && cursor_pos.y >= name_rect.y
                        && cursor_pos.y <= name_rect.y + name_rect.height
                    {
                        let _ = self.commit_current_edit(state);
                        state.start_edit(
                            SfcEditTarget::StepName {
                                step_id: step.id.clone(),
                            },
                            &step.name,
                        );
                        let key = SfcElementKey::Step(step.id.clone());
                        state.selected = Some(key.clone());
                        state.selected_elements = vec![key.clone()];
                        return (
                            event::Status::Captured,
                            Some(Message::SfcSelectElements(
                                self.pou_name.to_string(),
                                vec![key],
                            )),
                        );
                    }
                }

                // 5. Check if clicking on Transition Receptivity pill to edit inline
                for tr in self.program.transitions.iter().rev() {
                    let rec_rect = self.trans_receptivity_rect(tr, state);
                    if cursor_pos.x >= rec_rect.x
                        && cursor_pos.x <= rec_rect.x + rec_rect.width
                        && cursor_pos.y >= rec_rect.y
                        && cursor_pos.y <= rec_rect.y + rec_rect.height
                    {
                        let _ = self.commit_current_edit(state);
                        state.start_edit(
                            SfcEditTarget::TransitionReceptivity {
                                trans_id: tr.id.clone(),
                            },
                            &tr.receptivity,
                        );
                        let key = SfcElementKey::Transition(tr.id.clone());
                        state.selected = Some(key.clone());
                        state.selected_elements = vec![key.clone()];
                        return (
                            event::Status::Captured,
                            Some(Message::SfcSelectElements(
                                self.pou_name.to_string(),
                                vec![key],
                            )),
                        );
                    }
                }

                // 6. Check if clicking inside a Step box to drag or select
                for step in self.program.steps.iter().rev() {
                    let (x, y) = self.step_pos(step, state);
                    let s_width = self.step_width(step);
                    if cursor_pos.x >= x
                        && cursor_pos.x <= x + s_width
                        && cursor_pos.y >= y
                        && cursor_pos.y <= y + STEP_HEIGHT
                    {
                        let commit_msg = self.commit_current_edit(state);
                        let key = SfcElementKey::Step(step.id.clone());
                        state.selected = Some(key.clone());
                        state.selected_elements = vec![key.clone()];
                        state.drag = DragState::MovingElement {
                            key: key.clone(),
                            offset_x: cursor_pos.x - x,
                            offset_y: cursor_pos.y - y,
                            current_x: x,
                            current_y: y,
                        };
                        return (
                            event::Status::Captured,
                            commit_msg.or_else(|| {
                                Some(Message::SfcSelectElements(
                                    self.pou_name.to_string(),
                                    vec![key],
                                ))
                            }),
                        );
                    }
                }

                // 7. Check if clicking inside a Transition bar to drag or select
                for tr in self.program.transitions.iter().rev() {
                    let (x, y) = self.trans_pos(tr, state);
                    if cursor_pos.x >= x
                        && cursor_pos.x <= x + TRANS_WIDTH
                        && cursor_pos.y >= y
                        && cursor_pos.y <= y + TRANS_HEIGHT
                    {
                        let commit_msg = self.commit_current_edit(state);
                        let key = SfcElementKey::Transition(tr.id.clone());
                        state.selected = Some(key.clone());
                        state.selected_elements = vec![key.clone()];
                        state.drag = DragState::MovingElement {
                            key: key.clone(),
                            offset_x: cursor_pos.x - x,
                            offset_y: cursor_pos.y - y,
                            current_x: x,
                            current_y: y,
                        };
                        return (
                            event::Status::Captured,
                            commit_msg.or_else(|| {
                                Some(Message::SfcSelectElements(
                                    self.pou_name.to_string(),
                                    vec![key],
                                ))
                            }),
                        );
                    }
                }

                // 8. Clicked on empty canvas: commit active edit, clear selection and start box selecting
                let commit_msg = self.commit_current_edit(state);
                state.selected = None;
                state.selected_elements.clear();
                state.drag = DragState::BoxSelecting {
                    start_pt: cursor_pos,
                    current_pt: cursor_pos,
                };
                (
                    event::Status::Captured,
                    commit_msg.or_else(|| {
                        Some(Message::SfcSelectElements(
                            self.pou_name.to_string(),
                            Vec::new(),
                        ))
                    }),
                )
            }

            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                // 1. Check if right-clicking any Step Port to disconnect
                for step in &self.program.steps {
                    let in_p = self.step_in_port(step, state);
                    let out_p = self.step_out_port(step, state);
                    if cursor_pos.distance(in_p) <= PORT_CLICK_RADIUS {
                        return (
                            event::Status::Captured,
                            Some(Message::SfcDisconnectStepInput(
                                self.pou_name.to_string(),
                                step.id.clone(),
                            )),
                        );
                    }
                    if cursor_pos.distance(out_p) <= PORT_CLICK_RADIUS {
                        return (
                            event::Status::Captured,
                            Some(Message::SfcDisconnectStepOutput(
                                self.pou_name.to_string(),
                                step.id.clone(),
                            )),
                        );
                    }
                }
                // 2. Check if right-clicking any Transition Port to disconnect
                for tr in &self.program.transitions {
                    let in_p = self.trans_in_port(tr, state);
                    let out_p = self.trans_out_port(tr, state);
                    if cursor_pos.distance(in_p) <= PORT_CLICK_RADIUS {
                        return (
                            event::Status::Captured,
                            Some(Message::SfcDisconnectTransitionInput(
                                self.pou_name.to_string(),
                                tr.id.clone(),
                            )),
                        );
                    }
                    if cursor_pos.distance(out_p) <= PORT_CLICK_RADIUS {
                        return (
                            event::Status::Captured,
                            Some(Message::SfcDisconnectTransitionOutput(
                                self.pou_name.to_string(),
                                tr.id.clone(),
                            )),
                        );
                    }
                }
                // 3. Check if right-clicking inside a Step box to disconnect all its wires
                for step in self.program.steps.iter().rev() {
                    let (x, y) = self.step_pos(step, state);
                    if cursor_pos.x >= x
                        && cursor_pos.x <= x + STEP_WIDTH
                        && cursor_pos.y >= y
                        && cursor_pos.y <= y + STEP_HEIGHT
                    {
                        return (
                            event::Status::Captured,
                            Some(Message::SfcDisconnectAll(
                                self.pou_name.to_string(),
                                SfcElementKey::Step(step.id.clone()),
                            )),
                        );
                    }
                }
                // 4. Check if right-clicking inside a Transition bar to disconnect all its wires
                for tr in self.program.transitions.iter().rev() {
                    let (x, y) = self.trans_pos(tr, state);
                    if cursor_pos.x >= x
                        && cursor_pos.x <= x + TRANS_WIDTH
                        && cursor_pos.y >= y
                        && cursor_pos.y <= y + TRANS_HEIGHT
                    {
                        return (
                            event::Status::Captured,
                            Some(Message::SfcDisconnectAll(
                                self.pou_name.to_string(),
                                SfcElementKey::Transition(tr.id.clone()),
                            )),
                        );
                    }
                }
                (event::Status::Ignored, None)
            }

            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                match &mut state.drag {
                    DragState::MovingElement {
                        offset_x,
                        offset_y,
                        current_x,
                        current_y,
                        ..
                    } => {
                        *current_x = cursor_pos.x - *offset_x;
                        *current_y = cursor_pos.y - *offset_y;
                        return (event::Status::Captured, None);
                    }
                    DragState::DraggingWire { current_pt, .. } => {
                        *current_pt = cursor_pos;
                    }
                    DragState::BoxSelecting { current_pt, .. } => {
                        *current_pt = cursor_pos;
                    }
                    DragState::None => {}
                }

                if let DragState::BoxSelecting {
                    start_pt,
                    current_pt,
                } = &state.drag
                {
                    let min_x = start_pt.x.min(current_pt.x);
                    let max_x = start_pt.x.max(current_pt.x);
                    let min_y = start_pt.y.min(current_pt.y);
                    let max_y = start_pt.y.max(current_pt.y);

                    let mut selected = Vec::new();
                    for step in &self.program.steps {
                        let (sx, sy) = self.step_pos(step, state);
                        if sx < max_x
                            && sx + STEP_WIDTH > min_x
                            && sy < max_y
                            && sy + STEP_HEIGHT > min_y
                        {
                            selected.push(SfcElementKey::Step(step.id.clone()));
                        }
                    }
                    for tr in &self.program.transitions {
                        let (tx, ty) = self.trans_pos(tr, state);
                        if tx < max_x
                            && tx + TRANS_WIDTH > min_x
                            && ty < max_y
                            && ty + TRANS_HEIGHT > min_y
                        {
                            selected.push(SfcElementKey::Transition(tr.id.clone()));
                        }
                    }
                    state.selected_elements = selected;
                    state.selected = if state.selected_elements.len() == 1 {
                        Some(state.selected_elements[0].clone())
                    } else {
                        None
                    };
                    return (event::Status::Captured, None);
                }

                if let DragState::DraggingWire { source, .. } = &state.drag {
                    let source = source.clone();
                    state.hover_port = None;

                    // Check compatible targets based on source
                    match source {
                        SfcPortSource::StepOutput(src_id) => {
                            for tr in &self.program.transitions {
                                let in_p = self.trans_in_port(tr, state);
                                let out_p = self.trans_out_port(tr, state);
                                let (tx, ty) = self.trans_pos(tr, state);
                                let inside_box = cursor_pos.x >= tx - 12.0
                                    && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                    && cursor_pos.y >= ty - 12.0
                                    && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                    || cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                    || inside_box
                                {
                                    state.hover_port =
                                        Some(PortHover::TransitionInput(tr.id.clone(), in_p));
                                    break;
                                }
                            }
                            if state.hover_port.is_none() {
                                for step in &self.program.steps {
                                    if step.id == src_id {
                                        continue;
                                    }
                                    let in_p = self.step_in_port(step, state);
                                    let (sx, sy) = self.step_pos(step, state);
                                    let inside_box = cursor_pos.x >= sx - 12.0
                                        && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                        && cursor_pos.y >= sy - 12.0
                                        && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                    if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS || inside_box {
                                        state.hover_port =
                                            Some(PortHover::StepInput(step.id.clone(), in_p));
                                        break;
                                    }
                                }
                            }
                        }
                        SfcPortSource::StepInput(src_id) => {
                            for tr in &self.program.transitions {
                                let in_p = self.trans_in_port(tr, state);
                                let out_p = self.trans_out_port(tr, state);
                                let (tx, ty) = self.trans_pos(tr, state);
                                let inside_box = cursor_pos.x >= tx - 12.0
                                    && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                    && cursor_pos.y >= ty - 12.0
                                    && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                    || cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                    || inside_box
                                {
                                    state.hover_port =
                                        Some(PortHover::TransitionOutput(tr.id.clone(), out_p));
                                    break;
                                }
                            }
                            if state.hover_port.is_none() {
                                for step in &self.program.steps {
                                    if step.id == src_id {
                                        continue;
                                    }
                                    let out_p = self.step_out_port(step, state);
                                    let (sx, sy) = self.step_pos(step, state);
                                    let inside_box = cursor_pos.x >= sx - 12.0
                                        && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                        && cursor_pos.y >= sy - 12.0
                                        && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                    if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS || inside_box
                                    {
                                        state.hover_port =
                                            Some(PortHover::StepOutput(step.id.clone(), out_p));
                                        break;
                                    }
                                }
                            }
                        }
                        SfcPortSource::TransitionOutput(src_id) => {
                            for step in &self.program.steps {
                                let in_p = self.step_in_port(step, state);
                                let out_p = self.step_out_port(step, state);
                                let (sx, sy) = self.step_pos(step, state);
                                let inside_box = cursor_pos.x >= sx - 12.0
                                    && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                    && cursor_pos.y >= sy - 12.0
                                    && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                    || cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                    || inside_box
                                {
                                    state.hover_port =
                                        Some(PortHover::StepInput(step.id.clone(), in_p));
                                    break;
                                }
                            }
                            if state.hover_port.is_none() {
                                for tr in &self.program.transitions {
                                    if tr.id == src_id {
                                        continue;
                                    }
                                    let in_p = self.trans_in_port(tr, state);
                                    let (tx, ty) = self.trans_pos(tr, state);
                                    let inside_box = cursor_pos.x >= tx - 12.0
                                        && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                        && cursor_pos.y >= ty - 12.0
                                        && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                    if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS || inside_box {
                                        state.hover_port =
                                            Some(PortHover::TransitionInput(tr.id.clone(), in_p));
                                        break;
                                    }
                                }
                            }
                        }
                        SfcPortSource::TransitionInput(src_id) => {
                            for step in &self.program.steps {
                                let in_p = self.step_in_port(step, state);
                                let out_p = self.step_out_port(step, state);
                                let (sx, sy) = self.step_pos(step, state);
                                let inside_box = cursor_pos.x >= sx - 12.0
                                    && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                    && cursor_pos.y >= sy - 12.0
                                    && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                    || cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                    || inside_box
                                {
                                    state.hover_port =
                                        Some(PortHover::StepOutput(step.id.clone(), out_p));
                                    break;
                                }
                            }
                            if state.hover_port.is_none() {
                                for tr in &self.program.transitions {
                                    if tr.id == src_id {
                                        continue;
                                    }
                                    let out_p = self.trans_out_port(tr, state);
                                    let (tx, ty) = self.trans_pos(tr, state);
                                    let inside_box = cursor_pos.x >= tx - 12.0
                                        && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                        && cursor_pos.y >= ty - 12.0
                                        && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                    if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS || inside_box
                                    {
                                        state.hover_port =
                                            Some(PortHover::TransitionOutput(tr.id.clone(), out_p));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    return (event::Status::Captured, None);
                }

                if matches!(state.drag, DragState::None) {
                    if let Some(ref dd) = state.qualifier_dropdown
                        && dd.rect.contains(cursor_pos)
                    {
                        state.hover_port = None;
                        return (event::Status::Captured, None);
                    }
                    // Check hover on any port
                    state.hover_port = None;
                    for step in &self.program.steps {
                        let in_p = self.step_in_port(step, state);
                        if cursor_pos.distance(in_p) <= PORT_CLICK_RADIUS {
                            state.hover_port = Some(PortHover::StepInput(step.id.clone(), in_p));
                            break;
                        }
                        let out_p = self.step_out_port(step, state);
                        if cursor_pos.distance(out_p) <= PORT_CLICK_RADIUS {
                            state.hover_port = Some(PortHover::StepOutput(step.id.clone(), out_p));
                            break;
                        }
                    }
                    if state.hover_port.is_none() {
                        for tr in &self.program.transitions {
                            let in_p = self.trans_in_port(tr, state);
                            if cursor_pos.distance(in_p) <= PORT_CLICK_RADIUS {
                                state.hover_port =
                                    Some(PortHover::TransitionInput(tr.id.clone(), in_p));
                                break;
                            }
                            let out_p = self.trans_out_port(tr, state);
                            if cursor_pos.distance(out_p) <= PORT_CLICK_RADIUS {
                                state.hover_port =
                                    Some(PortHover::TransitionOutput(tr.id.clone(), out_p));
                                break;
                            }
                        }
                    }
                    return (event::Status::Ignored, None);
                }
                (event::Status::Ignored, None)
            }

            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                match std::mem::replace(&mut state.drag, DragState::None) {
                    DragState::MovingElement {
                        key,
                        current_x,
                        current_y,
                        ..
                    } => {
                        let (snapped_x, snapped_y) = snap_to_grid(current_x, current_y);
                        (
                            event::Status::Captured,
                            Some(Message::SfcMoveElement(
                                self.pou_name.to_string(),
                                key,
                                (snapped_x, snapped_y),
                            )),
                        )
                    }
                    DragState::DraggingWire { source, .. } => {
                        state.hover_port = None;
                        // Check if released over or near a valid target element/port
                        match source {
                            SfcPortSource::StepOutput(src_step) => {
                                for tr in &self.program.transitions {
                                    let in_p = self.trans_in_port(tr, state);
                                    let out_p = self.trans_out_port(tr, state);
                                    let (tx, ty) = self.trans_pos(tr, state);
                                    let inside_box = cursor_pos.x >= tx - 12.0
                                        && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                        && cursor_pos.y >= ty - 12.0
                                        && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                    if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                        || cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                        || inside_box
                                    {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectPorts {
                                                pou_name: self.pou_name.to_string(),
                                                source: SfcPortSource::StepOutput(src_step),
                                                target: SfcPortTarget::TransitionInput(
                                                    tr.id.clone(),
                                                ),
                                            }),
                                        );
                                    }
                                }
                                for step in &self.program.steps {
                                    if step.id == src_step {
                                        continue;
                                    }
                                    let in_p = self.step_in_port(step, state);
                                    let (sx, sy) = self.step_pos(step, state);
                                    let inside_box = cursor_pos.x >= sx - 12.0
                                        && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                        && cursor_pos.y >= sy - 12.0
                                        && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                    if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS || inside_box {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectSteps {
                                                pou_name: self.pou_name.to_string(),
                                                from_step: src_step,
                                                to_step: step.id.clone(),
                                            }),
                                        );
                                    }
                                }
                            }
                            SfcPortSource::StepInput(target_step) => {
                                for tr in &self.program.transitions {
                                    let in_p = self.trans_in_port(tr, state);
                                    let out_p = self.trans_out_port(tr, state);
                                    let (tx, ty) = self.trans_pos(tr, state);
                                    let inside_box = cursor_pos.x >= tx - 12.0
                                        && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                        && cursor_pos.y >= ty - 12.0
                                        && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                    if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                        || cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                        || inside_box
                                    {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectPorts {
                                                pou_name: self.pou_name.to_string(),
                                                source: SfcPortSource::StepInput(target_step),
                                                target: SfcPortTarget::TransitionOutput(
                                                    tr.id.clone(),
                                                ),
                                            }),
                                        );
                                    }
                                }
                                for step in &self.program.steps {
                                    if step.id == target_step {
                                        continue;
                                    }
                                    let out_p = self.step_out_port(step, state);
                                    let (sx, sy) = self.step_pos(step, state);
                                    let inside_box = cursor_pos.x >= sx - 12.0
                                        && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                        && cursor_pos.y >= sy - 12.0
                                        && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                    if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS || inside_box
                                    {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectSteps {
                                                pou_name: self.pou_name.to_string(),
                                                from_step: step.id.clone(),
                                                to_step: target_step,
                                            }),
                                        );
                                    }
                                }
                            }
                            SfcPortSource::TransitionOutput(src_trans) => {
                                for step in &self.program.steps {
                                    let in_p = self.step_in_port(step, state);
                                    let out_p = self.step_out_port(step, state);
                                    let (sx, sy) = self.step_pos(step, state);
                                    let inside_box = cursor_pos.x >= sx - 12.0
                                        && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                        && cursor_pos.y >= sy - 12.0
                                        && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                    if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                        || cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                        || inside_box
                                    {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectPorts {
                                                pou_name: self.pou_name.to_string(),
                                                source: SfcPortSource::TransitionOutput(src_trans),
                                                target: SfcPortTarget::StepInput(step.id.clone()),
                                            }),
                                        );
                                    }
                                }
                                for tr in &self.program.transitions {
                                    if tr.id == src_trans {
                                        continue;
                                    }
                                    let in_p = self.trans_in_port(tr, state);
                                    let (tx, ty) = self.trans_pos(tr, state);
                                    let inside_box = cursor_pos.x >= tx - 12.0
                                        && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                        && cursor_pos.y >= ty - 12.0
                                        && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                    if cursor_pos.distance(in_p) <= PORT_DROP_RADIUS || inside_box {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectTransitions {
                                                pou_name: self.pou_name.to_string(),
                                                from_trans: src_trans,
                                                to_trans: tr.id.clone(),
                                            }),
                                        );
                                    }
                                }
                            }
                            SfcPortSource::TransitionInput(target_trans) => {
                                for step in &self.program.steps {
                                    let in_p = self.step_in_port(step, state);
                                    let out_p = self.step_out_port(step, state);
                                    let (sx, sy) = self.step_pos(step, state);
                                    let inside_box = cursor_pos.x >= sx - 12.0
                                        && cursor_pos.x <= sx + STEP_WIDTH + 12.0
                                        && cursor_pos.y >= sy - 12.0
                                        && cursor_pos.y <= sy + STEP_HEIGHT + 12.0;
                                    if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS
                                        || cursor_pos.distance(in_p) <= PORT_DROP_RADIUS
                                        || inside_box
                                    {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectPorts {
                                                pou_name: self.pou_name.to_string(),
                                                source: SfcPortSource::TransitionInput(
                                                    target_trans,
                                                ),
                                                target: SfcPortTarget::StepOutput(step.id.clone()),
                                            }),
                                        );
                                    }
                                }
                                for tr in &self.program.transitions {
                                    if tr.id == target_trans {
                                        continue;
                                    }
                                    let out_p = self.trans_out_port(tr, state);
                                    let (tx, ty) = self.trans_pos(tr, state);
                                    let inside_box = cursor_pos.x >= tx - 12.0
                                        && cursor_pos.x <= tx + TRANS_WIDTH + 12.0
                                        && cursor_pos.y >= ty - 12.0
                                        && cursor_pos.y <= ty + TRANS_HEIGHT + 12.0;
                                    if cursor_pos.distance(out_p) <= PORT_DROP_RADIUS || inside_box
                                    {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectTransitions {
                                                pou_name: self.pou_name.to_string(),
                                                from_trans: tr.id.clone(),
                                                to_trans: target_trans,
                                            }),
                                        );
                                    }
                                }
                            }
                        }
                        (event::Status::Captured, None)
                    }
                    DragState::BoxSelecting { .. } => {
                        let selected = state.selected_elements.clone();
                        (
                            event::Status::Captured,
                            Some(Message::SfcSelectElements(
                                self.pou_name.to_string(),
                                selected,
                            )),
                        )
                    }
                    DragState::None => (event::Status::Ignored, None),
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
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        let is_occluded = |r: Rectangle| -> bool {
            state.qualifier_dropdown.as_ref().is_some_and(|dd| {
                let dd_expanded = Rectangle {
                    x: dd.rect.x - 2.0,
                    y: dd.rect.y - 1.0,
                    width: dd.rect.width + 4.0,
                    height: dd.rect.height + 3.0,
                };
                dd_expanded.intersects(&r)
            })
        };

        // 1. Background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), StudioTheme::bg_base(theme));

        // 2. Discrete 20px grid dots
        let dot_color = if StudioTheme::is_dark(theme) {
            Color::from_rgba(1.0, 1.0, 1.0, 0.05)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.08)
        };
        for x in (0..width as i32).step_by(20) {
            for y in (0..height as i32).step_by(20) {
                frame.fill_rectangle(
                    Point::new(x as f32, y as f32),
                    Size::new(1.5, 1.5),
                    dot_color,
                );
            }
        }

        // 3. Wires and IEC 60848 AND Double Lines (===)
        // 3. Render Inter-Element Directed Links & Wires
        // Tracks step-transition connections handled by multi-branch bars to prevent redundant wire rendering
        let mut handled_step_out_wires: HashSet<(String, String)> = HashSet::new(); // (step_id, trans_id)
        let mut handled_trans_out_wires: HashSet<(String, String)> = HashSet::new(); // (trans_id, step_id)

        // A. Check for OR Divergences (1 step -> 2+ transitions)
        for step in &self.program.steps {
            let out_trans: Vec<&SfcTransition> = self
                .program
                .transitions
                .iter()
                .filter(|tr| {
                    tr.from_steps.iter().any(|fs| {
                        fs == &step.id || fs == &step.name || fs == &format!("X{}", step.id)
                    })
                })
                .collect();

            if out_trans.len() >= 2 {
                // IEC 60848 OR Divergence: Single horizontal bar under the step!
                let sp = self.step_out_port(step, state);
                let mut min_x = sp.x;
                let mut max_x = sp.x;
                let mut min_tr_y = f32::MAX;

                for tr in &out_trans {
                    let tp = self.trans_in_port(tr, state);
                    min_x = min_x.min(tp.x);
                    max_x = max_x.max(tp.x);
                    min_tr_y = min_tr_y.min(tp.y);
                    handled_step_out_wires.insert((step.id.clone(), tr.id.clone()));
                }

                let bar_y = (sp.y + 18.0).min(min_tr_y - 12.0);
                let bar_left = min_x - 14.0;
                let bar_right = max_x + 14.0;

                // Central wire from step down to horizontal bar
                frame.stroke(
                    &Path::line(sp, Point::new(sp.x, bar_y)),
                    Stroke::default()
                        .with_color(COLOR_INACTIVE_STEP)
                        .with_width(2.0),
                );

                // Single horizontal bar
                frame.stroke(
                    &Path::line(Point::new(bar_left, bar_y), Point::new(bar_right, bar_y)),
                    Stroke::default()
                        .with_color(COLOR_INACTIVE_STEP)
                        .with_width(2.5),
                );

                // Branch wires from bar down to each transition
                for tr in &out_trans {
                    let tp = self.trans_in_port(tr, state);
                    frame.stroke(
                        &Path::line(Point::new(tp.x, bar_y), tp),
                        Stroke::default()
                            .with_color(COLOR_INACTIVE_STEP)
                            .with_width(2.0),
                    );
                }
            }
        }

        // B. Check for OR Convergences (2+ transitions -> 1 step)
        for step in &self.program.steps {
            let sp = self.step_in_port(step, state);
            let in_trans: Vec<&SfcTransition> = self
                .program
                .transitions
                .iter()
                .filter(|tr| {
                    let tp = self.trans_out_port(tr, state);
                    tp.y <= sp.y
                        && tr.to_steps.iter().any(|ts| {
                            ts == &step.id || ts == &step.name || ts == &format!("X{}", step.id)
                        })
                })
                .collect();

            if in_trans.len() >= 2 {
                // IEC 60848 OR Convergence: Single horizontal bar above the step!
                let mut min_x = sp.x;
                let mut max_x = sp.x;
                let mut max_tr_y = 0.0_f32;

                for tr in &in_trans {
                    let tp = self.trans_out_port(tr, state);
                    min_x = min_x.min(tp.x);
                    max_x = max_x.max(tp.x);
                    max_tr_y = max_tr_y.max(tp.y);
                    handled_trans_out_wires.insert((tr.id.clone(), step.id.clone()));
                }

                let bar_y = (sp.y - 18.0).max(max_tr_y + 12.0);
                let bar_left = min_x - 14.0;
                let bar_right = max_x + 14.0;

                // Wires from each transition down to the single horizontal bar
                for tr in &in_trans {
                    let tp = self.trans_out_port(tr, state);
                    frame.stroke(
                        &Path::line(tp, Point::new(tp.x, bar_y)),
                        Stroke::default()
                            .with_color(COLOR_INACTIVE_STEP)
                            .with_width(2.0),
                    );
                }

                // Single horizontal bar
                frame.stroke(
                    &Path::line(Point::new(bar_left, bar_y), Point::new(bar_right, bar_y)),
                    Stroke::default()
                        .with_color(COLOR_INACTIVE_STEP)
                        .with_width(2.5),
                );

                // Central wire from bar into step, with arrowhead
                frame.stroke(
                    &Path::line(Point::new(sp.x, bar_y), sp),
                    Stroke::default()
                        .with_color(COLOR_INACTIVE_STEP)
                        .with_width(2.0),
                );
                draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
            }
        }

        // C. Render transitions incoming (AND Convergence) and outgoing (AND Divergence) or 1-to-1 links
        for tr in &self.program.transitions {
            let tr_in = self.trans_in_port(tr, state);
            let tr_out = self.trans_out_port(tr, state);

            // Incoming wires from source steps -> transition
            let from_steps: Vec<&SfcStep> =
                tr.from_steps
                    .iter()
                    .filter_map(|sid| {
                        self.program.steps.iter().find(|s| {
                            &s.id == sid || &s.name == sid || sid == &format!("X{}", s.id)
                        })
                    })
                    .collect();

            if from_steps.len() >= 2 {
                let is_or = tr.convergence == ConvergenceKind::Or;
                let mut min_x = tr_in.x;
                let mut max_x = tr_in.x;
                let mut max_step_y = 0.0_f32;

                for s in &from_steps {
                    let sp = self.step_out_port(s, state);
                    min_x = min_x.min(sp.x);
                    max_x = max_x.max(sp.x);
                    max_step_y = max_step_y.max(sp.y);
                }

                let bar_y = (tr_in.y - 18.0).max(max_step_y + 10.0);
                let bar_left = min_x - 14.0;
                let bar_right = max_x + 14.0;

                let wire_stroke = Stroke::default()
                    .with_color(COLOR_INACTIVE_STEP)
                    .with_width(2.5);
                if is_or {
                    // IEC 60848 OR Convergence: Single horizontal bar!
                    frame.stroke(
                        &Path::line(Point::new(bar_left, bar_y), Point::new(bar_right, bar_y)),
                        wire_stroke,
                    );

                    // Drop wires from each step to single line
                    for s in &from_steps {
                        let sp = self.step_out_port(s, state);
                        frame.stroke(
                            &Path::line(sp, Point::new(sp.x, bar_y)),
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                    }
                    // Center wire from single line into transition
                    frame.stroke(
                        &Path::line(Point::new(tr_in.x, bar_y), tr_in),
                        Stroke::default()
                            .with_color(COLOR_INACTIVE_STEP)
                            .with_width(2.0),
                    );
                } else {
                    // IEC 60848 AND Convergence: Double horizontal lines!
                    frame.stroke(
                        &Path::line(
                            Point::new(bar_left, bar_y - 2.5),
                            Point::new(bar_right, bar_y - 2.5),
                        ),
                        wire_stroke,
                    );
                    frame.stroke(
                        &Path::line(
                            Point::new(bar_left, bar_y + 2.5),
                            Point::new(bar_right, bar_y + 2.5),
                        ),
                        wire_stroke,
                    );

                    // Drop wires from each step to top line
                    for s in &from_steps {
                        let sp = self.step_out_port(s, state);
                        frame.stroke(
                            &Path::line(sp, Point::new(sp.x, bar_y - 2.5)),
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                    }
                    // Center wire from bottom line into transition
                    frame.stroke(
                        &Path::line(Point::new(tr_in.x, bar_y + 2.5), tr_in),
                        Stroke::default()
                            .with_color(COLOR_INACTIVE_STEP)
                            .with_width(2.0),
                    );
                }
            } else if let Some(s) = from_steps.first() {
                // 1-to-1 link (if not already handled by OR Divergence)
                if !handled_step_out_wires.contains(&(s.id.clone(), tr.id.clone())) {
                    let sp = self.step_out_port(s, state);
                    if (sp.x - tr_in.x).abs() < 2.0 {
                        frame.stroke(
                            &Path::line(sp, tr_in),
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                    } else {
                        let mid_y = (sp.y + tr_in.y) * 0.5;
                        let p1 = Point::new(sp.x, mid_y);
                        let p2 = Point::new(tr_in.x, mid_y);
                        let path = Path::new(|b| {
                            b.move_to(sp);
                            b.line_to(p1);
                            b.line_to(p2);
                            b.line_to(tr_in);
                        });
                        frame.stroke(
                            &path,
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                    }
                }
            }

            // Outgoing wires from transition -> destination steps
            let to_steps: Vec<&SfcStep> =
                tr.to_steps
                    .iter()
                    .filter_map(|sid| {
                        self.program.steps.iter().find(|s| {
                            &s.id == sid || &s.name == sid || sid == &format!("X{}", s.id)
                        })
                    })
                    .collect();

            if to_steps.len() >= 2 {
                let is_or = tr.divergence == DivergenceKind::Or;
                let mut min_x = tr_out.x;
                let mut max_x = tr_out.x;
                let mut min_step_y = f32::MAX;

                for s in &to_steps {
                    let sp = self.step_in_port(s, state);
                    min_x = min_x.min(sp.x);
                    max_x = max_x.max(sp.x);
                    min_step_y = min_step_y.min(sp.y);
                }

                let bar_y = (tr_out.y + 18.0).min(min_step_y - 10.0);
                let bar_left = min_x - 14.0;
                let bar_right = max_x + 14.0;

                let wire_stroke = Stroke::default()
                    .with_color(COLOR_INACTIVE_STEP)
                    .with_width(2.5);
                if is_or {
                    // IEC 60848 OR Divergence: Single horizontal bar!
                    frame.stroke(
                        &Path::line(Point::new(bar_left, bar_y), Point::new(bar_right, bar_y)),
                        wire_stroke,
                    );

                    // Central wire from transition into single line
                    frame.stroke(
                        &Path::line(tr_out, Point::new(tr_out.x, bar_y)),
                        Stroke::default()
                            .with_color(COLOR_INACTIVE_STEP)
                            .with_width(2.0),
                    );

                    // Drop wires from single line to each target step, with arrowhead
                    for s in &to_steps {
                        let sp = self.step_in_port(s, state);
                        frame.stroke(
                            &Path::line(Point::new(sp.x, bar_y), sp),
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                        draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
                    }
                } else {
                    // IEC 60848 AND Divergence: Double horizontal lines!
                    frame.stroke(
                        &Path::line(
                            Point::new(bar_left, bar_y - 2.5),
                            Point::new(bar_right, bar_y - 2.5),
                        ),
                        wire_stroke,
                    );
                    frame.stroke(
                        &Path::line(
                            Point::new(bar_left, bar_y + 2.5),
                            Point::new(bar_right, bar_y + 2.5),
                        ),
                        wire_stroke,
                    );

                    // Central wire from transition into top line
                    frame.stroke(
                        &Path::line(tr_out, Point::new(tr_out.x, bar_y - 2.5)),
                        Stroke::default()
                            .with_color(COLOR_INACTIVE_STEP)
                            .with_width(2.0),
                    );

                    // Drop wires from bottom line to each target step, with arrowhead
                    for s in &to_steps {
                        let sp = self.step_in_port(s, state);
                        frame.stroke(
                            &Path::line(Point::new(sp.x, bar_y + 2.5), sp),
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                        draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
                    }
                }
            } else if let Some(s) = to_steps.first() {
                // 1-to-1 link (if not already handled by OR Convergence)
                if !handled_trans_out_wires.contains(&(tr.id.clone(), s.id.clone())) {
                    let sp = self.step_in_port(s, state);
                    if sp.y >= tr_out.y {
                        if (sp.x - tr_out.x).abs() < 2.0 {
                            frame.stroke(
                                &Path::line(tr_out, sp),
                                Stroke::default()
                                    .with_color(COLOR_INACTIVE_STEP)
                                    .with_width(2.0),
                            );
                        } else {
                            let mid_y = (tr_out.y + sp.y) * 0.5;
                            let p1 = Point::new(tr_out.x, mid_y);
                            let p2 = Point::new(sp.x, mid_y);
                            let path = Path::new(|b| {
                                b.move_to(tr_out);
                                b.line_to(p1);
                                b.line_to(p2);
                                b.line_to(sp);
                            });
                            frame.stroke(
                                &path,
                                Stroke::default()
                                    .with_color(COLOR_INACTIVE_STEP)
                                    .with_width(2.0),
                            );
                        }
                        draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
                    } else {
                        // Loopback link towards a previous step:
                        // Route horizontally to the clear left edge before going up to avoid clipping states
                        let side_x =
                            self.compute_sfc_loopback_side_x(sp, tr_out, &tr.id, &s.id, state);
                        let top_y = (sp.y - 12.0).max(6.0);
                        let bottom_y = tr_out.y + 12.0;
                        let p1 = Point::new(tr_out.x, bottom_y);
                        let p2 = Point::new(side_x, bottom_y);
                        let p3 = Point::new(side_x, top_y);
                        let p4 = Point::new(sp.x, top_y);
                        let path = Path::new(|b| {
                            b.move_to(tr_out);
                            b.line_to(p1);
                            b.line_to(p2);
                            b.line_to(p3);
                            b.line_to(p4);
                            b.line_to(sp);
                        });
                        frame.stroke(
                            &path,
                            Stroke::default()
                                .with_color(COLOR_INACTIVE_STEP)
                                .with_width(2.0),
                        );
                        draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
                    }
                }
            }
        }

        // 4. Render Steps
        for step in &self.program.steps {
            let (x, y) = self.step_pos(step, state);
            let (active, elapsed_ms) = self
                .runtime
                .map(|rt| is_step_active_with_name(rt, &self.program.name, &step.id, &step.name))
                .unwrap_or((false, 0));

            let is_selected = self.selected == Some(&SfcElementKey::Step(step.id.clone()))
                || state.selected.as_ref() == Some(&SfcElementKey::Step(step.id.clone()))
                || self
                    .selected_elements
                    .contains(&SfcElementKey::Step(step.id.clone()))
                || state
                    .selected_elements
                    .contains(&SfcElementKey::Step(step.id.clone()));

            let (tree_id, has_error, _err_msg) = if let Some(ref report) = self.validation_report {
                let t_id = report
                    .trees
                    .iter()
                    .find(|t| t.step_ids.contains(&step.id))
                    .map(|t| t.tree_id);
                let mut err = None;
                for e in &report.errors {
                    match e {
                        SfcValidationError::MissingInitialStepInTree { step_ids, .. } => {
                            if step_ids.contains(&step.id) {
                                err = Some("Missing Initial Step");
                                break;
                            }
                        }
                        SfcValidationError::InvalidAlternation { element_id, .. } => {
                            if element_id == &step.id {
                                err = Some("Alternation Error");
                                break;
                            }
                        }
                        SfcValidationError::OrphanElement {
                            element_id,
                            is_step,
                        } if *is_step && element_id == &step.id => {
                            err = Some("Unlinked Step");
                            break;
                        }
                        SfcValidationError::InvalidOrDivergence { element_id, .. }
                            if element_id == &step.id =>
                        {
                            err = Some("Invalid OR Divergence");
                            break;
                        }
                        SfcValidationError::InvalidOrConvergence { element_id, .. }
                            if element_id == &step.id =>
                        {
                            err = Some("Invalid OR Convergence");
                            break;
                        }
                        _ => {}
                    }
                }
                (t_id, err.is_some(), err)
            } else {
                (None, false, None)
            };

            let step_timeout = active
                && step
                    .max_duration_ms()
                    .is_some_and(|max_ms| elapsed_ms > max_ms);

            let border_color = if has_error || step_timeout {
                Color::from_rgb(1.0, 0.35, 0.35)
            } else if active {
                COLOR_ACTIVE_STEP
            } else if is_selected {
                COLOR_STEP_DROP
            } else {
                COLOR_INACTIVE_STEP
            };

            // Glow / fill
            let bg_color = if step_timeout {
                Color::from_rgb(56.0 / 255.0, 24.0 / 255.0, 28.0 / 255.0)
            } else if active {
                Color::from_rgb(22.0 / 255.0, 46.0 / 255.0, 34.0 / 255.0)
            } else if StudioTheme::is_dark(theme) {
                COLOR_STEP_BG
            } else {
                StudioTheme::bg_elevated(theme)
            };

            let s_width = self.step_width(step);

            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(s_width, STEP_HEIGHT),
                bg_color,
            );
            frame.stroke_rectangle(
                Point::new(x, y),
                Size::new(s_width, STEP_HEIGHT),
                Stroke::default()
                    .with_color(border_color)
                    .with_width(if is_selected || has_error || step_timeout { 2.5 } else { 2.0 }),
            );

            // Error halo outline
            if has_error || step_timeout {
                frame.stroke_rectangle(
                    Point::new(x - 2.0, y - 2.0),
                    Size::new(s_width + 4.0, STEP_HEIGHT + 4.0),
                    Stroke::default()
                        .with_color(Color::from_rgba(1.0, 0.3, 0.3, 0.4))
                        .with_width(2.0),
                );
            }

            // Initial Step: authentic double inner border
            if step.kind == StepKind::Initial {
                frame.stroke_rectangle(
                    Point::new(x + 3.0, y + 3.0),
                    Size::new(s_width - 6.0, STEP_HEIGHT - 6.0),
                    Stroke::default().with_color(border_color).with_width(1.2),
                );
            } else if step.kind == StepKind::Macro {
                // IEC 60848 Macro Step: double horizontal bars at top and bottom of step box
                frame.stroke(
                    &Path::line(Point::new(x + 2.0, y + 4.0), Point::new(x + s_width - 2.0, y + 4.0)),
                    Stroke::default().with_color(StudioTheme::ACCENT_PURPLE).with_width(1.5),
                );
                frame.stroke(
                    &Path::line(
                        Point::new(x + 2.0, y + STEP_HEIGHT - 4.0),
                        Point::new(x + s_width - 2.0, y + STEP_HEIGHT - 4.0),
                    ),
                    Stroke::default().with_color(StudioTheme::ACCENT_PURPLE).with_width(1.5),
                );
                let macro_badge = if step.macro_body.is_empty() {
                    "MACRO".to_string()
                } else {
                    format!("MACRO({})", step.macro_body.len())
                };
                let macro_rect = Rectangle {
                    x: x + s_width - 96.0,
                    y: y + 6.0,
                    width: 60.0,
                    height: 16.0,
                };
                if !is_occluded(macro_rect) {
                    frame.fill_text(canvas::Text {
                        content: macro_badge,
                        position: Point::new(x + s_width - 96.0, y + 6.0),
                        color: StudioTheme::ACCENT_PURPLE,
                        size: iced::Pixels(9.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });
                }
            }

            // Step ID text
            let num_id = step.id.trim_start_matches(['X', 'x']);
            let display_id = format!("X{num_id}");
            frame.fill_text(canvas::Text {
                content: display_id.clone(),
                position: Point::new(x + 10.0, y + 8.0),
                color: if StudioTheme::is_dark(theme) {
                    Color::WHITE
                } else {
                    StudioTheme::text_primary(theme)
                },
                size: iced::Pixels(13.0),
                font: iced::Font::MONOSPACE,
                ..Default::default()
            });

            // Tree badge or error badge
            if has_error {
                frame.fill_text(canvas::Text {
                    content: display_id,
                    position: Point::new(x + 10.0, y + 7.0),
                    color: Color::WHITE,
                    size: iced::Pixels(12.5),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            // Active dot & timer
            if active {
                let active_rect = Rectangle {
                    x: x + 48.0,
                    y: y + 7.0,
                    width: 70.0,
                    height: 16.0,
                };
                if !is_occluded(active_rect) {
                    frame.fill_text(canvas::Text {
                        content: "●".to_string(),
                        position: Point::new(x + 48.0, y + 7.0),
                        color: COLOR_ACTIVE_STEP,
                        size: iced::Pixels(11.0),
                        ..Default::default()
                    });
                    frame.fill_text(canvas::Text {
                        content: format!("T#{}ms", elapsed_ms),
                        position: Point::new(x + 64.0, y + 8.0),
                        color: COLOR_ACTIVE_STEP,
                        size: iced::Pixels(10.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });
                }
            }

            // On-canvas [+ Act] button on top right of step header
            let add_rect = self.step_add_action_rect(step, state);
            if !is_occluded(add_rect) {
                frame.fill_rectangle(
                    Point::new(add_rect.x, add_rect.y),
                    Size::new(add_rect.width, add_rect.height),
                    Color::from_rgba(0.2, 0.45, 0.7, 0.25),
                );
                frame.stroke_rectangle(
                    Point::new(add_rect.x, add_rect.y),
                    Size::new(add_rect.width, add_rect.height),
                    Stroke::default()
                        .with_color(Color::from_rgba(0.3, 0.6, 0.9, 0.5))
                        .with_width(1.0),
                );
                frame.fill_text(canvas::Text {
                    content: "+ Act".to_string(),
                    position: Point::new(add_rect.x + 5.0, add_rect.y + 2.5),
                    color: Color::from_rgb(0.7, 0.85, 1.0),
                    size: iced::Pixels(10.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            // Tree badge or error badge
            let badge_rect = Rectangle {
                x: add_rect.x - 24.0,
                y: y + 6.0,
                width: 22.0,
                height: 16.0,
            };
            if !is_occluded(badge_rect) {
                if has_error {
                    frame.fill_text(canvas::Text {
                        content: crate::icons::icon_warning().to_string(),
                        position: Point::new(add_rect.x - 20.0, y + 6.0),
                        color: Color::from_rgb(1.0, 0.35, 0.35),
                        size: iced::Pixels(11.0),
                        shaping: iced::widget::text::Shaping::Advanced,
                        ..Default::default()
                    });
                } else if let Some(tid) = tree_id
                    && self
                        .validation_report
                        .as_ref()
                        .map(|r| r.trees.len())
                        .unwrap_or(0)
                        > 1
                {
                    frame.fill_text(canvas::Text {
                        content: format!("T{}", tid + 1),
                        position: Point::new(add_rect.x - 22.0, y + 7.0),
                        color: COLOR_STEP_DROP,
                        size: iced::Pixels(10.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });
                }
            }

            // Step Name area with in-place edit support and anti-clipping
            let name_rect = self.step_name_rect(step, state);
            if !is_occluded(name_rect) {
                let is_editing_name = state.editing_target
                    == Some(SfcEditTarget::StepName {
                        step_id: step.id.clone(),
                    });

                frame.fill_rectangle(
                    Point::new(name_rect.x, name_rect.y),
                    Size::new(name_rect.width, name_rect.height),
                    if is_editing_name {
                        Color::from_rgba(0.08, 0.18, 0.28, 0.9)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.25)
                    },
                );
                frame.stroke_rectangle(
                    Point::new(name_rect.x, name_rect.y),
                    Size::new(name_rect.width, name_rect.height),
                    Stroke::default()
                        .with_color(if is_editing_name {
                            COLOR_STEP_DROP
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                        })
                        .with_width(if is_editing_name { 1.5 } else { 1.0 }),
                );

                let display_name = if is_editing_name {
                    format!("{}|", state.edit_buffer)
                } else {
                    step.name.clone()
                };

                frame.fill_text(canvas::Text {
                    content: display_name,
                    position: Point::new(name_rect.x + 6.0, name_rect.y + 4.0),
                    color: if is_editing_name {
                        Color::WHITE
                    } else {
                        Color::from_rgb(220.0 / 255.0, 230.0 / 255.0, 245.0 / 255.0)
                    },
                    size: iced::Pixels(11.5),
                    ..Default::default()
                });
            }

            // Action blocks attached on right side
            if !step.actions.is_empty() {
                let wire_start = Point::new(x + s_width, y + STEP_HEIGHT * 0.5);
                let wire_end = Point::new(x + s_width + 14.0, y + STEP_HEIGHT * 0.5);
                frame.stroke(
                    &Path::line(wire_start, wire_end),
                    Stroke::default().with_color(border_color).with_width(1.5),
                );

                let start_act_y = y + 4.0;
                let mut last_act_bottom = start_act_y;
                for (a_idx, act) in step.actions.iter().enumerate() {
                    let q_rect = self.action_qualifier_rect(step, a_idx, state);
                    let t_rect = self.action_target_rect(step, a_idx, state);
                    let dur_rect_opt = self.action_duration_rect(step, a_idx, state);
                    let del_rect = self.action_delete_rect(step, a_idx, state);
                    last_act_bottom = q_rect.y + q_rect.height;

                    let row_rect = Rectangle {
                        x: q_rect.x,
                        y: q_rect.y,
                        width: (del_rect.x + del_rect.width) - q_rect.x,
                        height: q_rect.height,
                    };
                    if is_occluded(row_rect) {
                        continue;
                    }

                    // Qualifier badge with color coding and dropdown indicator
                    let qual_color = qualifier_badge_color(act.qualifier);
                    let qual_str = SfcActionQualifierWrapper(act.qualifier).to_string();
                    let act_bg = if StudioTheme::is_dark(theme) {
                        COLOR_ACTION_BG
                    } else {
                        StudioTheme::bg_surface(theme)
                    };
                    frame.fill_rectangle(
                        Point::new(q_rect.x, q_rect.y),
                        Size::new(q_rect.width, q_rect.height),
                        act_bg,
                    );
                    frame.stroke_rectangle(
                        Point::new(q_rect.x, q_rect.y),
                        Size::new(q_rect.width, q_rect.height),
                        Stroke::default().with_color(qual_color).with_width(1.2),
                    );
                    frame.fill_text(canvas::Text {
                        content: qual_str.clone(),
                        position: Point::new(
                            q_rect.x + if qual_str.len() == 1 { 6.0 } else { 3.5 },
                            q_rect.y + 4.0,
                        ),
                        color: if StudioTheme::is_dark(theme) {
                            Color::WHITE
                        } else {
                            StudioTheme::text_primary(theme)
                        },
                        size: iced::Pixels(11.0),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });
                    // Downward indicator triangle
                    let tri_x = q_rect.x + if qual_str.len() == 1 { 20.0 } else { 23.0 };
                    let tri_y = q_rect.y + 9.0;
                    frame.fill(
                        &Path::new(|b| {
                            b.move_to(Point::new(tri_x, tri_y));
                            b.line_to(Point::new(tri_x + 6.0, tri_y));
                            b.line_to(Point::new(tri_x + 3.0, tri_y + 4.0));
                            b.close();
                        }),
                        Color::from_rgba(1.0, 1.0, 1.0, 0.75),
                    );

                    // Target box with in-place editing
                    let is_editing_target = state.editing_target
                        == Some(SfcEditTarget::ActionTarget {
                            step_id: step.id.clone(),
                            action_idx: a_idx,
                        });
                    let target_display = if is_editing_target {
                        format!("{}|", state.edit_buffer)
                    } else {
                        act.target.clone()
                    };
                    let target_bg = if StudioTheme::is_dark(theme) {
                        COLOR_STEP_BG
                    } else {
                        StudioTheme::bg_elevated(theme)
                    };
                    frame.fill_rectangle(
                        Point::new(t_rect.x, t_rect.y),
                        Size::new(t_rect.width, t_rect.height),
                        if is_editing_target {
                            Color::from_rgba(0.08, 0.18, 0.28, 0.95)
                        } else {
                            target_bg
                        },
                    );
                    frame.stroke_rectangle(
                        Point::new(t_rect.x, t_rect.y),
                        Size::new(t_rect.width, t_rect.height),
                        Stroke::default()
                            .with_color(if is_editing_target {
                                COLOR_STEP_DROP
                            } else {
                                COLOR_INACTIVE_STEP
                            })
                            .with_width(if is_editing_target { 1.5 } else { 1.0 }),
                    );
                    frame.fill_text(canvas::Text {
                        content: target_display,
                        position: Point::new(t_rect.x + 6.0, t_rect.y + 4.0),
                        color: if StudioTheme::is_dark(theme) {
                            Color::from_rgb(200.0 / 255.0, 215.0 / 255.0, 230.0 / 255.0)
                        } else {
                            StudioTheme::text_primary(theme)
                        },
                        size: iced::Pixels(10.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    // Duration box (if L or D)
                    if let Some(dur_rect) = dur_rect_opt {
                        let is_editing_dur = state.editing_target
                            == Some(SfcEditTarget::ActionDuration {
                                step_id: step.id.clone(),
                                action_idx: a_idx,
                            });
                        let dur_display = if is_editing_dur {
                            format!("{}|", state.edit_buffer)
                        } else {
                            act.duration.clone().unwrap_or_else(|| "T#5s".to_string())
                        };

                        frame.fill_rectangle(
                            Point::new(dur_rect.x, dur_rect.y),
                            Size::new(dur_rect.width, dur_rect.height),
                            if is_editing_dur {
                                Color::from_rgba(0.08, 0.18, 0.28, 0.95)
                            } else {
                                COLOR_ACTION_BG
                            },
                        );
                        frame.stroke_rectangle(
                            Point::new(dur_rect.x, dur_rect.y),
                            Size::new(dur_rect.width, dur_rect.height),
                            Stroke::default()
                                .with_color(if is_editing_dur {
                                    COLOR_STEP_DROP
                                } else {
                                    COLOR_INACTIVE_STEP
                                })
                                .with_width(1.0),
                        );
                        frame.fill_text(canvas::Text {
                            content: dur_display,
                            position: Point::new(dur_rect.x + 4.0, dur_rect.y + 4.0),
                            color: Color::from_rgb(0.7, 0.85, 1.0),
                            size: iced::Pixels(10.0),
                            font: iced::Font::MONOSPACE,
                            ..Default::default()
                        });
                    }

                    // Delete [×] button
                    frame.fill_rectangle(
                        Point::new(del_rect.x, del_rect.y),
                        Size::new(del_rect.width, del_rect.height),
                        Color::from_rgba(1.0, 0.3, 0.3, 0.08),
                    );
                    frame.stroke_rectangle(
                        Point::new(del_rect.x, del_rect.y),
                        Size::new(del_rect.width, del_rect.height),
                        Stroke::default()
                            .with_color(Color::from_rgba(1.0, 0.3, 0.3, 0.25))
                            .with_width(1.0),
                    );
                    frame.fill_text(canvas::Text {
                        content: "×".to_string(),
                        position: Point::new(del_rect.x + 5.0, del_rect.y + 3.0),
                        color: Color::from_rgb(1.0, 0.45, 0.45),
                        size: iced::Pixels(12.0),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });
                }

                if step.actions.len() > 1 {
                    let first_act_y = start_act_y + 11.0;
                    let last_act_y = last_act_bottom - 11.0;
                    frame.stroke(
                        &Path::line(
                            Point::new(wire_end.x, first_act_y.min(wire_end.y)),
                            Point::new(wire_end.x, last_act_y.max(wire_end.y)),
                        ),
                        Stroke::default().with_color(border_color).with_width(1.5),
                    );
                }
            }

            // Input / Output Ports
            let in_p = self.step_in_port(step, state);
            let out_p = self.step_out_port(step, state);

            if !is_occluded(Rectangle {
                x: in_p.x - 6.0,
                y: in_p.y - 6.0,
                width: 12.0,
                height: 12.0,
            }) {
                let is_in_hovered =
                    state.hover_port.as_ref() == Some(&PortHover::StepInput(step.id.clone(), in_p));
                frame.fill(
                    &Path::circle(in_p, if is_in_hovered { 6.0 } else { PORT_RADIUS }),
                    if is_in_hovered {
                        COLOR_STEP_DROP
                    } else {
                        border_color
                    },
                );
            }
            if !is_occluded(Rectangle {
                x: out_p.x - 6.0,
                y: out_p.y - 6.0,
                width: 12.0,
                height: 12.0,
            }) {
                let is_out_hovered =
                    state.hover_port.as_ref() == Some(&PortHover::StepOutput(step.id.clone(), out_p));
                frame.fill(
                    &Path::circle(out_p, if is_out_hovered { 6.0 } else { PORT_RADIUS }),
                    if is_out_hovered {
                        COLOR_STEP_DROP
                    } else {
                        border_color
                    },
                );
            }
        }

        // 5. Render Transitions
        for tr in &self.program.transitions {
            let (x, y) = self.trans_pos(tr, state);
            let is_selected = self.selected == Some(&SfcElementKey::Transition(tr.id.clone()))
                || state.selected.as_ref() == Some(&SfcElementKey::Transition(tr.id.clone()))
                || self
                    .selected_elements
                    .contains(&SfcElementKey::Transition(tr.id.clone()))
                || state
                    .selected_elements
                    .contains(&SfcElementKey::Transition(tr.id.clone()));

            let (tree_id, has_error, _err_msg) = if let Some(ref report) = self.validation_report {
                let t_id = report
                    .trees
                    .iter()
                    .find(|t| t.transition_ids.contains(&tr.id))
                    .map(|t| t.tree_id);
                let mut err = None;
                for e in &report.errors {
                    match e {
                        SfcValidationError::DanglingStepReference { transition_id, .. } => {
                            if transition_id == &tr.id {
                                err = Some("Dangling Step");
                                break;
                            }
                        }
                        SfcValidationError::UnclosedAndBranch { transition_id, .. } => {
                            if transition_id == &tr.id {
                                err = Some("Unclosed Branch");
                                break;
                            }
                        }
                        SfcValidationError::InvalidAlternation { element_id, .. } => {
                            if element_id == &tr.id {
                                err = Some("Alternation Error");
                                break;
                            }
                        }
                        SfcValidationError::OrphanElement {
                            element_id,
                            is_step,
                        } if !*is_step && element_id == &tr.id => {
                            err = Some("Unlinked Transition");
                            break;
                        }
                        SfcValidationError::InvalidAndDivergence { transition_id, .. }
                            if transition_id == &tr.id =>
                        {
                            err = Some("Invalid AND Divergence");
                            break;
                        }
                        SfcValidationError::InvalidAndConvergence { transition_id, .. }
                            if transition_id == &tr.id =>
                        {
                            err = Some("Invalid AND Convergence");
                            break;
                        }
                        SfcValidationError::InvalidOrDivergence { element_id, .. }
                            if element_id == &tr.id =>
                        {
                            err = Some("Invalid OR Divergence");
                            break;
                        }
                        SfcValidationError::InvalidOrConvergence { element_id, .. }
                            if element_id == &tr.id =>
                        {
                            err = Some("Invalid OR Convergence");
                            break;
                        }
                        _ => {}
                    }
                }
                (t_id, err.is_some(), err)
            } else {
                (None, false, None)
            };

            let color = if has_error {
                Color::from_rgb(1.0, 0.35, 0.35)
            } else if is_selected {
                COLOR_STEP_DROP
            } else {
                COLOR_TRANSITION
            };

            let cy = y + TRANS_HEIGHT * 0.5;

            let bar_rect = Rectangle {
                x: x - 4.0,
                y: cy - 4.0,
                width: TRANS_WIDTH + 8.0,
                height: 8.0,
            };
            if !is_occluded(bar_rect) {
                // Error halo around transition crossbar
                if has_error {
                    frame.stroke(
                        &Path::line(
                            Point::new(x - 3.0, cy),
                            Point::new(x + TRANS_WIDTH + 3.0, cy),
                        ),
                        Stroke::default()
                            .with_color(Color::from_rgba(1.0, 0.3, 0.3, 0.4))
                            .with_width(6.0),
                    );
                }

                // Authentic IEC 60848 transition crossbar
                frame.stroke(
                    &Path::line(Point::new(x, cy), Point::new(x + TRANS_WIDTH, cy)),
                    Stroke::default()
                        .with_color(color)
                        .with_width(if is_selected || has_error { 4.0 } else { 3.2 }),
                );
            }

            // Receptivity text & pill with in-place edit support
            let rec_rect = self.trans_receptivity_rect(tr, state);
            if !is_occluded(rec_rect) {
                let is_editing_rec = state.editing_target
                    == Some(SfcEditTarget::TransitionReceptivity {
                        trans_id: tr.id.clone(),
                    });

                frame.fill_rectangle(
                    Point::new(rec_rect.x, rec_rect.y),
                    Size::new(rec_rect.width, rec_rect.height),
                    if is_editing_rec {
                        Color::from_rgba(0.08, 0.18, 0.28, 0.95)
                    } else {
                        COLOR_STEP_BG
                    },
                );
                frame.stroke_rectangle(
                    Point::new(rec_rect.x, rec_rect.y),
                    Size::new(rec_rect.width, rec_rect.height),
                    Stroke::default()
                        .with_color(if is_editing_rec {
                            COLOR_STEP_DROP
                        } else if has_error {
                            Color::from_rgb(1.0, 0.4, 0.4)
                        } else {
                            Color::from_rgba(1.0, 210.0 / 255.0, 120.0 / 255.0, 0.35)
                        })
                        .with_width(if is_editing_rec { 1.5 } else { 1.0 }),
                );

                let rec_text = if is_editing_rec {
                    format!("{}|", state.edit_buffer)
                } else if tr.receptivity.is_empty() {
                    "TRUE".to_string()
                } else {
                    tr.receptivity.clone()
                };

                frame.fill_text(canvas::Text {
                    content: rec_text,
                    position: Point::new(rec_rect.x + 5.0, rec_rect.y + 3.5),
                    color: if has_error {
                        Color::from_rgb(1.0, 0.4, 0.4)
                    } else {
                        Color::from_rgb(1.0, 210.0 / 255.0, 120.0 / 255.0)
                    },
                    size: iced::Pixels(11.5),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            // Transition ID and tree or error badge
            let tid_rect = Rectangle {
                x: x - 65.0,
                y: cy - 10.0,
                width: 60.0,
                height: 18.0,
            };
            if !is_occluded(tid_rect) {
                let tid_str = if has_error {
                    format!("{} {}", tr.id, crate::icons::icon_warning())
                } else if let Some(tid) = tree_id
                    && self
                        .validation_report
                        .as_ref()
                        .map(|r| r.trees.len())
                        .unwrap_or(0)
                        > 1
                {
                    format!("{} T{}", tr.id, tid + 1)
                } else {
                    tr.id.clone()
                };

                frame.fill_text(canvas::Text {
                    content: tid_str,
                    position: Point::new(x - 8.0, cy - 6.0),
                    color: if has_error {
                        Color::from_rgb(1.0, 0.35, 0.35)
                    } else {
                        COLOR_INACTIVE_STEP
                    },
                    size: iced::Pixels(10.5),
                    font: iced::Font::MONOSPACE,
                    horizontal_alignment: iced::alignment::Horizontal::Right,
                    shaping: iced::widget::text::Shaping::Advanced,
                    ..Default::default()
                });
            }

            // Input / Output Ports
            let in_p = self.trans_in_port(tr, state);
            let out_p = self.trans_out_port(tr, state);

            if !is_occluded(Rectangle {
                x: in_p.x - 6.0,
                y: in_p.y - 6.0,
                width: 12.0,
                height: 12.0,
            }) {
                let is_in_hovered =
                    state.hover_port.as_ref() == Some(&PortHover::TransitionInput(tr.id.clone(), in_p));
                frame.fill(
                    &Path::circle(in_p, if is_in_hovered { 6.0 } else { PORT_RADIUS }),
                    if is_in_hovered {
                        COLOR_STEP_DROP
                    } else {
                        color
                    },
                );
            }
            if !is_occluded(Rectangle {
                x: out_p.x - 6.0,
                y: out_p.y - 6.0,
                width: 12.0,
                height: 12.0,
            }) {
                let is_out_hovered = state.hover_port.as_ref()
                    == Some(&PortHover::TransitionOutput(tr.id.clone(), out_p));
                frame.fill(
                    &Path::circle(out_p, if is_out_hovered { 6.0 } else { PORT_RADIUS }),
                    if is_out_hovered {
                        COLOR_STEP_DROP
                    } else {
                        color
                    },
                );
            }
        }

        // 6. Active Wire Dragging Elastic Rubber-Band
        if let DragState::DraggingWire {
            start_pt,
            current_pt,
            ..
        } = state.drag
        {
            let snap_pt = state
                .hover_port
                .as_ref()
                .map(|h| match h {
                    PortHover::StepInput(_, p)
                    | PortHover::StepOutput(_, p)
                    | PortHover::TransitionInput(_, p)
                    | PortHover::TransitionOutput(_, p) => *p,
                })
                .unwrap_or(current_pt);
            let wire_stroke = Stroke::default()
                .with_color(COLOR_STEP_DROP)
                .with_width(2.2);
            frame.stroke(&Path::line(start_pt, snap_pt), wire_stroke);
            frame.fill(&Path::circle(snap_pt, 5.0), COLOR_STEP_DROP);
        }

        // 7. Active Box Selection Rubber-Band
        if let DragState::BoxSelecting {
            start_pt,
            current_pt,
        } = state.drag
        {
            let min_x = start_pt.x.min(current_pt.x);
            let max_x = start_pt.x.max(current_pt.x);
            let min_y = start_pt.y.min(current_pt.y);
            let max_y = start_pt.y.max(current_pt.y);
            let w = (max_x - min_x).max(1.0);
            let h = (max_y - min_y).max(1.0);

            let sel_rect_bg = Color::from_rgba(72.0 / 255.0, 202.0 / 255.0, 228.0 / 255.0, 0.12);
            frame.fill_rectangle(Point::new(min_x, min_y), Size::new(w, h), sel_rect_bg);
            frame.stroke_rectangle(
                Point::new(min_x, min_y),
                Size::new(w, h),
                Stroke::default()
                    .with_color(COLOR_STEP_DROP)
                    .with_width(1.5),
            );
        }

        let base_geom = frame.into_geometry();

        // 8. Open Qualifier Dropdown Floating Menu (in a dedicated overlay Geometry layer rendered in front of everything)
        if let Some(ref dd) = state.qualifier_dropdown {
            let mut overlay_frame = Frame::new(renderer, bounds.size());

            // Dark diffuse outer drop shadow
            overlay_frame.fill_rectangle(
                Point::new(dd.rect.x - 3.0, dd.rect.y - 2.0),
                Size::new(dd.rect.width + 6.0, dd.rect.height + 6.0),
                Color::from_rgba(0.0, 0.0, 0.0, 0.70),
            );
            // Dropdown container background: 100% solid and fully opaque
            overlay_frame.fill_rectangle(
                Point::new(dd.rect.x, dd.rect.y),
                Size::new(dd.rect.width, dd.rect.height),
                Color::from_rgb(18.0 / 255.0, 24.0 / 255.0, 36.0 / 255.0),
            );
            overlay_frame.stroke_rectangle(
                Point::new(dd.rect.x, dd.rect.y),
                Size::new(dd.rect.width, dd.rect.height),
                Stroke::default()
                    .with_color(COLOR_STEP_DROP)
                    .with_width(1.5),
            );

            // Current qualifier for this action
            let current_q = self
                .program
                .steps
                .iter()
                .find(|s| s.id == dd.step_id)
                .and_then(|s| s.actions.get(dd.action_idx))
                .map(|a| a.qualifier);

            let hovered_idx = cursor.position_in(bounds).and_then(|pos| {
                if dd.rect.contains(pos) {
                    let rel_y = pos.y - (dd.rect.y + 2.0);
                    let idx = (rel_y / 22.0) as usize;
                    if idx < SFC_QUALIFIER_ITEMS.len() {
                        Some(idx)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            for (idx, (qual, code, desc)) in SFC_QUALIFIER_ITEMS.iter().enumerate() {
                let row_y = dd.rect.y + 2.0 + (idx as f32) * 22.0;
                let is_current = current_q == Some(*qual);
                let is_hovered = hovered_idx == Some(idx);

                // Row background: solid, fully opaque
                let row_bg = if is_hovered {
                    Color::from_rgb(38.0 / 255.0, 56.0 / 255.0, 84.0 / 255.0)
                } else if is_current {
                    Color::from_rgb(26.0 / 255.0, 44.0 / 255.0, 68.0 / 255.0)
                } else {
                    Color::from_rgb(20.0 / 255.0, 26.0 / 255.0, 38.0 / 255.0)
                };
                overlay_frame.fill_rectangle(
                    Point::new(dd.rect.x + 2.0, row_y),
                    Size::new(dd.rect.width - 4.0, 21.0),
                    row_bg,
                );

                let q_col = qualifier_badge_color(*qual);
                // Badge
                overlay_frame.fill_rectangle(
                    Point::new(dd.rect.x + 6.0, row_y + 2.0),
                    Size::new(20.0, 17.0),
                    Color::from_rgba(q_col.r, q_col.g, q_col.b, 0.25),
                );
                overlay_frame.stroke_rectangle(
                    Point::new(dd.rect.x + 6.0, row_y + 2.0),
                    Size::new(20.0, 17.0),
                    Stroke::default().with_color(q_col).with_width(1.0),
                );
                overlay_frame.fill_text(canvas::Text {
                    content: (*code).to_string(),
                    position: Point::new(
                        dd.rect.x + if code.len() == 1 { 11.0 } else { 7.5 },
                        row_y + 3.5,
                    ),
                    color: Color::WHITE,
                    size: iced::Pixels(10.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });

                // Description
                overlay_frame.fill_text(canvas::Text {
                    content: (*desc).to_string(),
                    position: Point::new(dd.rect.x + 32.0, row_y + 4.0),
                    color: if is_current || is_hovered {
                        COLOR_STEP_DROP
                    } else {
                        Color::from_rgb(0.85, 0.90, 0.95)
                    },
                    size: iced::Pixels(10.5),
                    ..Default::default()
                });
            }

            vec![base_geom, overlay_frame.into_geometry()]
        } else {
            vec![base_geom]
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if let Some(pos) = cursor.position_in(bounds) {
            if let Some(ref dd) = state.qualifier_dropdown
                && dd.rect.contains(pos)
            {
                return mouse::Interaction::Pointer;
            }
            if state.hover_port.is_some() {
                return mouse::Interaction::Crosshair;
            }
        }
        mouse::Interaction::default()
    }
}

// Styling helper for monospace inputs
fn sfc_input_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
    let is_dark = StudioTheme::is_dark(theme);
    text_input::Style {
        background: Background::Color(if is_dark {
            COLOR_INPUT_BG
        } else {
            Color::WHITE
        }),
        border: Border {
            color: if is_dark {
                Color::from_rgb(44.0 / 255.0, 56.0 / 255.0, 72.0 / 255.0)
            } else {
                StudioTheme::border_card(theme)
            },
            width: 1.0,
            radius: Radius::from(3.0),
        },
        placeholder: StudioTheme::text_muted(theme),
        value: if is_dark {
            Color::from_rgb(226.0 / 255.0, 232.0 / 255.0, 240.0 / 255.0)
        } else {
            StudioTheme::text_primary(theme)
        },
        selection: StudioTheme::ACCENT_CYAN,
        icon: StudioTheme::text_secondary(theme),
    }
}

fn sfc_subtle_button(theme: &Theme, status: button::Status) -> button::Style {
    let is_dark = StudioTheme::is_dark(theme);
    let (bg, txt) = match status {
        button::Status::Hovered => (
            if is_dark {
                Color::from_rgb(37.0 / 255.0, 49.0 / 255.0, 65.0 / 255.0)
            } else {
                StudioTheme::bg_hover(theme)
            },
            StudioTheme::text_primary(theme),
        ),
        button::Status::Pressed => (
            if is_dark {
                Color::from_rgb(28.0 / 255.0, 38.0 / 255.0, 50.0 / 255.0)
            } else {
                Color::from_rgb(0.85, 0.88, 0.92)
            },
            StudioTheme::text_primary(theme),
        ),
        _ => (
            if is_dark {
                Color::from_rgb(32.0 / 255.0, 42.0 / 255.0, 56.0 / 255.0)
            } else {
                StudioTheme::bg_surface(theme)
            },
            if is_dark {
                Color::from_rgb(200.0 / 255.0, 210.0 / 255.0, 225.0 / 255.0)
            } else {
                StudioTheme::text_secondary(theme)
            },
        ),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: txt,
        border: Border {
            color: if is_dark {
                Color::from_rgb(50.0 / 255.0, 65.0 / 255.0, 85.0 / 255.0)
            } else {
                StudioTheme::border_card(theme)
            },
            width: 1.0,
            radius: Radius::from(3.0),
        },
        ..Default::default()
    }
}

/// Renders the quick-editor context bar when a Step is selected
#[allow(dead_code)]
fn render_step_inspector<'a>(
    pou_name: &'a str,
    program: &'a SfcProgram,
    step_id: &str,
    _lang: UiLanguage,
) -> Option<Element<'a, Message>> {
    let s_idx = program.steps.iter().position(|s| s.id == step_id)?;
    let step = &program.steps[s_idx];
    let p_name = pou_name.to_string();

    let num_id = step.id.trim_start_matches(['X', 'x']);
    let id_label = text(format!("Step X{num_id}:"))
        .size(13)
        .font(iced::Font::MONOSPACE)
        .color(COLOR_STEP_DROP);

    let name_input = text_input("Step Name", &step.name)
        .on_input({
            let p = p_name.clone();
            move |s| Message::SfcStepNameChanged(p.clone(), s_idx, s)
        })
        .font(iced::Font::MONOSPACE)
        .size(12)
        .padding([4, 8])
        .style(sfc_input_style)
        .width(160);

    let kind_picker = pick_list(&ALL_STEP_KINDS[..], Some(SfcStepKindWrapper(step.kind)), {
        let p = p_name.clone();
        move |w| Message::SfcStepKindChanged(p.clone(), s_idx, w.0)
    })
    .width(90)
    .style(StudioTheme::styled_pick_list);

    let add_act_btn = button(text("+ Action").size(11))
        .on_press(Message::SfcAddAction(p_name.clone(), s_idx))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let discon_btn = button(text("Disconnect").size(11))
        .on_press(Message::SfcDisconnectAll(
            p_name.clone(),
            SfcElementKey::Step(step.id.clone()),
        ))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let del_btn = button(text("Delete").size(11))
        .on_press(Message::SfcRemoveStep(p_name.clone(), s_idx))
        .style(StudioTheme::danger_button)
        .padding([4, 8]);

    let mut step_row = row![id_label, name_input, kind_picker, add_act_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    if step.kind == StepKind::Macro {
        let open_macro_btn = button(text("Open Macro SFC [->]").size(11))
            .on_press(Message::SfcEnterMacro(p_name, step.id.clone()))
            .style(StudioTheme::primary_button)
            .padding([4, 8]);

        step_row = step_row.push(open_macro_btn);
    }

    for (a_idx, act) in step.actions.iter().enumerate() {
        let p = pou_name.to_string();
        let qual_picker = pick_list(
            &ALL_ACTION_QUALIFIERS[..],
            Some(SfcActionQualifierWrapper(act.qualifier)),
            {
                let p = p.clone();
                move |w| Message::SfcActionQualifierChanged(p.clone(), s_idx, a_idx, w.0)
            },
        )
        .width(55)
        .style(StudioTheme::styled_pick_list);

        let target_input = text_input("Action Target", &act.target)
            .on_input({
                let p = p.clone();
                move |t| Message::SfcActionTargetChanged(p.clone(), s_idx, a_idx, t)
            })
            .font(iced::Font::MONOSPACE)
            .size(11)
            .padding([3, 6])
            .style(sfc_input_style)
            .width(140);

        let mut act_row = row![
            text(format!("Action #{}:", a_idx + 1))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
            qual_picker,
            target_input,
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        if act.qualifier == ActionQualifier::L || act.qualifier == ActionQualifier::D {
            let dur_val = act.duration.clone().unwrap_or_default();
            let dur_input = text_input("T#5s", &dur_val)
                .on_input({
                    let p = p.clone();
                    move |d| Message::SfcActionDurationChanged(p.clone(), s_idx, a_idx, d)
                })
                .font(iced::Font::MONOSPACE)
                .size(11)
                .padding([3, 6])
                .style(sfc_input_style)
                .width(65);
            act_row = act_row.push(dur_input);
        }

        let del_act_btn = button(text("x").size(10))
            .on_press(Message::SfcRemoveAction(p, s_idx, a_idx))
            .style(StudioTheme::ghost_button)
            .padding([2, 5]);

        act_row = act_row.push(del_act_btn);

        let act_card = container(act_row)
            .padding([2, 6])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.25, 0.45, 0.65, 0.18,
            )));

        step_row = step_row.push(act_card);
    }

    step_row = step_row.push(discon_btn).push(del_btn);
    Some(step_row.into())
}

/// Renders the quick-editor context bar when a Transition is selected
#[allow(dead_code)]
fn render_transition_inspector<'a>(
    pou_name: &'a str,
    program: &'a SfcProgram,
    trans_id: &str,
    _lang: UiLanguage,
) -> Option<Element<'a, Message>> {
    let t_idx = program.transitions.iter().position(|t| t.id == trans_id)?;
    let tr = &program.transitions[t_idx];
    let p_name = pou_name.to_string();

    let id_label = text(format!("Transition {}:", tr.id))
        .size(13)
        .font(iced::Font::MONOSPACE)
        .color(COLOR_TRANSITION);

    let rec_input = text_input("Receptivity (e.g. TRUE, S1 AND S2)", &tr.receptivity)
        .on_input({
            let p = p_name.clone();
            move |r| Message::SfcTransitionReceptivityChanged(p.clone(), t_idx, r)
        })
        .font(iced::Font::MONOSPACE)
        .size(11.5)
        .padding([4, 6])
        .style(sfc_input_style)
        .width(180);

    let from_str = tr.from_steps.join(", ");
    let from_input = text_input("From Steps (e.g. 1, 2, 3)", &from_str)
        .on_input({
            let p = p_name.clone();
            move |s| Message::SfcTransitionFromChanged(p.clone(), t_idx, s)
        })
        .font(iced::Font::MONOSPACE)
        .size(11.5)
        .padding([4, 6])
        .style(sfc_input_style)
        .width(130);

    let to_str = tr.to_steps.join(", ");
    let to_input = text_input("To Steps (e.g. 4, 5, 6)", &to_str)
        .on_input({
            let p = p_name.clone();
            move |s| Message::SfcTransitionToChanged(p.clone(), t_idx, s)
        })
        .font(iced::Font::MONOSPACE)
        .size(11.5)
        .padding([4, 6])
        .style(sfc_input_style)
        .width(130);

    let discon_btn = button(text("Disconnect").size(11))
        .on_press(Message::SfcDisconnectAll(
            p_name.clone(),
            SfcElementKey::Transition(tr.id.clone()),
        ))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let del_btn = button(text("Delete").size(11))
        .on_press(Message::SfcRemoveTransition(p_name, t_idx))
        .style(StudioTheme::danger_button)
        .padding([4, 8]);

    let content = row![
        id_label,
        rec_input,
        text("From:").size(11).color(StudioTheme::TEXT_MUTED),
        from_input,
        text("To:").size(11).color(StudioTheme::TEXT_MUTED),
        to_input,
        discon_btn,
        del_btn
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    Some(content.into())
}

/// Primary SFC Editor view builder
pub fn render_sfc_editor<'a>(
    program: &'a SfcProgram,
    runtime: Option<&'a PlcRuntime>,
    _lang: UiLanguage,
    selected: Option<&'a SfcElementKey>,
    selected_elements: &'a [SfcElementKey],
    breadcrumb: Option<(String, Vec<String>)>,
) -> Element<'a, Message> {
    let pou_name = &program.name;

    // Validate topology to find independent trees and error count
    let validation_report = validate_sfc_topology(program);
    let _tree_count = validation_report.trees.len();
    let _error_count = validation_report.errors.len();

    // Toolbar buttons: always visible at the top, scrollable horizontally so never squished
    let add_step_btn = button(text("+ Step").size(11.5))
        .on_press(Message::SfcAddStep(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let add_trans_btn = button(text("+ Transition").size(11.5))
        .on_press(Message::SfcAddTransition(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let add_and_div_btn = button(text("+ AND Div (===)").size(11.5))
        .on_press(Message::SfcAddAndDivergence(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let add_and_conv_btn = button(text("+ AND Conv (===)").size(11.5))
        .on_press(Message::SfcAddAndConvergence(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let add_or_div_btn = button(text("+ OR Div (—)").size(11.5))
        .on_press(Message::SfcAddOrDivergence(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let add_or_conv_btn = button(text("+ OR Conv (—)").size(11.5))
        .on_press(Message::SfcAddOrConvergence(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let auto_layout_btn = button(text("Auto-Sort Layout").size(11.5))
        .on_press(Message::SfcAutoLayout(pou_name.clone()))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let validate_btn = button(text("Check Topology").size(11.5))
        .on_press(Message::SfcValidate(pou_name.clone()))
        .style(StudioTheme::primary_button)
        .padding([4, 8]);

    let status_badge: Option<Element<'a, Message>> = if !validation_report.is_valid {
        Some(
            container(
                text(format!(
                    "{} error{}",
                    validation_report.errors.len(),
                    if validation_report.errors.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                ))
                .size(11)
                .color(Color::from_rgb(1.0, 0.4, 0.4)),
            )
            .padding([3, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                1.0, 0.2, 0.2, 0.15,
            )))
            .into(),
        )
    } else {
        None
    };

    let mut toolbar_row = row![
        add_step_btn,
        add_trans_btn,
        add_and_div_btn,
        add_and_conv_btn,
        add_or_div_btn,
        add_or_conv_btn,
        auto_layout_btn,
        validate_btn,
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    if let Some(sb) = status_badge {
        toolbar_row = toolbar_row.push(sb);
    }

    let scrollable_toolbar = scrollable(
        container(toolbar_row).padding(Padding {
            top: 2.0,
            right: 4.0,
            bottom: 8.0,
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

    let breadcrumb_bar: Option<Element<'a, Message>> = breadcrumb.and_then(|(root_pou, path)| {
        if path.is_empty() {
            return None;
        }
        let back_btn = button(text("<- Back to Main SFC").size(11.5))
            .on_press(Message::SfcExitMacro(root_pou.clone()))
            .style(StudioTheme::primary_button)
            .padding([3, 8]);

        let path_str = path.join(" / ");
        let crumb_label = text(format!("Macro Sub-SFC: {root_pou} > {path_str}"))
            .size(11.5)
            .font(iced::Font::MONOSPACE)
            .color(StudioTheme::ACCENT_CYAN);

        Some(
            container(
                row![back_btn, crumb_label]
                    .spacing(12)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([4, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.15, 0.45, 0.85, 0.18,
            )))
            .into(),
        )
    });

    let tip = text("Tip: Direct in-place editing on canvas (click names/actions/transitions to edit). Double-click or click [+ Act] on steps. Drag wires (● to ●). Right-click any port to disconnect.")
        .size(10.5)
        .color(StudioTheme::TEXT_MUTED);

    let mut top_bar_col = column![
        container(scrollable_toolbar)
            .width(Length::Fill)
            .padding([4, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.2, 0.4, 0.6, 0.12,
            ))),
        tip,
    ]
    .spacing(4);

    if let Some(bc) = breadcrumb_bar {
        top_bar_col = top_bar_col.push(bc);
    }

    let top_bar = top_bar_col;

    // Bottom Bar: Multi-selection bulk actions pop at the bottom when multiple elements are selected
    let bottom_bar: Option<Element<'a, Message>> = if selected_elements.len() > 1 {
        let count_label = text(format!("{} items selected", selected_elements.len()))
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(COLOR_STEP_DROP);

        let delete_btn = button(text("Delete (Del)").size(11.5))
            .on_press(Message::SfcRequestDeleteSelected(pou_name.to_string()))
            .style(StudioTheme::danger_button)
            .padding([4, 10]);

        let and_conv_btn = button(text("+ AND Conv (===)").size(11.5))
            .on_press(Message::SfcAddAndConvergence(pou_name.to_string()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let or_conv_btn = button(text("+ OR Conv (—)").size(11.5))
            .on_press(Message::SfcAddOrConvergence(pou_name.to_string()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let and_div_btn = button(text("+ AND Div (===)").size(11.5))
            .on_press(Message::SfcAddAndDivergence(pou_name.to_string()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let or_div_btn = button(text("+ OR Div (—)").size(11.5))
            .on_press(Message::SfcAddOrDivergence(pou_name.to_string()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let deselect_btn = button(text("Deselect [x]").size(11))
            .on_press(Message::SfcSelectElements(pou_name.to_string(), Vec::new()))
            .style(StudioTheme::ghost_button)
            .padding([3, 8]);

        let multi_row = row![
            count_label,
            delete_btn,
            and_conv_btn,
            or_conv_btn,
            and_div_btn,
            or_div_btn,
            deselect_btn
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let scrollable_multi = scrollable(
            container(multi_row).padding(Padding {
                top: 2.0,
                right: 4.0,
                bottom: 8.0,
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

        Some(
            container(scrollable_multi)
                .width(Length::Fill)
                .padding([4, 8])
                .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                    0.2, 0.4, 0.6, 0.15,
                )))
                .into(),
        )
    } else {
        None
    };

    // Compute bounding dimensions for the canvas
    let max_x = program
        .steps
        .iter()
        .filter_map(|s| s.grid_pos.map(|p| p.0 + 360))
        .chain(
            program
                .transitions
                .iter()
                .filter_map(|t| t.grid_pos.map(|p| p.0 + 360)),
        )
        .max()
        .unwrap_or(1600)
        .max(1800) as f32;

    let max_y = program
        .steps
        .iter()
        .filter_map(|s| s.grid_pos.map(|p| p.1 + 300))
        .chain(
            program
                .transitions
                .iter()
                .filter_map(|t| t.grid_pos.map(|p| p.1 + 300)),
        )
        .max()
        .unwrap_or(1200)
        .max(1400) as f32;

    let canvas_widget = canvas::Canvas::new(SfcCanvasProgram {
        pou_name,
        program,
        runtime,
        selected,
        selected_elements,
        validation_report: Some(validation_report),
    })
    .width(Length::Fixed(max_x))
    .height(Length::Fixed(max_y));

    let scrollable_canvas = scrollable(canvas_widget)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::new(),
            horizontal: scrollable::Scrollbar::new(),
        })
        .width(Length::Fill)
        .height(Length::Fill);

    // Single element selected inspector bar: display step or transition editor
    let single_inspector: Option<Element<'a, Message>> = if selected_elements.len() <= 1 {
        if let Some(SfcElementKey::Step(sid)) = selected {
            render_step_inspector(pou_name, program, sid, _lang)
        } else if let Some(SfcElementKey::Transition(tid)) = selected {
            render_transition_inspector(pou_name, program, tid, _lang)
        } else {
            None
        }
    } else {
        None
    };

    let inspector_bar: Option<Element<'a, Message>> = single_inspector.map(|content| {
        let scrollable_insp = scrollable(
            container(content).padding(Padding {
                top: 2.0,
                right: 4.0,
                bottom: 8.0,
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

        container(scrollable_insp)
            .width(Length::Fill)
            .padding([4, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.2, 0.4, 0.6, 0.15,
            )))
            .into()
    });

    let mut main_col = column![top_bar, scrollable_canvas]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill);

    if let Some(insp) = inspector_bar {
        main_col = main_col.push(insp);
    }

    if let Some(bot) = bottom_bar {
        main_col = main_col.push(bot);
    }

    main_col.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_lang_sfc::{
        ActionQualifier, ConvergenceKind, DivergenceKind, SfcAction, SfcProgram, SfcStep,
        SfcTransition, StepKind,
    };
    use iced::widget::canvas::Program;

    #[test]
    fn test_snap_to_grid() {
        assert_eq!(snap_to_grid(42.3, 78.9), (40, 80));
        assert_eq!(snap_to_grid(15.0, 19.0), (20, 20));
        assert_eq!(snap_to_grid(101.0, 204.0), (100, 200));
    }

    #[test]
    fn test_port_coordinates_and_hit_testing() {
        let step = SfcStep {
            id: "1".to_string(),
            name: "Step_1".to_string(),
            kind: StepKind::Normal,
            actions: vec![],
            grid_pos: Some((60, 40)),
                    ..Default::default()
                };
        let canvas = SfcCanvasProgram {
            pou_name: "test",
            program: &SfcProgram {
                name: "test".to_string(),
                comment: "".to_string(),
                steps: vec![step],
                transitions: vec![],
            },
            runtime: None,
            selected: None,
            selected_elements: &[],
            validation_report: None,
        };
        let state = SfcCanvasState::default();
        let in_p = canvas.step_in_port(&canvas.program.steps[0], &state);
        let out_p = canvas.step_out_port(&canvas.program.steps[0], &state);
        let w = canvas.step_width(&canvas.program.steps[0]);

        assert_eq!(in_p, Point::new(60.0 + w * 0.5, 40.0));
        assert_eq!(
            out_p,
            Point::new(60.0 + w * 0.5, 40.0 + STEP_HEIGHT)
        );

        // Hit testing within PORT_HIT_DIST
        assert!(Point::new(60.0 + w * 0.5, 40.0 + STEP_HEIGHT + 2.0).distance(out_p) <= PORT_HIT_DIST);
    }

    #[test]
    fn test_sfc_coordinate_grid_and_step_box_anchoring() {
        let base_spine_x = 560.0_f32;
        let col_stride = COLUMN_STRIDE;

        // Single-column row
        let cx_single = step_center_x(0, 1, base_spine_x, col_stride);
        assert!((cx_single - base_spine_x).abs() < f32::EPSILON);

        let left_margin_single = step_box_left_margin(cx_single);
        let step_center_reconstructed = left_margin_single + STEP_BOX_WIDTH * 0.5;
        assert!((step_center_reconstructed - base_spine_x).abs() < f32::EPSILON);

        // Parallel branches
        let cx_branch0 = step_center_x(0, 2, base_spine_x, col_stride);
        let cx_branch1 = step_center_x(1, 2, base_spine_x, col_stride);

        assert!((cx_branch0 - (base_spine_x - col_stride * 0.5)).abs() < f32::EPSILON);
        assert!((cx_branch1 - (base_spine_x + col_stride * 0.5)).abs() < f32::EPSILON);

        let program = SfcProgram {
            name: "test_seq".to_string(),
            comment: "Test sequence".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_Init".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((60, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_Busy".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::N,
                        target: "Motor_Run := TRUE".to_string(),
                        duration: None,
                    }],
                    grid_pos: Some((60, 240)),
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "T0".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string()],
                receptivity: "Start_PB".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                grid_pos: Some((110, 140)),
            }],
        };

        // Render editor without panic
        let _elem = render_sfc_editor(&program, None, UiLanguage::En, None, &[], None);
    }

    #[test]
    fn test_wire_drag_and_connect_ports_logic() -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::FerriteStudioApp;
        use crate::resolve_or_scaffold_project;
        use ferrite_project::PouSource;

        let temp_dir = std::env::temp_dir().join("ferrite_sfc_canvas_wire_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let mut proj = resolve_or_scaffold_project(Some(&temp_dir))?;

        let mut sfc = SfcProgram {
            name: "seq".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("X1".to_string(), "Init".to_string(), StepKind::Initial),
                SfcStep::new("X2".to_string(), "Step2".to_string(), StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T1".to_string(),
                from_steps: vec![],
                to_steps: vec![],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };
        sfc.assign_default_grid_positions();
        proj.pous.insert("seq".to_string(), PouSource::Sfc(sfc));
        let mut app = FerriteStudioApp::new(proj);

        // 1. Connect Step X1 bottom port to Transition T1 top port
        let _ = app.update(Message::SfcConnectPorts {
            pou_name: "seq".to_string(),
            source: SfcPortSource::StepOutput("X1".to_string()),
            target: SfcPortTarget::TransitionInput("T1".to_string()),
        });

        // 2. Connect Transition T1 bottom port to Step X2 top port
        let _ = app.update(Message::SfcConnectPorts {
            pou_name: "seq".to_string(),
            source: SfcPortSource::TransitionOutput("T1".to_string()),
            target: SfcPortTarget::StepInput("X2".to_string()),
        });

        if let Some(PouSource::Sfc(sfc_after)) = app.project.pous.get("seq") {
            assert_eq!(sfc_after.transitions[0].from_steps, vec!["X1"]);
            assert_eq!(sfc_after.transitions[0].to_steps, vec!["X2"]);
        } else {
            return Err("POU seq not found".into());
        }

        // 3. Reverse connections test: StepInput -> TransitionOutput and TransitionInput -> StepOutput
        if let Some(PouSource::Sfc(sfc_mut)) = app.project.pous.get_mut("seq") {
            sfc_mut.transitions[0].from_steps.clear();
            sfc_mut.transitions[0].to_steps.clear();
        }
        let _ = app.update(Message::SfcConnectPorts {
            pou_name: "seq".to_string(),
            source: SfcPortSource::StepInput("X2".to_string()),
            target: SfcPortTarget::TransitionOutput("T1".to_string()),
        });
        let _ = app.update(Message::SfcConnectPorts {
            pou_name: "seq".to_string(),
            source: SfcPortSource::TransitionInput("T1".to_string()),
            target: SfcPortTarget::StepOutput("X1".to_string()),
        });
        if let Some(PouSource::Sfc(sfc_after)) = app.project.pous.get("seq") {
            assert_eq!(sfc_after.transitions[0].from_steps, vec!["X1"]);
            assert_eq!(sfc_after.transitions[0].to_steps, vec!["X2"]);
        } else {
            return Err("POU seq not found".into());
        }

        // 4. Test SfcCanvasProgram::update directly for top-port dragging and forgiving body hit testing
        if let Some(PouSource::Sfc(sfc_ref)) = app.project.pous.get("seq") {
            let canvas = SfcCanvasProgram {
                pou_name: "seq",
                program: sfc_ref,
                runtime: None,
                selected: None,
                selected_elements: &[],
                validation_report: None,
            };
            let mut state = SfcCanvasState::default();
            let bounds = Rectangle::new(Point::ORIGIN, Size::new(2000.0, 2000.0));

            // A. Clicking on top port of Step X1 must initiate DraggingWire(StepInput), NOT MovingElement
            let step_in = canvas.step_in_port(&sfc_ref.steps[0], &state);
            let (status, msg) = canvas.update(
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(step_in),
            );
            assert_eq!(status, event::Status::Captured);
            assert!(msg.is_none());
            assert!(
                matches!(state.drag, DragState::DraggingWire { source: SfcPortSource::StepInput(ref id), .. } if id == "X1")
            );

            // B. Releasing cursor anywhere inside Transition T1 body connects successfully
            let (tx, ty) = canvas.trans_pos(&sfc_ref.transitions[0], &state);
            let inside_trans_body = Point::new(tx + TRANS_WIDTH * 0.5, ty + TRANS_HEIGHT * 0.5);
            let (status, msg) = canvas.update(
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(inside_trans_body),
            );
            assert_eq!(status, event::Status::Captured);
            assert!(matches!(
                msg,
                Some(Message::SfcConnectPorts {
                    ref pou_name,
                    source: SfcPortSource::StepInput(ref s),
                    target: SfcPortTarget::TransitionOutput(ref t),
                }) if pou_name == "seq" && s == "X1" && t == "T1"
            ));

            // C. Clicking on Step X1 bottom port and releasing inside Transition T1 body
            let step_out = canvas.step_out_port(&sfc_ref.steps[0], &state);
            let _ = canvas.update(
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(step_out),
            );
            assert!(
                matches!(state.drag, DragState::DraggingWire { source: SfcPortSource::StepOutput(ref id), .. } if id == "X1")
            );
            let (_, msg) = canvas.update(
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(inside_trans_body),
            );
            assert!(matches!(
                msg,
                Some(Message::SfcConnectPorts {
                    ref pou_name,
                    source: SfcPortSource::StepOutput(ref s),
                    target: SfcPortTarget::TransitionInput(ref t),
                }) if pou_name == "seq" && s == "X1" && t == "T1"
            ));

            // D. Clicking on Transition T1 top port initiates DraggingWire(TransitionInput), NOT MovingElement
            let trans_in = canvas.trans_in_port(&sfc_ref.transitions[0], &state);
            let _ = canvas.update(
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(trans_in),
            );
            assert!(
                matches!(state.drag, DragState::DraggingWire { source: SfcPortSource::TransitionInput(ref id), .. } if id == "T1")
            );

            // E. Releasing cursor inside Step X1 body connects TransitionInput -> StepOutput
            let (sx, sy) = canvas.step_pos(&sfc_ref.steps[0], &state);
            let inside_step_body = Point::new(sx + STEP_WIDTH * 0.5, sy + STEP_HEIGHT * 0.5);
            let (_, msg) = canvas.update(
                &mut state,
                canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                bounds,
                mouse::Cursor::Available(inside_step_body),
            );
            assert!(matches!(
                msg,
                Some(Message::SfcConnectPorts {
                    ref pou_name,
                    source: SfcPortSource::TransitionInput(ref t),
                    target: SfcPortTarget::StepOutput(ref s),
                }) if pou_name == "seq" && t == "T1" && s == "X1"
            ));
        }

        // 5. Move Step X1 and verify grid snap
        let _ = app.update(Message::SfcMoveElement(
            "seq".to_string(),
            SfcElementKey::Step("X1".to_string()),
            (140, 80),
        ));

        if let Some(PouSource::Sfc(sfc_after)) = app.project.pous.get("seq") {
            assert_eq!(sfc_after.steps[0].grid_pos, Some((140, 80)));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_canvas_and_or_divergence_rendering() {
        let mut program = SfcProgram {
            name: "and_or_canvas_test".to_string(),
            comment: "Testing AND and OR divergences".to_string(),
            steps: vec![
                SfcStep::new("0".to_string(), "Step_Init".to_string(), StepKind::Initial),
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_BranchA".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::N,
                        target: "Valve_Water".to_string(),
                        duration: None,
                    }],
                    grid_pos: None, ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Step_BranchB".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::P,
                        target: "Count := Count + 1;".to_string(),
                        duration: None,
                    }],
                    grid_pos: None, ..Default::default()
                },
                SfcStep::new("3".to_string(), "Step_Choice".to_string(), StepKind::Normal),
                SfcStep::new("4".to_string(), "Step_AltA".to_string(), StepKind::Normal),
                SfcStep::new("5".to_string(), "Step_AltB".to_string(), StepKind::Normal),
                SfcStep::new("6".to_string(), "Step_Merged".to_string(), StepKind::Normal),
            ],
            transitions: vec![
                // T1: AND Divergence (1 transition -> 2 steps)
                SfcTransition {
                    id: "T1_AndDiv".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string(), "2".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
                // T2: AND Convergence (2 steps -> 1 transition)
                SfcTransition {
                    id: "T2_AndConv".to_string(),
                    from_steps: vec!["1".to_string(), "2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
                // T3 & T4: OR Divergence (1 step -> 2 transitions)
                SfcTransition {
                    id: "T3_OrA".to_string(),
                    from_steps: vec!["3".to_string()],
                    to_steps: vec!["4".to_string()],
                    receptivity: "Fast_Mode".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::Or,
                    grid_pos: None,
                },
                SfcTransition {
                    id: "T4_OrB".to_string(),
                    from_steps: vec!["3".to_string()],
                    to_steps: vec!["5".to_string()],
                    receptivity: "NOT Fast_Mode".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::Or,
                    grid_pos: None,
                },
                // T5 & T6: OR Convergence (2 transitions -> 1 step)
                SfcTransition {
                    id: "T5_MergeA".to_string(),
                    from_steps: vec!["4".to_string()],
                    to_steps: vec!["6".to_string()],
                    receptivity: "DoneA".to_string(),
                    convergence: ConvergenceKind::Or,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
                SfcTransition {
                    id: "T6_MergeB".to_string(),
                    from_steps: vec!["5".to_string()],
                    to_steps: vec!["6".to_string()],
                    receptivity: "DoneB".to_string(),
                    convergence: ConvergenceKind::Or,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
            ],
        };

        program.assign_default_grid_positions();
        // Render canvas elements without panic
        let _elem = render_sfc_editor(&program, None, UiLanguage::En, None, &[], None);
    }

    #[test]
    fn test_sfc_loopback_arrow_left_edge_routing() {
        let program = SfcProgram {
            name: "loopback_test".to_string(),
            comment: "Test return loop arrow clearance".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_0".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((340, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_1".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((60, 240)),
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Step_2".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((340, 440)),
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "T_Return".to_string(),
                from_steps: vec!["2".to_string()],
                to_steps: vec!["0".to_string()],
                receptivity: "Cycle_Done".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                grid_pos: Some((390, 560)),
            }],
        };

        let canvas = SfcCanvasProgram {
            pou_name: "loopback_test",
            program: &program,
            runtime: None,
            selected: None,
            selected_elements: &[],
            validation_report: None,
        };

        let state = SfcCanvasState::default();
        let sp = canvas.step_in_port(&program.steps[0], &state);
        let tr_out = canvas.trans_out_port(&program.transitions[0], &state);

        // Target step is Step 0 (sp.y = 40.0), return transition is T_Return (tr_out.y = 580.0)
        assert!(sp.y < tr_out.y, "Loopback must lead backwards/upwards");

        let side_x = canvas.compute_sfc_loopback_side_x(sp, tr_out, "T_Return", "0", &state);

        // 1. Should route all the way to the canvas left margin (20.0) when unobstructed
        assert_eq!(side_x, 20.0);

        // 2. Verify that side_x is strictly to the left of all intervening step bounding boxes
        for step in &program.steps {
            let (sx, sy) = canvas.step_pos(step, &state);
            let in_span = sy + STEP_HEIGHT >= sp.y - 20.0 && sy <= tr_out.y + 20.0;
            if in_span {
                assert!(
                    sx >= side_x + 40.0,
                    "Step {} at x={} must have at least 40px clearance from return wire at side_x={}",
                    step.id,
                    sx,
                    side_x
                );
            }
        }
    }

    #[test]
    fn test_sfc_connect_steps_and_transitions_drag_drop() {
        use iced::widget::canvas::Program;

        let program = SfcProgram {
            name: "test_drag".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_0".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((60, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_1".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((60, 240)),
                    ..Default::default()
                },
            ],
            transitions: vec![
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec![],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: Some((110, 140)),
                },
                SfcTransition {
                    id: "T2".to_string(),
                    from_steps: vec![],
                    to_steps: vec!["1".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: Some((110, 340)),
                },
            ],
        };

        let canvas = SfcCanvasProgram {
            pou_name: "test",
            program: &program,
            runtime: None,
            selected: None,
            selected_elements: &[],
            validation_report: None,
        };

        let mut state = SfcCanvasState::default();
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };

        // 1. Drag wire from Step 0 and drop on Step 1 -> SfcConnectSteps
        state.drag = DragState::DraggingWire {
            source: SfcPortSource::StepOutput("0".to_string()),
            start_pt: Point::new(110.0, 100.0),
            current_pt: Point::new(110.0, 240.0),
        };
        let step1_in = canvas.step_in_port(&program.steps[1], &state);
        let event = canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let (_, msg) = canvas.update(
            &mut state,
            event,
            bounds,
            mouse::Cursor::Available(step1_in),
        );

        assert!(
            matches!(
                msg,
                Some(Message::SfcConnectSteps { ref pou_name, ref from_step, ref to_step })
                    if pou_name == "test" && from_step == "0" && to_step == "1"
            ),
            "Expected SfcConnectSteps from 0 to 1, got {:?}",
            msg
        );

        // 2. Drag wire from Transition T1 and drop on Transition T2 -> SfcConnectTransitions
        state.drag = DragState::DraggingWire {
            source: SfcPortSource::TransitionOutput("T1".to_string()),
            start_pt: Point::new(135.0, 152.0),
            current_pt: Point::new(135.0, 340.0),
        };
        let t2_in = canvas.trans_in_port(&program.transitions[1], &state);
        let event = canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let (_, msg) = canvas.update(&mut state, event, bounds, mouse::Cursor::Available(t2_in));

        assert!(
            matches!(
                msg,
                Some(Message::SfcConnectTransitions { ref pou_name, ref from_trans, ref to_trans })
                    if pou_name == "test" && from_trans == "T1" && to_trans == "T2"
            ),
            "Expected SfcConnectTransitions from T1 to T2, got {:?}",
            msg
        );
    }

    #[test]
    fn test_sfc_or_convergence_single_bar_rendering() {
        // Standard IEC 60848 OR Convergence: multiple branch transitions converging to one step
        let valid_or_conv = SfcProgram {
            name: "valid_or_conv".to_string(),
            comment: "Standard IEC 60848 OR convergence".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_0".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((60, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_1".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((260, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Step_2".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((160, 240)),
                    ..Default::default()
                },
            ],
            transitions: vec![
                SfcTransition {
                    id: "T0".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["2".to_string()],
                    receptivity: "TRUE".to_string(),
                    grid_pos: Some((110, 140)),
                    ..Default::default()
                },
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["1".to_string()],
                    to_steps: vec!["2".to_string()],
                    receptivity: "TRUE".to_string(),
                    grid_pos: Some((310, 140)),
                    ..Default::default()
                },
            ],
        };

        // Render editor without panic or errors
        let _elem = render_sfc_editor(&valid_or_conv, None, UiLanguage::En, None, &[], None);
        let report = validate_sfc_topology(&valid_or_conv);
        assert!(report.is_valid, "Valid IEC 60848 OR convergence with multiple branch transitions is valid");

        // Single transition attempting to merge multiple steps with ConvergenceKind::Or is rejected by IEC 60848 Rule 4
        let invalid_program = SfcProgram {
            name: "invalid_or_conv".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_0".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((60, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_1".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((260, 40)),
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Step_2".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((160, 240)),
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "T_OrConv".to_string(),
                from_steps: vec!["0".to_string(), "1".to_string()],
                to_steps: vec!["2".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::Or,
                divergence: DivergenceKind::And,
                grid_pos: Some((210, 140)),
            }],
        };
        let invalid_rep = validate_sfc_topology(&invalid_program);
        assert!(!invalid_rep.is_valid);
        assert!(invalid_rep.errors.iter().any(|e| matches!(e, SfcValidationError::InvalidOrConvergence { element_id, .. } if element_id == "T_OrConv")));
    }

    #[test]
    fn test_sfc_marquee_box_select_and_delete() {
        let program = SfcProgram {
            name: "test_marquee".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_1".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((100, 100)),
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Step_2".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((300, 100)),
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "T1".to_string(),
                from_steps: vec!["1".to_string()],
                to_steps: vec!["2".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                grid_pos: Some((200, 100)),
            }],
        };

        let canvas = SfcCanvasProgram {
            pou_name: "test_marquee",
            program: &program,
            runtime: None,
            selected: None,
            selected_elements: &[],
            validation_report: None,
        };

        let mut state = SfcCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(2000.0, 2000.0));

        // 1. Click on empty space (0, 0) starts BoxSelecting
        let (status, msg) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(10.0, 10.0)),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert!(
            matches!(msg, Some(Message::SfcSelectElements(ref p, ref list)) if p == "test_marquee" && list.is_empty())
        );
        assert!(matches!(state.drag, DragState::BoxSelecting { .. }));

        // 2. Drag rectangle covering (10, 10) to (450, 250), which bounds Step 1, T1, and Step 2
        let (status, _) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(450.0, 250.0),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(450.0, 250.0)),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert_eq!(state.selected_elements.len(), 3);
        assert!(
            state
                .selected_elements
                .contains(&SfcElementKey::Step("1".to_string()))
        );
        assert!(
            state
                .selected_elements
                .contains(&SfcElementKey::Step("2".to_string()))
        );
        assert!(
            state
                .selected_elements
                .contains(&SfcElementKey::Transition("T1".to_string()))
        );

        // 3. Release mouse -> emits SfcSelectElements with all 3 items
        let (status, release_msg) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(450.0, 250.0)),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert!(matches!(release_msg, Some(Message::SfcSelectElements(..))));

        // 4. Pressing Delete key -> emits SfcRequestDeleteSelected
        let (status, del_msg) = canvas.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::key::Key::Named(iced::keyboard::key::Named::Delete),
                physical_key: iced::keyboard::key::Physical::Code(
                    iced::keyboard::key::Code::Delete,
                ),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(450.0, 250.0)),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert!(
            matches!(del_msg, Some(Message::SfcRequestDeleteSelected(ref p)) if p == "test_marquee")
        );
    }

    #[test]
    fn test_sfc_multi_branch_manual_div_conv_and_delete_modal()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::FerriteStudioApp;
        use crate::resolve_or_scaffold_project;

        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("fplc_div_conv_{now_nanos}"));
        let mut project = resolve_or_scaffold_project(Some(&temp_dir))?;

        let sfc_pou = SfcProgram {
            name: "seq_test".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep {
                    id: "1".into(),
                    name: "Step_1".into(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((100, 100)),
                    ..Default::default()
                },
                SfcStep {
                    id: "2".into(),
                    name: "Step_2".into(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((260, 100)),
                    ..Default::default()
                },
                SfcStep {
                    id: "3".into(),
                    name: "Step_3".into(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((420, 100)),
                    ..Default::default()
                },
            ],
            transitions: vec![],
        };
        project.pous.insert(
            "seq_test".to_string(),
            ferrite_project::PouSource::Sfc(sfc_pou),
        );

        let mut app = FerriteStudioApp::new(project);

        // 1. Multi-select 3 steps
        let _ = app.update(Message::SfcSelectElements(
            "seq_test".to_string(),
            vec![
                SfcElementKey::Step("1".to_string()),
                SfcElementKey::Step("2".to_string()),
                SfcElementKey::Step("3".to_string()),
            ],
        ));
        assert_eq!(app.sfc_selected_elements.len(), 3);

        // 2. Add manual AND Convergence with 3 branches
        let _ = app.update(Message::SfcAddAndConvergence("seq_test".to_string()));
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("seq_test") {
            let conv_tr = sfc
                .transitions
                .iter()
                .find(|t| t.convergence == ConvergenceKind::And)
                .ok_or("AND convergence transition must exist")?;
            assert_eq!(conv_tr.from_steps.len(), 3);
            assert!(conv_tr.from_steps.contains(&"1".to_string()));
            assert!(conv_tr.from_steps.contains(&"2".to_string()));
            assert!(conv_tr.from_steps.contains(&"3".to_string()));
        }

        // 3. Multi-select the 3 steps again and add manual OR Divergence
        let _ = app.update(Message::SfcSelectElements(
            "seq_test".to_string(),
            vec![
                SfcElementKey::Step("1".to_string()),
                SfcElementKey::Step("2".to_string()),
                SfcElementKey::Step("3".to_string()),
            ],
        ));
        let _ = app.update(Message::SfcAddOrDivergence("seq_test".to_string()));
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("seq_test") {
            // In IEC 60848, OR divergence has an independent transition on each branch:
            let branch1 = sfc.transitions.iter().find(|t| t.to_steps.contains(&"1".to_string())).ok_or("Branch 1 transition must exist")?;
            let branch2 = sfc.transitions.iter().find(|t| t.to_steps.contains(&"2".to_string())).ok_or("Branch 2 transition must exist")?;
            let branch3 = sfc.transitions.iter().find(|t| t.to_steps.contains(&"3".to_string())).ok_or("Branch 3 transition must exist")?;
            assert_eq!(branch1.to_steps, vec!["1".to_string()]);
            assert_eq!(branch2.to_steps, vec!["2".to_string()]);
            assert_eq!(branch3.to_steps, vec!["3".to_string()]);
            assert_eq!(branch1.divergence, DivergenceKind::Or);
            assert_eq!(branch2.divergence, DivergenceKind::Or);
            assert_eq!(branch3.divergence, DivergenceKind::Or);
        }

        // 4. Test delete: single-item selection deletes immediately without confirmation modal
        let _ = app.update(Message::SfcSelectElements(
            "seq_test".to_string(),
            vec![SfcElementKey::Step("2".to_string())],
        ));
        let _ = app.update(Message::SfcRequestDeleteSelected("seq_test".to_string()));
        assert!(
            app.sfc_delete_modal.is_none(),
            "Single-element delete must not prompt confirmation modal"
        );
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("seq_test") {
            assert!(
                !sfc.steps.iter().any(|s| s.id == "2"),
                "Single-element must be deleted immediately"
            );
            for tr in &sfc.transitions {
                assert!(!tr.from_steps.contains(&"2".to_string()));
                assert!(!tr.to_steps.contains(&"2".to_string()));
            }
        }

        // 4b. Test SfcAddOrConvergence with Steps 1 and 3 selected:
        // Spawns individual branch transitions converging into a common destination step
        let _ = app.update(Message::SfcSelectElements(
            "seq_test".to_string(),
            vec![
                SfcElementKey::Step("1".to_string()),
                SfcElementKey::Step("3".to_string()),
            ],
        ));
        let _ = app.update(Message::SfcAddOrConvergence("seq_test".to_string()));
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("seq_test") {
            let dest_step = sfc
                .steps
                .iter()
                .find(|s| s.id != "1" && s.id != "3")
                .ok_or("Destination step must be created")?;
            let dest_id = dest_step.id.clone();
            let branch1_conv = sfc
                .transitions
                .iter()
                .find(|t| t.from_steps.contains(&"1".to_string()) && t.to_steps.contains(&dest_id))
                .ok_or("Branch 1 convergence transition must exist")?;
            let branch3_conv = sfc
                .transitions
                .iter()
                .find(|t| t.from_steps.contains(&"3".to_string()) && t.to_steps.contains(&dest_id))
                .ok_or("Branch 3 convergence transition must exist")?;
            assert_eq!(branch1_conv.from_steps, vec!["1".to_string()]);
            assert_eq!(branch3_conv.from_steps, vec!["3".to_string()]);
            assert_eq!(branch1_conv.to_steps, vec![dest_id.clone()]);
            assert_eq!(branch3_conv.to_steps, vec![dest_id]);
        }

        // 5. Test delete: multiple-item selection prompts confirmation modal
        let _ = app.update(Message::SfcSelectElements(
            "seq_test".to_string(),
            vec![
                SfcElementKey::Step("1".to_string()),
                SfcElementKey::Step("3".to_string()),
            ],
        ));
        let _ = app.update(Message::SfcRequestDeleteSelected("seq_test".to_string()));
        assert!(
            app.sfc_delete_modal.is_some(),
            "Multi-element delete must prompt confirmation modal"
        );

        // Cancel does not delete
        let _ = app.update(Message::SfcCancelDeleteSelected);
        assert!(app.sfc_delete_modal.is_none());
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("seq_test") {
            assert!(sfc.steps.iter().any(|s| s.id == "1"));
            assert!(sfc.steps.iter().any(|s| s.id == "3"));
        }

        // Request again and confirm deletion
        let _ = app.update(Message::SfcRequestDeleteSelected("seq_test".to_string()));
        let _ = app.update(Message::SfcConfirmDeleteSelected);
        assert!(app.sfc_delete_modal.is_none());
        assert!(app.sfc_selected_elements.is_empty());
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("seq_test") {
            assert!(!sfc.steps.iter().any(|s| s.id == "1"));
            assert!(!sfc.steps.iter().any(|s| s.id == "3"));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_step_with_multiple_actions_display() {
        let step = SfcStep {
            id: "10".to_string(),
            name: "Step_MultiAction".to_string(),
            kind: StepKind::Normal,
            actions: vec![
                SfcAction {
                    qualifier: ActionQualifier::N,
                    target: "Pump_1".to_string(),
                    duration: None,
                },
                SfcAction {
                    qualifier: ActionQualifier::S,
                    target: "Valve_Open".to_string(),
                    duration: None,
                },
                SfcAction {
                    qualifier: ActionQualifier::L,
                    target: "Heater_On".to_string(),
                    duration: Some("T#10s".to_string()),
                },
                SfcAction {
                    qualifier: ActionQualifier::R,
                    target: "Alarm_Reset".to_string(),
                    duration: None,
                },
            ],
            grid_pos: Some((100, 100)),
                    ..Default::default()
                };

        let transition = SfcTransition {
            id: "1".to_string(),
            receptivity: "Start_PB AND Safety_OK".to_string(),
            from_steps: vec!["10".to_string()],
            to_steps: vec!["11".to_string()],
            convergence: ConvergenceKind::And,
            divergence: DivergenceKind::And,
            grid_pos: Some((100, 250)),
        };

        let program = SfcProgram {
            name: "test_actions".to_string(),
            comment: "".to_string(),
            steps: vec![step],
            transitions: vec![transition],
        };

        // 1. Verify render_step_inspector shows all 4 actions in horizontal layout without error
        let step_inspector = render_step_inspector("test_actions", &program, "10", UiLanguage::En);
        assert!(step_inspector.is_some());

        // 2. Verify render_transition_inspector renders horizontal inspector without error
        let trans_inspector =
            render_transition_inspector("test_actions", &program, "1", UiLanguage::En);
        assert!(trans_inspector.is_some());

        // 3. Verify render_sfc_editor renders full view with Step inspector (horizontal scrollable with bottom padding)
        let sel_step_key = SfcElementKey::Step("10".to_string());
        let _elem_step =
            render_sfc_editor(&program, None, UiLanguage::En, Some(&sel_step_key), &[], None);

        let multi_sel = [SfcElementKey::Step("10".to_string()), SfcElementKey::Transition("1".to_string())];
        let _elem_multi = render_sfc_editor(&program, None, UiLanguage::En, None, &multi_sel, None);
        // 4. Verify render_sfc_editor renders full view with Transition inspector
        let sel_trans_key = SfcElementKey::Transition("1".to_string());
        let _elem_trans =
            render_sfc_editor(&program, None, UiLanguage::En, Some(&sel_trans_key), &[], None);
    }

    #[test]
    fn test_sfc_disconnect_wires_via_ports_and_elements() -> Result<(), Box<dyn std::error::Error>>
    {
        use crate::app::FerriteStudioApp;
        use crate::resolve_or_scaffold_project;

        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("fplc_discon_{now_nanos}"));
        let mut project = resolve_or_scaffold_project(Some(&temp_dir))?;

        let sfc_pou = SfcProgram {
            name: "discon_test".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep {
                    id: "1".into(),
                    name: "Step_1".into(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((100, 100)),
                    ..Default::default()
                },
                SfcStep {
                    id: "2".into(),
                    name: "Step_2".into(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    grid_pos: Some((100, 300)),
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "T1".into(),
                from_steps: vec!["1".into()],
                to_steps: vec!["2".into()],
                receptivity: "TRUE".into(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                grid_pos: Some((150, 200)),
            }],
        };
        project.pous.insert(
            "discon_test".to_string(),
            ferrite_project::PouSource::Sfc(sfc_pou),
        );

        let mut app = FerriteStudioApp::new(project);

        // 1. Disconnect Step 1 Output -> T1.from_steps should be cleared
        let _ = app.update(Message::SfcDisconnectStepOutput(
            "discon_test".to_string(),
            "1".to_string(),
        ));
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("discon_test") {
            assert!(!sfc.transitions[0].from_steps.contains(&"1".to_string()));
            assert_eq!(sfc.transitions[0].to_steps, vec!["2".to_string()]);
        }

        // 2. Disconnect Step 2 Input -> T1.to_steps should be cleared
        let _ = app.update(Message::SfcDisconnectStepInput(
            "discon_test".to_string(),
            "2".to_string(),
        ));
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("discon_test") {
            assert!(sfc.transitions[0].to_steps.is_empty());
        }

        // Reconnect them and test SfcDisconnectAll
        if let Some(ferrite_project::PouSource::Sfc(sfc_mut)) =
            app.project.pous.get_mut("discon_test")
        {
            sfc_mut.transitions[0].from_steps = vec!["1".to_string()];
            sfc_mut.transitions[0].to_steps = vec!["2".to_string()];
        }
        let _ = app.update(Message::SfcDisconnectAll(
            "discon_test".to_string(),
            SfcElementKey::Transition("T1".to_string()),
        ));
        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("discon_test") {
            assert!(sfc.transitions[0].from_steps.is_empty());
            assert!(sfc.transitions[0].to_steps.is_empty());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_step_id_numeric_increment_no_accumulated_x()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::FerriteStudioApp;
        use crate::resolve_or_scaffold_project;

        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("fplc_step_id_{now_nanos}"));
        let mut project = resolve_or_scaffold_project(Some(&temp_dir))?;

        let sfc_pou = SfcProgram {
            name: "id_test".to_string(),
            comment: "".to_string(),
            steps: vec![SfcStep {
                id: "0".into(),
                name: "Step_0".into(),
                kind: StepKind::Initial,
                actions: vec![],
                grid_pos: Some((60, 40)),
                    ..Default::default()
                }],
            transitions: vec![],
        };
        project.pous.insert(
            "id_test".to_string(),
            ferrite_project::PouSource::Sfc(sfc_pou),
        );

        let mut app = FerriteStudioApp::new(project);

        // Add 3 new steps sequentially
        let _ = app.update(Message::SfcAddStep("id_test".to_string()));
        let _ = app.update(Message::SfcAddStep("id_test".to_string()));
        let _ = app.update(Message::SfcAddStep("id_test".to_string()));

        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get("id_test") {
            assert_eq!(sfc.steps.len(), 4);
            assert_eq!(sfc.steps[0].id, "0");
            assert_eq!(sfc.steps[1].id, "1");
            assert_eq!(sfc.steps[2].id, "2");
            assert_eq!(sfc.steps[3].id, "3");

            // Verify none of them has "XX"
            for step in &sfc.steps {
                assert!(
                    !step.id.contains("XX"),
                    "Step id {} contains multiple Xs!",
                    step.id
                );
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_io_rack_panel_sfc_filter() -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::FerriteStudioApp;
        use crate::resolve_or_scaffold_project;

        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("fplc_io_filter_{now_nanos}"));
        let project = resolve_or_scaffold_project(Some(&temp_dir))?;

        let mut app = FerriteStudioApp::new(project);
        assert!(!app.io_filter_sfc);

        // 1. Activate SFC filter
        let _ = app.update(Message::IoFilterSfcSelected);
        assert!(app.io_filter_sfc);
        assert_eq!(app.io_filter_area, None);

        // 2. Select an area like Input -> disables SFC filter
        let _ = app.update(Message::IoFilterAreaSelected(Some(
            ferrite_core::MemoryArea::Input,
        )));
        assert!(!app.io_filter_sfc);
        assert_eq!(app.io_filter_area, Some(ferrite_core::MemoryArea::Input));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_canvas_edit_buffer() {
        let mut state = SfcCanvasState::default();
        assert!(state.editing_target.is_none());
        assert!(state.edit_buffer.is_empty());

        // Start edit on a step name
        state.start_edit(
            SfcEditTarget::StepName {
                step_id: "10".to_string(),
            },
            "Step_Init",
        );
        assert_eq!(
            state.editing_target,
            Some(SfcEditTarget::StepName {
                step_id: "10".to_string()
            })
        );
        assert_eq!(state.edit_buffer, "Step_Init");
        assert_eq!(state.cursor_pos, 9);

        // Type characters
        state.insert_char('_');
        state.insert_char('2');
        assert_eq!(state.edit_buffer, "Step_Init_2");
        assert_eq!(state.cursor_pos, 11);

        // Backspace
        state.backspace();
        assert_eq!(state.edit_buffer, "Step_Init_");
        assert_eq!(state.cursor_pos, 10);

        // Cursor movement and middle insert
        state.move_cursor_left();
        state.insert_char('A');
        assert_eq!(state.edit_buffer, "Step_InitA_");

        // Delete forward
        state.move_cursor_left();
        state.delete_forward();
        assert_eq!(state.edit_buffer, "Step_Init_");

        // Cancel edit
        state.cancel_edit();
        assert!(state.editing_target.is_none());
        assert!(state.edit_buffer.is_empty());
        assert_eq!(state.cursor_pos, 0);
    }

    #[test]
    fn test_sfc_dynamic_dimensions() {
        use ferrite_lang_sfc::{SfcAction, SfcProgram, SfcStep, SfcTransition};

        let short_step = SfcStep {
            id: "1".to_string(),
            name: "S1".to_string(),
            kind: ferrite_lang_sfc::StepKind::Normal,
            actions: Vec::new(),
            grid_pos: Some((100, 100)),
            ..Default::default()
        };

        let long_step = SfcStep {
            id: "2".to_string(),
            name: "Conveyor_Sorting_Sequence_Active".to_string(),
            kind: ferrite_lang_sfc::StepKind::Normal,
            actions: vec![SfcAction {
                qualifier: ferrite_lang_sfc::ActionQualifier::N,
                target: "Conveyor_Motor_Start".to_string(),
                duration: None,
            }],
            grid_pos: Some((100, 250)),
            ..Default::default()
        };

        let prog = SfcProgram {
            name: "test_dim".to_string(),
            comment: String::new(),
            steps: vec![short_step.clone(), long_step.clone()],
            transitions: vec![SfcTransition {
                id: "1".to_string(),
                from_steps: vec!["1".to_string()],
                to_steps: vec!["2".to_string()],
                receptivity: "Sensor_1 AND NOT EStop".to_string(),
                grid_pos: Some((100, 180)),
                ..Default::default()
            }],
        };

        let canvas_prog = SfcCanvasProgram {
            pou_name: "test_dim",
            program: &prog,
            runtime: None,
            selected: None,
            selected_elements: &[],
            validation_report: None,
        };

        let state = SfcCanvasState::default();

        // 1. Verify dynamic step width expands for long names
        let w_short = canvas_prog.step_width(&short_step);
        let w_long = canvas_prog.step_width(&long_step);
        assert!(w_short >= 170.0, "Short step must have minimum width 170");
        assert!(w_long > w_short, "Long step must expand wider than short step");
        assert!(w_long <= 320.0, "Step width must cap at 320");

        // 2. Verify [+ Act] button rect is inside step bounds
        let (sx, _) = canvas_prog.step_pos(&short_step, &state);
        let add_btn_rect = canvas_prog.step_add_action_rect(&short_step, &state);
        assert!(add_btn_rect.x >= sx);
        assert!(add_btn_rect.x + add_btn_rect.width <= sx + w_short);

        // 3. Verify step name rect is inside step bounds
        let name_rect = canvas_prog.step_name_rect(&short_step, &state);
        assert!(name_rect.x >= sx);
        assert!(name_rect.x + name_rect.width <= sx + w_short);

        // 4. Verify transition receptivity badge width scales exactly to text plus small padding
        let tr_rect = canvas_prog.trans_receptivity_rect(&prog.transitions[0], &state);
        assert!(tr_rect.width >= 100.0, "Long receptivity must have wide badge");

        let mut short_trans = prog.transitions[0].clone();
        short_trans.receptivity = "1".to_string();
        let short_tr_rect = canvas_prog.trans_receptivity_rect(&short_trans, &state);
        assert_eq!(short_tr_rect.width, 18.0, "Single-char receptivity must be compact with small padding");
    }

    #[test]
    fn test_sfc_canvas_inline_events() {
        use ferrite_lang_sfc::{ActionQualifier, SfcAction, SfcProgram, SfcStep, SfcTransition};
        use iced::mouse;

        let step = SfcStep {
            id: "10".to_string(),
            name: "Step_Old".to_string(),
            kind: ferrite_lang_sfc::StepKind::Normal,
            actions: vec![
                SfcAction {
                    qualifier: ActionQualifier::N,
                    target: "Pump_Run".to_string(),
                    duration: None,
                },
                SfcAction {
                    qualifier: ActionQualifier::L,
                    target: "Timer_Fan".to_string(),
                    duration: Some("T#5s".to_string()),
                },
            ],
            grid_pos: Some((100, 100)),
            ..Default::default()
        };

        let trans = SfcTransition {
            id: "1".to_string(),
            from_steps: vec!["10".to_string()],
            to_steps: vec![],
            receptivity: "Sensor_OK".to_string(),
            grid_pos: Some((100, 200)),
            ..Default::default()
        };

        let prog = SfcProgram {
            name: "test_inline".to_string(),
            comment: String::new(),
            steps: vec![step.clone()],
            transitions: vec![trans.clone()],
        };

        let canvas_prog = SfcCanvasProgram {
            pou_name: "test_inline",
            program: &prog,
            runtime: None,
            selected: None,
            selected_elements: &[],
            validation_report: None,
        };

        let mut state = SfcCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(800.0, 600.0));

        // 1. Click [+ Act] button
        let add_btn = canvas_prog.step_add_action_rect(&step, &state);
        let click_pt = Point::new(add_btn.x + 5.0, add_btn.y + 5.0);
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(click_pt),
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::SfcAddAction(p, 0)) if p == "test_inline"
        ));

        // 2. Click Qualifier badge to open dropdown menu, then click item 'S' to select
        let q_rect = canvas_prog.action_qualifier_rect(&step, 0, &state);
        let q_click = Point::new(q_rect.x + 5.0, q_rect.y + 5.0);
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(q_click),
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(msg.is_none());
        assert_eq!(
            state.qualifier_dropdown,
            Some(SfcQualifierDropdown {
                step_id: "10".to_string(),
                action_idx: 0,
                rect: Rectangle {
                    x: q_rect.x,
                    y: q_rect.y + q_rect.height + 2.0,
                    width: 160.0,
                    height: 8.0 * 22.0 + 4.0,
                }
            })
        );

        // Verify occlusion of underlying elements when dropdown is open:
        if let Some(ref dd) = state.qualifier_dropdown {
            let act1_q_rect = canvas_prog.action_qualifier_rect(&step, 1, &state);
            let act1_del_rect = canvas_prog.action_delete_rect(&step, 1, &state);
            let act1_row = Rectangle {
                x: act1_q_rect.x,
                y: act1_q_rect.y,
                width: (act1_del_rect.x + act1_del_rect.width) - act1_q_rect.x,
                height: act1_q_rect.height,
            };
            assert!(dd.rect.intersects(&act1_row), "Underlying action row must be occluded by dropdown menu");

            // A transition positioned underneath the dropdown menu is occluded:
            let trans_under = SfcTransition {
                id: "2".to_string(),
                from_steps: vec!["10".to_string()],
                to_steps: vec![],
                receptivity: "Sensor_OK".to_string(),
                grid_pos: Some((240, 160)),
                ..Default::default()
            };
            let trans_rec = canvas_prog.trans_receptivity_rect(&trans_under, &state);
            assert!(dd.rect.intersects(&trans_rec), "Transition receptivity underneath dropdown menu must be occluded");
        }

        // Click 2nd item in dropdown: index 1 is ActionQualifier::S (S - Set)
        let item_click = Point::new(q_rect.x + 20.0, q_rect.y + q_rect.height + 2.0 + 2.0 + 22.0 * 1.0 + 5.0);
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(item_click),
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::SfcActionQualifierChanged(
                p,
                0,
                0,
                ActionQualifier::S
            )) if p == "test_inline"
        ));
        assert!(state.qualifier_dropdown.is_none());

        // Open dropdown again and press Escape to cancel
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(q_click),
        );
        assert!(state.qualifier_dropdown.is_some());
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(msg.is_none());
        assert!(state.qualifier_dropdown.is_none());

        // 3. Click Step Name to enter edit mode, type, and press Enter to commit
        let name_rect = canvas_prog.step_name_rect(&step, &state);
        let name_click = Point::new(name_rect.x + 5.0, name_rect.y + 5.0);
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(name_click),
        );
        assert_eq!(
            state.editing_target,
            Some(SfcEditTarget::StepName {
                step_id: "10".to_string()
            })
        );
        assert_eq!(state.edit_buffer, "Step_Old");

        // Type character
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Character("2".into()),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: Some("2".into()),
                modified_key: iced::keyboard::Key::Character("2".into()),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Digit2),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(state.edit_buffer, "Step_Old2");

        // Press Enter to commit
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::SfcStepNameChanged(
                p,
                0,
                name
            )) if p == "test_inline" && name == "Step_Old2"
        ));
        assert!(state.editing_target.is_none());

        // 4. Click Action Target, type, commit
        let target_rect = canvas_prog.action_target_rect(&step, 0, &state);
        let target_click = Point::new(target_rect.x + 2.0, target_rect.y + 2.0);
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(target_click),
        );
        assert_eq!(
            state.editing_target,
            Some(SfcEditTarget::ActionTarget {
                step_id: "10".to_string(),
                action_idx: 0
            })
        );
        assert_eq!(state.edit_buffer, "Pump_Run");

        // Type 'e' and Enter
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Character("e".into()),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: Some("e".into()),
                modified_key: iced::keyboard::Key::Character("e".into()),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::KeyE),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::SfcActionTargetChanged(
                p,
                0,
                0,
                target
            )) if p == "test_inline" && target == "Pump_Rune"
        ));

        // 5. Click Action Duration, type, cancel with Escape
        let dur_rect = canvas_prog
            .action_duration_rect(&step, 1, &state)
            .unwrap_or_default();
        let dur_click = Point::new(dur_rect.x + 2.0, dur_rect.y + 2.0);
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(dur_click),
        );
        assert_eq!(
            state.editing_target,
            Some(SfcEditTarget::ActionDuration {
                step_id: "10".to_string(),
                action_idx: 1
            })
        );
        assert_eq!(state.edit_buffer, "T#5s");
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(msg.is_none());
        assert!(state.editing_target.is_none());

        // 6. Click Transition Receptivity, type, commit
        let tr_rect = canvas_prog.trans_receptivity_rect(&trans, &state);
        let tr_click = Point::new(tr_rect.x + 5.0, tr_rect.y + 5.0);
        let _ = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(tr_click),
        );
        assert_eq!(
            state.editing_target,
            Some(SfcEditTarget::TransitionReceptivity {
                trans_id: "1".to_string()
            })
        );
        assert_eq!(state.edit_buffer, "Sensor_OK");

        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
            }),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::SfcTransitionReceptivityChanged(
                p,
                0,
                rec
            )) if p == "test_inline" && rec == "Sensor_OK"
        ));

        // 7. Click Action Delete button [×]
        let del_rect = canvas_prog.action_delete_rect(&step, 0, &state);
        let del_click = Point::new(del_rect.x + 2.0, del_rect.y + 2.0);
        let (status, msg) = canvas_prog.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(del_click),
        );
        assert_eq!(status, iced::event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::SfcRemoveAction(
                p,
                0,
                0
            )) if p == "test_inline"
        ));
    }
}
