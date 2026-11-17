use std::collections::HashMap;

use iced::border::Radius;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, event};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer,
    Size, Theme, Vector, mouse,
};

use ferrite_core::{IecValue, StandardFbKind};
use ferrite_lang_fbd::{FbdBlock, FbdBlockKind, FbdInputSource, FbdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::fbd_geometry::{
    self, ADD_PIN_H, HEADER_H, MIN_BLOCK_W, ROW_INSET, block_glyph, block_size_with_edit,
    format_row_display, header_rect_with_width, id_rect_with_width, input_delete_rect,
    input_pin_point, input_pins, input_row_rect, input_row_text, input_value_rect, is_variadic,
    kind_badge_rect, output_delete_rect, output_pin_point, output_pins, output_row_rect,
    output_value_rect,
};
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

fn format_live_iec_value(val: IecValue) -> String {
    match val {
        IecValue::Bool(b) => {
            if b {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
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
    pub fn accent_color(self) -> Color {
        match self {
            Self::Logic => Color::from_rgb(0.388, 0.400, 0.945),
            Self::Arithmetic => Color::from_rgb(0.961, 0.620, 0.043),
            Self::Comparator => Color::from_rgb(0.918, 0.702, 0.031),
            Self::Timer => Color::from_rgb(0.024, 0.714, 0.831),
            Self::Counter => Color::from_rgb(0.659, 0.333, 0.969),
            Self::Edge => Color::from_rgb(0.925, 0.282, 0.600),
            Self::Bistable => Color::from_rgb(0.231, 0.510, 0.965),
            Self::Conditioning => Color::from_rgb(0.078, 0.722, 0.651),
            Self::Corrector => Color::from_rgb(0.063, 0.725, 0.506),
            Self::Composite => Color::from_rgb(0.957, 0.247, 0.369),
        }
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
            StandardFbKind::Hysteresis | StandardFbKind::RampReal | StandardFbKind::ScaleLin => {
                FbdCategory::Conditioning
            }
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

fn operator_symbol(kind: &FbdBlockKind) -> &str {
    match kind {
        FbdBlockKind::And => "&",
        FbdBlockKind::Or => "\u{2265}1",
        FbdBlockKind::Xor => "=1",
        FbdBlockKind::Not => "1",
        FbdBlockKind::Add => "+",
        FbdBlockKind::Sub => "\u{2212}",
        FbdBlockKind::Mul => "\u{00D7}",
        FbdBlockKind::Div => "\u{00F7}",
        FbdBlockKind::Gt => ">",
        FbdBlockKind::Ge => "\u{2265}",
        FbdBlockKind::Lt => "<",
        FbdBlockKind::Le => "\u{2264}",
        FbdBlockKind::Eq => "==",
        FbdBlockKind::Ne => "\u{2260}",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ton),
            ..
        } => "TON",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Tof),
            ..
        } => "TOF",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Tp),
            ..
        } => "TP",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctu),
            ..
        } => "CTU",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctd),
            ..
        } => "CTD",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctud),
            ..
        } => "CTUD",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Blink),
            ..
        } => "BLINK",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::RTrig),
            ..
        } => "R_TRIG",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::FTrig),
            ..
        } => "F_TRIG",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Sr),
            ..
        } => "SR",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Rs),
            ..
        } => "RS",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Hysteresis),
            ..
        } => "HYST",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::RampReal),
            ..
        } => "RAMP",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::ScaleLin),
            ..
        } => "SCALE",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidParallel),
            ..
        } => "PID_PAR",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidSeries),
            ..
        } => "PID_SER",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidMixed),
            ..
        } => "PID_MIX",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Pi),
            ..
        } => "PI",
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::P),
            ..
        } => "P",
        FbdBlockKind::StandardFb { instance, .. } => instance.as_str(),
        FbdBlockKind::Composite { type_name } => type_name.as_str(),
    }
}

const FBD_GRID_SIZE: f32 = 20.0;
const FBD_PIN_HIT_RADIUS: f32 = 12.0;
const FBD_MIN_ZOOM: f32 = 0.25;
const FBD_MAX_ZOOM: f32 = 3.0;

const MENU_COLS: usize = 3;
const MENU_ITEM_W: f32 = 100.0;
const MENU_ITEM_H: f32 = 20.0;
const MENU_PAD: f32 = 4.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FbdEditTarget {
    BlockId { block_id: String },
    InputValue { block_id: String, pin: String },
    OutputTarget { block_id: String, pin: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinHover {
    Input { block_id: String, pin: String },
    Output { block_id: String, pin: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FbdKindMenu {
    pub block_id: String,
    pub anchor: Point,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FbdDragState {
    None,
    MovingBlock {
        block_id: String,
        grab_offset: Vector,
        origin_pos: Point,
        preview_pos: Point,
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
    pub drag: FbdDragState,
    pub hover_pin: Option<PinHover>,
    pub menu: Option<FbdKindMenu>,
    pub editing_target: Option<FbdEditTarget>,
    pub edit_buffer: Vec<char>,
    pub cursor_pos: usize,
    pub selected_block: Option<String>,
    pub pan: Vector,
    pub zoom: f32,
    pub is_panning: bool,
    pub pan_start: Option<Point>,
    pub per_pou_viewport: HashMap<String, (Vector, f32)>,
    pub active_pou: String,
    last_reported_viewport: Option<(String, Vector, f32, Size)>,
}

impl Default for FbdCanvasState {
    fn default() -> Self {
        Self {
            drag: FbdDragState::None,
            hover_pin: None,
            menu: None,
            editing_target: None,
            edit_buffer: Vec::new(),
            cursor_pos: 0,
            selected_block: None,
            pan: Vector::default(),
            zoom: 1.0,
            is_panning: false,
            pan_start: None,
            per_pou_viewport: HashMap::new(),
            active_pou: String::new(),
            last_reported_viewport: None,
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
        self.hover_pin = None;
        self.menu = None;
        if !was_uninitialized {
            self.cancel_edit();
            self.selected_block = None;
        }
    }

    fn store_current_viewport(&mut self, pou_name: &str) {
        if !pou_name.is_empty() {
            self.active_pou = pou_name.to_string();
            self.per_pou_viewport
                .insert(pou_name.to_string(), (self.pan, self.effective_zoom()));
        }
    }

    pub fn start_edit(&mut self, target: FbdEditTarget, initial_text: &str) {
        self.menu = None;
        self.editing_target = Some(target);
        self.edit_buffer = initial_text.chars().collect();
        self.cursor_pos = self.edit_buffer.len();
    }

    pub fn cancel_edit(&mut self) {
        self.menu = None;
        self.editing_target = None;
        self.edit_buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn edit_text(&self) -> String {
        self.edit_buffer.iter().collect()
    }

    pub fn edit_display_with_caret(&self) -> String {
        let pos = self.cursor_pos.min(self.edit_buffer.len());
        let mut out = String::with_capacity(self.edit_buffer.len() + 1);
        for ch in &self.edit_buffer[..pos] {
            out.push(*ch);
        }
        out.push('|');
        for ch in &self.edit_buffer[pos..] {
            out.push(*ch);
        }
        out
    }

    pub fn insert_char(&mut self, c: char) {
        if !c.is_control() {
            let pos = self.cursor_pos.min(self.edit_buffer.len());
            self.edit_buffer.insert(pos, c);
            self.cursor_pos = pos + 1;
        }
    }

    pub fn backspace(&mut self) {
        let pos = self.cursor_pos.min(self.edit_buffer.len());
        if pos > 0 {
            self.edit_buffer.remove(pos - 1);
            self.cursor_pos = pos - 1;
        }
    }

    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.edit_buffer.len() {
            self.edit_buffer.remove(self.cursor_pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.cursor_pos.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.edit_buffer.len() {
            self.cursor_pos += 1;
        }
    }

    pub fn resolved_layout(&self, program: &FbdProgram) -> HashMap<String, Point> {
        let mut layout = compute_default_block_positions(program);
        for blk in &program.blocks {
            if let Some([gx, gy]) = blk.grid_pos {
                layout.insert(blk.id.clone(), Point::new(gx as f32, gy as f32));
            }
        }
        if let FbdDragState::MovingBlock {
            ref block_id,
            preview_pos,
            ..
        } = self.drag
        {
            layout.insert(block_id.clone(), preview_pos);
        }
        layout
    }
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

    let mut col_widths: HashMap<usize, f32> = HashMap::new();
    for (idx, blk) in program.blocks.iter().enumerate() {
        let col = depths[idx];
        let w = fbd_geometry::block_size(blk, &program.composites).width;
        col_widths
            .entry(col)
            .and_modify(|cur| *cur = cur.max(w))
            .or_insert(w);
    }

    let max_col = depths.iter().copied().max().unwrap_or(0);
    let mut col_x: HashMap<usize, f32> = HashMap::with_capacity(max_col + 1);
    let mut cur_x = 28.0_f32;
    for col in 0..=max_col {
        col_x.insert(col, cur_x);
        cur_x += col_widths.get(&col).copied().unwrap_or(MIN_BLOCK_W) + 68.0;
    }

    let mut col_y: HashMap<usize, f32> = HashMap::new();
    let mut positions = HashMap::with_capacity(count);
    for (idx, blk) in program.blocks.iter().enumerate() {
        let col = depths[idx];
        let x = col_x.get(&col).copied().unwrap_or(28.0);
        let y = *col_y.get(&col).unwrap_or(&24.0);
        positions.insert(blk.id.clone(), Point::new(x, y));
        let size = fbd_geometry::block_size(blk, &program.composites);
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

fn editing_text_for_block(state: &FbdCanvasState, block_id: &str) -> Option<String> {
    match state.editing_target.as_ref()? {
        FbdEditTarget::BlockId { block_id: id }
        | FbdEditTarget::InputValue { block_id: id, .. }
        | FbdEditTarget::OutputTarget { block_id: id, .. }
            if id == block_id =>
        {
            Some(state.edit_display_with_caret())
        }
        _ => None,
    }
}

fn default_expr_for_new_pin(kind: &FbdBlockKind) -> &'static str {
    match kind {
        FbdBlockKind::Add | FbdBlockKind::Mul => "0",
        _ => "TRUE",
    }
}

#[derive(Debug, Clone)]
enum FbdHit {
    KindMenuItem(FbdBlockKind),
    KindMenuBg,
    InputDelete {
        block_id: String,
        pin: String,
    },
    OutputDelete {
        block_id: String,
        pin: String,
    },
    AddPin {
        block_id: String,
    },
    KindBadge {
        block_id: String,
        badge: Rectangle,
    },
    Id {
        block_id: String,
    },
    InputField {
        block_id: String,
        pin: String,
    },
    OutputField {
        block_id: String,
        pin: String,
    },
    OutputPin {
        block_id: String,
        pin: String,
        pos: Point,
    },
    InputPin,
    Body {
        block_id: String,
        origin: Point,
    },
    Canvas,
}

pub struct FbdSignalFlowCanvas<'a> {
    pub pou_name: &'a str,
    pub program: &'a FbdProgram,
    pub runtime: Option<&'a PlcRuntime>,
    pub selected_block: Option<&'a str>,
}

impl<'a> FbdSignalFlowCanvas<'a> {
    pub fn kind_menu_items(&self) -> Vec<(FbdBlockKind, String)> {
        let mut items: Vec<(FbdBlockKind, String)> = vec![
            (FbdBlockKind::And, "AND".to_string()),
            (FbdBlockKind::Or, "OR".to_string()),
            (FbdBlockKind::Xor, "XOR".to_string()),
            (FbdBlockKind::Not, "NOT".to_string()),
            (FbdBlockKind::Add, "ADD".to_string()),
            (FbdBlockKind::Sub, "SUB".to_string()),
            (FbdBlockKind::Mul, "MUL".to_string()),
            (FbdBlockKind::Div, "DIV".to_string()),
            (FbdBlockKind::Eq, "EQ (=)".to_string()),
            (FbdBlockKind::Ne, "NE (\u{2260})".to_string()),
            (FbdBlockKind::Lt, "LT (<)".to_string()),
            (FbdBlockKind::Le, "LE (\u{2264})".to_string()),
            (FbdBlockKind::Gt, "GT (>)".to_string()),
            (FbdBlockKind::Ge, "GE (\u{2265})".to_string()),
            (
                FbdBlockKind::StandardFb {
                    instance: "TON_1".to_string(),
                    fb_type: Some(StandardFbKind::Ton),
                },
                "TON".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "TOF_1".to_string(),
                    fb_type: Some(StandardFbKind::Tof),
                },
                "TOF".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "TP_1".to_string(),
                    fb_type: Some(StandardFbKind::Tp),
                },
                "TP".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "CTU_1".to_string(),
                    fb_type: Some(StandardFbKind::Ctu),
                },
                "CTU".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "CTD_1".to_string(),
                    fb_type: Some(StandardFbKind::Ctd),
                },
                "CTD".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "CTUD_1".to_string(),
                    fb_type: Some(StandardFbKind::Ctud),
                },
                "CTUD".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "BLINK_1".to_string(),
                    fb_type: Some(StandardFbKind::Blink),
                },
                "BLINK".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "R_TRIG_1".to_string(),
                    fb_type: Some(StandardFbKind::RTrig),
                },
                "R_TRIG".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "F_TRIG_1".to_string(),
                    fb_type: Some(StandardFbKind::FTrig),
                },
                "F_TRIG".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "SR_1".to_string(),
                    fb_type: Some(StandardFbKind::Sr),
                },
                "SR".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "RS_1".to_string(),
                    fb_type: Some(StandardFbKind::Rs),
                },
                "RS".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "HYST_1".to_string(),
                    fb_type: Some(StandardFbKind::Hysteresis),
                },
                "HYST".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "RAMP_1".to_string(),
                    fb_type: Some(StandardFbKind::RampReal),
                },
                "RAMP".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "SCALE_1".to_string(),
                    fb_type: Some(StandardFbKind::ScaleLin),
                },
                "SCALE".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "P_1".to_string(),
                    fb_type: Some(StandardFbKind::P),
                },
                "P".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "PI_1".to_string(),
                    fb_type: Some(StandardFbKind::Pi),
                },
                "PI".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "PID_Par_1".to_string(),
                    fb_type: Some(StandardFbKind::PidParallel),
                },
                "PID_PAR".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "PID_Ser_1".to_string(),
                    fb_type: Some(StandardFbKind::PidSeries),
                },
                "PID_SER".to_string(),
            ),
            (
                FbdBlockKind::StandardFb {
                    instance: "PID_Mix_1".to_string(),
                    fb_type: Some(StandardFbKind::PidMixed),
                },
                "PID_MIX".to_string(),
            ),
        ];
        for comp in &self.program.composites {
            items.push((
                FbdBlockKind::Composite {
                    type_name: comp.name.clone(),
                },
                comp.name.clone(),
            ));
        }
        items
    }

    pub fn kind_menu_size(&self) -> Size {
        let count = self.kind_menu_items().len();
        let rows = count.div_ceil(MENU_COLS);
        Size::new(
            MENU_COLS as f32 * MENU_ITEM_W + MENU_PAD * 2.0,
            rows as f32 * MENU_ITEM_H + MENU_PAD * 2.0,
        )
    }

    pub fn kind_menu_item_rect(&self, anchor: Point, idx: usize) -> Rectangle {
        let col = idx % MENU_COLS;
        let row = idx / MENU_COLS;
        Rectangle::new(
            Point::new(
                anchor.x + MENU_PAD + col as f32 * MENU_ITEM_W,
                anchor.y + MENU_PAD + row as f32 * MENU_ITEM_H,
            ),
            Size::new(MENU_ITEM_W - 2.0, MENU_ITEM_H - 2.0),
        )
    }

    pub fn commit_current_edit(&self, state: &mut FbdCanvasState) -> Option<Message> {
        let target = state.editing_target.take()?;
        let buffer: String = std::mem::take(&mut state.edit_buffer).into_iter().collect();
        state.cursor_pos = 0;
        let pou = self.pou_name.to_string();

        match target {
            FbdEditTarget::BlockId { block_id } => {
                let trimmed = buffer.trim().to_string();
                if trimmed.is_empty() || trimmed == block_id {
                    None
                } else {
                    Some(Message::FbdBlockIdChanged(pou, block_id, trimmed))
                }
            }
            FbdEditTarget::InputValue { block_id, pin } => {
                let trimmed = buffer.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(Message::FbdInputSourceChanged(pou, block_id, pin, trimmed))
                }
            }
            FbdEditTarget::OutputTarget { block_id, pin } => Some(Message::FbdOutputTargetChanged(
                pou,
                block_id,
                pin,
                buffer.trim().to_string(),
            )),
        }
    }

    fn hit_test(&self, state: &FbdCanvasState, cursor: Point) -> FbdHit {
        if let Some(ref menu) = state.menu {
            let menu_rect = Rectangle::new(menu.anchor, self.kind_menu_size());
            if menu_rect.contains(cursor) {
                for (idx, (kind, _label)) in self.kind_menu_items().into_iter().enumerate() {
                    if self.kind_menu_item_rect(menu.anchor, idx).contains(cursor) {
                        return FbdHit::KindMenuItem(kind);
                    }
                }
                return FbdHit::KindMenuBg;
            }
        }

        let layout = state.resolved_layout(self.program);
        let hit_r_sq = FBD_PIN_HIT_RADIUS * FBD_PIN_HIT_RADIUS;

        for blk in self.program.blocks.iter().rev() {
            let Some(&origin) = layout.get(&blk.id) else {
                continue;
            };
            let edit_txt = editing_text_for_block(state, &blk.id);
            let size = block_size_with_edit(blk, &self.program.composites, edit_txt.as_deref());
            let width = size.width;
            let in_pins = input_pins(blk);
            let out_pins = output_pins(blk, &self.program.composites);

            if is_variadic(&blk.kind) {
                for (i, pin) in in_pins.iter().enumerate() {
                    if input_delete_rect(origin, width, i).contains(cursor) {
                        return FbdHit::InputDelete {
                            block_id: blk.id.clone(),
                            pin: pin.clone(),
                        };
                    }
                }
            }

            for (i, pin) in out_pins.iter().enumerate() {
                if blk.outputs.contains_key(pin)
                    && output_delete_rect(origin, width, in_pins.len(), i).contains(cursor)
                {
                    return FbdHit::OutputDelete {
                        block_id: blk.id.clone(),
                        pin: pin.clone(),
                    };
                }
            }

            if let Some(add_r) =
                fbd_geometry::add_pin_rect_with_width(blk, &self.program.composites, origin, width)
                && add_r.contains(cursor)
            {
                return FbdHit::AddPin {
                    block_id: blk.id.clone(),
                };
            }

            let badge = kind_badge_rect(blk, &self.program.composites, origin);
            if badge.contains(cursor) {
                return FbdHit::KindBadge {
                    block_id: blk.id.clone(),
                    badge,
                };
            }

            let id_r = id_rect_with_width(blk, &self.program.composites, origin, width);
            if id_r.contains(cursor) {
                return FbdHit::Id {
                    block_id: blk.id.clone(),
                };
            }

            for (i, pin) in in_pins.iter().enumerate() {
                if input_value_rect(origin, width, i).contains(cursor) {
                    return FbdHit::InputField {
                        block_id: blk.id.clone(),
                        pin: pin.clone(),
                    };
                }
            }

            for (i, pin) in out_pins.iter().enumerate() {
                if output_value_rect(origin, width, in_pins.len(), i).contains(cursor) {
                    return FbdHit::OutputField {
                        block_id: blk.id.clone(),
                        pin: pin.clone(),
                    };
                }
            }

            for (i, pin) in out_pins.iter().enumerate() {
                let pt = output_pin_point(origin, width, in_pins.len(), i);
                if point_distance_sq(cursor, pt) <= hit_r_sq {
                    return FbdHit::OutputPin {
                        block_id: blk.id.clone(),
                        pin: pin.clone(),
                        pos: pt,
                    };
                }
            }

            for (i, _pin) in in_pins.iter().enumerate() {
                let pt = input_pin_point(origin, i);
                if point_distance_sq(cursor, pt) <= hit_r_sq {
                    return FbdHit::InputPin;
                }
            }

            if Rectangle::new(origin, size).contains(cursor) {
                return FbdHit::Body {
                    block_id: blk.id.clone(),
                    origin,
                };
            }
        }

        FbdHit::Canvas
    }

    fn begin_inline_edit(
        &self,
        state: &mut FbdCanvasState,
        block_id: &str,
        target: FbdEditTarget,
        initial_text: &str,
    ) -> (event::Status, Option<Message>) {
        if state.editing_target.as_ref() == Some(&target) {
            return (
                event::Status::Captured,
                Some(Message::FbdInlineEditingChanged(true)),
            );
        }
        if state.editing_target.is_some()
            && let Some(commit_msg) = self.commit_current_edit(state)
        {
            return (event::Status::Captured, Some(commit_msg));
        }
        state.selected_block = Some(block_id.to_string());
        state.start_edit(target, initial_text);
        (
            event::Status::Captured,
            Some(Message::FbdInlineEditingChanged(true)),
        )
    }

    fn compute_kind_menu_anchor(
        &self,
        state: &FbdCanvasState,
        bounds: Rectangle,
        badge: Rectangle,
    ) -> Point {
        let zoom = state.effective_zoom();
        let menu_size = self.kind_menu_size();
        let vis_min_x = -state.pan.x / zoom;
        let vis_min_y = -state.pan.y / zoom;
        let vis_max_x = (bounds.width - state.pan.x) / zoom;
        let vis_max_y = (bounds.height - state.pan.y) / zoom;

        let anchor_y = if badge.y + badge.height + 2.0 + menu_size.height > vis_max_y
            && badge.y - vis_min_y >= menu_size.height + 4.0
        {
            badge.y - menu_size.height - 2.0
        } else {
            badge.y + badge.height + 2.0
        };
        let anchor_x = if badge.x + menu_size.width > vis_max_x {
            (vis_max_x - menu_size.width - 4.0).max(vis_min_x)
        } else {
            badge.x
        };
        Point::new(anchor_x, anchor_y)
    }

    fn find_block(&self, block_id: &str) -> Option<&FbdBlock> {
        self.program.blocks.iter().find(|b| b.id == block_id)
    }
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

        if let canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, text, .. }) = event
        {
            if state.editing_target.is_some() {
                use iced::keyboard::Key;
                use iced::keyboard::key::Named;
                return match key {
                    Key::Named(Named::Enter) => {
                        let msg = self
                            .commit_current_edit(state)
                            .or(Some(Message::FbdInlineEditingChanged(false)));
                        (event::Status::Captured, msg)
                    }
                    Key::Named(Named::Escape) => {
                        state.cancel_edit();
                        (
                            event::Status::Captured,
                            Some(Message::FbdInlineEditingChanged(false)),
                        )
                    }
                    Key::Named(Named::Backspace) => {
                        state.backspace();
                        (event::Status::Captured, None)
                    }
                    Key::Named(Named::Delete) => {
                        state.delete_forward();
                        (
                            event::Status::Captured,
                            Some(Message::FbdInlineEditingChanged(true)),
                        )
                    }
                    Key::Named(Named::ArrowLeft) => {
                        state.move_cursor_left();
                        (event::Status::Captured, None)
                    }
                    Key::Named(Named::ArrowRight) => {
                        state.move_cursor_right();
                        (event::Status::Captured, None)
                    }
                    Key::Named(Named::Home) => {
                        state.cursor_pos = 0;
                        (event::Status::Captured, None)
                    }
                    Key::Named(Named::End) => {
                        state.cursor_pos = state.edit_buffer.len();
                        (event::Status::Captured, None)
                    }
                    Key::Named(Named::Space) => {
                        state.insert_char(' ');
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
                        } else if let Key::Character(s) = key {
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
            }

            if state.menu.is_some()
                && matches!(
                    key,
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                )
            {
                state.menu = None;
                return (event::Status::Captured, None);
            }

            // Delete when not editing is owned by the global DeletePressed shortcut in app.rs.
            return (event::Status::Ignored, None);
        }

        let raw_cursor_pos = match cursor.position_in(bounds) {
            Some(p) => p,
            None => {
                if let canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) =
                    event
                {
                    state.is_panning = false;
                    state.pan_start = None;
                    state.store_current_viewport(self.pou_name);
                    return (
                        event::Status::Captured,
                        Some(Message::FbdViewportChanged(
                            self.pou_name.to_string(),
                            state.pan,
                            state.effective_zoom(),
                            bounds.size(),
                        )),
                    );
                }
                if let canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) =
                    event
                {
                    let prev_drag = std::mem::replace(&mut state.drag, FbdDragState::None);
                    state.hover_pin = None;
                    if let FbdDragState::MovingBlock {
                        block_id,
                        origin_pos,
                        preview_pos,
                        ..
                    } = prev_drag
                    {
                        let moved = (preview_pos.x - origin_pos.x).abs() > f32::EPSILON
                            || (preview_pos.y - origin_pos.y).abs() > f32::EPSILON;
                        if moved {
                            return (
                                event::Status::Captured,
                                Some(Message::FbdBlockMoved(
                                    self.pou_name.to_string(),
                                    block_id,
                                    [preview_pos.x.round() as i32, preview_pos.y.round() as i32],
                                )),
                            );
                        }
                        return (event::Status::Captured, None);
                    }
                    if !matches!(prev_drag, FbdDragState::None) {
                        return (event::Status::Captured, None);
                    }
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
                    let vp = (state.pan, state.effective_zoom(), bounds.size());
                    state.last_reported_viewport =
                        Some((self.pou_name.to_string(), vp.0, vp.1, vp.2));
                    (
                        event::Status::Captured,
                        Some(Message::FbdViewportChanged(
                            self.pou_name.to_string(),
                            vp.0,
                            vp.1,
                            vp.2,
                        )),
                    )
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
                    let vp = (state.pan, state.effective_zoom(), bounds.size());
                    state.last_reported_viewport =
                        Some((self.pou_name.to_string(), vp.0, vp.1, vp.2));
                    return (
                        event::Status::Captured,
                        Some(Message::FbdViewportChanged(
                            self.pou_name.to_string(),
                            vp.0,
                            vp.1,
                            vp.2,
                        )),
                    );
                }
                (event::Status::Ignored, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let hit = self.hit_test(state, cursor_pos);
                match hit {
                    FbdHit::KindMenuItem(kind) => {
                        let block_id = state.menu.take().map(|m| m.block_id).unwrap_or_default();
                        if block_id.is_empty() {
                            (event::Status::Captured, None)
                        } else {
                            (
                                event::Status::Captured,
                                Some(Message::FbdChangeKind(
                                    self.pou_name.to_string(),
                                    block_id,
                                    kind,
                                )),
                            )
                        }
                    }
                    FbdHit::KindMenuBg => {
                        state.menu = None;
                        (event::Status::Captured, None)
                    }
                    FbdHit::InputDelete { block_id, pin } => {
                        state.menu = None;
                        let commit_msg = self.commit_current_edit(state);
                        if commit_msg.is_some() {
                            return (event::Status::Captured, commit_msg);
                        }
                        let count = self.find_block(&block_id).map_or(0, |b| b.inputs.len());
                        if count > 2 {
                            (
                                event::Status::Captured,
                                Some(Message::FbdRemoveInputPin(
                                    self.pou_name.to_string(),
                                    block_id,
                                    pin,
                                )),
                            )
                        } else {
                            (event::Status::Captured, None)
                        }
                    }
                    FbdHit::OutputDelete { block_id, pin } => {
                        state.menu = None;
                        let commit_msg = self.commit_current_edit(state);
                        if commit_msg.is_some() {
                            return (event::Status::Captured, commit_msg);
                        }
                        (
                            event::Status::Captured,
                            Some(Message::FbdDisconnectOutput(
                                self.pou_name.to_string(),
                                block_id,
                                pin,
                            )),
                        )
                    }
                    FbdHit::AddPin { block_id } => {
                        state.menu = None;
                        let commit_msg = self.commit_current_edit(state);
                        if commit_msg.is_some() {
                            return (event::Status::Captured, commit_msg);
                        }
                        if let Some(blk) = self.find_block(&block_id)
                            && is_variadic(&blk.kind)
                        {
                            let mut n = blk.inputs.len() + 1;
                            while blk.inputs.contains_key(&format!("IN{n}")) {
                                n += 1;
                            }
                            let new_pin = format!("IN{n}");
                            let default_expr = default_expr_for_new_pin(&blk.kind);
                            state.selected_block = Some(block_id.clone());
                            state.start_edit(
                                FbdEditTarget::InputValue {
                                    block_id: block_id.clone(),
                                    pin: new_pin,
                                },
                                default_expr,
                            );
                            return (
                                event::Status::Captured,
                                Some(Message::FbdAddInputPin(self.pou_name.to_string(), block_id)),
                            );
                        }
                        (event::Status::Captured, None)
                    }
                    FbdHit::KindBadge { block_id, badge } => {
                        let commit_msg = self.commit_current_edit(state);
                        if commit_msg.is_some() {
                            return (event::Status::Captured, commit_msg);
                        }
                        let anchor = self.compute_kind_menu_anchor(state, bounds, badge);
                        state.selected_block = Some(block_id.clone());
                        state.menu = Some(FbdKindMenu { block_id, anchor });
                        (event::Status::Captured, None)
                    }
                    FbdHit::Id { block_id } => {
                        let init = self
                            .find_block(&block_id)
                            .map(|b| b.id.as_str())
                            .unwrap_or("");
                        self.begin_inline_edit(
                            state,
                            &block_id,
                            FbdEditTarget::BlockId {
                                block_id: block_id.clone(),
                            },
                            init,
                        )
                    }
                    FbdHit::InputField { block_id, pin } => {
                        let init = self
                            .find_block(&block_id)
                            .map(|b| input_row_text(b.inputs.get(&pin)))
                            .unwrap_or_default();
                        self.begin_inline_edit(
                            state,
                            &block_id,
                            FbdEditTarget::InputValue {
                                block_id: block_id.clone(),
                                pin,
                            },
                            &init,
                        )
                    }
                    FbdHit::OutputField { block_id, pin } => {
                        let init = self
                            .find_block(&block_id)
                            .and_then(|b| b.outputs.get(&pin))
                            .cloned()
                            .unwrap_or_default();
                        self.begin_inline_edit(
                            state,
                            &block_id,
                            FbdEditTarget::OutputTarget {
                                block_id: block_id.clone(),
                                pin,
                            },
                            &init,
                        )
                    }
                    FbdHit::OutputPin { block_id, pin, pos } => {
                        state.menu = None;
                        let commit_msg = self.commit_current_edit(state);
                        if commit_msg.is_some() {
                            return (event::Status::Captured, commit_msg);
                        }
                        state.drag = FbdDragState::Wiring {
                            from_block: block_id,
                            from_pin: pin,
                            start_pt: pos,
                            current_pt: cursor_pos,
                        };
                        (event::Status::Captured, None)
                    }
                    FbdHit::InputPin => {
                        state.menu = None;
                        let commit_msg = self.commit_current_edit(state);
                        (event::Status::Captured, commit_msg)
                    }
                    FbdHit::Body { block_id, origin } => {
                        state.menu = None;
                        let was_editing = state.editing_target.is_some();
                        let commit_msg = self.commit_current_edit(state);
                        state.selected_block = Some(block_id.clone());
                        state.drag = FbdDragState::MovingBlock {
                            block_id: block_id.clone(),
                            grab_offset: Vector::new(
                                cursor_pos.x - origin.x,
                                cursor_pos.y - origin.y,
                            ),
                            origin_pos: origin,
                            preview_pos: origin,
                        };
                        let msg = commit_msg
                            .or(if was_editing {
                                Some(Message::FbdInlineEditingChanged(false))
                            } else {
                                None
                            })
                            .or_else(|| {
                                Some(Message::FbdBlockSelected(
                                    self.pou_name.to_string(),
                                    Some(block_id),
                                ))
                            });
                        (event::Status::Captured, msg)
                    }
                    FbdHit::Canvas => {
                        state.menu = None;
                        let was_editing = state.editing_target.is_some();
                        let commit_msg = self.commit_current_edit(state);
                        let had_sel = state.selected_block.take().is_some();
                        if let Some(msg) = commit_msg {
                            (event::Status::Captured, Some(msg))
                        } else if was_editing {
                            (
                                event::Status::Captured,
                                Some(Message::FbdInlineEditingChanged(false)),
                            )
                        } else if had_sel {
                            (
                                event::Status::Captured,
                                Some(Message::FbdBlockSelected(self.pou_name.to_string(), None)),
                            )
                        } else {
                            (event::Status::Ignored, None)
                        }
                    }
                }
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

                let layout = state.resolved_layout(self.program);
                match &mut state.drag {
                    FbdDragState::MovingBlock {
                        grab_offset,
                        preview_pos,
                        ..
                    } => {
                        let snapped_x = ((cursor_pos.x - grab_offset.x) / 10.0).round() * 10.0;
                        let snapped_y = ((cursor_pos.y - grab_offset.y) / 10.0).round() * 10.0;
                        *preview_pos = Point::new(snapped_x, snapped_y);
                        (event::Status::Captured, None)
                    }
                    FbdDragState::Wiring {
                        from_block,
                        current_pt,
                        ..
                    } => {
                        *current_pt = cursor_pos;
                        let src_block = from_block.clone();
                        state.hover_pin = fbd_geometry::resolve_input_drop(
                            &self.program.blocks,
                            &layout,
                            &self.program.composites,
                            Some(src_block.as_str()),
                            cursor_pos,
                        )
                        .map(|(blk, pin)| PinHover::Input {
                            block_id: blk.id.clone(),
                            pin,
                        });
                        (event::Status::Captured, None)
                    }
                    FbdDragState::None => {
                        let hit_r_sq = FBD_PIN_HIT_RADIUS * FBD_PIN_HIT_RADIUS;
                        let mut next_hover = None;
                        for blk in self.program.blocks.iter().rev() {
                            let Some(&origin) = layout.get(&blk.id) else {
                                continue;
                            };
                            let edit_txt = editing_text_for_block(state, &blk.id);
                            let width = fbd_geometry::block_width(
                                blk,
                                &self.program.composites,
                                edit_txt.as_deref(),
                            );
                            let in_pins = input_pins(blk);
                            let out_pins = output_pins(blk, &self.program.composites);
                            for (i, pin) in out_pins.iter().enumerate() {
                                let pt = output_pin_point(origin, width, in_pins.len(), i);
                                if point_distance_sq(cursor_pos, pt) <= hit_r_sq {
                                    next_hover = Some(PinHover::Output {
                                        block_id: blk.id.clone(),
                                        pin: pin.clone(),
                                    });
                                    break;
                                }
                            }
                            if next_hover.is_some() {
                                break;
                            }
                            for (i, pin) in in_pins.iter().enumerate() {
                                let pt = input_pin_point(origin, i);
                                if point_distance_sq(cursor_pos, pt) <= hit_r_sq {
                                    next_hover = Some(PinHover::Input {
                                        block_id: blk.id.clone(),
                                        pin: pin.clone(),
                                    });
                                    break;
                                }
                            }
                            if next_hover.is_some() {
                                break;
                            }
                        }
                        state.hover_pin = next_hover;
                        let vp = (state.pan, state.effective_zoom(), bounds.size());
                        if state.last_reported_viewport.as_ref()
                            != Some(&(self.pou_name.to_string(), vp.0, vp.1, vp.2))
                        {
                            state.last_reported_viewport =
                                Some((self.pou_name.to_string(), vp.0, vp.1, vp.2));
                            return (
                                event::Status::Ignored,
                                Some(Message::FbdViewportChanged(
                                    self.pou_name.to_string(),
                                    vp.0,
                                    vp.1,
                                    vp.2,
                                )),
                            );
                        }
                        (event::Status::Ignored, None)
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let prev_drag = std::mem::replace(&mut state.drag, FbdDragState::None);
                state.hover_pin = None;
                match prev_drag {
                    FbdDragState::MovingBlock {
                        block_id,
                        origin_pos,
                        preview_pos,
                        ..
                    } => {
                        let moved = (preview_pos.x - origin_pos.x).abs() > f32::EPSILON
                            || (preview_pos.y - origin_pos.y).abs() > f32::EPSILON;
                        if moved {
                            (
                                event::Status::Captured,
                                Some(Message::FbdBlockMoved(
                                    self.pou_name.to_string(),
                                    block_id,
                                    [preview_pos.x.round() as i32, preview_pos.y.round() as i32],
                                )),
                            )
                        } else {
                            (event::Status::Captured, None)
                        }
                    }
                    FbdDragState::Wiring {
                        from_block,
                        from_pin,
                        ..
                    } => {
                        let layout = state.resolved_layout(self.program);
                        if let Some((target_blk, target_pin)) = fbd_geometry::resolve_input_drop(
                            &self.program.blocks,
                            &layout,
                            &self.program.composites,
                            Some(&from_block),
                            cursor_pos,
                        ) {
                            (
                                event::Status::Captured,
                                Some(Message::FbdInputSourceChanged(
                                    self.pou_name.to_string(),
                                    target_blk.id.clone(),
                                    target_pin,
                                    format!("{from_block}.{from_pin}"),
                                )),
                            )
                        } else {
                            (event::Status::Captured, None)
                        }
                    }
                    FbdDragState::None => (event::Status::Ignored, None),
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                state.menu = None;
                let layout = state.resolved_layout(self.program);
                for blk in self.program.blocks.iter().rev() {
                    let Some(&origin) = layout.get(&blk.id) else {
                        continue;
                    };
                    let edit_txt = editing_text_for_block(state, &blk.id);
                    let width = fbd_geometry::block_width(
                        blk,
                        &self.program.composites,
                        edit_txt.as_deref(),
                    );
                    if header_rect_with_width(origin, width).contains(cursor_pos) {
                        return (
                            event::Status::Captured,
                            Some(Message::FbdRemoveBlock(
                                self.pou_name.to_string(),
                                blk.id.clone(),
                            )),
                        );
                    }
                    let in_pins = input_pins(blk);
                    for (i, pin) in in_pins.iter().enumerate() {
                        if input_row_rect(origin, width, i).contains(cursor_pos) {
                            return (
                                event::Status::Captured,
                                Some(Message::FbdDisconnectInput(
                                    self.pou_name.to_string(),
                                    blk.id.clone(),
                                    pin.clone(),
                                )),
                            );
                        }
                    }
                    let out_pins = output_pins(blk, &self.program.composites);
                    for (i, pin) in out_pins.iter().enumerate() {
                        if output_row_rect(origin, width, in_pins.len(), i).contains(cursor_pos) {
                            return (
                                event::Status::Captured,
                                Some(Message::FbdDisconnectOutput(
                                    self.pou_name.to_string(),
                                    blk.id.clone(),
                                    pin.clone(),
                                )),
                            );
                        }
                    }
                }
                (event::Status::Ignored, None)
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

        let menu_rect_opt = state
            .menu
            .as_ref()
            .map(|m| Rectangle::new(m.anchor, self.kind_menu_size()));
        let is_occluded = |r: Rectangle| -> bool {
            menu_rect_opt.is_some_and(|mr| {
                let expanded = Rectangle::new(
                    Point::new(mr.x - 2.0, mr.y - 2.0),
                    Size::new(mr.width + 4.0, mr.height + 4.0),
                );
                expanded.intersection(&r).is_some()
            })
        };

        // Pass 1: Signal-flow Bezier wires and live values
        for blk in &self.program.blocks {
            let Some(&dst_pos) = layout.get(&blk.id) else {
                continue;
            };
            let in_pins = input_pins(blk);
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
                    let src_edit = editing_text_for_block(state, &src_blk.id);
                    let src_width = fbd_geometry::block_width(
                        src_blk,
                        &self.program.composites,
                        src_edit.as_deref(),
                    );
                    let src_in_len = input_pins(src_blk).len();
                    let out_pins = output_pins(src_blk, &self.program.composites);
                    let out_idx = out_pins
                        .iter()
                        .position(|p| p.eq_ignore_ascii_case(from_pin))
                        .unwrap_or(0);
                    let p1 = output_pin_point(src_pos, src_width, src_in_len, out_idx);
                    let p2 = input_pin_point(dst_pos, in_idx);

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
                        let label_rect = Rectangle::new(
                            Point::new(mid.x - 18.0, mid.y - 14.0),
                            Size::new(48.0, 14.0),
                        );
                        if !is_occluded(label_rect) {
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

        // Pass 2: On-block direct-editable Function Blocks
        let block_bg = if is_dark {
            Color::from_rgb(0.085, 0.100, 0.135)
        } else {
            Color::from_rgb(0.96, 0.97, 0.99)
        };
        let field_bg = if is_dark {
            Color::from_rgba(0.0, 0.0, 0.0, 0.22)
        } else {
            Color::from_rgba(0.0, 0.0, 0.0, 0.05)
        };
        let edit_bg = if is_dark {
            StudioTheme::BG_HOVER
        } else {
            Color::WHITE
        };
        let pin_text_color = if is_dark {
            Color::from_rgb(0.86, 0.90, 0.95)
        } else {
            Color::from_rgb(0.12, 0.15, 0.22)
        };
        let muted_color = StudioTheme::text_muted(theme);
        let active_sel = self.selected_block.or(state.selected_block.as_deref());

        for blk in &self.program.blocks {
            let Some(&origin) = layout.get(&blk.id) else {
                continue;
            };
            let edit_txt = editing_text_for_block(state, &blk.id);
            let size = block_size_with_edit(blk, &self.program.composites, edit_txt.as_deref());
            let width = size.width;
            let cat = block_category(&blk.kind);
            let accent = cat.accent_color();
            let op_sym = operator_symbol(&blk.kind);
            let is_selected = active_sel == Some(blk.id.as_str());

            // Body & header
            frame.fill_rectangle(origin, size, block_bg);
            frame.fill_rectangle(
                origin,
                Size::new(width, HEADER_H),
                Color::from_rgba(accent.r, accent.g, accent.b, 0.85),
            );
            frame.stroke_rectangle(
                origin,
                size,
                Stroke::default()
                    .with_color(if is_selected {
                        StudioTheme::ACCENT_AMBER
                    } else {
                        accent
                    })
                    .with_width(if is_selected { 2.0 } else { 1.1 }),
            );

            // Kind badge
            let badge = kind_badge_rect(blk, &self.program.composites, origin);
            if !is_occluded(badge) {
                frame.fill_rectangle(
                    badge.position(),
                    badge.size(),
                    Color::from_rgba(0.0, 0.0, 0.0, 0.30),
                );
                frame.stroke_rectangle(
                    badge.position(),
                    badge.size(),
                    Stroke::default()
                        .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.45))
                        .with_width(0.8),
                );
                let badge_label = if op_sym.chars().count() <= 3 {
                    op_sym
                } else {
                    block_glyph(&blk.kind)
                };
                frame.fill_text(canvas::Text {
                    content: badge_label.to_string(),
                    position: Point::new(badge.x + 4.0, badge.y + 3.0),
                    color: Color::WHITE,
                    size: iced::Pixels(10.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            // Inline block ID field
            let id_r = id_rect_with_width(blk, &self.program.composites, origin, width);
            if !is_occluded(id_r) {
                let is_editing_id = state.editing_target.as_ref()
                    == Some(&FbdEditTarget::BlockId {
                        block_id: blk.id.clone(),
                    });
                if is_editing_id {
                    frame.fill_rectangle(id_r.position(), id_r.size(), edit_bg);
                    frame.stroke_rectangle(
                        id_r.position(),
                        id_r.size(),
                        Stroke::default()
                            .with_color(StudioTheme::ACCENT_CYAN)
                            .with_width(1.5),
                    );
                }
                let id_text = if is_editing_id {
                    state.edit_display_with_caret()
                } else {
                    format_row_display(&blk.id, id_r.width, false)
                };
                frame.fill_text(canvas::Text {
                    content: id_text,
                    position: Point::new(id_r.x + 3.0, id_r.y + 3.0),
                    color: Color::WHITE,
                    size: iced::Pixels(10.5),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            // Input rows
            let in_pins = input_pins(blk);
            let variadic = is_variadic(&blk.kind);
            for (i, in_pin) in in_pins.iter().enumerate() {
                let row_r = input_row_rect(origin, width, i);
                let val_r = input_value_rect(origin, width, i);
                let del_r = input_delete_rect(origin, width, i);
                let pin_pt = input_pin_point(origin, i);
                let src_opt = blk.inputs.get(in_pin);
                let is_linked = matches!(src_opt, Some(FbdInputSource::Link { .. }));
                let energized = self
                    .runtime
                    .is_some_and(|rt| src_opt.is_some_and(|s| is_input_energized(rt, s)));
                let is_pin_hovered = state.hover_pin.as_ref()
                    == Some(&PinHover::Input {
                        block_id: blk.id.clone(),
                        pin: in_pin.clone(),
                    });

                if is_pin_hovered {
                    frame.fill_rectangle(
                        row_r.position(),
                        row_r.size(),
                        Color::from_rgba(0.220, 0.741, 0.973, 0.14),
                    );
                    frame.stroke_rectangle(
                        row_r.position(),
                        row_r.size(),
                        Stroke::default()
                            .with_color(StudioTheme::ACCENT_CYAN)
                            .with_width(1.0),
                    );
                }

                if is_linked {
                    frame.fill_rectangle(
                        Point::new(row_r.x + 1.5, row_r.y + 2.0),
                        Size::new(3.0, row_r.height - 4.0),
                        if energized {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            StudioTheme::ACCENT_CYAN
                        },
                    );
                }

                if !is_occluded(row_r) {
                    frame.fill_text(canvas::Text {
                        content: in_pin.clone(),
                        position: Point::new(origin.x + ROW_INSET, row_r.y + 5.0),
                        color: muted_color,
                        size: iced::Pixels(9.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    let is_editing = state.editing_target.as_ref()
                        == Some(&FbdEditTarget::InputValue {
                            block_id: blk.id.clone(),
                            pin: in_pin.clone(),
                        });
                    frame.fill_rectangle(
                        val_r.position(),
                        val_r.size(),
                        if is_editing { edit_bg } else { field_bg },
                    );
                    frame.stroke_rectangle(
                        val_r.position(),
                        val_r.size(),
                        Stroke::default()
                            .with_color(if is_editing {
                                StudioTheme::ACCENT_CYAN
                            } else {
                                StudioTheme::border_hairline(theme)
                            })
                            .with_width(if is_editing { 1.5 } else { 0.8 }),
                    );

                    let raw_text = input_row_text(src_opt);
                    let (content, txt_color) = if is_editing {
                        (state.edit_display_with_caret(), pin_text_color)
                    } else if raw_text.trim().is_empty() {
                        ("(0)".to_string(), muted_color)
                    } else {
                        (
                            format_row_display(&raw_text, val_r.width, false),
                            if is_linked {
                                StudioTheme::ACCENT_CYAN
                            } else {
                                pin_text_color
                            },
                        )
                    };
                    frame.fill_text(canvas::Text {
                        content,
                        position: Point::new(val_r.x + 4.0, val_r.y + 3.0),
                        color: txt_color,
                        size: iced::Pixels(9.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    if variadic {
                        let can_remove = blk.inputs.len() > 2;
                        frame.fill_text(canvas::Text {
                            content: "\u{00D7}".to_string(),
                            position: Point::new(del_r.x + 3.0, del_r.y + 1.0),
                            color: if can_remove {
                                StudioTheme::ACCENT_ROSE
                            } else {
                                Color::from_rgba(muted_color.r, muted_color.g, muted_color.b, 0.35)
                            },
                            size: iced::Pixels(11.0),
                            font: iced::Font::MONOSPACE,
                            ..Default::default()
                        });
                    }
                }

                let pin_box = Rectangle::new(
                    Point::new(pin_pt.x - 6.0, pin_pt.y - 6.0),
                    Size::new(12.0, 12.0),
                );
                if !is_occluded(pin_box) {
                    let pin_color = if is_pin_hovered {
                        StudioTheme::ACCENT_AMBER
                    } else if energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::ACCENT_CYAN
                    };
                    frame.fill(
                        &Path::circle(pin_pt, if is_pin_hovered { 6.0 } else { 4.5 }),
                        pin_color,
                    );
                }
            }

            // Output rows
            let out_pins = output_pins(blk, &self.program.composites);
            for (i, out_pin) in out_pins.iter().enumerate() {
                let row_r = output_row_rect(origin, width, in_pins.len(), i);
                let val_r = output_value_rect(origin, width, in_pins.len(), i);
                let del_r = output_delete_rect(origin, width, in_pins.len(), i);
                let pin_pt = output_pin_point(origin, width, in_pins.len(), i);
                let target_opt = blk.outputs.get(out_pin);
                let energized = self
                    .runtime
                    .is_some_and(|rt| target_opt.is_some_and(|t| is_output_energized(rt, t)));
                let is_pin_hovered = state.hover_pin.as_ref()
                    == Some(&PinHover::Output {
                        block_id: blk.id.clone(),
                        pin: out_pin.clone(),
                    });

                if i == 0 && !in_pins.is_empty() {
                    frame.stroke(
                        &Path::line(
                            Point::new(origin.x + 4.0, row_r.y),
                            Point::new(origin.x + width - 4.0, row_r.y),
                        ),
                        Stroke::default()
                            .with_color(StudioTheme::border_hairline(theme))
                            .with_width(0.8),
                    );
                }

                if !is_occluded(row_r) {
                    frame.fill_text(canvas::Text {
                        content: out_pin.clone(),
                        position: Point::new(origin.x + ROW_INSET, row_r.y + 5.0),
                        color: muted_color,
                        size: iced::Pixels(9.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    let is_editing = state.editing_target.as_ref()
                        == Some(&FbdEditTarget::OutputTarget {
                            block_id: blk.id.clone(),
                            pin: out_pin.clone(),
                        });
                    frame.fill_rectangle(
                        val_r.position(),
                        val_r.size(),
                        if is_editing { edit_bg } else { field_bg },
                    );
                    frame.stroke_rectangle(
                        val_r.position(),
                        val_r.size(),
                        Stroke::default()
                            .with_color(if is_editing {
                                StudioTheme::ACCENT_CYAN
                            } else {
                                StudioTheme::border_hairline(theme)
                            })
                            .with_width(if is_editing { 1.5 } else { 0.8 }),
                    );

                    let (content, txt_color) = if is_editing {
                        (state.edit_display_with_caret(), pin_text_color)
                    } else if let Some(target) = target_opt
                        && !target.trim().is_empty()
                    {
                        (
                            format_row_display(target, val_r.width, false),
                            if energized {
                                StudioTheme::ACCENT_EMERALD
                            } else {
                                pin_text_color
                            },
                        )
                    } else {
                        ("(unbound)".to_string(), muted_color)
                    };
                    frame.fill_text(canvas::Text {
                        content,
                        position: Point::new(val_r.x + 4.0, val_r.y + 3.0),
                        color: txt_color,
                        size: iced::Pixels(9.5),
                        font: iced::Font::MONOSPACE,
                        ..Default::default()
                    });

                    if target_opt.is_some() {
                        frame.fill_text(canvas::Text {
                            content: "\u{00D7}".to_string(),
                            position: Point::new(del_r.x + 3.0, del_r.y + 1.0),
                            color: StudioTheme::ACCENT_ROSE,
                            size: iced::Pixels(11.0),
                            font: iced::Font::MONOSPACE,
                            ..Default::default()
                        });
                    }
                }

                let pin_box = Rectangle::new(
                    Point::new(pin_pt.x - 6.0, pin_pt.y - 6.0),
                    Size::new(12.0, 12.0),
                );
                if !is_occluded(pin_box) {
                    let pin_color = if is_pin_hovered {
                        StudioTheme::ACCENT_AMBER
                    } else if energized {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        accent
                    };
                    frame.fill(
                        &Path::circle(pin_pt, if is_pin_hovered { 6.0 } else { 4.5 }),
                        pin_color,
                    );
                }
            }

            // `+ pin` footer for variadic blocks
            if let Some(add_r) =
                fbd_geometry::add_pin_rect_with_width(blk, &self.program.composites, origin, width)
                && !is_occluded(add_r)
            {
                let pill = Rectangle::new(
                    Point::new(add_r.x, add_r.y + 2.0),
                    Size::new(add_r.width, (ADD_PIN_H - 4.0).max(12.0)),
                );
                frame.fill_rectangle(
                    pill.position(),
                    pill.size(),
                    Color::from_rgba(accent.r, accent.g, accent.b, 0.12),
                );
                frame.stroke_rectangle(
                    pill.position(),
                    pill.size(),
                    Stroke::default()
                        .with_color(Color::from_rgba(accent.r, accent.g, accent.b, 0.35))
                        .with_width(0.8),
                );
                frame.fill_text(canvas::Text {
                    content: "+ pin".to_string(),
                    position: Point::new(pill.x + (pill.width * 0.5 - 16.0).max(4.0), pill.y + 2.0),
                    color: pin_text_color,
                    size: iced::Pixels(9.5),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }
        }

        let base_geom = frame.into_geometry();

        // Pass 3: Kind dropdown menu overlay
        if let Some(ref menu) = state.menu {
            let mut overlay = Frame::new(renderer, bounds.size());
            overlay.translate(pan);
            overlay.scale(zoom);

            let menu_size = self.kind_menu_size();
            let menu_rect = Rectangle::new(menu.anchor, menu_size);
            overlay.fill_rectangle(
                Point::new(menu_rect.x - 3.0, menu_rect.y - 2.0),
                Size::new(menu_rect.width + 6.0, menu_rect.height + 6.0),
                Color::from_rgba(0.0, 0.0, 0.0, 0.65),
            );
            let menu_bg = if is_dark {
                Color::from_rgb(0.070, 0.094, 0.141)
            } else {
                Color::WHITE
            };
            overlay.fill_rectangle(menu_rect.position(), menu_rect.size(), menu_bg);
            overlay.stroke_rectangle(
                menu_rect.position(),
                menu_rect.size(),
                Stroke::default()
                    .with_color(StudioTheme::ACCENT_CYAN)
                    .with_width(1.5),
            );

            let cursor_world = cursor
                .position_in(bounds)
                .map(|p| Point::new((p.x - pan.x) / zoom, (p.y - pan.y) / zoom));
            let current_kind = self.find_block(&menu.block_id).map(|b| &b.kind);

            for (idx, (kind, label)) in self.kind_menu_items().into_iter().enumerate() {
                let item_r = self.kind_menu_item_rect(menu.anchor, idx);
                let is_hovered = cursor_world.is_some_and(|cp| item_r.contains(cp));
                let is_current = current_kind.is_some_and(|cur| {
                    std::mem::discriminant(cur) == std::mem::discriminant(&kind)
                        && match (cur, &kind) {
                            (
                                FbdBlockKind::StandardFb {
                                    fb_type: Some(a), ..
                                },
                                FbdBlockKind::StandardFb {
                                    fb_type: Some(b), ..
                                },
                            ) => a == b,
                            (
                                FbdBlockKind::Composite { type_name: a },
                                FbdBlockKind::Composite { type_name: b },
                            ) => a == b,
                            _ => true,
                        }
                });

                let item_bg = if is_hovered {
                    if is_dark {
                        Color::from_rgb(0.149, 0.220, 0.329)
                    } else {
                        Color::from_rgb(0.86, 0.92, 0.98)
                    }
                } else if is_current {
                    if is_dark {
                        Color::from_rgb(0.102, 0.173, 0.267)
                    } else {
                        Color::from_rgb(0.91, 0.95, 0.99)
                    }
                } else if is_dark {
                    Color::from_rgb(0.082, 0.106, 0.153)
                } else {
                    Color::from_rgb(0.97, 0.98, 0.99)
                };

                overlay.fill_rectangle(item_r.position(), item_r.size(), item_bg);
                let item_accent = block_category(&kind).accent_color();
                overlay.fill_rectangle(
                    Point::new(item_r.x + 2.0, item_r.y + 3.0),
                    Size::new(3.0, item_r.height - 6.0),
                    item_accent,
                );
                overlay.fill_text(canvas::Text {
                    content: label,
                    position: Point::new(item_r.x + 8.0, item_r.y + 3.5),
                    color: if is_current || is_hovered {
                        StudioTheme::ACCENT_CYAN
                    } else {
                        pin_text_color
                    },
                    size: iced::Pixels(10.0),
                    font: iced::Font::MONOSPACE,
                    ..Default::default()
                });
            }

            vec![base_geom, overlay.into_geometry()]
        } else {
            vec![base_geom]
        }
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
    selected_block: Option<&'a str>,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

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
                background: Some(Background::Color(Color::from_rgba(
                    0.220, 0.741, 0.973, 0.12
                ))),
                border: Border {
                    color: Color::from_rgba(0.220, 0.741, 0.973, 0.35),
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
            .on_press(Message::ConfirmClearPou(pou_name))
            .style(StudioTheme::danger_button)
            .padding([6, 12]),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let scrollable_header = scrollable(container(header_row).padding(Padding {
        top: 2.0,
        right: 4.0,
        bottom: 6.0,
        left: 2.0,
    }))
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

    let top_bar = row![
        header_container,
        crate::icons::info_tip(lang.tr(
            "Direct FBD Canvas: Click a block ID, pin value, or output tag to edit inline. Click the kind badge to switch operator, drag an output pin onto a block to wire, and right-click a row to disconnect or a header to delete.",
            "Canvas FBD direct : Cliquez sur l'ID, une entrée ou une sortie pour éditer en place. Cliquez sur le badge pour changer d'opérateur, glissez une sortie vers un bloc pour câbler, clic droit sur une ligne pour débrancher ou sur l'en-tête pour supprimer.",
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
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(StudioTheme::bg_base(theme))),
        border: Border {
            color: StudioTheme::border_hairline(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    });

    column![top_bar, signal_flow_canvas]
        .spacing(8)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{Key, Location, Modifiers, key};
    use iced::widget::canvas::Program;

    fn val_inputs(pairs: &[(&str, &str)]) -> HashMap<String, FbdInputSource> {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    FbdInputSource::Value {
                        expr: (*v).to_string(),
                    },
                )
            })
            .collect()
    }

    fn key_press(key: Key, text: Option<&str>) -> canvas::Event {
        canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: key.clone(),
            location: Location::Standard,
            modifiers: Modifiers::default(),
            text: text.map(Into::into),
            modified_key: key,
            physical_key: key::Physical::Unidentified(key::NativeCode::Unidentified),
        })
    }

    fn click_at(
        canvas: &FbdSignalFlowCanvas<'_>,
        state: &mut FbdCanvasState,
        bounds: Rectangle,
        pt: Point,
        btn: mouse::Button,
    ) -> (event::Status, Option<Message>) {
        canvas.update(
            state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(btn)),
            bounds,
            mouse::Cursor::Available(pt),
        )
    }

    fn sample_program() -> FbdProgram {
        FbdProgram {
            name: "flow_pou".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![
                FbdBlock {
                    id: "U1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: val_inputs(&[("IN1", "TRUE")]),
                    outputs: HashMap::new(),
                    grid_pos: Some([40, 40]),
                },
                FbdBlock {
                    id: "U2".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: val_inputs(&[("IN1", "FALSE"), ("IN2", "TRUE")]),
                    outputs: HashMap::new(),
                    grid_pos: Some([340, 40]),
                },
            ],
        }
    }

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
        delete_fbd_block(&mut program, 5);
        assert_eq!(program.blocks.len(), 1);
    }

    #[test]
    fn test_fbd_editor_renders_empty_program() {
        let program = FbdProgram {
            name: "empty_prog".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: Vec::new(),
        };
        let _view = render_fbd_editor(&program, None, UiLanguage::En, None);
    }

    #[test]
    fn test_fbd_edit_id_commits_on_enter() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        let origin = Point::new(40.0, 40.0);
        let id_r = id_rect_with_width(
            &program.blocks[0],
            &program.composites,
            origin,
            fbd_geometry::block_width(&program.blocks[0], &program.composites, None),
        );
        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(id_r.x + 4.0, id_r.y + 4.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(msg, Some(Message::FbdInlineEditingChanged(true))));
        assert_eq!(
            state.editing_target,
            Some(FbdEditTarget::BlockId {
                block_id: "U1".to_string()
            })
        );

        let _ = canvas.update(
            &mut state,
            key_press(Key::Character("X".into()), Some("X")),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let (st, msg) = canvas.update(
            &mut state,
            key_press(Key::Named(key::Named::Enter), None),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::FbdBlockIdChanged(ref p, ref old, ref new_id))
                if p == "flow_pou" && old == "U1" && new_id == "U1X"
        ));
        assert!(state.editing_target.is_none());
    }

    #[test]
    fn test_fbd_edit_escape_reverts() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        let origin = Point::new(40.0, 40.0);
        let id_r = id_rect_with_width(
            &program.blocks[0],
            &program.composites,
            origin,
            fbd_geometry::block_width(&program.blocks[0], &program.composites, None),
        );
        let _ = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(id_r.x + 4.0, id_r.y + 4.0),
            mouse::Button::Left,
        );
        let _ = canvas.update(
            &mut state,
            key_press(Key::Character("Z".into()), Some("Z")),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let (st, msg) = canvas.update(
            &mut state,
            key_press(Key::Named(key::Named::Escape), None),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(msg, Some(Message::FbdInlineEditingChanged(false))));
        assert!(state.editing_target.is_none());
        assert!(state.edit_buffer.is_empty());
    }

    #[test]
    fn test_fbd_edit_delete_edits_text_not_block() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        let origin = Point::new(40.0, 40.0);
        let id_r = id_rect_with_width(
            &program.blocks[0],
            &program.composites,
            origin,
            fbd_geometry::block_width(&program.blocks[0], &program.composites, None),
        );
        let (_, open_msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(id_r.x + 4.0, id_r.y + 4.0),
            mouse::Button::Left,
        );
        assert!(matches!(
            open_msg,
            Some(Message::FbdInlineEditingChanged(true))
        ));

        // Move caret left onto '1' and press Delete forward.
        let _ = canvas.update(
            &mut state,
            key_press(Key::Named(key::Named::ArrowLeft), None),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let (st, msg) = canvas.update(
            &mut state,
            key_press(Key::Named(key::Named::Delete), None),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(msg, Some(Message::FbdInlineEditingChanged(true))));
        assert_eq!(state.edit_text(), "U");

        // When not editing, canvas does not consume or translate bare Delete into a deletion.
        state.cancel_edit();
        let (st_closed, msg_closed) = canvas.update(
            &mut state,
            key_press(Key::Named(key::Named::Delete), None),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(st_closed, event::Status::Ignored);
        assert!(msg_closed.is_none());
    }

    #[test]
    fn test_fbd_edit_buffer_handles_multibyte_chars() {
        let mut state = FbdCanvasState::default();
        state.start_edit(
            FbdEditTarget::BlockId {
                block_id: "ab".to_string(),
            },
            "ab",
        );
        state.move_cursor_left();
        state.insert_char('é');
        assert_eq!(state.edit_text(), "aéb");
        state.move_cursor_left();
        state.delete_forward();
        assert_eq!(state.edit_text(), "ab");
        state.insert_char('ç');
        state.backspace();
        assert_eq!(state.edit_text(), "ab");
    }

    #[test]
    fn test_fbd_click_id_starts_edit() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));
        let origin = Point::new(340.0, 40.0);
        let w = fbd_geometry::block_width(&program.blocks[1], &program.composites, None);
        let id_r = id_rect_with_width(&program.blocks[1], &program.composites, origin, w);
        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(id_r.x + 6.0, id_r.y + 5.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(msg, Some(Message::FbdInlineEditingChanged(true))));
        assert_eq!(
            state.editing_target,
            Some(FbdEditTarget::BlockId {
                block_id: "U2".to_string()
            })
        );
        assert_eq!(state.selected_block.as_deref(), Some("U2"));
    }

    #[test]
    fn test_fbd_click_input_row_targets_that_pin() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));
        let origin = Point::new(340.0, 40.0);
        let w = fbd_geometry::block_width(&program.blocks[1], &program.composites, None);
        let val_r = input_value_rect(origin, w, 1);

        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(val_r.x + 8.0, val_r.y + 6.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(msg, Some(Message::FbdInlineEditingChanged(true))));
        assert_eq!(
            state.editing_target,
            Some(FbdEditTarget::InputValue {
                block_id: "U2".to_string(),
                pin: "IN2".to_string(),
            })
        );
    }

    #[test]
    fn test_fbd_click_add_pin_emits_message_and_opens_edit() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        // Non-variadic NOT block has no + pin affordance.
        assert!(
            fbd_geometry::add_pin_rect(
                &program.blocks[0],
                &program.composites,
                Point::new(40.0, 40.0)
            )
            .is_none()
        );

        // Variadic AND block emits FbdAddInputPin and opens edit on IN3.
        let Some(add_r) = fbd_geometry::add_pin_rect(
            &program.blocks[1],
            &program.composites,
            Point::new(340.0, 40.0),
        ) else {
            panic!("AND has + pin");
        };
        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(add_r.x + 10.0, add_r.y + 6.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::FbdAddInputPin(ref p, ref id)) if p == "flow_pou" && id == "U2"
        ));
        assert_eq!(
            state.editing_target,
            Some(FbdEditTarget::InputValue {
                block_id: "U2".to_string(),
                pin: "IN3".to_string(),
            })
        );
    }

    #[test]
    fn test_fbd_click_remove_pin_refused_at_two_inputs() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));
        let origin = Point::new(340.0, 40.0);
        let w = fbd_geometry::block_width(&program.blocks[1], &program.composites, None);
        let del_r = input_delete_rect(origin, w, 0);

        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(del_r.x + 4.0, del_r.y + 4.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(msg.is_none(), "cannot shrink below 2 inputs");
    }

    #[test]
    fn test_fbd_long_expression_is_truncated_at_rest() {
        let long_expr = "Tank_Pressure_Sensor_Upstream_Primary_Control_Tag_Exceeding_Max_Width";
        let mut program = sample_program();
        program.blocks[1].inputs.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: long_expr.to_string(),
            },
        );
        let rest_w = fbd_geometry::block_width(&program.blocks[1], &program.composites, None);
        assert!(rest_w <= fbd_geometry::MAX_BLOCK_W);

        let val_w = input_value_rect(Point::new(340.0, 40.0), rest_w, 0).width;
        let rest_display = format_row_display(long_expr, val_w, false);
        assert!(rest_display.ends_with('\u{2026}'));
        assert!(rest_display.len() < long_expr.len());

        let mut state = FbdCanvasState::default();
        state.start_edit(
            FbdEditTarget::InputValue {
                block_id: "U2".to_string(),
                pin: "IN1".to_string(),
            },
            long_expr,
        );
        let editing_text = state.edit_display_with_caret();
        assert!(editing_text.starts_with(long_expr));
        let edit_w =
            fbd_geometry::block_width(&program.blocks[1], &program.composites, Some(&editing_text));
        assert!(edit_w >= rest_w);
    }

    #[test]
    fn test_fbd_kind_menu_opens_and_commits() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 700.0));
        let badge = kind_badge_rect(
            &program.blocks[1],
            &program.composites,
            Point::new(340.0, 40.0),
        );

        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(badge.x + 5.0, badge.y + 5.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(msg.is_none());
        let Some(menu) = state.menu.clone() else {
            panic!("kind menu should open");
        };
        assert_eq!(menu.block_id, "U2");

        let Some(or_idx) = canvas
            .kind_menu_items()
            .iter()
            .position(|(k, _)| *k == FbdBlockKind::Or)
        else {
            panic!("OR present in menu");
        };
        let item_r = canvas.kind_menu_item_rect(menu.anchor, or_idx);
        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(item_r.x + 8.0, item_r.y + 6.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::FbdChangeKind(ref p, ref id, FbdBlockKind::Or))
                if p == "flow_pou" && id == "U2"
        ));
        assert!(state.menu.is_none());
    }

    #[test]
    fn test_fbd_kind_menu_flips_when_clipping() {
        let mut program = sample_program();
        program.blocks[1].grid_pos = Some([340, 520]);
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));
        let badge = kind_badge_rect(
            &program.blocks[1],
            &program.composites,
            Point::new(340.0, 520.0),
        );

        let _ = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(badge.x + 5.0, badge.y + 5.0),
            mouse::Button::Left,
        );
        let Some(menu) = state.menu else {
            panic!("menu opened");
        };
        assert!(
            menu.anchor.y < badge.y,
            "menu near bottom edge must flip above the badge (anchor.y={}, badge.y={})",
            menu.anchor.y,
            badge.y
        );
    }

    #[test]
    fn test_fbd_drag_block_commits_position_once() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        // Click body area (below the header, inside the row label column so no input box captures).
        let body_pt = Point::new(40.0 + 18.0, 40.0 + HEADER_H + 8.0);
        let (_, press_msg) = click_at(&canvas, &mut state, bounds, body_pt, mouse::Button::Left);
        assert!(matches!(
            press_msg,
            Some(Message::FbdBlockSelected(ref p, Some(ref id))) if p == "flow_pou" && id == "U1"
        ));
        assert!(matches!(state.drag, FbdDragState::MovingBlock { .. }));

        // Intermediate move emits nothing and updates preview only.
        let (st_mv, msg_mv) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(body_pt.x + 72.0, body_pt.y),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(body_pt.x + 72.0, body_pt.y)),
        );
        assert_eq!(st_mv, event::Status::Captured);
        assert!(msg_mv.is_none());

        // Release commits FbdBlockMoved once.
        let (st_rel, msg_rel) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(body_pt.x + 72.0, body_pt.y)),
        );
        assert_eq!(st_rel, event::Status::Captured);
        assert!(matches!(
            msg_rel,
            Some(Message::FbdBlockMoved(ref p, ref id, pos))
                if p == "flow_pou" && id == "U1" && pos[0] > 40
        ));
    }

    #[test]
    fn test_fbd_wire_drop_on_body_targets_nearest_row() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        let u1_origin = Point::new(40.0, 40.0);
        let u1_w = fbd_geometry::block_width(&program.blocks[0], &program.composites, None);
        let u1_out = output_pin_point(u1_origin, u1_w, 1, 0);

        let (st, _) = click_at(&canvas, &mut state, bounds, u1_out, mouse::Button::Left);
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(state.drag, FbdDragState::Wiring { .. }));

        // Drop inside inflated U2 body near row 2 (IN2).
        let u2_origin = Point::new(340.0, 40.0);
        let drop_pt = Point::new(u2_origin.x + 90.0, input_pin_point(u2_origin, 1).y);
        let (st_rel, msg_rel) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(drop_pt),
        );
        assert_eq!(st_rel, event::Status::Captured);
        assert!(matches!(
            msg_rel,
            Some(Message::FbdInputSourceChanged(ref p, ref id, ref pin, ref val))
                if p == "flow_pou" && id == "U2" && pin == "IN2" && val == "U1.OUT"
        ));
    }

    #[test]
    fn test_fbd_wire_release_on_empty_emits_nothing() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        let u1_origin = Point::new(40.0, 40.0);
        let u1_w = fbd_geometry::block_width(&program.blocks[0], &program.composites, None);
        let u1_out = output_pin_point(u1_origin, u1_w, 1, 0);
        let _ = click_at(&canvas, &mut state, bounds, u1_out, mouse::Button::Left);

        let (st_rel, msg_rel) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(Point::new(800.0, 500.0)),
        );
        assert_eq!(st_rel, event::Status::Captured);
        assert!(msg_rel.is_none());
    }

    #[test]
    fn test_fbd_right_click_row_unlinks() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));
        let u2_origin = Point::new(340.0, 40.0);
        let u2_w = fbd_geometry::block_width(&program.blocks[1], &program.composites, None);
        let row_r = input_row_rect(u2_origin, u2_w, 0);

        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(row_r.x + 20.0, row_r.y + 8.0),
            mouse::Button::Right,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::FbdDisconnectInput(ref p, ref id, ref pin))
                if p == "flow_pou" && id == "U2" && pin == "IN1"
        ));
    }

    #[test]
    fn test_fbd_right_click_header_removes_block() {
        let program = sample_program();
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));
        let u2_origin = Point::new(340.0, 40.0);
        let u2_w = fbd_geometry::block_width(&program.blocks[1], &program.composites, None);
        let hdr_r = header_rect_with_width(u2_origin, u2_w);

        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(hdr_r.x + 20.0, hdr_r.y + 8.0),
            mouse::Button::Right,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(
            msg,
            Some(Message::FbdRemoveBlock(ref p, ref id))
                if p == "flow_pou" && id == "U2"
        ));
    }

    #[test]
    fn test_fbd_pan_zoom_and_legacy_layout() {
        let legacy = FbdProgram {
            name: "legacy_pou".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![
                FbdBlock {
                    id: "U1".to_string(),
                    kind: FbdBlockKind::Not,
                    inputs: val_inputs(&[("IN1", "TRUE")]),
                    outputs: HashMap::new(),
                    grid_pos: None,
                },
                FbdBlock {
                    id: "U2".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: [
                        (
                            "IN1".to_string(),
                            FbdInputSource::Link {
                                from_block: "U1".to_string(),
                                from_pin: "OUT".to_string(),
                            },
                        ),
                        (
                            "IN2".to_string(),
                            FbdInputSource::Value {
                                expr: "TRUE".to_string(),
                            },
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    outputs: HashMap::new(),
                    grid_pos: None,
                },
            ],
        };

        let canvas = FbdSignalFlowCanvas {
            pou_name: "legacy_pou",
            program: &legacy,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        // 1. Legacy blocks without grid_pos lay out deterministically with U2 right of U1.
        let layout = state.resolved_layout(&legacy);
        let u1_pos = layout["U1"];
        let u2_pos = layout["U2"];
        assert!(u2_pos.x > u1_pos.x);

        // 2. Middle-mouse pan shifts pan and emits FbdViewportChanged on release.
        let _ = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(100.0, 100.0),
            mouse::Button::Middle,
        );
        let _ = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(160.0, 130.0),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(160.0, 130.0)),
        );
        let (st, msg) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)),
            bounds,
            mouse::Cursor::Available(Point::new(160.0, 130.0)),
        );
        assert_eq!(st, event::Status::Captured);
        assert_eq!(state.pan, Vector::new(60.0, 30.0));
        assert!(matches!(
            msg,
            Some(Message::FbdViewportChanged(ref p, pan, _, _))
                if p == "legacy_pou" && pan == Vector::new(60.0, 30.0)
        ));

        // 3. Wheel zoom emits FbdViewportChanged and clamps zoom cleanly.
        let (st_z, msg_z) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            bounds,
            mouse::Cursor::Available(Point::new(300.0, 200.0)),
        );
        assert_eq!(st_z, event::Status::Captured);
        assert!(state.effective_zoom() > 1.0);
        assert!(matches!(msg_z, Some(Message::FbdViewportChanged(..))));
    }

    #[test]
    fn test_fbd_output_row_edit_and_right_click_disconnect() {
        let mut program = sample_program();
        program.blocks[0]
            .outputs
            .insert("OUT".to_string(), "Motor_Cmd".to_string());
        let canvas = FbdSignalFlowCanvas {
            pou_name: "flow_pou",
            program: &program,
            runtime: None,
            selected_block: None,
        };
        let mut state = FbdCanvasState::default();
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(1000.0, 600.0));

        let origin = Point::new(40.0, 40.0);
        let w = fbd_geometry::block_width(&program.blocks[0], &program.composites, None);
        let out_val_r = output_value_rect(origin, w, 1, 0);

        let (st, msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(out_val_r.x + 8.0, out_val_r.y + 6.0),
            mouse::Button::Left,
        );
        assert_eq!(st, event::Status::Captured);
        assert!(matches!(msg, Some(Message::FbdInlineEditingChanged(true))));
        assert_eq!(
            state.editing_target,
            Some(FbdEditTarget::OutputTarget {
                block_id: "U1".to_string(),
                pin: "OUT".to_string(),
            })
        );

        let _ = canvas.update(
            &mut state,
            key_press(Key::Character("2".into()), Some("2")),
            bounds,
            mouse::Cursor::Unavailable,
        );
        let (_, commit_msg) = canvas.update(
            &mut state,
            key_press(Key::Named(key::Named::Enter), None),
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert!(matches!(
            commit_msg,
            Some(Message::FbdOutputTargetChanged(ref p, ref id, ref pin, ref target))
                if p == "flow_pou" && id == "U1" && pin == "OUT" && target == "Motor_Cmd2"
        ));

        let out_row_r = output_row_rect(origin, w, 1, 0);
        let (_, rc_msg) = click_at(
            &canvas,
            &mut state,
            bounds,
            Point::new(out_row_r.x + 12.0, out_row_r.y + 8.0),
            mouse::Button::Right,
        );
        assert!(matches!(
            rc_msg,
            Some(Message::FbdDisconnectOutput(ref p, ref id, ref pin))
                if p == "flow_pou" && id == "U1" && pin == "OUT"
        ));
    }
}
