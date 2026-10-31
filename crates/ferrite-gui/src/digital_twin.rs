use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, event};
use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, slider, stack, text, text_input,
};
use iced::{
    Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer,
    Size, Theme,
};

pub use ferrite_twin::*;

use crate::app::{FerriteStudioApp, Message};
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

#[derive(Debug, Clone, Default)]
pub struct TwinCanvasState {
    pub is_mouse_down: bool,
}

pub struct TwinCanvasProgram<'a> {
    pub workspace: &'a DigitalTwinWorkspace,
    pub io_snapshot: SharedIoSnapshot,
    pub elapsed_ms: u64,
}

impl<'a> canvas::Program<Message> for TwinCanvasProgram<'a> {
    type State = TwinCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        canvas_event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        match canvas_event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.is_mouse_down = true;
                    return (
                        event::Status::Captured,
                        Some(Message::TwinCanvasMouseDown {
                            pos: (pos.x, pos.y),
                            right_button: false,
                        }),
                    );
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    return (
                        event::Status::Captured,
                        Some(Message::TwinCanvasMouseDown {
                            pos: (pos.x, pos.y),
                            right_button: true,
                        }),
                    );
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.is_mouse_down
                    && let Some(pos) = cursor.position_in(bounds)
                {
                    return (
                        event::Status::Captured,
                        Some(Message::TwinCanvasMouseMove((pos.x, pos.y))),
                    );
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.is_mouse_down =>
            {
                state.is_mouse_down = false;
                return (event::Status::Captured, Some(Message::TwinCanvasMouseUp));
            }
            _ => {}
        }
        (event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        // Background
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(width, height),
            StudioTheme::BG_BASE,
        );

        // Grid dots (stronger in Edit mode, subtle in Runtime mode)
        let dot_alpha = if self.workspace.mode == TwinMode::Edit {
            0.09
        } else {
            0.03
        };
        let dot_color = Color::from_rgba(1.0, 1.0, 1.0, dot_alpha);
        let step = (self.workspace.grid_size * 2.0).max(10.0) as i32;
        for x in (0..width as i32).step_by(step as usize) {
            for y in (0..height as i32).step_by(step as usize) {
                frame.fill_rectangle(
                    Point::new(x as f32, y as f32),
                    Size::new(1.5, 1.5),
                    dot_color,
                );
            }
        }

        let time_sec = (self.elapsed_ms as f32) / 1000.0;

        // Sort elements by z_index
        let mut sorted_indices: Vec<usize> = (0..self.workspace.elements.len()).collect();
        sorted_indices.sort_by_key(|&i| self.workspace.elements[i].z_index);

        for idx in sorted_indices {
            let elem = &self.workspace.elements[idx];
            let px = elem.position.0;
            let py = elem.position.1;
            let sw = elem.size.0;
            let sh = elem.size.1;
            let cx = px + sw * 0.5;
            let cy = py + sh * 0.5;

            let raw_bool = elem.binding.as_ref().and_then(|b| {
                let var = &b.variable_id;
                self.io_snapshot
                    .get_bool(var)
                    .or_else(|| match var.as_str() {
                        "Valve_Fill" | "Fill_Valve" => self
                            .io_snapshot
                            .get_bool("Fill_Valve")
                            .or_else(|| self.io_snapshot.get_bool("Valve_Fill")),
                        "Valve_Dose" | "Dosing_Valve" => self
                            .io_snapshot
                            .get_bool("Dosing_Valve")
                            .or_else(|| self.io_snapshot.get_bool("Valve_Dose")),
                        "Mixer_Running" | "Agitator_Fast" => self
                            .io_snapshot
                            .get_bool("Agitator_Fast")
                            .or_else(|| self.io_snapshot.get_bool("Mixer_Running")),
                        "Valve_Drain" | "Drain_Valve" => self
                            .io_snapshot
                            .get_bool("Drain_Valve")
                            .or_else(|| self.io_snapshot.get_bool("Valve_Drain")),
                        "Pump_Running" | "Pump_Active" => self
                            .io_snapshot
                            .get_bool("Pump_Active")
                            .or_else(|| self.io_snapshot.get_bool("Pump_Running")),
                        _ => None,
                    })
            });

            let analog_val = elem.binding.as_ref().and_then(|b| {
                let var = &b.variable_id;
                self.io_snapshot
                    .get_f32(var)
                    .or_else(|| match var.as_str() {
                        "Tank_Level" => self.io_snapshot.get_f32("Tank_Level"),
                        "Tank_Temp" => self.io_snapshot.get_f32("Tank_Temp"),
                        _ => None,
                    })
            });

            let mut is_active = raw_bool.unwrap_or_else(|| analog_val.is_some_and(|v| v > 0.0));

            // Apply blinking rule if configured
            if let Some(ref binding) = elem.binding
                && let Some(ref blink) = binding.blink
                && blink.enabled
                && is_active == blink.trigger_state
            {
                let interval = blink.interval_ms.max(50);
                let phase_on = (self.elapsed_ms / interval).is_multiple_of(2);
                is_active = phase_on;
            }

            let elem_color = if let Some(ref b) = elem.binding {
                let c = if is_active {
                    b.active_color
                } else {
                    b.inactive_color
                };
                Color::from_rgb8(c[0], c[1], c[2])
            } else {
                Color::from_rgb8(elem.base_color[0], elem.base_color[1], elem.base_color[2])
            };

            let is_selected = self.workspace.mode == TwinMode::Edit
                && self.workspace.selected_id == Some(elem.id);

            // Draw rotated element in local coordinates centered at (0, 0)
            frame.with_save(|frame| {
                frame.translate(iced::Vector::new(cx, cy));
                if elem.rotation.abs() > f32::EPSILON {
                    frame.rotate(elem.rotation.to_radians());
                }
                let local_tl = Point::new(-sw * 0.5, -sh * 0.5);

                match elem.element_type {
                    ElementKind::BasicShape(ShapeType::Line) => {
                        let thickness = sh.clamp(2.0, 28.0);
                        frame.stroke(
                            &Path::line(
                                Point::new(-sw * 0.5, 0.0),
                                Point::new(sw * 0.5, 0.0),
                            ),
                            Stroke::default().with_color(elem_color).with_width(thickness),
                        );
                    }
                    ElementKind::BasicShape(ShapeType::Square | ShapeType::Rectangle) => {
                        frame.fill_rectangle(local_tl, Size::new(sw, sh), elem_color);
                        frame.stroke_rectangle(
                            local_tl,
                            Size::new(sw, sh),
                            Stroke::default()
                                .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.65))
                                .with_width(1.5),
                        );
                    }
                    ElementKind::BasicShape(ShapeType::RoundedRect) => {
                        frame.fill_rectangle(local_tl, Size::new(sw, sh), elem_color);
                        frame.stroke_rectangle(
                            local_tl,
                            Size::new(sw, sh),
                            Stroke::default()
                                .with_color(StudioTheme::TEXT_PRIMARY)
                                .with_width(1.5),
                        );
                    }
                    ElementKind::BasicShape(ShapeType::Circle) => {
                        let radius = (sw.min(sh) * 0.5).max(4.0);
                        let circle = Path::circle(Point::ORIGIN, radius);
                        frame.fill(&circle, elem_color);
                        frame.stroke(
                            &circle,
                            Stroke::default()
                                .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.7))
                                .with_width(1.5),
                        );
                    }
                    ElementKind::BasicShape(ShapeType::Triangle) => {
                        let tri = Path::new(|b| {
                            b.move_to(Point::new(0.0, -sh * 0.5));
                            b.line_to(Point::new(sw * 0.5, sh * 0.5));
                            b.line_to(Point::new(-sw * 0.5, sh * 0.5));
                            b.close();
                        });
                        frame.fill(&tri, elem_color);
                        frame.stroke(
                            &tri,
                            Stroke::default()
                                .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.7))
                                .with_width(1.5),
                        );
                    }
                    ElementKind::Tank => {
                        let border_stroke = Stroke::default()
                            .with_color(if is_selected {
                                StudioTheme::ACCENT_CYAN
                            } else {
                                Color::from_rgb(0.55, 0.65, 0.75)
                            })
                            .with_width(2.5);
                        frame.fill_rectangle(
                            local_tl,
                            Size::new(sw, sh),
                            Color::from_rgba(0.08, 0.11, 0.15, 0.85),
                        );
                        frame.stroke_rectangle(local_tl, Size::new(sw, sh), border_stroke);

                        let level_pct = analog_val
                            .map(|v| (v / 100.0).clamp(0.0, 1.0))
                            .unwrap_or(if is_active { 0.8 } else { 0.0 });
                        if level_pct > 0.0 {
                            let liquid_h = (sh - 6.0) * level_pct;
                            let liquid_y = local_tl.y + sh - 3.0 - liquid_h;
                            frame.fill_rectangle(
                                Point::new(local_tl.x + 3.0, liquid_y),
                                Size::new((sw - 6.0).max(2.0), liquid_h.max(2.0)),
                                Color::from_rgba(elem_color.r, elem_color.g, elem_color.b, 0.65),
                            );
                        }

                        frame.fill_text(canvas::Text {
                            content: format!("{} ({:.0}%)", elem.label, level_pct * 100.0),
                            position: Point::new(local_tl.x + 8.0, local_tl.y + 8.0),
                            color: StudioTheme::TEXT_PRIMARY,
                            size: iced::Pixels(11.5),
                            ..Default::default()
                        });
                    }
                    ElementKind::Pipe => {
                        let base_c = Color::from_rgb8(
                            elem.base_color[0],
                            elem.base_color[1],
                            elem.base_color[2],
                        );
                        frame.fill_rectangle(local_tl, Size::new(sw, sh), base_c);
                        if is_active {
                            let dash_step = 24.0;
                            let phase = (time_sec * 48.0).rem_euclid(dash_step);
                            let mut x = local_tl.x + phase;
                            while x + 10.0 <= local_tl.x + sw {
                                frame.fill_rectangle(
                                    Point::new(x, -sh * 0.25),
                                    Size::new(10.0, (sh * 0.5).max(3.0)),
                                    elem_color,
                                );
                                x += dash_step;
                            }
                        }
                    }
                    ElementKind::Valve => {
                        let bowtie = Path::new(|b| {
                            b.move_to(Point::new(-sw * 0.5, -sh * 0.4));
                            b.line_to(Point::new(sw * 0.5, sh * 0.4));
                            b.line_to(Point::new(sw * 0.5, -sh * 0.4));
                            b.line_to(Point::new(-sw * 0.5, sh * 0.4));
                            b.close();
                        });
                        frame.fill(&bowtie, elem_color);
                        frame.stroke(
                            &bowtie,
                            Stroke::default()
                                .with_color(StudioTheme::TEXT_PRIMARY)
                                .with_width(1.5),
                        );
                        // Stem & actuator cap
                        frame.stroke(
                            &Path::line(Point::ORIGIN, Point::new(0.0, -sh * 0.5)),
                            Stroke::default()
                                .with_color(StudioTheme::TEXT_PRIMARY)
                                .with_width(2.0),
                        );
                        frame.fill(
                            &Path::circle(Point::new(0.0, -sh * 0.5), 5.0),
                            elem_color,
                        );
                    }
                    ElementKind::Actuator(kind) => {
                        let radius = (sw.min(sh) * 0.45).max(8.0);
                        let circle = Path::circle(Point::ORIGIN, radius);
                        frame.fill(
                            &circle,
                            Color::from_rgba(elem_color.r, elem_color.g, elem_color.b, 0.28),
                        );
                        frame.stroke(
                            &circle,
                            Stroke::default().with_color(elem_color).with_width(2.5),
                        );

                        let angle = if is_active { time_sec * 8.0 } else { 0.0 };
                        match kind {
                            ActuatorKind::CentrifugalPump | ActuatorKind::ElectricMotor => {
                                for i in 0..3 {
                                    let a = angle + (i as f32) * (std::f32::consts::TAU / 3.0);
                                    let tip = Point::new(
                                        a.cos() * (radius - 4.0),
                                        a.sin() * (radius - 4.0),
                                    );
                                    frame.stroke(
                                        &Path::line(Point::ORIGIN, tip),
                                        Stroke::default().with_color(elem_color).with_width(2.0),
                                    );
                                }
                                if kind == ActuatorKind::ElectricMotor {
                                    frame.fill_text(canvas::Text {
                                        content: "M".to_string(),
                                        position: Point::new(-5.0, -7.0),
                                        color: Color::WHITE,
                                        size: iced::Pixels(13.0),
                                        ..Default::default()
                                    });
                                }
                            }
                            ActuatorKind::AgitatorMixer => {
                                let blade = (angle.cos() * (radius * 0.85)).abs().max(5.0);
                                frame.stroke(
                                    &Path::line(
                                        Point::new(-blade, 0.0),
                                        Point::new(blade, 0.0),
                                    ),
                                    Stroke::default().with_color(elem_color).with_width(3.0),
                                );
                            }
                        }
                    }
                    ElementKind::IndicatorLed => {
                        let radius = (sw.min(sh) * 0.45).max(5.0);
                        let outer = Path::circle(Point::ORIGIN, radius + 2.0);
                        let inner = Path::circle(Point::ORIGIN, radius);
                        frame.fill(&outer, Color::from_rgb(0.12, 0.15, 0.2));
                        frame.fill(&inner, elem_color);
                        frame.stroke(
                            &inner,
                            Stroke::default().with_color(Color::WHITE).with_width(1.5),
                        );
                        if is_active {
                            frame.stroke(
                                &Path::circle(Point::ORIGIN, radius + 5.0),
                                Stroke::default()
                                    .with_color(Color::from_rgba(
                                        elem_color.r,
                                        elem_color.g,
                                        elem_color.b,
                                        0.45,
                                    ))
                                    .with_width(2.5),
                            );
                        }
                    }
                    ElementKind::PushButton => {
                        frame.fill_rectangle(local_tl, Size::new(sw, sh), elem_color);
                        frame.stroke_rectangle(
                            local_tl,
                            Size::new(sw, sh),
                            Stroke::default()
                                .with_color(if is_active {
                                    Color::WHITE
                                } else {
                                    StudioTheme::BORDER_ACTIVE
                                })
                                .with_width(if is_active { 2.5 } else { 1.5 }),
                        );
                        frame.fill_text(canvas::Text {
                            content: elem.label.clone(),
                            position: Point::new(local_tl.x + 8.0, -6.0),
                            color: Color::WHITE,
                            size: iced::Pixels(11.5),
                            ..Default::default()
                        });
                    }
                    ElementKind::Switch => {
                        frame.fill_rectangle(
                            local_tl,
                            Size::new(sw, sh),
                            Color::from_rgb(0.16, 0.20, 0.26),
                        );
                        frame.stroke_rectangle(
                            local_tl,
                            Size::new(sw, sh),
                            Stroke::default()
                                .with_color(StudioTheme::BORDER_ACTIVE)
                                .with_width(1.5),
                        );
                        let knob_r = (sh * 0.38).max(5.0);
                        let knob_x = if is_active {
                            sw * 0.5 - knob_r - 5.0
                        } else {
                            -sw * 0.5 + knob_r + 5.0
                        };
                        let knob = Path::circle(Point::new(knob_x, 0.0), knob_r);
                        frame.fill(&knob, elem_color);
                        frame.stroke(
                            &knob,
                            Stroke::default().with_color(Color::WHITE).with_width(1.5),
                        );
                    }
                }
            });

            // Draw label below non-button/non-tank elements
            if !matches!(
                elem.element_type,
                ElementKind::Tank | ElementKind::PushButton
            ) && !elem.label.is_empty()
            {
                let var_suffix = elem
                    .binding
                    .as_ref()
                    .map(|b| format!(" [{}]", b.variable_id))
                    .unwrap_or_default();
                frame.fill_text(canvas::Text {
                    content: format!("{}{}", elem.label, var_suffix),
                    position: Point::new(px, py + sh + 4.0),
                    color: StudioTheme::TEXT_SECONDARY,
                    size: iced::Pixels(10.5),
                    ..Default::default()
                });
            }

            // Selection handles in Edit mode
            if is_selected {
                let sel_stroke = Stroke::default()
                    .with_color(StudioTheme::ACCENT_CYAN)
                    .with_width(1.5);
                frame.stroke_rectangle(
                    Point::new(px - 2.0, py - 2.0),
                    Size::new(sw + 4.0, sh + 4.0),
                    sel_stroke,
                );

                // 4 Corner resize handles
                let corners = [
                    Point::new(px, py),
                    Point::new(px + sw, py),
                    Point::new(px, py + sh),
                    Point::new(px + sw, py + sh),
                ];
                for pt in corners {
                    let h_circle = Path::circle(pt, 5.0);
                    frame.fill(&h_circle, StudioTheme::ACCENT_CYAN);
                    frame.stroke(
                        &h_circle,
                        Stroke::default().with_color(Color::WHITE).with_width(1.2),
                    );
                }

                // Top Rotation Handle stem & circle
                let (rx, ry) = elem.rotation_handle_pos();
                frame.stroke(
                    &Path::line(Point::new(cx, py), Point::new(rx, ry)),
                    Stroke::default()
                        .with_color(StudioTheme::ACCENT_AMBER)
                        .with_width(1.5),
                );
                let rot_circle = Path::circle(Point::new(rx, ry), 6.0);
                frame.fill(&rot_circle, StudioTheme::ACCENT_AMBER);
                frame.stroke(
                    &rot_circle,
                    Stroke::default().with_color(Color::WHITE).with_width(1.2),
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

/// Interactive 2D Saturation/Value Color Picker Canvas.
pub struct SvPickerCanvas {
    pub hue: f32,
    pub sat: f32,
    pub val: f32,
}

#[derive(Debug, Clone, Default)]
pub struct PickerDragState {
    pub dragging: bool,
}

impl canvas::Program<Message> for SvPickerCanvas {
    type State = PickerDragState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.dragging = true;
                    let s = (pos.x / bounds.width).clamp(0.0, 1.0);
                    let v = (1.0 - (pos.y / bounds.height)).clamp(0.0, 1.0);
                    return (
                        event::Status::Captured,
                        Some(Message::TwinPickColorSv(s, v)),
                    );
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging
                    && let Some(pos) = cursor.position_in(bounds)
                {
                    let s = (pos.x / bounds.width).clamp(0.0, 1.0);
                    let v = (1.0 - (pos.y / bounds.height)).clamp(0.0, 1.0);
                    return (
                        event::Status::Captured,
                        Some(Message::TwinPickColorSv(s, v)),
                    );
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            _ => {}
        }
        (event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let cols = 24;
        let rows = 16;
        let cell_w = bounds.width / cols as f32;
        let cell_h = bounds.height / rows as f32;

        for ix in 0..cols {
            for iy in 0..rows {
                let s = (ix as f32 + 0.5) / cols as f32;
                let v = 1.0 - (iy as f32 + 0.5) / rows as f32;
                let [r, g, b, _] = hsv_to_rgb(self.hue, s, v, 255);
                frame.fill_rectangle(
                    Point::new(ix as f32 * cell_w, iy as f32 * cell_h),
                    Size::new(cell_w + 0.8, cell_h + 0.8),
                    Color::from_rgb8(r, g, b),
                );
            }
        }

        let cx = self.sat.clamp(0.0, 1.0) * bounds.width;
        let cy = (1.0 - self.val.clamp(0.0, 1.0)) * bounds.height;
        let marker = Path::circle(Point::new(cx, cy), 5.5);
        frame.stroke(
            &marker,
            Stroke::default().with_color(Color::BLACK).with_width(2.5),
        );
        frame.stroke(
            &marker,
            Stroke::default().with_color(Color::WHITE).with_width(1.5),
        );

        vec![frame.into_geometry()]
    }
}

/// Interactive Hue Rainbow Spectrum Bar Canvas.
pub struct HueBarCanvas {
    pub hue: f32,
}

impl canvas::Program<Message> for HueBarCanvas {
    type State = PickerDragState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(bounds) {
                    state.dragging = true;
                    let h = ((pos.x / bounds.width) * 360.0).clamp(0.0, 360.0);
                    return (event::Status::Captured, Some(Message::TwinPickColorHue(h)));
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging
                    && let Some(pos) = cursor.position_in(bounds)
                {
                    let h = ((pos.x / bounds.width) * 360.0).clamp(0.0, 360.0);
                    return (event::Status::Captured, Some(Message::TwinPickColorHue(h)));
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            _ => {}
        }
        (event::Status::Ignored, None)
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let steps = 36;
        let step_w = bounds.width / steps as f32;
        for i in 0..steps {
            let h = (i as f32 + 0.5) * (360.0 / steps as f32);
            let [r, g, b, _] = hsv_to_rgb(h, 1.0, 1.0, 255);
            frame.fill_rectangle(
                Point::new(i as f32 * step_w, 0.0),
                Size::new(step_w + 0.8, bounds.height),
                Color::from_rgb8(r, g, b),
            );
        }

        let hx = (self.hue.clamp(0.0, 360.0) / 360.0) * bounds.width;
        frame.stroke_rectangle(
            Point::new((hx - 2.0).max(0.0), 0.0),
            Size::new(4.0, bounds.height),
            Stroke::default().with_color(Color::WHITE).with_width(2.0),
        );

        vec![frame.into_geometry()]
    }
}

/// Renders the dedicated Digital Twin OS Window with Edit/Runtime Modes, Context Menu, and Inspector.
pub fn render_digital_twin_window<'a>(app: &'a FerriteStudioApp) -> Element<'a, Message> {
    let ws = &app.digital_twin_workspace;
    let lang = app.ui_lang;
    let io_snapshot = app
        .shared_io
        .read()
        .map(|lock| lock.snapshot())
        .unwrap_or_default();

    let is_edit = ws.mode == TwinMode::Edit;
    let is_runtime = ws.mode == TwinMode::Runtime;

    // 1. Header Bar: Title + Mode Switcher + PLC Status + Save
    let edit_mode_btn = button(
        text(lang.tr("Edit Mode (CAD)", "Mode Édition (CAO)"))
            .size(12)
            .shaping(iced::widget::text::Shaping::Advanced),
    )
    .on_press(Message::TwinSetMode(TwinMode::Edit))
    .style(if is_edit {
        StudioTheme::primary_button
    } else {
        StudioTheme::secondary_button
    })
    .padding([5, 12]);

    let runtime_mode_btn = button(
        text(lang.tr("Runtime Mode (PLC Running)", "Mode Runtime (API en marche)"))
            .size(12)
            .shaping(iced::widget::text::Shaping::Advanced),
    )
    .on_press(Message::TwinSetMode(TwinMode::Runtime))
    .style(if is_runtime {
        StudioTheme::success_button
    } else {
        StudioTheme::secondary_button
    })
    .padding([5, 12]);

    let plc_badge_color = if app.sim_running {
        StudioTheme::ACCENT_EMERALD
    } else {
        StudioTheme::ACCENT_AMBER
    };
    let plc_status_badge = container(
        text(if app.sim_running {
            lang.tr("PLC: RUNNING", "API : EN MARCHE")
        } else {
            lang.tr("PLC: PAUSED", "API : PAUSE")
        })
        .size(11)
        .color(plc_badge_color),
    )
    .padding([4, 10])
    .style(StudioTheme::subtle_badge_style(Color::from_rgba(
        plc_badge_color.r,
        plc_badge_color.g,
        plc_badge_color.b,
        0.15,
    )));

    let save_btn = button(text(lang.tr("Save Twin", "Sauvegarder Jumeau")).size(11.5))
        .on_press(Message::TwinSaveToProject)
        .style(StudioTheme::secondary_button)
        .padding([5, 10]);

    let header_bar = container(
        row![
            text(lang.tr("Digital Twin — 2D Plant & HMI", "Jumeau Numérique — Usine 2D & IHM"))
                .size(15),
            container(
                text(format!("{} obj", ws.elements.len()))
                    .size(11)
                    .color(StudioTheme::ACCENT_CYAN),
            )
            .padding([3, 8])
            .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                0.22, 0.74, 0.97, 0.14,
            ))),
            Space::with_width(12),
            edit_mode_btn,
            runtime_mode_btn,
            plc_status_badge,
            Space::with_width(Length::Fill),
            save_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 12])
    .style(StudioTheme::header_container_style);

    // 2. Edit Mode Spawn Toolbar (Hidden in Runtime Mode)
    let spawn_toolbar: Option<Element<'a, Message>> = if is_edit {
        let spawn_items: Vec<(&str, ElementKind)> = vec![
            ("+ Button", ElementKind::PushButton),
            ("+ Switch", ElementKind::Switch),
            ("+ LED", ElementKind::IndicatorLed),
            ("+ Tank", ElementKind::Tank),
            ("+ Valve", ElementKind::Valve),
            (
                "+ Motor",
                ElementKind::Actuator(ActuatorKind::ElectricMotor),
            ),
            (
                "+ Pump",
                ElementKind::Actuator(ActuatorKind::CentrifugalPump),
            ),
            (
                "+ Mixer",
                ElementKind::Actuator(ActuatorKind::AgitatorMixer),
            ),
            ("+ Pipe", ElementKind::Pipe),
            ("+ Line", ElementKind::BasicShape(ShapeType::Line)),
            ("+ Square", ElementKind::BasicShape(ShapeType::Square)),
            ("+ Rect", ElementKind::BasicShape(ShapeType::Rectangle)),
            ("+ Circle", ElementKind::BasicShape(ShapeType::Circle)),
            ("+ Triangle", ElementKind::BasicShape(ShapeType::Triangle)),
        ];

        let mut tools_row = row![].spacing(6).align_y(Alignment::Center);
        for (label, kind) in spawn_items {
            tools_row = tools_row.push(
                button(text(label).size(11))
                    .on_press(Message::TwinSpawnElement(kind))
                    .style(StudioTheme::secondary_button)
                    .padding([4, 8]),
            );
        }

        let snap_btn = button(
            text(if ws.snap_to_grid {
                "Snap: ON"
            } else {
                "Snap: OFF"
            })
            .size(11),
        )
        .on_press(Message::TwinToggleSnapGrid)
        .style(if ws.snap_to_grid {
            StudioTheme::primary_button
        } else {
            StudioTheme::ghost_button
        })
        .padding([4, 8]);

        let reset_btn = button(text(lang.tr("Starter Plant", "Usine Démo")).size(11))
            .on_press(Message::TwinResetStarter)
            .style(StudioTheme::ghost_button)
            .padding([4, 8]);

        let clear_btn = button(text(lang.tr("Clear", "Vider")).size(11))
            .on_press(Message::TwinClearAll)
            .style(StudioTheme::danger_button)
            .padding([4, 8]);

        tools_row = tools_row
            .push(Space::with_width(12))
            .push(snap_btn)
            .push(reset_btn)
            .push(clear_btn);

        let scrollable_tools = scrollable(
            container(tools_row).padding(Padding {
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
            container(scrollable_tools)
                .width(Length::Fill)
                .padding([4, 10])
                .style(StudioTheme::subtle_badge_style(Color::from_rgba(
                    0.2, 0.4, 0.6, 0.12,
                )))
                .into(),
        )
    } else {
        None
    };

    // 3. Central Interactive Canvas + Floating Right-Click Context Menu Overlay
    let base_canvas: Element<'a, Message> = canvas::Canvas::new(TwinCanvasProgram {
        workspace: ws,
        io_snapshot: io_snapshot.clone(),
        elapsed_ms: app.twin_elapsed_ms,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into();

    let canvas_with_menu: Element<'a, Message> =
        if let (true, Some((menu_pos, elem_id))) = (is_edit, ws.context_menu) {
            let menu_card = container(
                column![
                    text(format!("Object #{elem_id}"))
                        .size(11)
                        .color(StudioTheme::ACCENT_CYAN),
                    button(text(lang.tr("Parameters...", "Paramètres...")).size(11.5))
                        .on_press(Message::TwinOpenInspector(elem_id))
                        .style(StudioTheme::primary_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Rotate +90°", "Pivoter +90°")).size(11.5))
                        .on_press(Message::TwinContextAction {
                            action: "rotate".to_string(),
                            id: elem_id,
                        })
                        .style(StudioTheme::secondary_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Rotate -90°", "Pivoter -90°")).size(11.5))
                        .on_press(Message::TwinContextAction {
                            action: "rotate_ccw".to_string(),
                            id: elem_id,
                        })
                        .style(StudioTheme::secondary_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Duplicate", "Dupliquer")).size(11.5))
                        .on_press(Message::TwinContextAction {
                            action: "duplicate".to_string(),
                            id: elem_id,
                        })
                        .style(StudioTheme::secondary_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Bring to Front", "Premier plan")).size(11.5))
                        .on_press(Message::TwinContextAction {
                            action: "front".to_string(),
                            id: elem_id,
                        })
                        .style(StudioTheme::ghost_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Send to Back", "Arrière-plan")).size(11.5))
                        .on_press(Message::TwinContextAction {
                            action: "back".to_string(),
                            id: elem_id,
                        })
                        .style(StudioTheme::ghost_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Delete", "Supprimer")).size(11.5))
                        .on_press(Message::TwinContextAction {
                            action: "delete".to_string(),
                            id: elem_id,
                        })
                        .style(StudioTheme::danger_button)
                        .width(Length::Fill)
                        .padding([4, 8]),
                    button(text(lang.tr("Close Menu", "Fermer")).size(11))
                        .on_press(Message::TwinCloseContextMenu)
                        .style(StudioTheme::ghost_button)
                        .width(Length::Fill)
                        .padding([3, 8]),
                ]
                .spacing(4)
                .width(Length::Fixed(165.0)),
            )
            .padding(8)
            .style(StudioTheme::card_style);

            let clamped_x = menu_pos.x.clamp(8.0, 820.0);
            let clamped_y = menu_pos.y.clamp(8.0, 520.0);
            let positioned_menu = column![
                Space::with_height(Length::Fixed(clamped_y)),
                row![Space::with_width(Length::Fixed(clamped_x)), menu_card],
            ];
            stack![base_canvas, positioned_menu]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            base_canvas
        };

    let canvas_card = container(canvas_with_menu)
        .padding(1)
        .style(StudioTheme::canvas_container_style)
        .width(Length::Fill)
        .height(Length::Fill);

    // 4. Right-Side Parameter & Color Picker Inspector (in Edit Mode when opened)
    let body_row: Element<'a, Message> = if is_edit
        && let Some(insp_id) = ws.inspector_open_for
        && let Some(elem) = ws.elements.iter().find(|e| e.id == insp_id)
    {
        let inspector_panel = render_element_inspector(app, elem, &io_snapshot);
        row![canvas_card, inspector_panel]
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        canvas_card.into()
    };

    let footer_tip = text(if is_edit {
        lang.tr(
            "Edit Mode: Drag objects to move, drag corners to resize, drag top amber handle to rotate. Right-click any object -> 'Parameters...' to bind variables, pick ON/OFF colors, and set blinking.",
            "Mode Édition : Glissez pour déplacer, coins pour redimensionner, poignée ambre pour pivoter. Clic droit -> 'Paramètres...' pour lier une variable, choisir les couleurs ON/OFF et le clignotement.",
        )
    } else {
        lang.tr(
            "Runtime Mode: Layout is locked and PLC is running. Click PushButtons and Switches to drive bound PLC variables in real time.",
            "Mode Runtime : Disposition verrouillée et API en marche. Cliquez sur les boutons et commutateurs pour piloter les variables API en direct.",
        )
    })
    .size(11)
    .color(StudioTheme::TEXT_MUTED);

    let mut root_col = column![header_bar].spacing(6);
    if let Some(tb) = spawn_toolbar {
        root_col = root_col.push(tb);
    }
    root_col = root_col
        .push(
            container(body_row)
                .padding([0, 8])
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .push(container(footer_tip).padding([4, 12]));

    container(root_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(StudioTheme::canvas_container_style)
        .into()
}

fn render_element_inspector<'a>(
    app: &'a FerriteStudioApp,
    elem: &'a CanvasElement,
    io_snapshot: &SharedIoSnapshot,
) -> Element<'a, Message> {
    let ws = &app.digital_twin_workspace;
    let lang = app.ui_lang;
    let elem_id = elem.id;

    let close_btn = button(
        text(crate::icons::icon_close())
            .size(12)
            .shaping(iced::widget::text::Shaping::Advanced),
    )
    .on_press(Message::TwinCloseInspector)
    .style(StudioTheme::ghost_button)
    .padding([3, 7]);

    let header = row![
        text(lang.tr("Object Parameters", "Paramètres de l'Objet"))
            .size(13.5)
            .color(StudioTheme::ACCENT_CYAN),
        Space::with_width(Length::Fill),
        close_btn,
    ]
    .align_y(Alignment::Center);

    // Transform Section
    let label_input = text_input("Label...", &elem.label)
        .on_input(move |s| Message::TwinSetLabel(elem_id, s))
        .padding([4, 8])
        .style(StudioTheme::styled_input);

    let pos_row = row![
        text("X:").size(11).color(StudioTheme::TEXT_MUTED),
        text_input("X", &format!("{:.0}", elem.position.0))
            .on_input(move |s| Message::TwinSetPositionX(elem_id, s))
            .width(Length::Fixed(64.0))
            .padding([3, 6])
            .style(StudioTheme::styled_input),
        text("Y:").size(11).color(StudioTheme::TEXT_MUTED),
        text_input("Y", &format!("{:.0}", elem.position.1))
            .on_input(move |s| Message::TwinSetPositionY(elem_id, s))
            .width(Length::Fixed(64.0))
            .padding([3, 6])
            .style(StudioTheme::styled_input),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let size_row = row![
        text("W:").size(11).color(StudioTheme::TEXT_MUTED),
        text_input("W", &format!("{:.0}", elem.size.0))
            .on_input(move |s| Message::TwinSetWidth(elem_id, s))
            .width(Length::Fixed(64.0))
            .padding([3, 6])
            .style(StudioTheme::styled_input),
        text("H:").size(11).color(StudioTheme::TEXT_MUTED),
        text_input("H", &format!("{:.0}", elem.size.1))
            .on_input(move |s| Message::TwinSetHeight(elem_id, s))
            .width(Length::Fixed(64.0))
            .padding([3, 6])
            .style(StudioTheme::styled_input),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let rot_slider = slider(0.0..=360.0, elem.rotation, move |deg| {
        Message::TwinSetRotation(elem_id, deg)
    })
    .step(1.0_f32);

    let quick_rot_row = row![
        text(format!("Rot: {:.0}°", elem.rotation))
            .size(11)
            .color(StudioTheme::TEXT_SECONDARY),
        button(text("0°").size(10.5))
            .on_press(Message::TwinSetRotation(elem_id, 0.0))
            .style(StudioTheme::ghost_button)
            .padding([2, 6]),
        button(text("90°").size(10.5))
            .on_press(Message::TwinSetRotation(elem_id, 90.0))
            .style(StudioTheme::ghost_button)
            .padding([2, 6]),
        button(text("180°").size(10.5))
            .on_press(Message::TwinSetRotation(elem_id, 180.0))
            .style(StudioTheme::ghost_button)
            .padding([2, 6]),
        button(text("270°").size(10.5))
            .on_press(Message::TwinSetRotation(elem_id, 270.0))
            .style(StudioTheme::ghost_button)
            .padding([2, 6]),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let mut transform_col = column![
        text(lang.tr("Transform & Geometry", "Transformation & Géométrie"))
            .size(12)
            .color(StudioTheme::TEXT_PRIMARY),
        label_input,
        pos_row,
        size_row,
        rot_slider,
        quick_rot_row,
    ]
    .spacing(6);

    if elem.element_type == ElementKind::PushButton {
        let is_toggle = elem.button_mode == ButtonActionMode::Toggle;
        transform_col = transform_col.push(
            row![
                text(lang.tr("Action:", "Action :"))
                    .size(11)
                    .color(StudioTheme::TEXT_MUTED),
                button(text("Toggle (Click)").size(10.5))
                    .on_press(Message::TwinSetButtonMode(
                        elem_id,
                        ButtonActionMode::Toggle,
                    ))
                    .style(if is_toggle {
                        StudioTheme::primary_button
                    } else {
                        StudioTheme::secondary_button
                    })
                    .padding([3, 8]),
                button(text("Momentary (Hold)").size(10.5))
                    .on_press(Message::TwinSetButtonMode(
                        elem_id,
                        ButtonActionMode::Momentary,
                    ))
                    .style(if !is_toggle {
                        StudioTheme::primary_button
                    } else {
                        StudioTheme::secondary_button
                    })
                    .padding([3, 8]),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }

    // Variable Binding Section
    let is_bound = elem.binding.is_some();
    let bind_toggle_btn = button(
        text(if is_bound {
            lang.tr("Bound to PLC Variable (ON)", "Lié à une variable API (OUI)")
        } else {
            lang.tr("Link to PLC Variable (OFF)", "Lier à une variable API (NON)")
        })
        .size(11.5),
    )
    .on_press(Message::TwinToggleBinding(elem_id, !is_bound))
    .style(if is_bound {
        StudioTheme::primary_button
    } else {
        StudioTheme::secondary_button
    })
    .width(Length::Fill)
    .padding([5, 10]);

    let mut binding_col = column![
        text(lang.tr("PLC Variable Link", "Liaison Variable API"))
            .size(12)
            .color(StudioTheme::TEXT_PRIMARY),
        bind_toggle_btn,
    ]
    .spacing(6);

    let mut available_vars: Vec<String> = app
        .project
        .global_tags
        .tags
        .iter()
        .map(|t| t.name.clone())
        .collect();
    if let Ok(io) = app.shared_io.read() {
        for k in io.list_variables() {
            if !available_vars.contains(&k) {
                available_vars.push(k);
            }
        }
    }
    available_vars.sort();

    if let Some(ref binding) = elem.binding {
        let var_picker = pick_list(
            available_vars,
            Some(binding.variable_id.clone()),
            move |selected_var| Message::TwinSetVariable(elem_id, selected_var),
        )
        .width(Length::Fill)
        .text_size(11.5)
        .padding([4, 8]);

        let var_input = text_input("Variable / %QX0.0...", &binding.variable_id)
            .on_input(move |s| Message::TwinSetVariable(elem_id, s))
            .padding([4, 8])
            .style(StudioTheme::styled_input);

        let is_rw = binding.access_mode == AccessMode::ReadWrite;
        let access_row = row![
            button(text("Read-Only (Display)").size(10.5))
                .on_press(Message::TwinSetAccessMode(elem_id, AccessMode::ReadOnly))
                .style(if !is_rw {
                    StudioTheme::primary_button
                } else {
                    StudioTheme::secondary_button
                })
                .padding([3, 8]),
            button(text("Read-Write (Control)").size(10.5))
                .on_press(Message::TwinSetAccessMode(elem_id, AccessMode::ReadWrite))
                .style(if is_rw {
                    StudioTheme::primary_button
                } else {
                    StudioTheme::secondary_button
                })
                .padding([3, 8]),
        ]
        .spacing(6);

        let live_val_str = if let Some(b) = io_snapshot.get_bool(&binding.variable_id) {
            if b {
                "TRUE (ON)".to_string()
            } else {
                "FALSE (OFF)".to_string()
            }
        } else if let Some(f) = io_snapshot.get_f32(&binding.variable_id) {
            format!("{f:.2}")
        } else {
            "Unresolved".to_string()
        };

        // Blinking rules
        let blink_enabled = binding.blink.as_ref().is_some_and(|b| b.enabled);
        let blink_toggle = button(
            text(if blink_enabled {
                lang.tr("Blinking: ENABLED", "Clignotement : ACTIVÉ")
            } else {
                lang.tr("Enable Blinking", "Activer le clignotement")
            })
            .size(11),
        )
        .on_press(Message::TwinToggleBlink(elem_id, !blink_enabled))
        .style(if blink_enabled {
            StudioTheme::primary_button
        } else {
            StudioTheme::secondary_button
        })
        .width(Length::Fill)
        .padding([4, 8]);

        binding_col = binding_col
            .push(var_picker)
            .push(var_input)
            .push(access_row)
            .push(
                text(format!("Live Value: {live_val_str}"))
                    .size(11)
                    .color(StudioTheme::ACCENT_EMERALD),
            )
            .push(blink_toggle);

        if let Some(ref blink) = binding.blink
            && blink.enabled
        {
            let trig_row = row![
                button(text("Blink when ON").size(10.5))
                    .on_press(Message::TwinSetBlinkTrigger(elem_id, true))
                    .style(if blink.trigger_state {
                        StudioTheme::primary_button
                    } else {
                        StudioTheme::secondary_button
                    })
                    .padding([3, 8]),
                button(text("Blink when OFF").size(10.5))
                    .on_press(Message::TwinSetBlinkTrigger(elem_id, false))
                    .style(if !blink.trigger_state {
                        StudioTheme::primary_button
                    } else {
                        StudioTheme::secondary_button
                    })
                    .padding([3, 8]),
            ]
            .spacing(6);

            let interval_slider = slider(100.0..=2000.0, blink.interval_ms as f32, move |v| {
                Message::TwinSetBlinkInterval(elem_id, v as u64)
            })
            .step(50.0_f32);

            binding_col = binding_col.push(trig_row).push(
                column![
                    text(format!("Blink Period: {} ms", blink.interval_ms))
                        .size(11)
                        .color(StudioTheme::TEXT_MUTED),
                    interval_slider,
                ]
                .spacing(2),
            );
        }
    }

    // Color Picker Section
    let off_rgba = elem
        .binding
        .as_ref()
        .map(|b| b.inactive_color)
        .unwrap_or(elem.base_color);
    let on_rgba = elem
        .binding
        .as_ref()
        .map(|b| b.active_color)
        .unwrap_or(elem.base_color);
    let base_rgba = elem.base_color;

    let target_tabs = row![
        button(text(format!("OFF ({})", rgb_to_hex(off_rgba))).size(10.5))
            .on_press(Message::TwinSelectColorTarget(
                ColorEditTarget::InactiveColor,
            ))
            .style(
                if ws.active_color_target == ColorEditTarget::InactiveColor {
                    StudioTheme::primary_button
                } else {
                    StudioTheme::secondary_button
                }
            )
            .padding([4, 7]),
        button(text(format!("ON ({})", rgb_to_hex(on_rgba))).size(10.5))
            .on_press(Message::TwinSelectColorTarget(ColorEditTarget::ActiveColor))
            .style(if ws.active_color_target == ColorEditTarget::ActiveColor {
                StudioTheme::primary_button
            } else {
                StudioTheme::secondary_button
            })
            .padding([4, 7]),
        button(text(format!("Base ({})", rgb_to_hex(base_rgba))).size(10.5))
            .on_press(Message::TwinSelectColorTarget(ColorEditTarget::BaseColor))
            .style(if ws.active_color_target == ColorEditTarget::BaseColor {
                StudioTheme::primary_button
            } else {
                StudioTheme::secondary_button
            })
            .padding([4, 7]),
    ]
    .spacing(4);

    let (hue, sat, val) = ws.hsv_state;
    let current_rgb = hsv_to_rgb(hue, sat, val, 255);

    let sv_canvas = canvas::Canvas::new(SvPickerCanvas { hue, sat, val })
        .width(Length::Fill)
        .height(Length::Fixed(110.0));

    let hue_canvas = canvas::Canvas::new(HueBarCanvas { hue })
        .width(Length::Fill)
        .height(Length::Fixed(18.0));

    let hex_input = text_input("#RRGGBB", &ws.hex_input)
        .on_input(Message::TwinSetColorHex)
        .width(Length::Fixed(95.0))
        .padding([4, 8])
        .style(StudioTheme::styled_input);

    let preview_swatch = container(Space::with_width(Length::Fixed(36.0)))
        .height(Length::Fixed(26.0))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(
                current_rgb[0],
                current_rgb[1],
                current_rgb[2],
            ))),
            border: Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        });

    let r_val = current_rgb[0];
    let g_val = current_rgb[1];
    let b_val = current_rgb[2];

    let rgb_sliders = column![
        row![
            text(format!("R:{r_val:3}")).size(10.5).width(42),
            slider(0.0..=255.0, r_val as f32, move |r| {
                Message::TwinSetColorRgb(r as u8, g_val, b_val)
            }),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text(format!("G:{g_val:3}")).size(10.5).width(42),
            slider(0.0..=255.0, g_val as f32, move |g| {
                Message::TwinSetColorRgb(r_val, g as u8, b_val)
            }),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
        row![
            text(format!("B:{b_val:3}")).size(10.5).width(42),
            slider(0.0..=255.0, b_val as f32, move |b| {
                Message::TwinSetColorRgb(r_val, g_val, b as u8)
            }),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(3);

    // Quick industrial swatches (Red OFF, Green ON, Amber, Blue, Cyan, Purple, Gray, White)
    let presets: [(&str, [u8; 3]); 6] = [
        ("Red", [231, 76, 60]),
        ("Green", [46, 204, 113]),
        ("Amber", [241, 196, 15]),
        ("Blue", [52, 152, 219]),
        ("Cyan", [0, 210, 211]),
        ("Gray", [127, 140, 141]),
    ];
    let mut swatch_row = row![].spacing(4);
    for (name, [pr, pg, pb]) in presets {
        swatch_row = swatch_row.push(
            button(text(name).size(10))
                .on_press(Message::TwinSetColorRgb(pr, pg, pb))
                .style(StudioTheme::ghost_button)
                .padding([2, 6]),
        );
    }

    let color_col = column![
        text(lang.tr(
            "Color Picker (OFF / ON / Base)",
            "Sélecteur de Couleur (OFF / ON / Base)"
        ))
        .size(12)
        .color(StudioTheme::TEXT_PRIMARY),
        target_tabs,
        sv_canvas,
        hue_canvas,
        row![
            text("Hex:").size(11).color(StudioTheme::TEXT_MUTED),
            hex_input,
            preview_swatch,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        rgb_sliders,
        swatch_row,
    ]
    .spacing(6);

    let content = scrollable(
        column![
            header,
            container(transform_col)
                .padding(8)
                .width(Length::Fill)
                .style(StudioTheme::card_style),
            container(binding_col)
                .padding(8)
                .width(Length::Fill)
                .style(StudioTheme::card_style),
            container(color_col)
                .padding(8)
                .width(Length::Fill)
                .style(StudioTheme::card_style),
        ]
        .spacing(10)
        .padding(Padding {
            top: 4.0,
            right: 10.0,
            bottom: 12.0,
            left: 4.0,
        }),
    )
    .height(Length::Fill);

    container(content)
        .width(Length::Fixed(340.0))
        .height(Length::Fill)
        .padding(8)
        .style(StudioTheme::sidebar_container_style)
        .into()
}

pub fn render_digital_twin<'a>(
    workspace: &'a DigitalTwinWorkspace,
    io_snapshot: SharedIoSnapshot,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let open_win_btn = button(
        text(lang.tr(
            "Open Digital Twin OS Window",
            "Ouvrir la fenêtre OS du Jumeau Numérique",
        ))
        .size(13),
    )
    .on_press(Message::OpenDigitalTwinWindow)
    .style(StudioTheme::primary_button)
    .padding([8, 16]);

    let canvas_widget = canvas::Canvas::new(TwinCanvasProgram {
        workspace,
        io_snapshot,
        elapsed_ms: 0,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    column![
        row![
            text(lang.tr(
                "2D Visual Digital Twin Plant Mimic & Interactive HMI",
                "Jumeau Numérique 2D & IHM Interactive",
            ))
            .size(16),
            Space::with_width(Length::Fill),
            open_win_btn,
        ]
        .align_y(Alignment::Center),
        container(canvas_widget)
            .padding(1)
            .style(StudioTheme::canvas_container_style)
            .width(Length::Fill)
            .height(Length::Fill),
    ]
    .spacing(12)
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
