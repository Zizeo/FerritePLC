use egui::{Color32, FontId, Rect, Stroke, StrokeKind, pos2};

use crate::model::{ActuatorKind, CanvasElement, ElementKind, ResizeHandle, ShapeType};

/// Renders a single equipment canvas element with its real-time active state and animations.
pub fn draw_canvas_element(
    painter: &egui::Painter,
    elem: &CanvasElement,
    is_active: bool,
    analog_val: f32,
    time: f32,
) {
    let rect = elem.rect();
    let display_color = if let Some(ref b) = elem.binding {
        if is_active {
            Color32::from_rgba_premultiplied(
                b.active_color[0],
                b.active_color[1],
                b.active_color[2],
                b.active_color[3],
            )
        } else {
            Color32::from_rgba_premultiplied(
                b.inactive_color[0],
                b.inactive_color[1],
                b.inactive_color[2],
                b.inactive_color[3],
            )
        }
    } else {
        elem.base_color32()
    };

    match elem.element_type {
        ElementKind::BasicShape(ShapeType::Line) => {
            painter.line_segment(
                [
                    pos2(rect.min.x, rect.center().y),
                    pos2(rect.max.x, rect.center().y),
                ],
                Stroke::new(rect.height().clamp(2.0, 24.0), display_color),
            );
        }
        ElementKind::BasicShape(ShapeType::Rectangle | ShapeType::Square) => {
            painter.rect_filled(rect, 0.0, display_color);
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0_f32, Color32::WHITE),
                StrokeKind::Inside,
            );
        }
        ElementKind::BasicShape(ShapeType::RoundedRect) => {
            painter.rect_filled(rect, 8.0, display_color);
            painter.rect_stroke(
                rect,
                8.0,
                Stroke::new(1.0_f32, Color32::WHITE),
                StrokeKind::Inside,
            );
        }
        ElementKind::BasicShape(ShapeType::Circle) => {
            let radius = rect.width().min(rect.height()) * 0.5;
            painter.circle_filled(rect.center(), radius, display_color);
            painter.circle_stroke(rect.center(), radius, Stroke::new(1.0_f32, Color32::WHITE));
        }
        ElementKind::BasicShape(ShapeType::Triangle) => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(rect.center().x, rect.min.y),
                    pos2(rect.max.x, rect.max.y),
                    pos2(rect.min.x, rect.max.y),
                ],
                display_color,
                Stroke::new(1.0_f32, Color32::WHITE),
            ));
        }
        ElementKind::PushButton => {
            let fill = if is_active {
                Color32::from_rgb(80, 240, 120)
            } else {
                display_color
            };
            painter.rect_filled(rect, 6.0, fill);
            painter.rect_stroke(
                rect,
                6.0,
                Stroke::new(2.0_f32, Color32::WHITE),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &elem.label,
                FontId::proportional(12.0),
                Color32::WHITE,
            );
        }
        ElementKind::Switch => {
            painter.rect_filled(rect, 14.0, Color32::from_rgb(45, 55, 70));
            painter.rect_stroke(
                rect,
                14.0,
                Stroke::new(1.5_f32, Color32::from_rgb(110, 125, 145)),
                StrokeKind::Inside,
            );
            let knob_x = if is_active {
                rect.max.x - 16.0
            } else {
                rect.min.x + 16.0
            };
            let knob_col = if is_active {
                Color32::GREEN
            } else {
                Color32::GRAY
            };
            painter.circle_filled(pos2(knob_x, rect.center().y), 12.0, knob_col);
            painter.circle_stroke(
                pos2(knob_x, rect.center().y),
                12.0,
                Stroke::new(1.5_f32, Color32::WHITE),
            );
        }
        ElementKind::IndicatorLed => {
            let radius = rect.width().min(rect.height()) * 0.5 - 2.0;
            let center = rect.center();
            painter.circle_filled(center, radius + 2.0, Color32::from_rgb(30, 35, 45));
            painter.circle_filled(center, radius, display_color);
            painter.circle_stroke(center, radius, Stroke::new(1.5_f32, Color32::WHITE));
            if is_active {
                painter.circle_stroke(
                    center,
                    radius + 4.0,
                    Stroke::new(1.5_f32, display_color.gamma_multiply(0.6)),
                );
            }
        }
        ElementKind::Valve => {
            let center = rect.center();
            let size = rect.width() * 0.35;
            // Left triangle
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(center.x - size, center.y - size * 0.7),
                    pos2(center.x - size, center.y + size * 0.7),
                    center,
                ],
                display_color,
                Stroke::new(1.0_f32, Color32::BLACK),
            ));
            // Right triangle
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(center.x + size, center.y - size * 0.7),
                    pos2(center.x + size, center.y + size * 0.7),
                    center,
                ],
                display_color,
                Stroke::new(1.0_f32, Color32::BLACK),
            ));
            // Stem and actuator
            painter.line_segment(
                [center, pos2(center.x, center.y - 12.0)],
                Stroke::new(2.0_f32, Color32::WHITE),
            );
            painter.circle_filled(pos2(center.x, center.y - 12.0), 4.0, display_color);
        }
        ElementKind::Pipe => {
            painter.rect_filled(rect, 2.0, elem.base_color32());
            if is_active {
                let dash = (time * 50.0) % 20.0;
                painter.line_segment(
                    [
                        pos2(rect.min.x + dash, rect.center().y),
                        pos2(rect.max.x, rect.center().y),
                    ],
                    Stroke::new(rect.height() * 0.5, display_color),
                );
            }
        }
        ElementKind::Tank => {
            painter.rect_stroke(
                rect,
                10.0,
                Stroke::new(3.0_f32, elem.base_color32()),
                StrokeKind::Inside,
            );
            let fill_pct = (analog_val / 100.0).clamp(0.0, 1.0);
            if fill_pct > 0.0 {
                let fluid_h = (rect.height() - 8.0) * fill_pct;
                let fluid_rect = Rect::from_min_max(
                    pos2(rect.min.x + 4.0, rect.max.y - 4.0 - fluid_h),
                    pos2(rect.max.x - 4.0, rect.max.y - 4.0),
                );
                painter.rect_filled(fluid_rect, 4.0, display_color);
            }
        }
        ElementKind::Actuator(ActuatorKind::CentrifugalPump) => {
            let center = rect.center();
            let radius = rect.width().min(rect.height()) * 0.45;
            painter.circle_filled(center, radius, display_color);
            painter.circle_stroke(center, radius, Stroke::new(2.0_f32, Color32::WHITE));
            let angle = if is_active { time * 12.0 } else { 0.0 };
            for i in 0..3 {
                let a = angle + i as f32 * (std::f32::consts::PI * 2.0 / 3.0);
                let tip = pos2(
                    center.x + a.cos() * (radius - 4.0),
                    center.y + a.sin() * (radius - 4.0),
                );
                painter.line_segment([center, tip], Stroke::new(2.0_f32, Color32::WHITE));
            }
        }
        ElementKind::Actuator(ActuatorKind::AgitatorMixer) => {
            let center = rect.center();
            painter.rect_filled(rect, 4.0, display_color);
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(1.5_f32, Color32::WHITE),
                StrokeKind::Inside,
            );
            let angle = if is_active { time * 14.0 } else { 0.0 };
            let blade = (angle.cos() * (rect.width() * 0.4)).abs().max(6.0);
            painter.line_segment(
                [
                    pos2(center.x - blade, center.y),
                    pos2(center.x + blade, center.y),
                ],
                Stroke::new(3.0_f32, Color32::YELLOW),
            );
        }
        ElementKind::Actuator(ActuatorKind::ElectricMotor) => {
            painter.rect_filled(rect, 4.0, display_color);
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(2.0_f32, Color32::WHITE),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "M",
                FontId::proportional(16.0),
                Color32::WHITE,
            );
        }
    }
}

/// Draws an accent bounding box and 4 interactive corner resize handles around the selected element.
pub fn draw_selection_overlay(
    painter: &egui::Painter,
    elem: &CanvasElement,
    hovered_handle: Option<ResizeHandle>,
    handle_radius: f32,
) {
    let elem_rect = elem.rect();

    // Selection border
    painter.rect_stroke(
        elem_rect,
        2.0,
        Stroke::new(2.0_f32, Color32::from_rgb(0, 170, 255)),
        StrokeKind::Outside,
    );

    // 4 Corner resize handles
    let handles = elem.corner_handles(handle_radius);
    for (handle, pt, _) in handles {
        let is_hovered = hovered_handle == Some(handle);
        let fill_col = if is_hovered {
            Color32::from_rgb(255, 230, 0)
        } else {
            Color32::from_rgb(0, 170, 255)
        };
        let r = if is_hovered {
            handle_radius + 2.0
        } else {
            handle_radius
        };

        painter.circle_filled(pt, r, fill_col);
        painter.circle_stroke(pt, r, Stroke::new(1.5_f32, Color32::WHITE));
    }

    // Dimension tooltip badge below element
    painter.text(
        pos2(elem_rect.center().x, elem_rect.max.y + 14.0),
        egui::Align2::CENTER_CENTER,
        format!("{:.0} × {:.0}", elem.size.0, elem.size.1),
        FontId::proportional(11.0),
        Color32::from_rgb(0, 170, 255),
    );
}
