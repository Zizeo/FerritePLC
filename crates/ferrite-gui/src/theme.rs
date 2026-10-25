use egui::{vec2, Color32, CornerRadius, FontId, Frame, Margin, Pos2, Response, RichText, Stroke, Ui};

/// Industrial obsidian design tokens for FerritePLC Studio.
pub struct StudioTheme;

impl StudioTheme {
    // Backgrounds
    pub const BG_OBSIDIAN: Color32 = Color32::from_rgb(9, 12, 16);     // Deep canvas / editor
    pub const BG_PANEL: Color32 = Color32::from_rgb(13, 17, 23);       // Dock / sidebar background
    pub const BG_ELEVATED: Color32 = Color32::from_rgb(22, 27, 34);    // Card / header background
    pub const BG_SURFACE: Color32 = Color32::from_rgb(30, 36, 46);     // Interactive surface
    pub const BG_HOVER: Color32 = Color32::from_rgb(40, 48, 62);       // Hovered surface
    pub const BG_ACTIVE: Color32 = Color32::from_rgb(14, 116, 144);    // Selected / pressed

    // Borders & Strokes
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(38, 46, 58);
    pub const BORDER_CARD: Color32 = Color32::from_rgb(48, 56, 70);
    pub const BORDER_ACTIVE: Color32 = Color32::from_rgb(56, 189, 248);

    // Accent Colors
    pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);   // High-tech electric cyan
    pub const ACCENT_EMERALD: Color32 = Color32::from_rgb(16, 185, 129);// Run / Normal / Good
    pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);  // Warning / Paused / Modified
    pub const ACCENT_ROSE: Color32 = Color32::from_rgb(244, 63, 94);    // Alarm / Error / Stop
    pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(168, 85, 247);// Structured Text (ST)

    // Typography Colors
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 246, 252);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(180, 192, 206);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(125, 138, 155);

    /// Standard card container frame.
    pub fn card_frame() -> Frame {
        Frame::NONE
            .fill(Self::BG_ELEVATED)
            .stroke(Stroke::new(1.0_f32, Self::BORDER_CARD))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::same(8))
    }

    /// Renders a modern industrial pill badge (e.g. `[LD]`, `[RUNNING]`, `[v0.1.0]`).
    pub fn badge(ui: &mut Ui, text: &str, bg_color: Color32, text_color: Color32) -> Response {
        let (rect, response) = ui.allocate_exact_size(vec2(text.len() as f32 * 7.5 + 14.0, 18.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 4.0, bg_color);
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::monospace(10.5),
            text_color,
        );
        response
    }

    /// Renders a telemetry capsule with a pulsing status dot.
    pub fn telemetry_pill(
        ui: &mut Ui,
        is_running: bool,
        scan_count: u64,
        time_ms: u64,
        duration_us: u64,
    ) {
        let dot_color = if is_running { Self::ACCENT_EMERALD } else { Self::ACCENT_AMBER };
        let border_color = if is_running {
            Color32::from_rgb(20, 80, 50)
        } else {
            Color32::from_rgb(80, 60, 20)
        };

        Frame::NONE
            .fill(Color32::from_rgb(16, 21, 29))
            .stroke(Stroke::new(1.0_f32, border_color))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Pulsing Dot
                    let (dot_rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot_rect.center(), 4.0, dot_color);
                    if is_running {
                        ui.painter().circle_stroke(
                            dot_rect.center(),
                            5.5,
                            Stroke::new(1.0_f32, dot_color.gamma_multiply(0.5)),
                        );
                    }

                    // Status Text
                    let status_txt = if is_running { "RUN" } else { "PAUSED" };
                    ui.colored_label(dot_color, RichText::new(status_txt).strong().size(11.0));

                    ui.colored_label(Self::BORDER_CARD, "|");

                    // Metrics in crisp monospace
                    ui.monospace(
                        RichText::new(format!("#{scan_count}  {time_ms}ms  {duration_us}µs"))
                            .color(Self::TEXT_SECONDARY)
                            .size(11.0),
                    );
                });
            });
    }

    /// Renders a sleek elevated document tab with language badge and active underline.
    pub fn document_tab(
        ui: &mut Ui,
        title: &str,
        badge_txt: &str,
        badge_color: Color32,
        is_active: bool,
        is_modified: bool,
    ) -> Response {
        let bg_fill = if is_active {
            Self::BG_SURFACE
        } else {
            Self::BG_PANEL
        };
        let border_stroke = if is_active {
            Stroke::new(1.0_f32, Self::ACCENT_CYAN)
        } else {
            Stroke::new(1.0_f32, Self::BORDER_SUBTLE)
        };

        let response = Frame::NONE
            .fill(bg_fill)
            .stroke(border_stroke)
            .corner_radius(CornerRadius::same(5))
            .inner_margin(Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        badge_color,
                        RichText::new(badge_txt).monospace().size(10.0).strong(),
                    );
                    let title_color = if is_active {
                        Self::TEXT_PRIMARY
                    } else {
                        Self::TEXT_SECONDARY
                    };
                    ui.label(RichText::new(title).color(title_color).size(12.0));

                    if is_modified {
                        ui.colored_label(Self::ACCENT_AMBER, "●");
                    }
                });
            })
            .response;

        let interactive = ui.interact(response.rect, response.id, egui::Sense::click());

        // Highlight line at bottom of active tab
        if is_active {
            let line_y = response.rect.max.y - 1.0;
            ui.painter().line_segment(
                [
                    Pos2::new(response.rect.min.x + 4.0, line_y),
                    Pos2::new(response.rect.max.x - 4.0, line_y),
                ],
                Stroke::new(2.0_f32, Self::ACCENT_CYAN),
            );
        }

        interactive
    }
}
