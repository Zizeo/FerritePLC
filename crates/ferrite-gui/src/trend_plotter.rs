use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use egui::{Color32, Ui};
use egui_plot::{Corner, Legend, Line, Plot, PlotPoints};
use ferrite_core::IecValue;
use ferrite_runtime::PlcRuntime;

use crate::i18n::UiLanguage;

const MAX_POINTS_PER_CHANNEL: usize = 2000;

#[derive(Debug, Clone)]
pub struct TrendChannel {
    pub name: String,
    pub color: Color32,
    pub enabled: bool,
    pub points: VecDeque<[f64; 2]>,
}

impl TrendChannel {
    pub fn new(name: String, color: Color32) -> Self {
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
            TrendChannel::new("SP (Setpoint)".to_string(), Color32::from_rgb(255, 215, 0)),
        );
        channels.insert(
            "PV".to_string(),
            TrendChannel::new("PV (Process Var)".to_string(), Color32::from_rgb(72, 202, 228)),
        );
        channels.insert(
            "OUT".to_string(),
            TrendChannel::new("OUT (Control)".to_string(), Color32::from_rgb(46, 213, 115)),
        );
        channels.insert(
            "ERR".to_string(),
            TrendChannel::new("ERR (Error)".to_string(), Color32::from_rgb(255, 107, 107)),
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
                    IecValue::Bool(b) => if b { 1.0 } else { 0.0 },
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

    pub fn render(&mut self, ui: &mut Ui, rt_opt: Option<&PlcRuntime>, lang: UiLanguage) {
        ui.horizontal_wrapped(|ui| {
            ui.strong(lang.tr("Trend Oscilloscope", "Oscilloscope Temps Réel"));
            ui.separator();

            let pause_txt = if self.paused {
                lang.tr("▶ Resume", "▶ Reprendre")
            } else {
                lang.tr("⏸ Pause", "⏸ Pause")
            };
            if ui.button(pause_txt).clicked() {
                self.paused = !self.paused;
            }
            if ui.button(lang.tr("× Clear", "× Effacer")).clicked() {
                self.clear_all();
            }

            ui.separator();
            ui.label(lang.tr("Window (s):", "Fenêtre (s) :"));
            ui.add(
                egui::DragValue::new(&mut self.time_window_s)
                    .range(5.0..=300.0)
                    .speed(1.0),
            );

            ui.separator();
            // Channel checkboxes
            let mut keys: Vec<String> = self.channels.keys().cloned().collect();
            keys.sort();
            for k in keys {
                if let Some(ch) = self.channels.get_mut(&k) {
                    ui.colored_label(ch.color, "■");
                    ui.checkbox(&mut ch.enabled, &ch.name);
                }
            }

            // Add custom tag channel
            if let Some(rt) = rt_opt {
                ui.separator();
                ui.label(lang.tr("Add Tag:", "Ajouter Variable :"));
                egui::ComboBox::from_id_salt("trend_add_tag_combo")
                    .selected_text(if self.new_channel_tag.is_empty() {
                        lang.tr("Select Tag...", "Sélectionner...")
                    } else {
                        &self.new_channel_tag
                    })
                    .show_ui(ui, |ui| {
                        for tag in rt.tags.tags() {
                            if !self.channels.contains_key(&tag.name)
                                && ui
                                    .selectable_value(
                                        &mut self.new_channel_tag,
                                        tag.name.clone(),
                                        &tag.name,
                                    )
                                    .clicked()
                            {
                                let color = match self.channels.len() % 4 {
                                    0 => Color32::from_rgb(198, 120, 221),
                                    1 => Color32::from_rgb(229, 192, 123),
                                    2 => Color32::from_rgb(97, 175, 239),
                                    _ => Color32::from_rgb(152, 195, 121),
                                };
                                self.channels
                                    .insert(tag.name.clone(), TrendChannel::new(tag.name.clone(), color));
                                self.new_channel_tag.clear();
                            }
                        }
                    });
            }
        });

        ui.add_space(6.0);

        let now_s = self.start_time.elapsed().as_secs_f64();
        let min_x = (now_s - self.time_window_s).max(0.0);
        let max_x = (now_s + 0.5).max(self.time_window_s);

        let plot = Plot::new("live_trend_plot")
            .legend(Legend::default().position(Corner::RightTop))
            .show_x(true)
            .show_y(true)
            .allow_zoom(true)
            .allow_drag(true)
            .include_x(min_x)
            .include_x(max_x)
            .height(180.0);

        plot.show(ui, |plot_ui| {
            for ch in self.channels.values() {
                if !ch.enabled || ch.points.is_empty() {
                    continue;
                }
                let pts: PlotPoints = ch.points.iter().copied().collect();
                let line = Line::new(pts)
                    .color(ch.color)
                    .name(&ch.name)
                    .width(1.8_f32);
                plot_ui.line(line);
            }
        });
    }
}
