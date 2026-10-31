use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, Text};
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{event, mouse};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Size, Theme, Vector};
use serde::{Deserialize, Serialize};

use ferrite_core::IecValue;
use ferrite_lang_ld::{CoilKind, ContactKind, LdElement, LdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub const CONTACT_CARD_WIDTH: f32 = 108.0;
pub const COIL_CARD_WIDTH: f32 = 108.0;
pub const COIL_CARD_HEIGHT: f32 = 44.0;
pub const WIRE_GAP: f32 = 10.0;
pub const RAIL_HEIGHT: f32 = 48.0;

pub const RAIL_LEFT_X: f32 = 80.0;
pub const SLOT_WIDTH: f32 = 104.0;
pub const SLOT_HEIGHT: f32 = 48.0;
pub const BRANCH_VERTICAL_STEP: f32 = 64.0;
pub const RUNG_GAP_Y: f32 = 44.0;
pub const RUNG_HEADER_HEIGHT: f32 = 28.0;
pub const WIRE_PAD: f32 = 20.0;

/// Identifies a specific parallel branch step-down within an element list:
/// `(branch_element_index, path_index)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LdBranchPathSegment {
    pub branch_idx: usize,
    pub path_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LdElementKey {
    Rung(usize),
    Contact {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    Coil {
        rung: usize,
        index: usize,
    },
    Branch {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    InlineFb {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdDropTarget {
    BeforeElement {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    AfterElement {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    NewBranch {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    CoilSlot {
        rung: usize,
        index: usize,
    },
    NewRung {
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub struct LdPlacedElement {
    pub key: LdElementKey,
    pub bounds: Rectangle,
    pub energized: bool,
}

#[derive(Debug, Clone)]
pub struct LdPlacedBranch {
    pub key: LdElementKey,
    pub split_x: f32,
    pub merge_x: f32,
    pub start_y: f32,
    pub paths_y: Vec<f32>,
    pub energized: bool,
}

#[derive(Debug, Clone)]
pub struct LdPlacedRung {
    pub rung_idx: usize,
    pub bounds: Rectangle,
    pub wire_y: f32,
    pub elements: Vec<LdPlacedElement>,
    pub branches: Vec<LdPlacedBranch>,
    pub coils: Vec<LdPlacedElement>,
    pub power_flowing: bool,
}

#[derive(Debug, Clone)]
pub struct LdLayoutResult {
    pub rail_left_x: f32,
    pub rail_right_x: f32,
    pub rungs: Vec<LdPlacedRung>,
    pub total_bounds: Rectangle,
}

impl LdLayoutResult {
    pub fn hit_test(&self, point: Point) -> Option<LdElementKey> {
        for rung in &self.rungs {
            // 1. Contacts
            for elem in &rung.elements {
                if elem.bounds.contains(point) {
                    return Some(elem.key.clone());
                }
            }
            // 2. Coils
            for coil in &rung.coils {
                if coil.bounds.contains(point) {
                    return Some(coil.key.clone());
                }
            }
            // 3. Branches
            for br in &rung.branches {
                let max_y = br.paths_y.iter().cloned().fold(br.start_y, f32::max);
                let br_box = Rectangle {
                    x: br.split_x - 10.0,
                    y: br.start_y - 10.0,
                    width: (br.merge_x - br.split_x + 20.0).max(20.0),
                    height: (max_y - br.start_y + 20.0).max(20.0),
                };
                if br_box.contains(point) {
                    let near_split = (point.x - br.split_x).abs() <= 10.0 && point.y >= br.start_y - 5.0 && point.y <= max_y + 5.0;
                    let near_merge = (point.x - br.merge_x).abs() <= 10.0 && point.y >= br.start_y - 5.0 && point.y <= max_y + 5.0;
                    if near_split || near_merge {
                        return Some(br.key.clone());
                    }
                }
            }
            // 4. Rung header (left margin + title strip across the top of the rung)
            let rung_header = Rectangle {
                x: self.rail_left_x - 70.0,
                y: rung.bounds.y,
                width: (self.rail_right_x - self.rail_left_x) + 70.0,
                height: RUNG_HEADER_HEIGHT,
            };
            if rung_header.contains(point) {
                return Some(LdElementKey::Rung(rung.rung_idx));
            }
        }
        None
    }

    pub fn elements_in_rect(&self, rect: Rectangle) -> Vec<LdElementKey> {
        let mut keys = Vec::new();
        let min_x = rect.x;
        let max_x = rect.x + rect.width;
        let min_y = rect.y;
        let max_y = rect.y + rect.height;

        let intersects = |b: &Rectangle| -> bool {
            b.x < max_x && b.x + b.width > min_x && b.y < max_y && b.y + b.height > min_y
        };

        for rung in &self.rungs {
            for elem in &rung.elements {
                if intersects(&elem.bounds) {
                    keys.push(elem.key.clone());
                }
            }
            for coil in &rung.coils {
                if intersects(&coil.bounds) {
                    keys.push(coil.key.clone());
                }
            }
            for br in &rung.branches {
                let max_y = br.paths_y.iter().cloned().fold(br.start_y, f32::max);
                let split_bar = Rectangle {
                    x: br.split_x - 6.0,
                    y: br.start_y,
                    width: 12.0,
                    height: (max_y - br.start_y).max(10.0),
                };
                let merge_bar = Rectangle {
                    x: br.merge_x - 6.0,
                    y: br.start_y,
                    width: 12.0,
                    height: (max_y - br.start_y).max(10.0),
                };
                if intersects(&split_bar) || intersects(&merge_bar) {
                    keys.push(br.key.clone());
                }
            }
        }
        keys
    }

    pub fn find_nearest_drop_target(
        &self,
        point: Point,
        dragged_keys: &[LdElementKey],
    ) -> Option<LdDropTarget> {
        let has_coil = dragged_keys.iter().any(|k| matches!(k, LdElementKey::Coil { .. }));
        let has_contact = dragged_keys.iter().any(|k| {
            matches!(
                k,
                LdElementKey::Contact { .. }
                    | LdElementKey::Branch { .. }
                    | LdElementKey::InlineFb { .. }
            )
        });
        if has_coil && has_contact {
            // Disallow mixed contact/coil dragging to avoid unintended element loss
            return None;
        }
        let is_dragging_coil = has_coil;

        for rung in &self.rungs {
            if is_dragging_coil {
                let right_area = Rectangle {
                    x: self.rail_right_x - WIRE_PAD - COIL_CARD_WIDTH - 30.0,
                    y: rung.bounds.y,
                    width: COIL_CARD_WIDTH + 60.0,
                    height: rung.bounds.height,
                };
                if right_area.contains(point) {
                    return Some(LdDropTarget::CoilSlot {
                        rung: rung.rung_idx,
                        index: rung.coils.len(),
                    });
                }
            } else {
                for elem in &rung.elements {
                    if dragged_keys.contains(&elem.key) {
                        continue;
                    }
                    if let LdElementKey::Contact { branch_path, index, .. } = &elem.key {
                        if point.x >= elem.bounds.x - 20.0
                            && point.x < elem.bounds.x + elem.bounds.width * 0.35
                            && (point.y - (elem.bounds.y + elem.bounds.height / 2.0)).abs() <= 25.0
                        {
                            return Some(LdDropTarget::BeforeElement {
                                rung: rung.rung_idx,
                                branch_path: branch_path.clone(),
                                index: *index,
                            });
                        }
                        if point.x > elem.bounds.x + elem.bounds.width * 0.65
                            && point.x <= elem.bounds.x + elem.bounds.width + 20.0
                            && (point.y - (elem.bounds.y + elem.bounds.height / 2.0)).abs() <= 25.0
                        {
                            return Some(LdDropTarget::AfterElement {
                                rung: rung.rung_idx,
                                branch_path: branch_path.clone(),
                                index: *index,
                            });
                        }
                        if elem.bounds.contains(point)
                            || (point.x >= elem.bounds.x
                                && point.x <= elem.bounds.x + elem.bounds.width
                                && (point.y - (elem.bounds.y + elem.bounds.height)).abs() <= 20.0)
                        {
                            return Some(LdDropTarget::NewBranch {
                                rung: rung.rung_idx,
                                branch_path: branch_path.clone(),
                                index: *index,
                            });
                        }
                    }
                }
                if rung.elements.is_empty() {
                    let wire_area = Rectangle {
                        x: self.rail_left_x,
                        y: rung.wire_y - 20.0,
                        width: self.rail_right_x - self.rail_left_x,
                        height: 40.0,
                    };
                    if wire_area.contains(point) {
                        return Some(LdDropTarget::BeforeElement {
                            rung: rung.rung_idx,
                            branch_path: Vec::new(),
                            index: 0,
                        });
                    }
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct LdCanvasState {
    pub pan: Vector,
    pub zoom: f32,
    pub initialized: bool,
    pub selected: Option<LdElementKey>,
    pub selected_elements: Vec<LdElementKey>,
    pub drag_marquee: Option<(Point, Point)>,
    pub dragging: Option<(Vec<LdElementKey>, Point)>,
    pub drop_hover: Option<LdDropTarget>,
    pub is_panning: bool,
    pub pan_start: Option<Point>,
    pub modifiers: iced::keyboard::Modifiers,
}

impl Default for LdCanvasState {
    fn default() -> Self {
        Self {
            pan: Vector::default(),
            zoom: 1.0,
            initialized: false,
            selected: None,
            selected_elements: Vec::new(),
            drag_marquee: None,
            dragging: None,
            drop_hover: None,
            is_panning: false,
            pan_start: None,
            modifiers: iced::keyboard::Modifiers::default(),
        }
    }
}

pub struct LdCanvasProgram<'a> {
    pub pou_name: &'a str,
    pub program: &'a LdProgram,
    pub runtime: Option<&'a PlcRuntime>,
    pub selected: Option<&'a LdElementKey>,
    pub selected_elements: &'a [LdElementKey],
    pub lang: UiLanguage,
    pub initial_pan: Vector,
    pub initial_zoom: f32,
}

impl<'a> LdCanvasProgram<'a> {
    pub fn lookup_element<'b>(
        elements: &'b [LdElement],
        branch_path: &[LdBranchPathSegment],
        index: usize,
    ) -> Option<&'b LdElement> {
        let mut curr_slice = elements;
        for seg in branch_path {
            if let Some(LdElement::Branch { paths }) = curr_slice.get(seg.branch_idx) {
                let sub_path = paths.get(seg.path_idx)?;
                curr_slice = sub_path.as_slice();
            } else {
                return None;
            }
        }
        curr_slice.get(index)
    }

    pub fn lookup_element_mut<'b>(
        elements: &'b mut [LdElement],
        branch_path: &[LdBranchPathSegment],
        index: usize,
    ) -> Option<&'b mut LdElement> {
        let mut curr_slice = elements;
        for seg in branch_path {
            if let Some(LdElement::Branch { paths }) = curr_slice.get_mut(seg.branch_idx) {
                let sub_path = paths.get_mut(seg.path_idx)?;
                curr_slice = sub_path.as_mut_slice();
            } else {
                return None;
            }
        }
        curr_slice.get_mut(index)
    }
}

pub fn lookup_element_mut<'b>(
    elements: &'b mut [LdElement],
    branch_path: &[LdBranchPathSegment],
    index: usize,
) -> Option<&'b mut LdElement> {
    LdCanvasProgram::lookup_element_mut(elements, branch_path, index)
}

impl<'a> canvas::Program<Message> for LdCanvasProgram<'a> {
    type State = LdCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        if !state.initialized {
            state.pan = self.initial_pan;
            state.zoom = self.initial_zoom;
            state.initialized = true;
        }

        let cursor_pos = match cursor.position_in(bounds) {
            Some(p) => p,
            None => {
                if let canvas::Event::Mouse(mouse::Event::ButtonReleased(_)) = event {
                    state.is_panning = false;
                    state.pan_start = None;
                    state.drag_marquee = None;
                    state.dragging = None;
                    state.drop_hover = None;
                }
                return (event::Status::Ignored, None);
            }
        };

        let canvas_pos = Point::new(
            (cursor_pos.x - state.pan.x) / state.zoom.max(0.1),
            (cursor_pos.y - state.pan.y) / state.zoom.max(0.1),
        );

        match event {
            // 1. Keyboard Events
            canvas::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(m)) => {
                state.modifiers = m;
                (event::Status::Captured, None)
            }
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                state.modifiers = modifiers;
                if matches!(
                    key,
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Delete | iced::keyboard::key::Named::Backspace
                    )
                ) {
                    (
                        event::Status::Captured,
                        Some(Message::LdRequestDeleteSelected(self.pou_name.to_string())),
                    )
                } else {
                    (event::Status::Ignored, None)
                }
            }

            // 2. Middle-click pan
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                state.is_panning = true;
                state.pan_start = Some(cursor_pos);
                (event::Status::Captured, None)
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                state.is_panning = false;
                state.pan_start = None;
                (event::Status::Captured, None)
            }

            // 3. Scroll Wheel Zoom
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let factor = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => 1.0 + y * 0.08,
                    mouse::ScrollDelta::Pixels { y, .. } => 1.0 + y * 0.003,
                };
                let old_zoom = state.zoom;
                let new_zoom = (old_zoom * factor).clamp(0.25, 3.0);
                if (new_zoom - old_zoom).abs() > 0.001 {
                    let new_pan_x = cursor_pos.x - canvas_pos.x * new_zoom;
                    let new_pan_y = cursor_pos.y - canvas_pos.y * new_zoom;
                    state.zoom = new_zoom;
                    state.pan = Vector::new(new_pan_x, new_pan_y);
                    (
                        event::Status::Captured,
                        Some(Message::LdCanvasZoomChanged(
                            self.pou_name.to_string(),
                            new_zoom,
                            state.pan,
                        )),
                    )
                } else {
                    (event::Status::Ignored, None)
                }
            }

            // 4. Cursor Movement
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.is_panning
                    && let Some(start) = state.pan_start
                {
                    let dx = cursor_pos.x - start.x;
                    let dy = cursor_pos.y - start.y;
                    state.pan = Vector::new(state.pan.x + dx, state.pan.y + dy);
                    state.pan_start = Some(cursor_pos);
                    return (
                        event::Status::Captured,
                        Some(Message::LdCanvasPanChanged(
                            self.pou_name.to_string(),
                            state.pan,
                        )),
                    );
                }

                if let Some((start, _)) = state.drag_marquee {
                    state.drag_marquee = Some((start, canvas_pos));
                    let min_x = start.x.min(canvas_pos.x);
                    let max_x = start.x.max(canvas_pos.x);
                    let min_y = start.y.min(canvas_pos.y);
                    let max_y = start.y.max(canvas_pos.y);
                    let rect = Rectangle {
                        x: min_x,
                        y: min_y,
                        width: max_x - min_x,
                        height: max_y - min_y,
                    };
                    let layout = LdLayoutEngine::compute_layout(self.program, self.runtime);
                    let selected = layout.elements_in_rect(rect);
                    state.selected_elements = selected;
                    state.selected = if state.selected_elements.len() == 1 {
                        Some(state.selected_elements[0].clone())
                    } else {
                        None
                    };
                    return (event::Status::Captured, None);
                }

                if let Some((ref keys, _origin)) = state.dragging {
                    let layout = LdLayoutEngine::compute_layout(self.program, self.runtime);
                    state.drop_hover = layout.find_nearest_drop_target(canvas_pos, keys);
                    return (event::Status::Captured, None);
                }

                (event::Status::Ignored, None)
            }

            // 5. Left Click Pressed
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let layout = LdLayoutEngine::compute_layout(self.program, self.runtime);
                let hit = layout.hit_test(canvas_pos);

                let is_ctrl = state.modifiers.control() || state.modifiers.command();

                if let Some(key) = hit {
                    if is_ctrl {
                        if let Some(pos) = state.selected_elements.iter().position(|k| k == &key) {
                            state.selected_elements.remove(pos);
                        } else {
                            state.selected_elements.push(key);
                        }
                        state.selected = if state.selected_elements.len() == 1 {
                            Some(state.selected_elements[0].clone())
                        } else {
                            None
                        };
                        (
                            event::Status::Captured,
                            Some(Message::LdSelectElements(
                                self.pou_name.to_string(),
                                state.selected_elements.clone(),
                            )),
                        )
                    } else {
                        let to_drag = if state.selected_elements.contains(&key) && state.selected_elements.len() > 1 {
                            state.selected_elements.clone()
                        } else {
                            state.selected = Some(key.clone());
                            state.selected_elements = vec![key.clone()];
                            vec![key]
                        };
                        state.dragging = Some((to_drag, canvas_pos));
                        state.drop_hover = None;
                        (
                            event::Status::Captured,
                            Some(Message::LdSelectElements(
                                self.pou_name.to_string(),
                                state.selected_elements.clone(),
                            )),
                        )
                    }
                } else {
                    state.selected = None;
                    state.selected_elements.clear();
                    state.drag_marquee = Some((canvas_pos, canvas_pos));
                    (
                        event::Status::Captured,
                        Some(Message::LdSelectElements(
                            self.pou_name.to_string(),
                            Vec::new(),
                        )),
                    )
                }
            }

            // 6. Left Click Released
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some((start, end)) = state.drag_marquee.take() {
                    let min_x = start.x.min(end.x);
                    let max_x = start.x.max(end.x);
                    let min_y = start.y.min(end.y);
                    let max_y = start.y.max(end.y);
                    let rect = Rectangle {
                        x: min_x,
                        y: min_y,
                        width: max_x - min_x,
                        height: max_y - min_y,
                    };
                    let layout = LdLayoutEngine::compute_layout(self.program, self.runtime);
                    let selected = layout.elements_in_rect(rect);
                    state.selected_elements = selected.clone();
                    state.selected = if selected.len() == 1 {
                        Some(selected[0].clone())
                    } else {
                        None
                    };
                    return (
                        event::Status::Captured,
                        Some(Message::LdSelectElements(
                            self.pou_name.to_string(),
                            selected,
                        )),
                    );
                }

                if let Some((keys, _origin)) = state.dragging.take() {
                    let drop_target = state.drop_hover.take();
                    if let Some(target) = drop_target {
                        return (
                            event::Status::Captured,
                            Some(Message::LdMoveElements(
                                self.pou_name.to_string(),
                                keys,
                                target,
                            )),
                        );
                    }
                    return (event::Status::Captured, None);
                }

                (event::Status::Ignored, None)
            }

            _ => (event::Status::Ignored, None),
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let pan = if state.initialized { state.pan } else { self.initial_pan };
        let zoom = if state.initialized { state.zoom } else { self.initial_zoom };

        // Background grid dots
        let dot_spacing = 20.0 * zoom;
        if dot_spacing >= 8.0 {
            let offset_x = (pan.x * zoom) % dot_spacing;
            let offset_y = (pan.y * zoom) % dot_spacing;
            let dot_color = Color::from_rgba(0.25, 0.35, 0.5, 0.18);
            let mut x = offset_x;
            while x < bounds.width {
                let mut y = offset_y;
                while y < bounds.height {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        iced::Size::new(1.5, 1.5),
                        dot_color,
                    );
                    y += dot_spacing;
                }
                x += dot_spacing;
            }
        }

        // Apply pan and zoom
        frame.translate(pan);
        frame.scale(zoom);

        let layout = LdLayoutEngine::compute_layout(self.program, self.runtime);

        if layout.rungs.is_empty() {
            frame.fill_text(Text {
                content: "No rungs in program. Click '+ Add Rung' to begin.".to_string(),
                position: Point::new(80.0, 60.0),
                color: StudioTheme::TEXT_MUTED,
                size: 13.0.into(),
                ..Default::default()
            });
            return vec![frame.into_geometry()];
        }

        let first_wire_y = layout.rungs.first().map(|r| r.wire_y).unwrap_or(60.0);
        let last_wire_y = layout
            .rungs
            .last()
            .map(|last_rung| {
                last_rung.wire_y
                    + (last_rung.branches.len().max(last_rung.coils.len()).saturating_sub(1)) as f32
                        * BRANCH_VERTICAL_STEP
            })
            .unwrap_or(first_wire_y);

        let rail_top_y = (first_wire_y - 20.0).max(20.0);
        let rail_bottom_y = last_wire_y + 30.0;

        // Draw Left Power Rail (Energized emerald)
        frame.stroke(
            &Path::line(Point::new(layout.rail_left_x, rail_top_y), Point::new(layout.rail_left_x, rail_bottom_y)),
            Stroke::default().with_color(StudioTheme::ACCENT_EMERALD).with_width(5.0),
        );

        // Draw Right Power Rail (Neutral card border)
        frame.stroke(
            &Path::line(Point::new(layout.rail_right_x, rail_top_y), Point::new(layout.rail_right_x, rail_bottom_y)),
            Stroke::default().with_color(StudioTheme::BORDER_CARD).with_width(5.0),
        );

        // Render each rung
        for rung_layout in &layout.rungs {
            let r_idx = rung_layout.rung_idx;
            let rung = &self.program.rungs[r_idx];

            // Rung ID badge & comment
            let rung_header_y = rung_layout.bounds.y + 4.0;
            let is_rung_selected = self.selected == Some(&LdElementKey::Rung(r_idx))
                || state.selected_elements.contains(&LdElementKey::Rung(r_idx));

            if is_rung_selected {
                frame.fill_rectangle(
                    Point::new(layout.rail_left_x - 35.0, rung_layout.bounds.y + 1.0),
                    Size::new((layout.rail_right_x - layout.rail_left_x) + 45.0, RUNG_HEADER_HEIGHT - 2.0),
                    iced::Color::from_rgba(0.0, 0.85, 0.85, 0.10),
                );
            }

            // Rung index in left margin (e.g. 0, 1, 2)
            frame.fill_text(Text {
                content: format!("{r_idx}"),
                position: Point::new(layout.rail_left_x - 24.0, rung_header_y),
                color: if is_rung_selected { StudioTheme::ACCENT_CYAN } else { StudioTheme::TEXT_MUTED },
                size: 11.0.into(),
                ..Default::default()
            });

            // Rung Name badge (e.g. [Rung_0] or [Safety_Stop]) starting inside the rung at rail_left_x + 12.0
            let header_x = layout.rail_left_x + 12.0;
            let name_badge = format!("[{}]", rung.id);
            frame.fill_text(Text {
                content: name_badge.clone(),
                position: Point::new(header_x, rung_header_y),
                color: StudioTheme::ACCENT_CYAN,
                size: 11.5.into(),
                ..Default::default()
            });

            if !rung.comment.is_empty() {
                // Dynamically offset comment so it never clips or overlaps with the rung name
                let name_width = (name_badge.len() as f32 * 7.5 + 14.0).max(64.0);
                frame.fill_text(Text {
                    content: format!("// {}", rung.comment),
                    position: Point::new(header_x + name_width, rung_header_y),
                    color: StudioTheme::TEXT_MUTED,
                    size: 10.5.into(),
                    ..Default::default()
                });
            }

            // Power flow wire color for main rung feeder
            let wire_color = if rung_layout.power_flowing {
                StudioTheme::ACCENT_EMERALD
            } else {
                StudioTheme::BORDER_CARD
            };
            let wire_stroke = Stroke::default().with_color(wire_color).with_width(2.5);

            // Wire segments between rail, elements, and coils
            let first_coil_x = rung_layout.coils.first().map(|c| c.bounds.x).unwrap_or(layout.rail_right_x);
            if rung_layout.elements.is_empty() {
                // Straight through wire from left rail to coils
                frame.stroke(
                    &Path::line(Point::new(layout.rail_left_x, rung_layout.wire_y), Point::new(first_coil_x, rung_layout.wire_y)),
                    Stroke::default().with_color(StudioTheme::ACCENT_EMERALD).with_width(2.5),
                );
            } else {
                // Left rail to first contact / branch
                let first_elem_x = rung_layout.elements.first().map(|e| e.bounds.x).unwrap_or(layout.rail_right_x - 120.0);
                frame.stroke(
                    &Path::line(Point::new(layout.rail_left_x, rung_layout.wire_y), Point::new(first_elem_x, rung_layout.wire_y)),
                    Stroke::default().with_color(StudioTheme::ACCENT_EMERALD).with_width(2.5),
                );

                // Last contact / branch to coils
                let last_contact_x = rung_layout.elements.last().map(|e| e.bounds.x + e.bounds.width).unwrap_or(layout.rail_left_x + WIRE_PAD);
                frame.stroke(
                    &Path::line(Point::new(last_contact_x, rung_layout.wire_y), Point::new(first_coil_x, rung_layout.wire_y)),
                    wire_stroke,
                );

                // Inter-contact wire segments
                for i in 0..rung_layout.elements.len().saturating_sub(1) {
                    let e1 = &rung_layout.elements[i];
                    let e2 = &rung_layout.elements[i + 1];
                    let inter_wire_color = if e1.energized { StudioTheme::ACCENT_EMERALD } else { StudioTheme::BORDER_CARD };
                    frame.stroke(
                        &Path::line(Point::new(e1.bounds.x + e1.bounds.width, rung_layout.wire_y), Point::new(e2.bounds.x, rung_layout.wire_y)),
                        Stroke::default().with_color(inter_wire_color).with_width(2.5),
                    );
                }
            }

            // Draw parallel branch wires before contacts so contact cards sit cleanly on top of cables
            for branch in &rung_layout.branches {
                let branch_wire_color = if branch.energized { StudioTheme::ACCENT_EMERALD } else { StudioTheme::BORDER_CARD };
                let b_stroke = Stroke::default().with_color(branch_wire_color).with_width(2.0);

                if let Some(&lowest_y) = branch.paths_y.last() {
                    // Vertical drop wire at split_x
                    frame.stroke(
                        &Path::line(Point::new(branch.split_x, branch.start_y), Point::new(branch.split_x, lowest_y)),
                        b_stroke,
                    );
                    // Vertical rise wire at merge_x
                    frame.stroke(
                        &Path::line(Point::new(branch.merge_x, branch.start_y), Point::new(branch.merge_x, lowest_y)),
                        b_stroke,
                    );
                    // Horizontal feeder wires for each path
                    for &py in &branch.paths_y {
                        frame.stroke(
                            &Path::line(Point::new(branch.split_x, py), Point::new(branch.merge_x, py)),
                            b_stroke,
                        );
                    }
                }
            }

            // Draw contacts & inline FBs
            for elem in &rung_layout.elements {
                let is_selected = self.selected == Some(&elem.key)
                    || self.selected_elements.contains(&elem.key)
                    || state.selected_elements.contains(&elem.key)
                    || state.selected == Some(elem.key.clone());
                let border_color = if is_selected {
                    StudioTheme::ACCENT_CYAN
                } else if elem.energized {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    StudioTheme::BORDER_CARD
                };
                let bg_color = if elem.energized {
                    Color::from_rgba(0.063, 0.725, 0.506, 0.15)
                } else {
                    StudioTheme::BG_BASE
                };

                // Card background & border
                frame.fill_rectangle(
                    Point::new(elem.bounds.x, elem.bounds.y),
                    elem.bounds.size(),
                    bg_color,
                );
                frame.stroke(
                    &Path::rectangle(Point::new(elem.bounds.x, elem.bounds.y), elem.bounds.size()),
                    Stroke::default().with_color(border_color).with_width(if is_selected { 2.0 } else { 1.0 }),
                );

                // Symbol text and glyph
                match &elem.key {
                    LdElementKey::Contact { index, branch_path, .. } => {
                        let contact_elem = Self::lookup_element(&rung.elements, branch_path, *index);
                        if let Some(LdElement::Contact { symbol, kind }) = contact_elem {
                            frame.fill_text(Text {
                                content: symbol.clone(),
                                position: Point::new(elem.bounds.x + 6.0, elem.bounds.y + 6.0),
                                color: StudioTheme::ACCENT_CYAN,
                                size: 10.5.into(),
                                ..Default::default()
                            });
                            let glyph_str = match kind {
                                ContactKind::NormallyOpen => "--| |--",
                                ContactKind::NormallyClosed => "--|/|--",
                                ContactKind::RisingEdge => "--|P|--",
                                ContactKind::FallingEdge => "--|N|--",
                            };
                            frame.fill_text(Text {
                                content: glyph_str.to_string(),
                                position: Point::new(elem.bounds.x + 12.0, elem.bounds.y + 24.0),
                                color: if elem.energized { StudioTheme::ACCENT_EMERALD } else { StudioTheme::TEXT_MUTED },
                                size: 12.0.into(),
                                font: iced::Font::MONOSPACE,
                                ..Default::default()
                            });
                        }
                    }
                    LdElementKey::InlineFb { index, branch_path, .. } => {
                        let fb_elem = Self::lookup_element(&rung.elements, branch_path, *index);
                        if let Some(LdElement::InlineFb { instance, .. }) = fb_elem {
                            frame.fill_text(Text {
                                content: format!("[FB: {instance}]"),
                                position: Point::new(elem.bounds.x + 8.0, elem.bounds.y + 14.0),
                                color: if elem.energized { StudioTheme::ACCENT_EMERALD } else { StudioTheme::ACCENT_CYAN },
                                size: 11.0.into(),
                                ..Default::default()
                            });
                        }
                    }
                    _ => {}
                }
            }

            // Draw parallel branch selection highlights (drawn over contacts)
            for branch in &rung_layout.branches {
                let is_branch_selected = state.selected_elements.contains(&branch.key) || state.selected == Some(branch.key.clone());
                if is_branch_selected {
                    let min_y = branch.start_y - 6.0;
                    let max_y = branch.paths_y.last().copied().unwrap_or(branch.start_y) + 6.0;
                    frame.stroke(
                        &Path::rectangle(
                            Point::new(branch.split_x - 4.0, min_y),
                            iced::Size::new((branch.merge_x - branch.split_x) + 8.0, (max_y - min_y) + 12.0),
                        ),
                        Stroke::default().with_color(StudioTheme::ACCENT_CYAN).with_width(1.5),
                    );
                }
            }

            // Draw vertical bus wire for stacked coils if multiple coils exist
            if rung_layout.coils.len() > 1 {
                let lowest_coil_wire_y = rung_layout
                    .coils
                    .last()
                    .map(|c| c.bounds.y + c.bounds.height / 2.0)
                    .unwrap_or(rung_layout.wire_y);
                let any_coil_energized = rung_layout.coils.iter().any(|c| c.energized);
                let vbus_color = if any_coil_energized {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    StudioTheme::BORDER_CARD
                };
                frame.stroke(
                    &Path::line(
                        Point::new(first_coil_x, rung_layout.wire_y),
                        Point::new(first_coil_x, lowest_coil_wire_y),
                    ),
                    Stroke::default().with_color(vbus_color).with_width(2.0),
                );
            }

            // Draw coils
            for coil_placed in &rung_layout.coils {
                let is_coil_selected = self.selected == Some(&coil_placed.key)
                    || self.selected_elements.contains(&coil_placed.key)
                    || state.selected_elements.contains(&coil_placed.key)
                    || state.selected == Some(coil_placed.key.clone());
                let border_color = if is_coil_selected {
                    StudioTheme::ACCENT_CYAN
                } else if coil_placed.energized {
                    StudioTheme::ACCENT_EMERALD
                } else {
                    StudioTheme::BORDER_CARD
                };
                let bg_color = if coil_placed.energized {
                    Color::from_rgba(0.063, 0.725, 0.506, 0.15)
                } else {
                    StudioTheme::BG_BASE
                };

                // Coil wire to coil card
                let coil_wire_y = coil_placed.bounds.y + coil_placed.bounds.height / 2.0;
                let feeder_color = if coil_placed.energized { StudioTheme::ACCENT_EMERALD } else { StudioTheme::BORDER_CARD };
                frame.stroke(
                    &Path::line(Point::new(first_coil_x, coil_wire_y), Point::new(coil_placed.bounds.x, coil_wire_y)),
                    Stroke::default().with_color(feeder_color).with_width(2.0),
                );

                // Coil card to Right Rail
                frame.stroke(
                    &Path::line(Point::new(coil_placed.bounds.x + coil_placed.bounds.width, coil_wire_y), Point::new(layout.rail_right_x, coil_wire_y)),
                    Stroke::default().with_color(StudioTheme::BORDER_CARD).with_width(2.0),
                );

                // Coil card box
                frame.fill_rectangle(
                    Point::new(coil_placed.bounds.x, coil_placed.bounds.y),
                    coil_placed.bounds.size(),
                    bg_color,
                );
                frame.stroke(
                    &Path::rectangle(Point::new(coil_placed.bounds.x, coil_placed.bounds.y), coil_placed.bounds.size()),
                    Stroke::default().with_color(border_color).with_width(if is_coil_selected { 2.0 } else { 1.0 }),
                );

                // Coil text & glyph
                if let LdElementKey::Coil { index, .. } = coil_placed.key
                    && let Some(coil) = rung.coils.get(index)
                {
                        frame.fill_text(Text {
                            content: coil.symbol.clone(),
                            position: Point::new(coil_placed.bounds.x + 6.0, coil_placed.bounds.y + 6.0),
                            color: Color::from_rgb(0.98, 0.75, 0.14),
                            size: 10.5.into(),
                            ..Default::default()
                        });
                        let glyph_str = match coil.kind {
                            CoilKind::Normal => "--( )--",
                            CoilKind::Set => "--[S]--",
                            CoilKind::Reset => "--[R]--",
                        };
                        frame.fill_text(Text {
                            content: glyph_str.to_string(),
                            position: Point::new(coil_placed.bounds.x + 16.0, coil_placed.bounds.y + 24.0),
                            color: if coil_placed.energized { StudioTheme::ACCENT_EMERALD } else { StudioTheme::TEXT_MUTED },
                            size: 12.0.into(),
                            font: iced::Font::MONOSPACE,
                            ..Default::default()
                        });
                    }
            }
        }

        // Draw drop target preview indicator if dragging
        if let Some(target) = &state.drop_hover {
            match target {
                LdDropTarget::BeforeElement { rung, branch_path, index } => {
                    if let Some(r) = layout.rungs.get(*rung)
                        && let Some(elem) = r.elements.iter().find(|e| matches!(&e.key, LdElementKey::Contact { branch_path: bp, index: idx, .. } if bp == branch_path && idx == index))
                    {
                        let bar_x = elem.bounds.x - 4.0;
                        frame.stroke(
                            &Path::line(Point::new(bar_x, elem.bounds.y - 4.0), Point::new(bar_x, elem.bounds.y + elem.bounds.height + 4.0)),
                            Stroke::default().with_color(StudioTheme::ACCENT_AMBER).with_width(3.0),
                        );
                    }
                }
                LdDropTarget::AfterElement { rung, branch_path, index } => {
                    if let Some(r) = layout.rungs.get(*rung)
                        && let Some(elem) = r.elements.iter().find(|e| matches!(&e.key, LdElementKey::Contact { branch_path: bp, index: idx, .. } if bp == branch_path && idx == index))
                    {
                        let bar_x = elem.bounds.x + elem.bounds.width + 4.0;
                        frame.stroke(
                            &Path::line(Point::new(bar_x, elem.bounds.y - 4.0), Point::new(bar_x, elem.bounds.y + elem.bounds.height + 4.0)),
                            Stroke::default().with_color(StudioTheme::ACCENT_AMBER).with_width(3.0),
                        );
                    }
                }
                LdDropTarget::NewBranch { rung, branch_path, index } => {
                    if let Some(r) = layout.rungs.get(*rung)
                        && let Some(elem) = r.elements.iter().find(|e| matches!(&e.key, LdElementKey::Contact { branch_path: bp, index: idx, .. } if bp == branch_path && idx == index))
                    {
                        let pad = 6.0;
                        let box_rect = Rectangle {
                            x: elem.bounds.x - pad,
                            y: elem.bounds.y - pad,
                            width: elem.bounds.width + pad * 2.0,
                            height: elem.bounds.height + pad * 2.0,
                        };
                        frame.fill_rectangle(Point::new(box_rect.x, box_rect.y), box_rect.size(), Color::from_rgba(0.96, 0.62, 0.04, 0.15));
                        frame.stroke(
                            &Path::rectangle(Point::new(box_rect.x, box_rect.y), box_rect.size()),
                            Stroke::default().with_color(StudioTheme::ACCENT_AMBER).with_width(2.0),
                        );
                    }
                }
                LdDropTarget::CoilSlot { rung, .. } => {
                    if let Some(r) = layout.rungs.get(*rung) {
                        let next_y = r.coils.last().map(|c| c.bounds.y + c.bounds.height + 8.0).unwrap_or(r.wire_y - COIL_CARD_HEIGHT / 2.0);
                        let coil_box = Rectangle {
                            x: layout.rail_right_x - WIRE_PAD - COIL_CARD_WIDTH,
                            y: next_y,
                            width: COIL_CARD_WIDTH,
                            height: COIL_CARD_HEIGHT,
                        };
                        frame.fill_rectangle(Point::new(coil_box.x, coil_box.y), coil_box.size(), Color::from_rgba(0.96, 0.62, 0.04, 0.15));
                        frame.stroke(
                            &Path::rectangle(Point::new(coil_box.x, coil_box.y), coil_box.size()),
                            Stroke::default().with_color(StudioTheme::ACCENT_AMBER).with_width(2.0),
                        );
                    }
                }
                LdDropTarget::NewRung { index } => {
                    let y = if *index < layout.rungs.len() {
                        layout.rungs[*index].bounds.y - RUNG_GAP_Y / 2.0
                    } else if let Some(last) = layout.rungs.last() {
                        last.bounds.y + last.bounds.height + RUNG_GAP_Y / 2.0
                    } else {
                        50.0
                    };
                    frame.stroke(
                        &Path::line(Point::new(layout.rail_left_x - 40.0, y), Point::new(layout.rail_right_x + 40.0, y)),
                        Stroke::default().with_color(StudioTheme::ACCENT_AMBER).with_width(3.0),
                    );
                }
            }
        }

        // Draw Marquee selection box if active
        if let Some((p1, p2)) = state.drag_marquee {
            let min_x = p1.x.min(p2.x);
            let min_y = p1.y.min(p2.y);
            let w = (p1.x - p2.x).abs();
            let h = (p1.y - p2.y).abs();
            frame.fill_rectangle(
                Point::new(min_x, min_y),
                iced::Size::new(w, h),
                Color::from_rgba(0.22, 0.74, 0.97, 0.15),
            );
            frame.stroke(
                &Path::rectangle(Point::new(min_x, min_y), iced::Size::new(w, h)),
                Stroke::default().with_color(StudioTheme::ACCENT_CYAN).with_width(1.0),
            );
        }

        vec![frame.into_geometry()]
    }
}

pub struct LdLayoutEngine;

impl LdLayoutEngine {
    pub fn build(program: &LdProgram, runtime: Option<&PlcRuntime>) -> LdLayoutResult {
        Self::compute_layout(program, runtime)
    }

    pub fn calculate_branch_depth(elements: &[LdElement]) -> usize {
        elements
            .iter()
            .map(|e| match e {
                LdElement::Branch { paths } => {
                    let sub_depth: usize = paths.iter().map(|p| Self::calculate_branch_depth(p)).sum();
                    sub_depth.max(paths.len()).max(2)
                }
                _ => 1,
            })
            .max()
            .unwrap_or(1)
    }

    pub fn calculate_horizontal_slots(elements: &[LdElement]) -> usize {
        elements
            .iter()
            .map(|e| match e {
                LdElement::Branch { paths } => {
                    paths
                        .iter()
                        .map(|p| Self::calculate_horizontal_slots(p))
                        .max()
                        .unwrap_or(1)
                }
                _ => 1,
            })
            .sum()
    }

    pub fn compute_layout(program: &LdProgram, runtime: Option<&PlcRuntime>) -> LdLayoutResult {
        // Pass 1: compute max contact horizontal slots across all rungs
        let max_slots = program
            .rungs
            .iter()
            .map(|r| Self::calculate_horizontal_slots(&r.elements))
            .max()
            .unwrap_or(1)
            .max(2);

        let rail_left_x = RAIL_LEFT_X;
        let rail_right_x = (rail_left_x + WIRE_PAD + max_slots as f32 * SLOT_WIDTH + WIRE_PAD + COIL_CARD_WIDTH + WIRE_PAD).max(680.0);

        let mut rungs = Vec::new();
        let mut curr_y = 40.0_f32;

        for (r_idx, rung) in program.rungs.iter().enumerate() {
            let branch_depth = Self::calculate_branch_depth(&rung.elements);
            let coil_stack = rung.coils.len().max(1);
            let effective_depth = branch_depth.max(coil_stack);

            let rung_height = RUNG_HEADER_HEIGHT
                + SLOT_HEIGHT
                + (effective_depth.saturating_sub(1)) as f32 * BRANCH_VERTICAL_STEP
                + RUNG_GAP_Y;

            let rung_top_y = curr_y;
            let wire_y = rung_top_y + RUNG_HEADER_HEIGHT + SLOT_HEIGHT / 2.0;

            let mut placed_elements = Vec::new();
            let mut placed_branches = Vec::new();

            // Place contacts recursively
            let mut power_flowing = true;
            let start_x = rail_left_x + WIRE_PAD;

            Self::layout_elements_recursive(
                &rung.elements,
                r_idx,
                &[],
                start_x,
                wire_y,
                runtime,
                &mut power_flowing,
                &mut placed_elements,
                &mut placed_branches,
            );

            // Place coils stacked before right rail
            let coil_x = rail_right_x - WIRE_PAD - COIL_CARD_WIDTH;
            let mut placed_coils = Vec::new();
            for (c_idx, coil) in rung.coils.iter().enumerate() {
                let coil_y = if rung.coils.len() == 1 {
                    wire_y - SLOT_HEIGHT / 2.0
                } else {
                    wire_y + c_idx as f32 * BRANCH_VERTICAL_STEP - SLOT_HEIGHT / 2.0
                };
                let is_coil_on = power_flowing || runtime.is_some_and(|rt| is_symbol_energized(rt, &coil.symbol));
                placed_coils.push(LdPlacedElement {
                    key: LdElementKey::Coil { rung: r_idx, index: c_idx },
                    bounds: Rectangle {
                        x: coil_x,
                        y: coil_y,
                        width: COIL_CARD_WIDTH,
                        height: SLOT_HEIGHT,
                    },
                    energized: is_coil_on,
                });
            }

            let rung_bounds = Rectangle {
                x: rail_left_x - 20.0,
                y: rung_top_y,
                width: (rail_right_x - rail_left_x) + 40.0,
                height: rung_height,
            };

            rungs.push(LdPlacedRung {
                rung_idx: r_idx,
                bounds: rung_bounds,
                wire_y,
                elements: placed_elements,
                branches: placed_branches,
                coils: placed_coils,
                power_flowing,
            });

            curr_y += rung_height;
        }

        let total_bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: rail_right_x + 80.0,
            height: curr_y + 60.0,
        };

        LdLayoutResult {
            rail_left_x,
            rail_right_x,
            rungs,
            total_bounds,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_elements_recursive(
        elements: &[LdElement],
        rung_idx: usize,
        current_path: &[LdBranchPathSegment],
        start_x: f32,
        base_y: f32,
        runtime: Option<&PlcRuntime>,
        power_flowing: &mut bool,
        placed_elements: &mut Vec<LdPlacedElement>,
        placed_branches: &mut Vec<LdPlacedBranch>,
    ) -> f32 {
        let mut curr_x = start_x;

        for (e_idx, elem) in elements.iter().enumerate() {
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
                    let is_active = *power_flowing && contact_closed;
                    *power_flowing = is_active;

                    let bounds = Rectangle {
                        x: curr_x,
                        y: base_y - SLOT_HEIGHT / 2.0,
                        width: SLOT_WIDTH - 8.0,
                        height: SLOT_HEIGHT,
                    };
                    placed_elements.push(LdPlacedElement {
                        key: LdElementKey::Contact {
                            rung: rung_idx,
                            branch_path: current_path.to_vec(),
                            index: e_idx,
                        },
                        bounds,
                        energized: is_active,
                    });
                    curr_x += SLOT_WIDTH;
                }
                LdElement::InlineFb { instance, .. } => {
                    let fb_q = runtime.is_some_and(|rt| rt.tags.get_fb(instance).map(|fb| fb.q).unwrap_or(false));
                    let is_active = *power_flowing && fb_q;
                    *power_flowing = is_active;

                    let bounds = Rectangle {
                        x: curr_x,
                        y: base_y - SLOT_HEIGHT / 2.0,
                        width: SLOT_WIDTH - 8.0,
                        height: SLOT_HEIGHT,
                    };
                    placed_elements.push(LdPlacedElement {
                        key: LdElementKey::InlineFb {
                            rung: rung_idx,
                            branch_path: current_path.to_vec(),
                            index: e_idx,
                        },
                        bounds,
                        energized: is_active,
                    });
                    curr_x += SLOT_WIDTH;
                }
                LdElement::Branch { paths } => {
                    let split_x = curr_x;
                    let mut max_branch_x = curr_x;
                    let mut paths_y = Vec::new();
                    let initial_power = *power_flowing;
                    let mut any_path_energized = false;

                    for (p_idx, path) in paths.iter().enumerate() {
                        let path_y = base_y + p_idx as f32 * BRANCH_VERTICAL_STEP;
                        paths_y.push(path_y);

                        let mut next_path = current_path.to_vec();
                        next_path.push(LdBranchPathSegment {
                            branch_idx: e_idx,
                            path_idx: p_idx,
                        });

                        let mut path_power = initial_power;
                        let end_x = Self::layout_elements_recursive(
                            path,
                            rung_idx,
                            &next_path,
                            split_x + WIRE_PAD,
                            path_y,
                            runtime,
                            &mut path_power,
                            placed_elements,
                            placed_branches,
                        );
                        if path_power {
                            any_path_energized = true;
                        }
                        max_branch_x = max_branch_x.max(end_x);
                    }

                    let merge_x = max_branch_x + WIRE_PAD;
                    *power_flowing = initial_power && any_path_energized;

                    placed_branches.push(LdPlacedBranch {
                        key: LdElementKey::Branch {
                            rung: rung_idx,
                            branch_path: current_path.to_vec(),
                            index: e_idx,
                        },
                        split_x,
                        merge_x,
                        start_y: base_y,
                        paths_y,
                        energized: *power_flowing,
                    });

                    curr_x = merge_x;
                }
            }
        }

        curr_x
    }
}

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

pub fn render_ld_editor<'a>(
    program: &'a LdProgram,
    runtime: Option<&'a PlcRuntime>,
    selected: Option<&'a LdElementKey>,
    selected_elements: &'a [LdElementKey],
    lang: UiLanguage,
    initial_pan: Vector,
    initial_zoom: f32,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();
    // 1. Determine selected rung index (defaults to 0 or selected element's rung)
    let selected_rung = match selected {
        Some(LdElementKey::Rung(r))
        | Some(LdElementKey::Contact { rung: r, .. })
        | Some(LdElementKey::Coil { rung: r, .. })
        | Some(LdElementKey::Branch { rung: r, .. })
        | Some(LdElementKey::InlineFb { rung: r, .. }) => *r,
        None => selected_elements
            .iter()
            .map(|k| match k {
                LdElementKey::Rung(r)
                | LdElementKey::Contact { rung: r, .. }
                | LdElementKey::Coil { rung: r, .. }
                | LdElementKey::Branch { rung: r, .. }
                | LdElementKey::InlineFb { rung: r, .. } => *r,
            })
            .next()
            .unwrap_or(0),
    };

    let (selected_branch_path, selected_elem_idx) = match selected {
        Some(LdElementKey::Contact { branch_path, index, .. })
        | Some(LdElementKey::InlineFb { branch_path, index, .. }) => {
            (branch_path.clone(), Some(*index))
        }
        Some(LdElementKey::Branch {
            branch_path,
            index,
            ..
        }) => {
            let mut bp = branch_path.clone();
            bp.push(LdBranchPathSegment {
                branch_idx: *index,
                path_idx: 0,
            });
            (bp, None)
        }
        _ => (Vec::new(), None),
    };

    // 2. Top Toolbar Header
    let top_toolbar: Element<'a, Message> = if selected_elements.len() > 1 {
        let count_label = text(format!("{} items selected", selected_elements.len()))
            .size(12)
            .font(iced::Font::MONOSPACE)
            .color(StudioTheme::ACCENT_CYAN);

        let delete_btn = button(text("Delete Selected (Del)").size(11.5))
            .on_press(Message::LdRequestDeleteSelected(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([4, 10]);

        let deselect_btn = button(text("Deselect [x]").size(11))
            .on_press(Message::LdSelectElements(pou_name.clone(), Vec::new()))
            .style(StudioTheme::ghost_button)
            .padding([3, 8]);

        let multi_row = row![
            count_label,
            delete_btn,
            iced::widget::Space::with_width(Length::Fill),
            deselect_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let scrollable_multi = scrollable(multi_row).direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new()
                .width(4)
                .scroller_width(4)
                .margin(1),
        ));

        container(scrollable_multi)
            .width(Length::Fill)
            .padding([4, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.2, 0.4, 0.6, 0.15,
            )))
            .into()
    } else {
        let title = text(format!(
            "{} — {}",
            lang.tr("Ladder Diagram", "Diagramme à Contacts (Ladder)"),
            program.name
        ))
        .size(15);

        let add_rung_btn = button(text(lang.tr("+ Rung", "+ Réseau")).size(11))
            .on_press(Message::LdAddRung(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([4, 8]);

        let add_no_btn = button(text("+ NO [ ]").size(11))
            .on_press(Message::LdAddContactWithKind(
                pou_name.clone(),
                selected_rung,
                ContactKind::NormallyOpen,
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]);

        let add_nc_btn = button(text("+ NC [/]").size(11))
            .on_press(Message::LdAddContactWithKind(
                pou_name.clone(),
                selected_rung,
                ContactKind::NormallyClosed,
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]);

        let add_p_btn = button(text("+ |P|").size(11))
            .on_press(Message::LdAddContactWithKind(
                pou_name.clone(),
                selected_rung,
                ContactKind::RisingEdge,
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]);

        let add_n_btn = button(text("+ |N|").size(11))
            .on_press(Message::LdAddContactWithKind(
                pou_name.clone(),
                selected_rung,
                ContactKind::FallingEdge,
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]);

        let add_coil_btn = button(text("+ Coil ( )").size(11))
            .on_press(Message::LdAddCoil(pou_name.clone(), selected_rung))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]);

        let add_branch_btn = button(text("+ Branch").size(11))
            .on_press(Message::LdAddParallelBranch(
                pou_name.clone(),
                selected_rung,
                selected_branch_path,
                selected_elem_idx,
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]);

        let clear_btn = button(text(lang.tr("Clear", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([4, 8]);

        let toolbar_row = row![
            title,
            iced::widget::Space::with_width(Length::Fixed(12.0)),
            add_rung_btn,
            add_no_btn,
            add_nc_btn,
            add_p_btn,
            add_n_btn,
            add_coil_btn,
            add_branch_btn,
            iced::widget::Space::with_width(Length::Fixed(12.0)),
            clear_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let scrollable_toolbar = scrollable(toolbar_row).direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new()
                .width(4)
                .scroller_width(4)
                .margin(1),
        ));

        container(scrollable_toolbar)
            .width(Length::Fill)
            .padding([4, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.2, 0.4, 0.6, 0.15,
            )))
            .into()
    };

    // 3. Vector Canvas Widget
    let canvas_widget = iced::widget::canvas::Canvas::new(LdCanvasProgram {
        pou_name: &program.name,
        program,
        runtime,
        selected,
        selected_elements,
        lang,
        initial_pan,
        initial_zoom,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    // 4. Bottom Inspector Context Bar
    let inspector_bar: Element<'a, Message> = match selected {
        Some(LdElementKey::Contact {
            rung: r_idx,
            branch_path,
            index,
        }) => {
            let p_name = pou_name.clone();
            let b_path = branch_path.clone();
            let idx = *index;
            let r_i = *r_idx;

            let contact_data = program
                .rungs
                .get(r_i)
                .and_then(|r| LdCanvasProgram::lookup_element(&r.elements, &b_path, idx));

            if let Some(LdElement::Contact { symbol, kind }) = contact_data {
                let b_for_sym = b_path.clone();
                let tag_input = text_input("Tag...", symbol)
                    .on_input(move |s| {
                        Message::LdContactSymbolChanged(p_name.clone(), r_i, b_for_sym.clone(), idx, s)
                    })
                    .width(Length::Fixed(160.0))
                    .padding([3, 7])
                    .style(StudioTheme::styled_input);

                let p_for_kind = pou_name.clone();
                let b_for_kind = b_path.clone();
                let kind_picker = pick_list(
                    &ALL_CONTACT_KINDS[..],
                    Some(LdContactKindWrapper(*kind)),
                    move |w| Message::LdContactKindChanged(p_for_kind.clone(), r_i, b_for_kind.clone(), idx, w.0),
                )
                .width(Length::Fixed(80.0))
                .text_size(11.0)
                .padding([3, 7]);

                let p_for_del = pou_name.clone();
                let del_btn = button(text("Delete (Del)").size(11))
                    .on_press(Message::LdRequestDeleteSelected(p_for_del))
                    .style(StudioTheme::danger_button)
                    .padding([3, 8]);

                let deselect_btn = button(text("Close [x]").size(11))
                    .on_press(Message::LdSelectElement(pou_name, None))
                    .style(StudioTheme::ghost_button)
                    .padding([3, 8]);

                container(
                    row![
                        text("Contact:").size(12).color(StudioTheme::ACCENT_CYAN),
                        tag_input,
                        kind_picker,
                        del_btn,
                        iced::widget::Space::with_width(Length::Fill),
                        deselect_btn,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(StudioTheme::card_style)
                .width(Length::Fill)
                .into()
            } else {
                iced::widget::Space::with_height(Length::Shrink).into()
            }
        }
        Some(LdElementKey::Coil {
            rung: r_idx,
            index,
        }) => {
            let p_name = pou_name.clone();
            let idx = *index;
            let r_i = *r_idx;

            let coil_data = program.rungs.get(r_i).and_then(|r| r.coils.get(idx));

            if let Some(coil) = coil_data {
                let tag_input = text_input("Tag...", &coil.symbol)
                    .on_input(move |s| Message::LdCoilSymbolChanged(p_name.clone(), r_i, idx, s))
                    .width(Length::Fixed(160.0))
                    .padding([3, 7])
                    .style(StudioTheme::styled_input);

                let p_for_kind = pou_name.clone();
                let kind_picker = pick_list(
                    &ALL_COIL_KINDS[..],
                    Some(LdCoilKindWrapper(coil.kind)),
                    move |w| Message::LdCoilKindChanged(p_for_kind.clone(), r_i, idx, w.0),
                )
                .width(Length::Fixed(80.0))
                .text_size(11.0)
                .padding([3, 7]);

                let p_for_del = pou_name.clone();
                let del_btn = button(text("Delete (Del)").size(11))
                    .on_press(Message::LdRequestDeleteSelected(p_for_del))
                    .style(StudioTheme::danger_button)
                    .padding([3, 8]);

                let deselect_btn = button(text("Close [x]").size(11))
                    .on_press(Message::LdSelectElement(pou_name, None))
                    .style(StudioTheme::ghost_button)
                    .padding([3, 8]);

                container(
                    row![
                        text("Coil:").size(12).color(Color::from_rgb(0.98, 0.75, 0.14)),
                        tag_input,
                        kind_picker,
                        del_btn,
                        iced::widget::Space::with_width(Length::Fill),
                        deselect_btn,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(StudioTheme::card_style)
                .width(Length::Fill)
                .into()
            } else {
                iced::widget::Space::with_height(Length::Shrink).into()
            }
        }
        Some(LdElementKey::Rung(r_idx)) => {
            let p_name = pou_name.clone();
            let r_i = *r_idx;

            if let Some(rung) = program.rungs.get(r_i) {
                let p_for_name = p_name.clone();
                let name_input = text_input("Rung name (e.g. Safety_Stop)", &rung.id)
                    .on_input(move |s| Message::LdRungNameChanged(p_for_name.clone(), r_i, s))
                    .width(180)
                    .padding([3, 7])
                    .style(StudioTheme::styled_input);

                let comment_input = text_input("Rung comment...", &rung.comment)
                    .on_input(move |s| Message::LdRungCommentChanged(p_name.clone(), r_i, s))
                    .width(Length::Fill)
                    .padding([3, 7])
                    .style(StudioTheme::styled_input);

                let p_for_del = pou_name.clone();
                let del_btn = button(text("Delete Rung").size(11))
                    .on_press(Message::LdRemoveRung(p_for_del, r_i))
                    .style(StudioTheme::danger_button)
                    .padding([3, 8]);

                let deselect_btn = button(text("Close [x]").size(11))
                    .on_press(Message::LdSelectElement(pou_name, None))
                    .style(StudioTheme::ghost_button)
                    .padding([3, 8]);

                container(
                    row![
                        text(format!("Rung {r_i}:"))
                            .size(12)
                            .color(StudioTheme::ACCENT_CYAN),
                        text("Name:").size(11).color(StudioTheme::TEXT_MUTED),
                        name_input,
                        text("Comment:").size(11).color(StudioTheme::TEXT_MUTED),
                        comment_input,
                        del_btn,
                        deselect_btn,
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(StudioTheme::card_style)
                .width(Length::Fill)
                .into()
            } else {
                iced::widget::Space::with_height(Length::Shrink).into()
            }
        }
        _ => {
            let tip = text(lang.tr(
                "Tip: Click element to inspect, drag on empty canvas to marquee select, scroll wheel to zoom.",
                "Astuce : Cliquez pour inspecter, glissez pour sélectionner en rectangle, molette pour zoomer.",
            ))
            .size(11.0)
            .color(StudioTheme::TEXT_MUTED);

            container(tip)
                .padding([4, 12])
                .style(StudioTheme::canvas_container_style)
                .width(Length::Fill)
                .into()
        }
    };

    column![top_toolbar, canvas_widget, inspector_bar]
        .spacing(6)
        .padding(8)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use ferrite_lang_ld::{LdCoil, LdRung};
    use iced::widget::canvas::Program;

    #[test]
    fn test_ld_layout_straight_rung_geometry() {
        let program = LdProgram {
            name: "test_straight".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Rung 0".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Start_Btn".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Safety_OK".to_string(),
                        kind: ContactKind::NormallyClosed,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Motor_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let layout = LdLayoutEngine::compute_layout(&program, None);
        assert_eq!(layout.rungs.len(), 1);
        let r0 = &layout.rungs[0];
        assert_eq!(r0.elements.len(), 2);
        assert_eq!(r0.coils.len(), 1);
        assert!(layout.rail_left_x < layout.rail_right_x);
        assert!(r0.elements[0].bounds.x < r0.elements[1].bounds.x);
        assert!(r0.elements[1].bounds.x < r0.coils[0].bounds.x);
        assert!(r0.coils[0].bounds.x < layout.rail_right_x);
    }

    #[test]
    fn test_ld_layout_parallel_branch_geometry() {
        let program = LdProgram {
            name: "test_branch".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Parallel branch".to_string(),
                elements: vec![LdElement::Branch {
                    paths: vec![
                        vec![LdElement::Contact {
                            symbol: "Start_PB".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                        vec![LdElement::Contact {
                            symbol: "Motor_Aux".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                    ],
                }],
                coils: vec![LdCoil {
                    symbol: "Motor_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let layout = LdLayoutEngine::compute_layout(&program, None);
        let r0 = &layout.rungs[0];
        assert_eq!(r0.branches.len(), 1);
        let b0 = &r0.branches[0];
        assert_eq!(b0.paths_y.len(), 2);
        assert!(b0.paths_y[0] < b0.paths_y[1]); // Step down for second leg
        assert!(b0.split_x < b0.merge_x);
    }

    #[test]
    fn test_ld_layout_multiple_coils_stack_height() {
        let program_single = LdProgram {
            name: "test_single_coil".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![],
                coils: vec![LdCoil {
                    symbol: "C1".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let program_multi = LdProgram {
            name: "test_multi_coil".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![],
                coils: vec![
                    LdCoil {
                        symbol: "C1".to_string(),
                        kind: CoilKind::Normal,
                    },
                    LdCoil {
                        symbol: "C2".to_string(),
                        kind: CoilKind::Normal,
                    },
                    LdCoil {
                        symbol: "C3".to_string(),
                        kind: CoilKind::Normal,
                    },
                ],
            }],
        };

        let l_single = LdLayoutEngine::compute_layout(&program_single, None);
        let l_multi = LdLayoutEngine::compute_layout(&program_multi, None);
        assert!(l_multi.rungs[0].bounds.height > l_single.rungs[0].bounds.height);
        assert_eq!(l_multi.rungs[0].coils.len(), 3);
        assert!(l_multi.rungs[0].coils[0].bounds.y < l_multi.rungs[0].coils[1].bounds.y);
        assert!(l_multi.rungs[0].coils[1].bounds.y < l_multi.rungs[0].coils[2].bounds.y);
    }

    #[test]
    fn test_ld_canvas_draw_without_panic() {
        let program = LdProgram {
            name: "test_draw".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Rung 0".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "PB1".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Branch {
                        paths: vec![
                            vec![LdElement::Contact {
                                symbol: "B1".to_string(),
                                kind: ContactKind::NormallyClosed,
                            }],
                            vec![LdElement::Contact {
                                symbol: "B2".to_string(),
                                kind: ContactKind::RisingEdge,
                            }],
                        ],
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Out1".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let canvas_prog = LdCanvasProgram {
            pou_name: "test_draw",
            program: &program,
            runtime: None,
            selected: None,
            selected_elements: &[],
            lang: UiLanguage::En,
            initial_pan: Vector::default(),
            initial_zoom: 1.0,
        };
        assert_eq!(canvas_prog.program.name, "test_draw");

        let _elem = render_ld_editor(&program, None, None, &[], UiLanguage::En, Vector::default(), 1.0);
    }

    #[test]
    fn test_ld_canvas_update_mouse_and_keyboard() {
        let program = LdProgram {
            name: "test_update".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Rung 0".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Start_Btn".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Pump_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let mut state = LdCanvasState::default();
        let canvas = LdCanvasProgram {
            pou_name: "test_update",
            program: &program,
            runtime: None,
            selected: None,
            selected_elements: &[],
            lang: UiLanguage::En,
            initial_pan: Vector::default(),
            initial_zoom: 1.0,
        };

        let bounds = Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(1000.0, 600.0));
        let layout = LdLayoutEngine::build(&program, None);
        let contact_pt = Point::new(
            layout.rungs[0].elements[0].bounds.x + 10.0,
            layout.rungs[0].elements[0].bounds.y + 10.0,
        );

        // 1. Click on contact
        let (status, msg) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            bounds,
            mouse::Cursor::Available(contact_pt),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert!(matches!(msg, Some(Message::LdSelectElements(ref p, ref keys)) if p == "test_update" && keys.len() == 1));

        // 2. Wheel scroll zoom
        let (status, zoom_msg) = canvas.update(
            &mut state,
            canvas::Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 100.0)),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert!(matches!(zoom_msg, Some(Message::LdCanvasZoomChanged(ref p, z, _)) if p == "test_update" && z > 1.0));

        // 3. Delete key
        let (status, del_msg) = canvas.update(
            &mut state,
            canvas::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete),
                location: iced::keyboard::Location::Standard,
                modifiers: iced::keyboard::Modifiers::default(),
                text: None,
                modified_key: iced::keyboard::key::Key::Named(iced::keyboard::key::Named::Delete),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Delete),
            }),
            bounds,
            mouse::Cursor::Available(Point::new(100.0, 100.0)),
        );
        assert_eq!(status, iced::widget::canvas::event::Status::Captured);
        assert!(matches!(del_msg, Some(Message::LdRequestDeleteSelected(ref p)) if p == "test_update"));
    }

    #[test]
    fn test_ld_hit_test_elements() {
        let program = LdProgram {
            name: "test_hit".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Rung 0".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Start_Btn".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Stop_Btn".to_string(),
                        kind: ContactKind::NormallyClosed,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Pump_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let layout = LdLayoutEngine::build(&program, None);
        assert_eq!(layout.rungs[0].elements.len(), 2);
        assert_eq!(layout.rungs[0].coils.len(), 1);

        let c0_center = Point::new(
            layout.rungs[0].elements[0].bounds.x + 20.0,
            layout.rungs[0].elements[0].bounds.y + 15.0,
        );
        let hit0 = layout.hit_test(c0_center);
        assert_eq!(
            hit0,
            Some(LdElementKey::Contact {
                rung: 0,
                branch_path: vec![],
                index: 0,
            })
        );

        let coil_center = Point::new(
            layout.rungs[0].coils[0].bounds.x + 20.0,
            layout.rungs[0].coils[0].bounds.y + 15.0,
        );
        let hit_coil = layout.hit_test(coil_center);
        assert_eq!(hit_coil, Some(LdElementKey::Coil { rung: 0, index: 0 }));

        let hit_empty = layout.hit_test(Point::new(10.0, 10.0));
        assert_eq!(hit_empty, None);
    }

    #[test]
    fn test_ld_marquee_selection_intersection() {
        let program = LdProgram {
            name: "test_marquee".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Rung 0".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Start_Btn".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Stop_Btn".to_string(),
                        kind: ContactKind::NormallyClosed,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Pump_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let layout = LdLayoutEngine::build(&program, None);

        let c0_x = layout.rungs[0].elements[0].bounds.x;
        let c1_right = layout.rungs[0].elements[1].bounds.x + layout.rungs[0].elements[1].bounds.width;
        let c_top = layout.rungs[0].elements[0].bounds.y;
        let c_bot = layout.rungs[0].elements[0].bounds.y + layout.rungs[0].elements[0].bounds.height;

        let marquee_box = Rectangle::new(
            Point::new(c0_x - 5.0, c_top - 5.0),
            iced::Size::new((c1_right - c0_x) + 10.0, (c_bot - c_top) + 10.0),
        );

        let selected = layout.elements_in_rect(marquee_box);
        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&LdElementKey::Contact {
            rung: 0,
            branch_path: vec![],
            index: 0
        }));
        assert!(selected.contains(&LdElementKey::Contact {
            rung: 0,
            branch_path: vec![],
            index: 1
        }));
        assert!(!selected.contains(&LdElementKey::Coil { rung: 0, index: 0 }));
    }

    #[test]
    fn test_ld_move_contact_to_different_slot() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_move_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_move".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Start".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Stop".to_string(),
                        kind: ContactKind::NormallyClosed,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Motor".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        app.project
            .pous
            .insert("test_move".to_string(), ferrite_project::PouSource::Ld(ld_prog));

        app.perform_ld_move(
            "test_move",
            &[LdElementKey::Contact {
                rung: 0,
                branch_path: vec![],
                index: 1,
            }],
            LdDropTarget::BeforeElement {
                rung: 0,
                branch_path: vec![],
                index: 0,
            },
        );

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_move") else {
            panic!("POU not found");
        };

        assert_eq!(ld.rungs[0].elements.len(), 2);
        if let LdElement::Contact { symbol, .. } = &ld.rungs[0].elements[0] {
            assert_eq!(symbol, "Stop");
        } else {
            panic!("Expected Contact");
        }
        if let LdElement::Contact { symbol, .. } = &ld.rungs[0].elements[1] {
            assert_eq!(symbol, "Start");
        } else {
            panic!("Expected Contact");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_add_parallel_branch_wraps_contacts() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_branch_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_branch_wrap".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::Contact {
                    symbol: "Main_Contact".to_string(),
                    kind: ContactKind::NormallyOpen,
                }],
                coils: vec![],
            }],
        };

        app.project.pous.insert(
            "test_branch_wrap".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        app.perform_ld_add_parallel_branch("test_branch_wrap", 0, &[], None);

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_branch_wrap") else {
            panic!("POU not found");
        };

        assert_eq!(ld.rungs[0].elements.len(), 1);
        match &ld.rungs[0].elements[0] {
            LdElement::Branch { paths } => {
                assert_eq!(paths.len(), 2);
                assert_eq!(paths[0].len(), 1);
                assert_eq!(paths[1].len(), 1);
            }
            _ => panic!("Expected Branch"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_branch_dissolution_on_last_branch_drop() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_dissolve_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_dissolve".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::Branch {
                    paths: vec![
                        vec![LdElement::Contact {
                            symbol: "Leg_A".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                        vec![LdElement::Contact {
                            symbol: "Leg_B".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                    ],
                }],
                coils: vec![],
            }],
        };

        app.project.pous.insert(
            "test_dissolve".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        app.perform_ld_delete(
            "test_dissolve",
            &[LdElementKey::Contact {
                rung: 0,
                branch_path: vec![LdBranchPathSegment {
                    branch_idx: 0,
                    path_idx: 1,
                }],
                index: 0,
            }],
        );

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_dissolve") else {
            panic!("POU not found");
        };

        assert_eq!(ld.rungs[0].elements.len(), 1);
        match &ld.rungs[0].elements[0] {
            LdElement::Contact { symbol, .. } => {
                assert_eq!(symbol, "Leg_A");
            }
            _ => panic!("Expected branch to dissolve into single contact"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_batch_delete_selected_elements() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_batch_del_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_batch_del".to_string(),
            comment: "".to_string(),
            rungs: vec![
                LdRung {
                    id: "R0".to_string(),
                    comment: "".to_string(),
                    elements: vec![
                        LdElement::Contact {
                            symbol: "C0".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                        LdElement::Contact {
                            symbol: "C1".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                    ],
                    coils: vec![LdCoil {
                        symbol: "Out0".to_string(),
                        kind: CoilKind::Normal,
                    }],
                },
                LdRung {
                    id: "R1".to_string(),
                    comment: "".to_string(),
                    elements: vec![LdElement::Contact {
                        symbol: "C2".to_string(),
                        kind: ContactKind::NormallyOpen,
                    }],
                    coils: vec![
                        LdCoil {
                            symbol: "Out1".to_string(),
                            kind: CoilKind::Normal,
                        },
                        LdCoil {
                            symbol: "Out2".to_string(),
                            kind: CoilKind::Set,
                        },
                    ],
                },
            ],
        };

        app.project.pous.insert(
            "test_batch_del".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        let to_delete = vec![
            LdElementKey::Contact {
                rung: 0,
                branch_path: vec![],
                index: 1,
            },
            LdElementKey::Contact {
                rung: 1,
                branch_path: vec![],
                index: 0,
            },
            LdElementKey::Coil { rung: 1, index: 0 },
        ];

        app.perform_ld_delete("test_batch_del", &to_delete);

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_batch_del") else {
            panic!("POU not found");
        };

        assert_eq!(ld.rungs[0].elements.len(), 1);
        if let LdElement::Contact { symbol, .. } = &ld.rungs[0].elements[0] {
            assert_eq!(symbol, "C0");
        }
        assert_eq!(ld.rungs[0].coils.len(), 1);

        assert_eq!(ld.rungs[1].elements.len(), 0);
        assert_eq!(ld.rungs[1].coils.len(), 1);
        assert_eq!(ld.rungs[1].coils[0].symbol, "Out2");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_batch_delete_rung_and_subsequent_elements() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_del_rung_elem_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_del_rung_elem".to_string(),
            comment: "".to_string(),
            rungs: vec![
                LdRung {
                    id: "R0".to_string(),
                    comment: "Rung 0 to delete".to_string(),
                    elements: vec![LdElement::Contact {
                        symbol: "OldR0_C0".to_string(),
                        kind: ContactKind::NormallyOpen,
                    }],
                    coils: vec![],
                },
                LdRung {
                    id: "R1".to_string(),
                    comment: "Rung 1 to keep with deleted contact".to_string(),
                    elements: vec![
                        LdElement::Contact {
                            symbol: "R1_Keep".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                        LdElement::Contact {
                            symbol: "R1_Delete".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                    ],
                    coils: vec![],
                },
            ],
        };

        app.project.pous.insert(
            "test_del_rung_elem".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        // Delete Rung 0 AND Contact #1 in Rung 1 simultaneously
        let to_delete = vec![
            LdElementKey::Rung(0),
            LdElementKey::Contact {
                rung: 1,
                branch_path: vec![],
                index: 1,
            },
        ];

        app.perform_ld_delete("test_del_rung_elem", &to_delete);

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_del_rung_elem") else {
            panic!("POU not found");
        };

        assert_eq!(ld.rungs.len(), 1, "Rung 0 should be deleted, leaving exactly 1 rung");
        assert_eq!(ld.rungs[0].id, "R1");
        assert_eq!(ld.rungs[0].elements.len(), 1, "R1_Delete should be deleted, leaving R1_Keep");
        if let LdElement::Contact { symbol, .. } = &ld.rungs[0].elements[0] {
            assert_eq!(symbol, "R1_Keep");
        } else {
            panic!("Expected Contact");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_move_out_of_branch_does_not_lose_elements() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_move_branch_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_move_branch".to_string(),
            comment: "".to_string(),
            rungs: vec![
                LdRung {
                    id: "R0".to_string(),
                    comment: "".to_string(),
                    elements: vec![LdElement::Branch {
                        paths: vec![
                            vec![LdElement::Contact {
                                symbol: "Leg_0".to_string(),
                                kind: ContactKind::NormallyOpen,
                            }],
                            vec![LdElement::Contact {
                                symbol: "Leg_1".to_string(),
                                kind: ContactKind::NormallyOpen,
                            }],
                        ],
                    }],
                    coils: vec![],
                },
                LdRung {
                    id: "R1".to_string(),
                    comment: "".to_string(),
                    elements: vec![LdElement::Contact {
                        symbol: "Target_Contact".to_string(),
                        kind: ContactKind::NormallyOpen,
                    }],
                    coils: vec![],
                },
            ],
        };

        app.project.pous.insert(
            "test_move_branch".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        // Move Leg_1 out of Rung 0 branch into Rung 1 before Target_Contact
        app.perform_ld_move(
            "test_move_branch",
            &[LdElementKey::Contact {
                rung: 0,
                branch_path: vec![LdBranchPathSegment { branch_idx: 0, path_idx: 1 }],
                index: 0,
            }],
            LdDropTarget::BeforeElement {
                rung: 1,
                branch_path: vec![],
                index: 0,
            },
        );

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_move_branch") else {
            panic!("POU not found");
        };

        // Rung 0 branch should dissolve into single contact Leg_0
        assert_eq!(ld.rungs[0].elements.len(), 1);
        if let LdElement::Contact { symbol, .. } = &ld.rungs[0].elements[0] {
            assert_eq!(symbol, "Leg_0");
        } else {
            panic!("Expected dissolved single contact");
        }

        // Rung 1 should now have Leg_1 followed by Target_Contact
        assert_eq!(ld.rungs[1].elements.len(), 2);
        if let LdElement::Contact { symbol, .. } = &ld.rungs[1].elements[0] {
            assert_eq!(symbol, "Leg_1");
        } else {
            panic!("Expected Leg_1 moved into Rung 1");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_new_branch_preserves_target_contact_index() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_new_br_idx_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_new_br_idx".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "First_C0".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Target_C1".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Source_C2".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                ],
                coils: vec![],
            }],
        };

        app.project.pous.insert(
            "test_new_br_idx".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        // Drag Source_C2 (index 2) and drop onto Target_C1 (index 1) with NewBranch
        app.perform_ld_move(
            "test_new_br_idx",
            &[LdElementKey::Contact {
                rung: 0,
                branch_path: vec![],
                index: 2,
            }],
            LdDropTarget::NewBranch {
                rung: 0,
                branch_path: vec![],
                index: 1,
            },
        );

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_new_br_idx") else {
            panic!("POU not found");
        };

        assert_eq!(ld.rungs[0].elements.len(), 2);
        // First element must still be First_C0 (NOT wrapped!)
        if let LdElement::Contact { symbol, .. } = &ld.rungs[0].elements[0] {
            assert_eq!(symbol, "First_C0");
        } else {
            panic!("First element should remain First_C0");
        }
        // Second element must be the Branch containing Target_C1 and Source_C2
        match &ld.rungs[0].elements[1] {
            LdElement::Branch { paths } => {
                assert_eq!(paths.len(), 2);
                if let LdElement::Contact { symbol, .. } = &paths[0][0] {
                    assert_eq!(symbol, "Target_C1");
                }
                if let LdElement::Contact { symbol, .. } = &paths[1][0] {
                    assert_eq!(symbol, "Source_C2");
                }
            }
            _ => panic!("Expected second element to be wrapped into Branch"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_inspector_mutates_nested_branch_contact() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_ld_insp_nest_{}", std::process::id()));
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "test_insp_nest".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::Branch {
                    paths: vec![
                        vec![LdElement::Contact {
                            symbol: "Path0_C0".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                        vec![LdElement::Contact {
                            symbol: "Path1_C0".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                    ],
                }],
                coils: vec![],
            }],
        };

        app.project.pous.insert(
            "test_insp_nest".to_string(),
            ferrite_project::PouSource::Ld(ld_prog),
        );

        let bp = vec![LdBranchPathSegment { branch_idx: 0, path_idx: 1 }];

        // Mutate symbol of nested contact
        let _ = app.update(Message::LdContactSymbolChanged(
            "test_insp_nest".to_string(),
            0,
            bp.clone(),
            0,
            "Updated_Tag".to_string(),
        ));

        // Mutate kind of nested contact
        let _ = app.update(Message::LdContactKindChanged(
            "test_insp_nest".to_string(),
            0,
            bp,
            0,
            ContactKind::NormallyClosed,
        ));

        let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get("test_insp_nest") else {
            panic!("POU not found");
        };

        match &ld.rungs[0].elements[0] {
            LdElement::Branch { paths } => {
                if let LdElement::Contact { symbol, kind } = &paths[1][0] {
                    assert_eq!(symbol, "Updated_Tag");
                    assert_eq!(*kind, ContactKind::NormallyClosed);
                } else {
                    panic!("Expected Contact in Path 1");
                }
            }
            _ => panic!("Expected Branch"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ld_marquee_inside_branch_does_not_select_branch() {
        let program = LdProgram {
            name: "test_marquee_branch".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![LdElement::Branch {
                    paths: vec![
                        vec![LdElement::Contact {
                            symbol: "Branch_C0".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                        vec![LdElement::Contact {
                            symbol: "Branch_C1".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                    ],
                }],
                coils: vec![],
            }],
        };

        let layout = LdLayoutEngine::build(&program, None);
        // Find the bounding box of contact Branch_C0
        let c0 = &layout.rungs[0].elements[0];
        let marquee_box = Rectangle::new(
            Point::new(c0.bounds.x + 2.0, c0.bounds.y + 2.0),
            iced::Size::new(c0.bounds.width - 4.0, c0.bounds.height - 4.0),
        );

        let selected = layout.elements_in_rect(marquee_box);
        assert_eq!(selected.len(), 1, "Only the contact should be selected");
        assert!(matches!(&selected[0], LdElementKey::Contact { .. }));
        assert!(!selected.iter().any(|k| matches!(k, LdElementKey::Branch { .. })), "Branch itself must not be selected by interior marquee");
    }

    #[test]
    fn test_ld_rung_name_and_comment_header_layout_and_rename() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_ld_name_comment_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
        let proj = crate::resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = crate::app::FerriteStudioApp::new(proj);

        let ld_prog = LdProgram {
            name: "safety_ld".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "Rung_Safety_Master".to_string(),
                comment: "Master EStop & Guard Door permissive".to_string(),
                elements: vec![LdElement::Contact {
                    symbol: "EStop_OK".to_string(),
                    kind: ContactKind::NormallyOpen,
                }],
                coils: vec![LdCoil {
                    symbol: "Permissive_OK".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        app.project.pous.insert("safety_ld".to_string(), ferrite_project::PouSource::Ld(ld_prog.clone()));

        // 1. Verify layout engine hit-tests header across the top of the rung
        let layout = LdLayoutEngine::build(&ld_prog, None);
        let header_pt = Point::new(layout.rail_left_x + 40.0, layout.rungs[0].bounds.y + 10.0);
        assert_eq!(layout.hit_test(header_pt), Some(LdElementKey::Rung(0)));

        // 2. Render editor canvas with both long name and comment without panic
        let _elem = render_ld_editor(
            &ld_prog,
            None,
            Some(&LdElementKey::Rung(0)),
            &[LdElementKey::Rung(0)],
            UiLanguage::En,
            Vector::default(),
            1.0,
        );

        // 3. Test LdRungNameChanged mutates rung.id cleanly
        let _ = app.update(Message::LdRungNameChanged(
            "safety_ld".to_string(),
            0,
            "Rung_EStop_Active".to_string(),
        ));

        let Some(ferrite_project::PouSource::Ld(updated_ld)) = app.project.pous.get("safety_ld") else {
            panic!("POU not found");
        };
        assert_eq!(updated_ld.rungs[0].id, "Rung_EStop_Active");
        assert_eq!(updated_ld.rungs[0].comment, "Master EStop & Guard Door permissive");
        assert!(app.modified, "App should be marked modified after rung rename");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
