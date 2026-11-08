use egui::{Color32, CornerRadius, FontId, Frame, Margin, Response, Stroke, Ui, vec2};

/// Obsidian industrial design tokens for FerritePLC Digital Twin.
pub struct TwinTheme;

impl TwinTheme {
    // Backgrounds
    pub const BG_OBSIDIAN: Color32 = Color32::from_rgb(9, 12, 16);
    pub const BG_PANEL: Color32 = Color32::from_rgb(13, 17, 23);
    pub const BG_ELEVATED: Color32 = Color32::from_rgb(22, 27, 34);
    pub const BG_SURFACE: Color32 = Color32::from_rgb(30, 36, 46);
    pub const BG_HOVER: Color32 = Color32::from_rgb(40, 48, 62);
    pub const BG_ACTIVE: Color32 = Color32::from_rgb(14, 116, 144);

    // Borders
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(38, 46, 58);
    pub const BORDER_CARD: Color32 = Color32::from_rgb(48, 56, 70);
    pub const BORDER_ACTIVE: Color32 = Color32::from_rgb(56, 189, 248);

    // Accents
    pub const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);
    pub const ACCENT_EMERALD: Color32 = Color32::from_rgb(16, 185, 129);
    pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 158, 11);
    pub const ACCENT_ROSE: Color32 = Color32::from_rgb(244, 63, 94);
    pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(168, 85, 247);

    // Typography
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 246, 252);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(180, 192, 206);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(125, 138, 155);

    /// Standard elevated card container frame.
    pub fn card_frame() -> Frame {
        Frame::NONE
            .fill(Self::BG_ELEVATED)
            .stroke(Stroke::new(1.0_f32, Self::BORDER_CARD))
            .corner_radius(CornerRadius::same(6))
            .inner_margin(Margin::same(8))
    }

    /// Renders an industrial pill badge.
    pub fn badge(ui: &mut Ui, text: &str, bg_color: Color32, text_color: Color32) -> Response {
        let (rect, response) = ui.allocate_exact_size(
            vec2(text.len() as f32 * 7.5 + 14.0, 18.0),
            egui::Sense::hover(),
        );
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
}
