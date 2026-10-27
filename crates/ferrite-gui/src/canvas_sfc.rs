use iced::border::Radius;
use iced::mouse;
use iced::widget::canvas::{self, event, Frame, Geometry, Path, Stroke};
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use ferrite_core::IecValue;
pub use ferrite_lang_sfc::compute_sfc_step_rows;
use ferrite_lang_sfc::{
    validate_sfc_topology, ActionQualifier, ConvergenceKind, DivergenceKind, SfcProgram, SfcStep,
    SfcTransition, SfcValidationError, SfcValidationReport, StepKind,
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
pub const PORT_HIT_DIST: f32 = 12.0;

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
    TransitionOutput(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SfcPortTarget {
    StepInput(String),
    TransitionInput(String),
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
}

#[derive(Debug, Clone)]
pub struct SfcCanvasState {
    pub drag: DragState,
    pub hover_port: Option<PortHover>,
    pub selected: Option<SfcElementKey>,
}

impl Default for SfcCanvasState {
    fn default() -> Self {
        Self {
            drag: DragState::None,
            hover_port: None,
            selected: None,
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

pub fn is_step_active(rt: &PlcRuntime, pou_name: &str, step_id: &str) -> (bool, u64) {
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
    let p1 = Point::new(tip.x - dir.0 * len - dir.1 * width, tip.y - dir.1 * len + dir.0 * width);
    let p2 = Point::new(tip.x - dir.0 * len + dir.1 * width, tip.y - dir.1 * len - dir.0 * width);
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
        {
            if &step.id == id {
                let (gx, gy) = snap_to_grid(current_x, current_y);
                return (gx as f32, gy as f32);
            }
        }
        let (gx, gy) = step.grid_pos.unwrap_or((60, 40));
        (gx as f32, gy as f32)
    }

    fn trans_pos(&self, tr: &SfcTransition, state: &SfcCanvasState) -> (f32, f32) {
        if let DragState::MovingElement {
            key: SfcElementKey::Transition(ref id),
            current_x,
            current_y,
            ..
        } = state.drag
        {
            if &tr.id == id {
                let (gx, gy) = snap_to_grid(current_x, current_y);
                return (gx as f32, gy as f32);
            }
        }
        let (gx, gy) = tr.grid_pos.unwrap_or((110, 140));
        (gx as f32, gy as f32)
    }

    fn step_in_port(&self, step: &SfcStep, state: &SfcCanvasState) -> Point {
        let (x, y) = self.step_pos(step, state);
        Point::new(x + STEP_WIDTH * 0.5, y)
    }

    fn step_out_port(&self, step: &SfcStep, state: &SfcCanvasState) -> Point {
        let (x, y) = self.step_pos(step, state);
        Point::new(x + STEP_WIDTH * 0.5, y + STEP_HEIGHT)
    }

    fn trans_in_port(&self, tr: &SfcTransition, state: &SfcCanvasState) -> Point {
        let (x, y) = self.trans_pos(tr, state);
        Point::new(x + TRANS_WIDTH * 0.5, y)
    }

    fn trans_out_port(&self, tr: &SfcTransition, state: &SfcCanvasState) -> Point {
        let (x, y) = self.trans_pos(tr, state);
        Point::new(x + TRANS_WIDTH * 0.5, y + TRANS_HEIGHT)
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
        let cursor_pos = match cursor.position_in(bounds) {
            Some(p) => p,
            None => return (event::Status::Ignored, None),
        };

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                // 1. Check if clicking an Output Port to begin dragging a wire
                for step in &self.program.steps {
                    let out_p = self.step_out_port(step, state);
                    if cursor_pos.distance(out_p) <= PORT_HIT_DIST {
                        state.drag = DragState::DraggingWire {
                            source: SfcPortSource::StepOutput(step.id.clone()),
                            start_pt: out_p,
                            current_pt: cursor_pos,
                        };
                        return (event::Status::Captured, None);
                    }
                }
                for tr in &self.program.transitions {
                    let out_p = self.trans_out_port(tr, state);
                    if cursor_pos.distance(out_p) <= PORT_HIT_DIST {
                        state.drag = DragState::DraggingWire {
                            source: SfcPortSource::TransitionOutput(tr.id.clone()),
                            start_pt: out_p,
                            current_pt: cursor_pos,
                        };
                        return (event::Status::Captured, None);
                    }
                }

                // 2. Check if clicking inside a Step box to drag or select
                for step in self.program.steps.iter().rev() {
                    let (x, y) = self.step_pos(step, state);
                    if cursor_pos.x >= x
                        && cursor_pos.x <= x + STEP_WIDTH
                        && cursor_pos.y >= y
                        && cursor_pos.y <= y + STEP_HEIGHT
                    {
                        let key = SfcElementKey::Step(step.id.clone());
                        state.selected = Some(key.clone());
                        state.drag = DragState::MovingElement {
                            key: key.clone(),
                            offset_x: cursor_pos.x - x,
                            offset_y: cursor_pos.y - y,
                            current_x: x,
                            current_y: y,
                        };
                        return (
                            event::Status::Captured,
                            Some(Message::SfcSelectElement(self.pou_name.to_string(), Some(key))),
                        );
                    }
                }

                // 3. Check if clicking inside a Transition bar to drag or select
                for tr in self.program.transitions.iter().rev() {
                    let (x, y) = self.trans_pos(tr, state);
                    if cursor_pos.x >= x
                        && cursor_pos.x <= x + TRANS_WIDTH
                        && cursor_pos.y >= y
                        && cursor_pos.y <= y + TRANS_HEIGHT
                    {
                        let key = SfcElementKey::Transition(tr.id.clone());
                        state.selected = Some(key.clone());
                        state.drag = DragState::MovingElement {
                            key: key.clone(),
                            offset_x: cursor_pos.x - x,
                            offset_y: cursor_pos.y - y,
                            current_x: x,
                            current_y: y,
                        };
                        return (
                            event::Status::Captured,
                            Some(Message::SfcSelectElement(self.pou_name.to_string(), Some(key))),
                        );
                    }
                }

                // 4. Clicked on empty canvas: deselect
                state.selected = None;
                (
                    event::Status::Captured,
                    Some(Message::SfcSelectElement(self.pou_name.to_string(), None)),
                )
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
                        (event::Status::Captured, None)
                    }
                    DragState::DraggingWire { current_pt, .. } => {
                        *current_pt = cursor_pos;
                        // Check hovering over compatible input ports
                        state.hover_port = None;
                        for tr in &self.program.transitions {
                            let in_p = self.trans_in_port(tr, state);
                            if cursor_pos.distance(in_p) <= PORT_HIT_DIST {
                                state.hover_port = Some(PortHover::TransitionInput(tr.id.clone(), in_p));
                                break;
                            }
                        }
                        if state.hover_port.is_none() {
                            for step in &self.program.steps {
                                let in_p = self.step_in_port(step, state);
                                if cursor_pos.distance(in_p) <= PORT_HIT_DIST {
                                    state.hover_port = Some(PortHover::StepInput(step.id.clone(), in_p));
                                    break;
                                }
                            }
                        }
                        (event::Status::Captured, None)
                    }
                    DragState::None => {
                        // Check hover on any port
                        state.hover_port = None;
                        for step in &self.program.steps {
                            let in_p = self.step_in_port(step, state);
                            if cursor_pos.distance(in_p) <= PORT_HIT_DIST {
                                state.hover_port = Some(PortHover::StepInput(step.id.clone(), in_p));
                                break;
                            }
                            let out_p = self.step_out_port(step, state);
                            if cursor_pos.distance(out_p) <= PORT_HIT_DIST {
                                state.hover_port = Some(PortHover::StepOutput(step.id.clone(), out_p));
                                break;
                            }
                        }
                        if state.hover_port.is_none() {
                            for tr in &self.program.transitions {
                                let in_p = self.trans_in_port(tr, state);
                                if cursor_pos.distance(in_p) <= PORT_HIT_DIST {
                                    state.hover_port = Some(PortHover::TransitionInput(tr.id.clone(), in_p));
                                    break;
                                }
                                let out_p = self.trans_out_port(tr, state);
                                if cursor_pos.distance(out_p) <= PORT_HIT_DIST {
                                    state.hover_port = Some(PortHover::TransitionOutput(tr.id.clone(), out_p));
                                    break;
                                }
                            }
                        }
                        (event::Status::Ignored, None)
                    }
                }
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
                        // Check if released over a valid target port
                        match source {
                            SfcPortSource::StepOutput(_) => {
                                for tr in &self.program.transitions {
                                    let in_p = self.trans_in_port(tr, state);
                                    if cursor_pos.distance(in_p) <= PORT_HIT_DIST {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectPorts {
                                                pou_name: self.pou_name.to_string(),
                                                source,
                                                target: SfcPortTarget::TransitionInput(tr.id.clone()),
                                            }),
                                        );
                                    }
                                }
                            }
                            SfcPortSource::TransitionOutput(_) => {
                                for step in &self.program.steps {
                                    let in_p = self.step_in_port(step, state);
                                    if cursor_pos.distance(in_p) <= PORT_HIT_DIST {
                                        return (
                                            event::Status::Captured,
                                            Some(Message::SfcConnectPorts {
                                                pou_name: self.pou_name.to_string(),
                                                source,
                                                target: SfcPortTarget::StepInput(step.id.clone()),
                                            }),
                                        );
                                    }
                                }
                            }
                        }
                        (event::Status::Captured, None)
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
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        // 1. Background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), COLOR_CANVAS_BG);

        // 2. Discrete 20px grid dots
        let dot_color = Color::from_rgba(1.0, 1.0, 1.0, 0.05);
        for x in (0..width as i32).step_by(20) {
            for y in (0..height as i32).step_by(20) {
                frame.fill_rectangle(Point::new(x as f32, y as f32), Size::new(1.5, 1.5), dot_color);
            }
        }

        // 3. Wires and IEC 60848 AND Double Lines (===)
        for tr in &self.program.transitions {
            let tr_in = self.trans_in_port(tr, state);
            let tr_out = self.trans_out_port(tr, state);

            // A. Incoming wires from source steps -> transition
            let from_steps: Vec<&SfcStep> = tr
                .from_steps
                .iter()
                .filter_map(|sid| {
                    self.program
                        .steps
                        .iter()
                        .find(|s| &s.id == sid || &s.name == sid || sid == &format!("X{}", s.id))
                })
                .collect();

            if from_steps.len() > 1 || tr.convergence == ConvergenceKind::And {
                // IEC 60848 AND Convergence: Authentic double horizontal lines!
                if !from_steps.is_empty() {
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

                    let wire_stroke = Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.5);
                    frame.stroke(&Path::line(Point::new(bar_left, bar_y - 2.5), Point::new(bar_right, bar_y - 2.5)), wire_stroke);
                    frame.stroke(&Path::line(Point::new(bar_left, bar_y + 2.5), Point::new(bar_right, bar_y + 2.5)), wire_stroke);

                    // Drop wires from each step to top line
                    for s in &from_steps {
                        let sp = self.step_out_port(s, state);
                        frame.stroke(&Path::line(sp, Point::new(sp.x, bar_y - 2.5)), Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
                    }
                    // Center wire from bottom line into transition
                    frame.stroke(&Path::line(Point::new(tr_in.x, bar_y + 2.5), tr_in), Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
                }
            } else if let Some(s) = from_steps.first() {
                let sp = self.step_out_port(s, state);
                if (sp.x - tr_in.x).abs() < 2.0 {
                    frame.stroke(&Path::line(sp, tr_in), Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
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
                    frame.stroke(&path, Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
                }
            }

            // B. Outgoing wires from transition -> destination steps
            let to_steps: Vec<&SfcStep> = tr
                .to_steps
                .iter()
                .filter_map(|sid| {
                    self.program
                        .steps
                        .iter()
                        .find(|s| &s.id == sid || &s.name == sid || sid == &format!("X{}", s.id))
                })
                .collect();

            if to_steps.len() > 1 || tr.divergence == DivergenceKind::And {
                // IEC 60848 AND Divergence: Authentic double horizontal lines!
                if !to_steps.is_empty() {
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

                    let wire_stroke = Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.5);
                    frame.stroke(&Path::line(Point::new(bar_left, bar_y - 2.5), Point::new(bar_right, bar_y - 2.5)), wire_stroke);
                    frame.stroke(&Path::line(Point::new(bar_left, bar_y + 2.5), Point::new(bar_right, bar_y + 2.5)), wire_stroke);

                    // Central wire from transition into top line
                    frame.stroke(&Path::line(tr_out, Point::new(tr_out.x, bar_y - 2.5)), Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));

                    // Drop wires from bottom line to each target step, with arrowhead
                    for s in &to_steps {
                        let sp = self.step_in_port(s, state);
                        frame.stroke(&Path::line(Point::new(sp.x, bar_y + 2.5), sp), Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
                        draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
                    }
                }
            } else if let Some(s) = to_steps.first() {
                let sp = self.step_in_port(s, state);
                if sp.y >= tr_out.y {
                    if (sp.x - tr_out.x).abs() < 2.0 {
                        frame.stroke(&Path::line(tr_out, sp), Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
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
                        frame.stroke(&path, Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(2.0));
                    }
                    draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_INACTIVE_STEP);
                } else {
                    // Loopback link towards a previous step
                    let side_x = (tr_out.x.min(sp.x) - 40.0).max(20.0);
                    let p1 = Point::new(tr_out.x, tr_out.y + 12.0);
                    let p2 = Point::new(side_x, p1.y);
                    let p3 = Point::new(side_x, sp.y - 12.0);
                    let p4 = Point::new(sp.x, sp.y - 12.0);
                    let path = Path::new(|b| {
                        b.move_to(tr_out);
                        b.line_to(p1);
                        b.line_to(p2);
                        b.line_to(p3);
                        b.line_to(p4);
                        b.line_to(sp);
                    });
                    frame.stroke(&path, Stroke::default().with_color(COLOR_ACTIVE_STEP).with_width(1.8));
                    draw_arrowhead(&mut frame, sp, (0.0, 1.0), COLOR_ACTIVE_STEP);
                }
            }
        }

        // 4. Render Steps
        for step in &self.program.steps {
            let (x, y) = self.step_pos(step, state);
            let (active, elapsed_ms) = self
                .runtime
                .map(|rt| is_step_active(rt, &self.program.name, &step.id))
                .unwrap_or((false, 0));

            let is_selected = self.selected == Some(&SfcElementKey::Step(step.id.clone()))
                || state.selected.as_ref() == Some(&SfcElementKey::Step(step.id.clone()));

            let (tree_id, has_error, _err_msg) = if let Some(ref report) = self.validation_report {
                let t_id = report.trees.iter().find(|t| t.step_ids.contains(&step.id)).map(|t| t.tree_id);
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
                        SfcValidationError::OrphanElement { element_id, is_step } => {
                            if *is_step && element_id == &step.id {
                                err = Some("Unlinked Step");
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                (t_id, err.is_some(), err)
            } else {
                (None, false, None)
            };

            let border_color = if has_error {
                Color::from_rgb(1.0, 0.35, 0.35)
            } else if active {
                COLOR_ACTIVE_STEP
            } else if is_selected {
                COLOR_STEP_DROP
            } else {
                COLOR_INACTIVE_STEP
            };

            // Glow / fill
            let bg_color = if active {
                Color::from_rgb(22.0 / 255.0, 46.0 / 255.0, 34.0 / 255.0)
            } else {
                COLOR_STEP_BG
            };

            frame.fill_rectangle(Point::new(x, y), Size::new(STEP_WIDTH, STEP_HEIGHT), bg_color);
            frame.stroke_rectangle(
                Point::new(x, y),
                Size::new(STEP_WIDTH, STEP_HEIGHT),
                Stroke::default().with_color(border_color).with_width(if is_selected || has_error { 2.5 } else { 2.0 }),
            );

            // Error halo outline
            if has_error {
                frame.stroke_rectangle(
                    Point::new(x - 2.0, y - 2.0),
                    Size::new(STEP_WIDTH + 4.0, STEP_HEIGHT + 4.0),
                    Stroke::default().with_color(Color::from_rgba(1.0, 0.3, 0.3, 0.4)).with_width(2.0),
                );
            }

            // Initial Step: authentic double inner border
            if step.kind == StepKind::Initial {
                frame.stroke_rectangle(
                    Point::new(x + 3.0, y + 3.0),
                    Size::new(STEP_WIDTH - 6.0, STEP_HEIGHT - 6.0),
                    Stroke::default().with_color(border_color).with_width(1.2),
                );
            }

            // Step ID text
            frame.fill_text(canvas::Text {
                content: format!("X{}", step.id),
                position: Point::new(x + 10.0, y + 8.0),
                color: Color::WHITE,
                size: iced::Pixels(13.0),
                font: iced::Font::MONOSPACE,
                ..Default::default()
            });

            // Tree badge or error badge
            if has_error {
                frame.fill_text(canvas::Text {
                    content: "⚠".to_string(),
                    position: Point::new(x + STEP_WIDTH - 24.0, y + 7.0),
                    color: Color::from_rgb(1.0, 0.35, 0.35),
                    size: iced::Pixels(12.0),
                    ..Default::default()
                });
            } else if let Some(tid) = tree_id {
                if self.validation_report.as_ref().map(|r| r.trees.len()).unwrap_or(0) > 1 {
                    frame.fill_text(canvas::Text {
                        content: format!("T{}", tid + 1),
                        position: Point::new(x + STEP_WIDTH - 28.0, y + 8.0),
                        color: COLOR_STEP_DROP,
                        size: iced::Pixels(11.0),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });
                }
            }

            // Active dot & timer
            if active {
                frame.fill_text(canvas::Text {
                    content: "●".to_string(),
                    position: Point::new(x + 52.0, y + 8.0),
                    color: COLOR_ACTIVE_STEP,
                    size: iced::Pixels(11.0),
                    ..Default::default()
                });
                frame.fill_text(canvas::Text {
                    content: format!("T#{}ms", elapsed_ms),
                    position: Point::new(x + 72.0, y + 9.0),
                    color: COLOR_ACTIVE_STEP,
                    size: iced::Pixels(11.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            // Step Name
            frame.fill_text(canvas::Text {
                content: step.name.clone(),
                position: Point::new(x + 10.0, y + 34.0),
                color: Color::from_rgb(220.0 / 255.0, 230.0 / 255.0, 245.0 / 255.0),
                size: iced::Pixels(12.0),
                ..Default::default()
            });

            // Action blocks attached on right side
            if !step.actions.is_empty() {
                let wire_start = Point::new(x + STEP_WIDTH, y + STEP_HEIGHT * 0.5);
                let wire_end = Point::new(x + STEP_WIDTH + 14.0, y + STEP_HEIGHT * 0.5);
                frame.stroke(&Path::line(wire_start, wire_end), Stroke::default().with_color(border_color).with_width(1.5));

                let act_x = wire_end.x;
                let mut act_y = y + 4.0;
                for act in step.actions.iter().take(2) {
                    let qual_str = SfcActionQualifierWrapper(act.qualifier).to_string();
                    frame.fill_rectangle(Point::new(act_x, act_y), Size::new(26.0, 22.0), COLOR_ACTION_BG);
                    frame.stroke_rectangle(
                        Point::new(act_x, act_y),
                        Size::new(26.0, 22.0),
                        Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(1.0),
                    );
                    frame.fill_text(canvas::Text {
                        content: qual_str,
                        position: Point::new(act_x + 8.0, act_y + 4.0),
                        color: Color::WHITE,
                        size: iced::Pixels(11.0),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    let target_w = 110.0;
                    frame.fill_rectangle(Point::new(act_x + 26.0, act_y), Size::new(target_w, 22.0), COLOR_STEP_BG);
                    frame.stroke_rectangle(
                        Point::new(act_x + 26.0, act_y),
                        Size::new(target_w, 22.0),
                        Stroke::default().with_color(COLOR_INACTIVE_STEP).with_width(1.0),
                    );
                    frame.fill_text(canvas::Text {
                        content: act.target.clone(),
                        position: Point::new(act_x + 30.0, act_y + 4.0),
                        color: Color::from_rgb(200.0 / 255.0, 215.0 / 255.0, 230.0 / 255.0),
                        size: iced::Pixels(10.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    act_y += 25.0;
                }
            }

            // Input / Output Ports
            let in_p = self.step_in_port(step, state);
            let out_p = self.step_out_port(step, state);

            let is_in_hovered = state.hover_port.as_ref() == Some(&PortHover::StepInput(step.id.clone(), in_p));
            let is_out_hovered = state.hover_port.as_ref() == Some(&PortHover::StepOutput(step.id.clone(), out_p));

            frame.fill(&Path::circle(in_p, if is_in_hovered { 6.0 } else { PORT_RADIUS }), if is_in_hovered { COLOR_STEP_DROP } else { border_color });
            frame.fill(&Path::circle(out_p, if is_out_hovered { 6.0 } else { PORT_RADIUS }), if is_out_hovered { COLOR_STEP_DROP } else { border_color });
        }

        // 5. Render Transitions
        for tr in &self.program.transitions {
            let (x, y) = self.trans_pos(tr, state);
            let is_selected = self.selected == Some(&SfcElementKey::Transition(tr.id.clone()))
                || state.selected.as_ref() == Some(&SfcElementKey::Transition(tr.id.clone()));

            let (tree_id, has_error, _err_msg) = if let Some(ref report) = self.validation_report {
                let t_id = report.trees.iter().find(|t| t.transition_ids.contains(&tr.id)).map(|t| t.tree_id);
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
                        SfcValidationError::OrphanElement { element_id, is_step } => {
                            if !*is_step && element_id == &tr.id {
                                err = Some("Unlinked Transition");
                                break;
                            }
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

            // Error halo around transition crossbar
            if has_error {
                frame.stroke(
                    &Path::line(Point::new(x - 3.0, cy), Point::new(x + TRANS_WIDTH + 3.0, cy)),
                    Stroke::default().with_color(Color::from_rgba(1.0, 0.3, 0.3, 0.4)).with_width(6.0),
                );
            }

            // Authentic IEC 60848 transition crossbar
            frame.stroke(
                &Path::line(Point::new(x, cy), Point::new(x + TRANS_WIDTH, cy)),
                Stroke::default().with_color(color).with_width(if is_selected || has_error { 4.0 } else { 3.2 }),
            );

            // Receptivity text
            let rec_text = if tr.receptivity.is_empty() { "TRUE" } else { &tr.receptivity };
            frame.fill_text(canvas::Text {
                content: format!("[ {} ]", rec_text),
                position: Point::new(x + TRANS_WIDTH + 6.0, cy - 6.0),
                color: if has_error { Color::from_rgb(1.0, 0.4, 0.4) } else { Color::from_rgb(255.0 / 255.0, 210.0 / 255.0, 120.0 / 255.0) },
                size: iced::Pixels(11.5),
                font: iced::Font::MONOSPACE,
                ..Default::default()
            });

            // Transition ID and tree or error badge
            let tid_str = if has_error {
                format!("{} ⚠", tr.id)
            } else if let Some(tid) = tree_id {
                if self.validation_report.as_ref().map(|r| r.trees.len()).unwrap_or(0) > 1 {
                    format!("{} T{}", tr.id, tid + 1)
                } else {
                    tr.id.clone()
                }
            } else {
                tr.id.clone()
            };

            frame.fill_text(canvas::Text {
                content: tid_str,
                position: Point::new(x - 44.0, cy - 6.0),
                color: if has_error { Color::from_rgb(1.0, 0.35, 0.35) } else { COLOR_INACTIVE_STEP },
                size: iced::Pixels(10.5),
                font: iced::Font::MONOSPACE,
                ..Default::default()
            });

            // Input / Output Ports
            let in_p = self.trans_in_port(tr, state);
            let out_p = self.trans_out_port(tr, state);

            let is_in_hovered = state.hover_port.as_ref() == Some(&PortHover::TransitionInput(tr.id.clone(), in_p));
            let is_out_hovered = state.hover_port.as_ref() == Some(&PortHover::TransitionOutput(tr.id.clone(), out_p));

            frame.fill(&Path::circle(in_p, if is_in_hovered { 6.0 } else { PORT_RADIUS }), if is_in_hovered { COLOR_STEP_DROP } else { color });
            frame.fill(&Path::circle(out_p, if is_out_hovered { 6.0 } else { PORT_RADIUS }), if is_out_hovered { COLOR_STEP_DROP } else { color });
        }

        // 6. Active Wire Dragging Elastic Rubber-Band
        if let DragState::DraggingWire { start_pt, current_pt, .. } = state.drag {
            let wire_stroke = Stroke::default().with_color(COLOR_STEP_DROP).with_width(2.2);
            frame.stroke(&Path::line(start_pt, current_pt), wire_stroke);
            frame.fill(&Path::circle(current_pt, 5.0), COLOR_STEP_DROP);
        }

        vec![frame.into_geometry()]
    }
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

/// Renders the quick-editor context bar when a Step is selected
fn render_step_inspector<'a>(
    pou_name: &'a str,
    program: &'a SfcProgram,
    step_id: &str,
    _lang: UiLanguage,
) -> Option<Element<'a, Message>> {
    let s_idx = program.steps.iter().position(|s| s.id == step_id)?;
    let step = &program.steps[s_idx];
    let p_name = pou_name.to_string();

    let id_label = text(format!("Step X{}:", step.id))
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

    let kind_picker = pick_list(
        &ALL_STEP_KINDS[..],
        Some(SfcStepKindWrapper(step.kind)),
        {
            let p = p_name.clone();
            move |w| Message::SfcStepKindChanged(p.clone(), s_idx, w.0)
        },
    )
    .width(90)
    .style(StudioTheme::styled_pick_list);

    let add_act_btn = button(text("+ Action").size(11))
        .on_press(Message::SfcAddAction(p_name.clone(), s_idx))
        .style(sfc_subtle_button)
        .padding([4, 8]);

    let del_btn = button(text("Delete").size(11))
        .on_press(Message::SfcRemoveStep(p_name, s_idx))
        .style(StudioTheme::danger_button)
        .padding([4, 8]);

    let mut content = row![id_label, name_input, kind_picker, add_act_btn, del_btn]
        .spacing(10)
        .align_y(Alignment::Center);

    // If step has actions, display the first action inline
    if let Some(act) = step.actions.first() {
        let p = pou_name.to_string();
        let qual_picker = pick_list(
            &ALL_ACTION_QUALIFIERS[..],
            Some(SfcActionQualifierWrapper(act.qualifier)),
            {
                let p = p.clone();
                move |w| Message::SfcActionQualifierChanged(p.clone(), s_idx, 0, w.0)
            },
        )
        .width(50)
        .style(StudioTheme::styled_pick_list);

        let target_input = text_input("Action Target", &act.target)
            .on_input({
                let p = p.clone();
                move |t| Message::SfcActionTargetChanged(p.clone(), s_idx, 0, t)
            })
            .font(iced::Font::MONOSPACE)
            .size(11)
            .padding([4, 6])
            .style(sfc_input_style)
            .width(180);

        let del_act_btn = button(text("x").size(10))
            .on_press(Message::SfcRemoveAction(p, s_idx, 0))
            .style(StudioTheme::ghost_button)
            .padding([2, 5]);

        content = content.push(qual_picker).push(target_input).push(del_act_btn);
    }

    Some(content.into())
}

/// Renders the quick-editor context bar when a Transition is selected
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

    let rec_input = text_input("Receptivity Condition", &tr.receptivity)
        .on_input({
            let p = p_name.clone();
            move |r| Message::SfcTransitionReceptivityChanged(p.clone(), t_idx, r)
        })
        .font(iced::Font::MONOSPACE)
        .size(12)
        .padding([4, 8])
        .style(sfc_input_style)
        .width(220);

    let from_label = text(format!("From: [{}]", tr.from_steps.join(", ")))
        .size(11.5)
        .font(iced::Font::MONOSPACE)
        .color(COLOR_INACTIVE_STEP);

    let to_label = text(format!("To: [{}]", tr.to_steps.join(", ")))
        .size(11.5)
        .font(iced::Font::MONOSPACE)
        .color(COLOR_INACTIVE_STEP);

    let del_btn = button(text("Delete").size(11))
        .on_press(Message::SfcRemoveTransition(p_name, t_idx))
        .style(StudioTheme::danger_button)
        .padding([4, 8]);

    let content = row![id_label, rec_input, from_label, to_label, del_btn]
        .spacing(12)
        .align_y(Alignment::Center);

    Some(content.into())
}

/// Primary SFC Editor view builder
pub fn render_sfc_editor<'a>(
    program: &'a SfcProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
    selected: Option<&'a SfcElementKey>,
) -> Element<'a, Message> {
    let pou_name = &program.name;

    // Validate topology to find independent trees and error count
    let validation_report = validate_sfc_topology(program);
    let tree_count = validation_report.trees.len();
    let _error_count = validation_report.errors.len();

    // Check if an element is selected and render its inspector
    let inspector_elem: Option<Element<'a, Message>> = match selected {
        Some(SfcElementKey::Step(step_id)) => {
            render_step_inspector(pou_name, program, step_id, lang)
        }
        Some(SfcElementKey::Transition(trans_id)) => {
            render_transition_inspector(pou_name, program, trans_id, lang)
        }
        None => None,
    };

    let top_row: Element<'a, Message> = if let Some(insp) = inspector_elem {
        let deselect_btn = button(text("Close [x]").size(11))
            .on_press(Message::SfcSelectElement(pou_name.to_string(), None))
            .style(StudioTheme::ghost_button)
            .padding([3, 8]);
        container(
            row![insp, Space::with_width(Length::Fill), deselect_btn]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .padding([4, 8])
        .style(StudioTheme::subtle_badge_style(Color::from_rgba(0.2, 0.4, 0.6, 0.15)))
        .into()
    } else {
        // Toolbar buttons
        let add_step_btn = button(text("+ Step").size(11.5))
            .on_press(Message::SfcAddStep(pou_name.clone()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let add_trans_btn = button(text("+ Transition").size(11.5))
            .on_press(Message::SfcAddTransition(pou_name.clone()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let add_and_div_btn = button(text("+ AND Div (===)").size(11.5))
            .on_press(Message::SfcAddAndDivergence(pou_name.clone()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let auto_layout_btn = button(text("Auto-Layout Grid").size(11.5))
            .on_press(Message::SfcAutoLayout(pou_name.clone()))
            .style(sfc_subtle_button)
            .padding([4, 10]);

        let validate_btn = button(text("Check Topology").size(11.5))
            .on_press(Message::SfcValidate(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([4, 10]);

        let tree_badge = container(
            text(format!("{tree_count} Parallel Tree{}", if tree_count == 1 { "" } else { "s" }))
                .size(11)
                .color(COLOR_ACTIVE_STEP),
        )
        .padding([3, 8])
        .style(StudioTheme::subtle_badge_style(Color::from_rgba(0.18, 0.835, 0.45, 0.15)));

        let status_badge: Option<Element<'a, Message>> = if !validation_report.is_valid {
            Some(
                container(
                    text(format!("{} error{}", validation_report.errors.len(), if validation_report.errors.len() == 1 { "" } else { "s" }))
                        .size(11)
                        .color(Color::from_rgb(1.0, 0.4, 0.4)),
                )
                .padding([3, 8])
                .style(StudioTheme::subtle_badge_style(Color::from_rgba(1.0, 0.2, 0.2, 0.15)))
                .into(),
            )
        } else {
            None
        };

        let mut toolbar_row = row![
            add_step_btn,
            add_trans_btn,
            add_and_div_btn,
            auto_layout_btn,
            validate_btn,
            tree_badge,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        if let Some(sb) = status_badge {
            toolbar_row = toolbar_row.push(sb);
        }

        toolbar_row.into()
    };

    let tip = text("Tip: Drag elements to snap to 20px grid. Drag from Bottom Port (●) to Top Port (●) to connect wires. Click element to inspect.")
        .size(10.5)
        .color(StudioTheme::TEXT_MUTED);

    // Compute bounding dimensions for the canvas
    let max_x = program
        .steps
        .iter()
        .filter_map(|s| s.grid_pos.map(|p| p.0 + 360))
        .chain(program.transitions.iter().filter_map(|t| t.grid_pos.map(|p| p.0 + 360)))
        .max()
        .unwrap_or(1600)
        .max(1800) as f32;

    let max_y = program
        .steps
        .iter()
        .filter_map(|s| s.grid_pos.map(|p| p.1 + 300))
        .chain(program.transitions.iter().filter_map(|t| t.grid_pos.map(|p| p.1 + 300)))
        .max()
        .unwrap_or(1200)
        .max(1400) as f32;

    let canvas_widget = canvas::Canvas::new(SfcCanvasProgram {
        pou_name,
        program,
        runtime,
        selected,
        validation_report: Some(validation_report),
    })
    .width(Length::Fixed(max_x))
    .height(Length::Fixed(max_y));

    let scrollable_canvas = scrollable(canvas_widget).direction(scrollable::Direction::Both {
        vertical: scrollable::Scrollbar::new(),
        horizontal: scrollable::Scrollbar::new(),
    });

    let top_bar = column![top_row, tip].spacing(4);

    column![top_bar, scrollable_canvas]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_lang_sfc::{ActionQualifier, SfcAction, SfcProgram, SfcStep, SfcTransition, StepKind};

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
            validation_report: None,
        };
        let state = SfcCanvasState::default();
        let in_p = canvas.step_in_port(&canvas.program.steps[0], &state);
        let out_p = canvas.step_out_port(&canvas.program.steps[0], &state);

        assert_eq!(in_p, Point::new(60.0 + STEP_WIDTH * 0.5, 40.0));
        assert_eq!(out_p, Point::new(60.0 + STEP_WIDTH * 0.5, 40.0 + STEP_HEIGHT));

        // Hit testing within PORT_HIT_DIST
        assert!(Point::new(150.0, 102.0).distance(out_p) <= PORT_HIT_DIST);
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
        let _elem = render_sfc_editor(&program, None, UiLanguage::En, None);
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
            panic!("POU seq not found");
        }

        // 3. Move Step X1 and verify grid snap
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
}
