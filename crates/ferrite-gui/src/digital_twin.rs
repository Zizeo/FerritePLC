use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke, event};
use iced::widget::{column, container, row, text};
use iced::{Alignment, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

pub use ferrite_twin::*;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub struct TwinCanvasProgram<'a> {
    pub workspace: &'a DigitalTwinWorkspace,
    pub io_snapshot: SharedIoSnapshot,
}

impl<'a> canvas::Program<Message> for TwinCanvasProgram<'a> {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        canvas_event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        if let canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = canvas_event
            && let Some(cursor_pos) = cursor.position_in(bounds)
        {
            // Hit test elements in reverse order (top z-index first)
            for elem in self.workspace.elements.iter().rev() {
                let rx = elem.position.0;
                let ry = elem.position.1;
                let rw = elem.size.0;
                let rh = elem.size.1;
                if cursor_pos.x >= rx
                    && cursor_pos.x <= rx + rw
                    && cursor_pos.y >= ry
                    && cursor_pos.y <= ry + rh
                {
                    return (
                        event::Status::Captured,
                        Some(Message::DigitalTwinToggleElement(elem.id)),
                    );
                }
            }
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

        // Grid dots
        let dot_color = Color::from_rgba(1.0, 1.0, 1.0, 0.05);
        for x in (0..width as i32).step_by(20) {
            for y in (0..height as i32).step_by(20) {
                frame.fill_rectangle(
                    Point::new(x as f32, y as f32),
                    Size::new(1.5, 1.5),
                    dot_color,
                );
            }
        }

        // Draw elements
        for elem in &self.workspace.elements {
            let px = elem.position.0;
            let py = elem.position.1;
            let sw = elem.size.0;
            let sh = elem.size.1;

            let is_bound_active = elem
                .binding
                .as_ref()
                .and_then(|b| {
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
                })
                .unwrap_or(false);

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

            let elem_color = if is_bound_active {
                StudioTheme::ACCENT_EMERALD
            } else {
                Color::from_rgb8(elem.base_color[0], elem.base_color[1], elem.base_color[2])
            };

            let is_selected = self.workspace.selected_id == Some(elem.id);

            match elem.element_type {
                ElementKind::Tank => {
                    let border_stroke = Stroke::default()
                        .with_color(if is_selected {
                            StudioTheme::ACCENT_CYAN
                        } else {
                            Color::from_rgb(0.4, 0.5, 0.6)
                        })
                        .with_width(2.0);
                    frame.stroke_rectangle(Point::new(px, py), Size::new(sw, sh), border_stroke);

                    let level_pct = analog_val
                        .map(|v| (v / 100.0).clamp(0.0, 1.0))
                        .unwrap_or(0.65);
                    let liquid_h = sh * level_pct;
                    let liquid_y = py + (sh - liquid_h);
                    frame.fill_rectangle(
                        Point::new(px + 2.0, liquid_y),
                        Size::new(sw - 4.0, liquid_h - 2.0),
                        Color::from_rgba(0.2, 0.6, 0.9, 0.4),
                    );

                    frame.fill_text(canvas::Text {
                        content: format!("{} ({:.0}%)", elem.label, level_pct * 100.0),
                        position: Point::new(px + 6.0, py + 6.0),
                        color: StudioTheme::TEXT_PRIMARY,
                        size: iced::Pixels(11.0),
                        ..Default::default()
                    });
                }
                ElementKind::Pipe => {
                    frame.fill_rectangle(
                        Point::new(px, py),
                        Size::new(sw, sh),
                        if is_bound_active {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            Color::from_rgb(0.3, 0.35, 0.4)
                        },
                    );
                }
                ElementKind::Actuator(_) => {
                    let radius = (sw.min(sh) / 2.0).max(4.0);
                    let center = Point::new(px + sw / 2.0, py + sh / 2.0);
                    let stroke = Stroke::default()
                        .with_color(if is_selected {
                            StudioTheme::ACCENT_CYAN
                        } else {
                            elem_color
                        })
                        .with_width(2.0);
                    frame.stroke(&Path::circle(center, radius), stroke);
                    if is_bound_active {
                        frame.fill(
                            &Path::circle(center, radius - 2.0),
                            Color::from_rgba(0.063, 0.725, 0.506, 0.3),
                        );
                        let vane_stroke = Stroke::default()
                            .with_color(StudioTheme::ACCENT_EMERALD)
                            .with_width(1.5);
                        let vane_len = radius * 0.7;
                        frame.stroke(
                            &Path::line(
                                Point::new(center.x - vane_len, center.y),
                                Point::new(center.x + vane_len, center.y),
                            ),
                            vane_stroke,
                        );
                        frame.stroke(
                            &Path::line(
                                Point::new(center.x, center.y - vane_len),
                                Point::new(center.x, center.y + vane_len),
                            ),
                            vane_stroke,
                        );
                    }
                    frame.fill_text(canvas::Text {
                        content: elem.label.clone(),
                        position: Point::new(px, py + sh + 4.0),
                        color: StudioTheme::TEXT_SECONDARY,
                        size: iced::Pixels(11.0),
                        ..Default::default()
                    });
                }
                ElementKind::Valve => {
                    let path = Path::new(|b| {
                        b.move_to(Point::new(px, py));
                        b.line_to(Point::new(px + sw, py + sh));
                        b.line_to(Point::new(px + sw, py));
                        b.line_to(Point::new(px, py + sh));
                        b.close();
                    });
                    frame.fill(&path, elem_color);
                    frame.stroke(
                        &path,
                        Stroke::default()
                            .with_color(StudioTheme::TEXT_PRIMARY)
                            .with_width(1.0),
                    );
                    frame.fill_text(canvas::Text {
                        content: elem.label.clone(),
                        position: Point::new(px, py + sh + 4.0),
                        color: StudioTheme::TEXT_SECONDARY,
                        size: iced::Pixels(11.0),
                        ..Default::default()
                    });
                }
                ElementKind::IndicatorLed => {
                    let center = Point::new(px + sw / 2.0, py + sh / 2.0);
                    let radius = (sw.min(sh) / 2.0).max(4.0);
                    let led_color = if is_bound_active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        Color::from_rgb(0.3, 0.1, 0.1)
                    };
                    frame.fill(&Path::circle(center, radius), led_color);
                    frame.stroke(
                        &Path::circle(center, radius),
                        Stroke::default()
                            .with_color(StudioTheme::TEXT_PRIMARY)
                            .with_width(1.0),
                    );
                    frame.fill_text(canvas::Text {
                        content: elem.label.clone(),
                        position: Point::new(px + sw + 6.0, py + 2.0),
                        color: StudioTheme::TEXT_PRIMARY,
                        size: iced::Pixels(11.0),
                        ..Default::default()
                    });
                }
                ElementKind::PushButton | ElementKind::Switch => {
                    let btn_bg = if is_bound_active {
                        StudioTheme::ACCENT_INDIGO
                    } else {
                        StudioTheme::BG_ELEVATED
                    };
                    frame.fill_rectangle(Point::new(px, py), Size::new(sw, sh), btn_bg);
                    let border_color = if is_selected {
                        StudioTheme::ACCENT_CYAN
                    } else {
                        StudioTheme::BORDER_ACTIVE
                    };
                    frame.stroke_rectangle(
                        Point::new(px, py),
                        Size::new(sw, sh),
                        Stroke::default().with_color(border_color).with_width(1.5),
                    );
                    frame.fill_text(canvas::Text {
                        content: elem.label.clone(),
                        position: Point::new(px + 6.0, py + sh / 2.0 - 5.0),
                        color: StudioTheme::TEXT_PRIMARY,
                        size: iced::Pixels(11.0),
                        ..Default::default()
                    });
                }
                ElementKind::BasicShape(ShapeType::Rectangle | ShapeType::RoundedRect) => {
                    frame.fill_rectangle(Point::new(px, py), Size::new(sw, sh), elem_color);
                    frame.stroke_rectangle(
                        Point::new(px, py),
                        Size::new(sw, sh),
                        Stroke::default()
                            .with_color(StudioTheme::BORDER_CARD)
                            .with_width(1.0),
                    );
                }
                ElementKind::BasicShape(ShapeType::Circle) => {
                    let radius = sw.min(sh) / 2.0;
                    let center = Point::new(px + sw / 2.0, py + sh / 2.0);
                    frame.fill(&Path::circle(center, radius), elem_color);
                }
                _ => {
                    frame.fill_rectangle(Point::new(px, py), Size::new(sw, sh), elem_color);
                }
            }

            if is_selected {
                let sel_stroke = Stroke::default()
                    .with_color(StudioTheme::ACCENT_CYAN)
                    .with_width(1.5);
                frame.stroke_rectangle(
                    Point::new(px - 3.0, py - 3.0),
                    Size::new(sw + 6.0, sh + 6.0),
                    sel_stroke,
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

pub fn render_digital_twin<'a>(
    workspace: &'a DigitalTwinWorkspace,
    io_snapshot: SharedIoSnapshot,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let toolbar = row![
        text(lang.tr(
            "2D Visual Digital Twin Plant Mimic & Interactive HMI",
            "Jumeau Numérique 2D & IHM Interactive",
        ))
        .size(16),
        container(
            text(format!("{} Elements", workspace.elements.len()))
                .size(11)
                .color(StudioTheme::ACCENT_CYAN),
        )
        .padding([3, 8])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            0.220, 0.741, 0.973, 0.12
        ))),
        iced::widget::Space::with_width(Length::Fill),
        text(lang.tr(
            "Click PushButtons & Switches to toggle PLC inputs in real-time",
            "Cliquez sur les boutons/commutateurs pour basculer les entrées API"
        ))
        .size(12)
        .color(StudioTheme::TEXT_MUTED),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let canvas_widget = canvas::Canvas::new(TwinCanvasProgram {
        workspace,
        io_snapshot,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let canvas_card = container(canvas_widget)
        .padding(1)
        .style(StudioTheme::canvas_container_style)
        .width(Length::Fill)
        .height(Length::Fill);

    column![toolbar, canvas_card,]
        .spacing(12)
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
