use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

use ferrite_core::IecValue;
use ferrite_runtime::PlcRuntime;

use crate::theme::StudioTheme;

const MAX_POINTS_PER_CHANNEL: usize = 2000;

#[derive(Debug, Clone)]
pub struct TrendChannel {
    pub name: String,
    pub color: Color,
    pub enabled: bool,
    pub points: VecDeque<[f64; 2]>,
}

impl TrendChannel {
    pub fn new(name: String, color: Color) -> Self {
        Self {
            name,
            color,
            enabled: true,
            points: VecDeque::with_capacity(MAX_POINTS_PER_CHANNEL),
        }
    }

    pub fn push(&mut self, time_s: f64, val: f64, max_age_s: f64) {
        self.points.push_back([time_s, val]);
        while let Some(&[t, _]) = self.points.front() {
            if time_s - t > max_age_s || self.points.len() > MAX_POINTS_PER_CHANNEL {
                self.points.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }
}

pub struct TrendPlotter {
    pub start_time: Instant,
    pub paused: bool,
    pub time_window_s: f64,
    pub channels: HashMap<String, TrendChannel>,
    pub new_channel_tag: String,
}

impl Default for TrendPlotter {
    fn default() -> Self {
        let mut channels = HashMap::new();
        channels.insert(
            "SP".to_string(),
            TrendChannel::new(
                "SP (Setpoint)".to_string(),
                Color::from_rgb(1.0, 0.843, 0.0),
            ),
        );
        channels.insert(
            "PV".to_string(),
            TrendChannel::new(
                "PV (Process Var)".to_string(),
                Color::from_rgb(0.282, 0.792, 0.894),
            ),
        );
        channels.insert(
            "OUT".to_string(),
            TrendChannel::new(
                "OUT (Control)".to_string(),
                Color::from_rgb(0.180, 0.835, 0.451),
            ),
        );
        channels.insert(
            "ERR".to_string(),
            TrendChannel::new(
                "ERR (Error)".to_string(),
                Color::from_rgb(1.0, 0.420, 0.420),
            ),
        );

        Self {
            start_time: Instant::now(),
            paused: false,
            time_window_s: 30.0,
            channels,
            new_channel_tag: String::new(),
        }
    }
}

impl TrendPlotter {
    /// Samples current values from live `PlcRuntime` into the trend channels.
    pub fn sample(&mut self, rt: &PlcRuntime) {
        if self.paused {
            return;
        }
        let now_s = self.start_time.elapsed().as_secs_f64();
        let max_age = self.time_window_s * 1.5;

        // Auto-sample standard PID/corrector channels from first active corrector FB
        let first_corrector = rt.tags.fbs().iter().find(|fb| fb.kind.is_corrector());
        if let Some(fb) = first_corrector {
            if let Some(ch) = self.channels.get_mut("SP") {
                ch.push(now_s, fb.sp_real as f64, max_age);
            }
            if let Some(ch) = self.channels.get_mut("PV") {
                ch.push(now_s, fb.pv_real as f64, max_age);
            }
            if let Some(ch) = self.channels.get_mut("OUT") {
                ch.push(now_s, fb.out_real as f64, max_age);
            }
            if let Some(ch) = self.channels.get_mut("ERR") {
                ch.push(now_s, fb.err_real as f64, max_age);
            }
        }

        // Sample any custom registered symbolic tags
        for (tag_name, ch) in &mut self.channels {
            if matches!(tag_name.as_str(), "SP" | "PV" | "OUT" | "ERR") {
                continue;
            }
            if let Ok(val) = rt.read_value(tag_name) {
                let numeric_val = match val {
                    IecValue::Bool(b) => {
                        if b {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    IecValue::Int(i) => i as f64,
                    IecValue::Dint(d) => d as f64,
                    IecValue::Real(r) => r as f64,
                    IecValue::Time(ms) => ms as f64,
                    _ => 0.0,
                };
                ch.push(now_s, numeric_val, max_age);
            }
        }
    }

    pub fn clear_all(&mut self) {
        self.start_time = Instant::now();
        for ch in self.channels.values_mut() {
            ch.clear();
        }
    }
}

pub struct TrendCanvas<'a> {
    pub channels: &'a HashMap<String, TrendChannel>,
    pub time_window_s: f64,
    pub start_time: Instant,
}

impl<'a, Message> canvas::Program<Message> for TrendCanvas<'a> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = bounds.width;
        let height = bounds.height;

        // 1. Draw canvas dark background
        frame.fill_rectangle(
            Point::ORIGIN,
            Size::new(width, height),
            StudioTheme::BG_BASE,
        );

        let now_s = self.start_time.elapsed().as_secs_f64();
        let min_x = (now_s - self.time_window_s).max(0.0);
        let max_x = (now_s + 0.5).max(self.time_window_s);
        let dx = (max_x - min_x).max(1.0);

        // Find Y range
        let mut min_y = 0.0_f64;
        let mut max_y = 100.0_f64;
        for ch in self.channels.values() {
            if !ch.enabled {
                continue;
            }
            for &[_, y] in &ch.points {
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
        let dy = (max_y - min_y).max(1.0);

        // Margins for axes
        let pad_left = 48.0_f32;
        let pad_bottom = 24.0_f32;
        let pad_top = 10.0_f32;
        let pad_right = 16.0_f32;

        let plot_w = (width - pad_left - pad_right).max(10.0);
        let plot_h = (height - pad_top - pad_bottom).max(10.0);

        // 2. Draw Grid Lines
        let grid_stroke = Stroke::default()
            .with_color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))
            .with_width(1.0);

        for step in 0..=4 {
            let frac = step as f32 / 4.0;
            let y_pos = pad_top + plot_h * (1.0 - frac);
            let p1 = Point::new(pad_left, y_pos);
            let p2 = Point::new(pad_left + plot_w, y_pos);
            frame.stroke(&Path::line(p1, p2), grid_stroke);

            let val_label = min_y + (dy * frac as f64);
            frame.fill_text(canvas::Text {
                content: format!("{val_label:.1}"),
                position: Point::new(4.0, y_pos - 6.0),
                color: StudioTheme::TEXT_MUTED,
                size: iced::Pixels(10.0),
                ..Default::default()
            });
        }

        // Time axis labels
        for step in 0..=4 {
            let frac = step as f32 / 4.0;
            let x_pos = pad_left + plot_w * frac;
            let p1 = Point::new(x_pos, pad_top);
            let p2 = Point::new(x_pos, pad_top + plot_h);
            frame.stroke(&Path::line(p1, p2), grid_stroke);

            let t_label = min_x + (dx * frac as f64);
            frame.fill_text(canvas::Text {
                content: format!("{t_label:.0}s"),
                position: Point::new(x_pos - 10.0, pad_top + plot_h + 6.0),
                color: StudioTheme::TEXT_MUTED,
                size: iced::Pixels(10.0),
                ..Default::default()
            });
        }

        // 3. Draw Channel Data Lines
        for ch in self.channels.values() {
            if !ch.enabled || ch.points.len() < 2 {
                continue;
            }

            let path = Path::new(|builder| {
                let mut started = false;
                for &[t, val] in &ch.points {
                    if t < min_x - 1.0 {
                        continue;
                    }
                    let norm_x = ((t - min_x) / dx) as f32;
                    let norm_y = ((val - min_y) / dy) as f32;

                    let px = pad_left + norm_x * plot_w;
                    let py = pad_top + plot_h * (1.0 - norm_y.clamp(0.0, 1.0));

                    if !started {
                        builder.move_to(Point::new(px, py));
                        started = true;
                    } else {
                        builder.line_to(Point::new(px, py));
                    }
                }
            });

            let line_stroke = Stroke::default().with_color(ch.color).with_width(2.0);

            frame.stroke(&path, line_stroke);
        }

        vec![frame.into_geometry()]
    }
}
