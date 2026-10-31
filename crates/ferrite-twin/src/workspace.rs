use std::time::Instant;

use egui::{Color32, Pos2, RichText, Ui, pos2};
use ferrite_core::types::IecValue;
use serde::{Deserialize, Serialize};

use crate::draw::{draw_canvas_element, draw_selection_overlay};
use crate::model::{
    AccessMode, ActiveResize, ActuatorKind, BlinkConfig, ButtonActionMode, CanvasElement,
    ColorEditTarget, ElementKind, ResizeHandle, ShapeType, SharedIoHandle, TwinMode,
    VariableBinding, rgb_to_hex, rgb_to_hsv,
};
use crate::theme::TwinTheme;

const HANDLE_HIT_RADIUS: f32 = 8.0;
const MIN_ELEMENT_SIZE: f32 = 24.0;
pub const TWIN_REL_PATH: &str = "sim/digital_twin.json";

/// State for the Digital Twin Visual Canvas and Equipment Inspector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalTwinWorkspace {
    pub elements: Vec<CanvasElement>,
    pub selected_id: Option<u64>,
    #[serde(skip)]
    pub mode: TwinMode,
    #[serde(skip)]
    pub inspector_open_for: Option<u64>,
    #[serde(skip)]
    pub active_color_target: ColorEditTarget,
    #[serde(skip)]
    pub hsv_state: (f32, f32, f32),
    #[serde(skip)]
    pub hex_input: String,
    #[serde(skip)]
    pub active_rotate_id: Option<u64>,
    #[serde(skip)]
    pub active_momentary_id: Option<u64>,
    #[serde(skip)]
    pub drag_target_id: Option<u64>,
    #[serde(skip)]
    pub drag_start_mouse: Option<Pos2>,
    #[serde(skip)]
    pub drag_start_elem_pos: (f32, f32),
    #[serde(skip)]
    pub active_resize: Option<ActiveResize>,
    pub snap_to_grid: bool,
    pub grid_size: f32,
    #[serde(skip)]
    pub context_menu: Option<(Pos2, u64)>,
    pub next_id: u64,
    #[serde(skip)]
    pub session_start: Option<Instant>,
}

impl Default for DigitalTwinWorkspace {
    fn default() -> Self {
        Self::with_starter_plant()
    }
}

impl DigitalTwinWorkspace {
    /// Initialized with an industrial chemical reaction batch plant mimic.
    pub fn with_starter_plant() -> Self {
        Self {
            elements: Self::starter_model(),
            selected_id: None,
            mode: TwinMode::Edit,
            inspector_open_for: None,
            active_color_target: ColorEditTarget::InactiveColor,
            hsv_state: (0.0, 0.8, 0.8),
            hex_input: "#E74C3C".to_string(),
            active_rotate_id: None,
            active_momentary_id: None,
            drag_target_id: None,
            drag_start_mouse: None,
            drag_start_elem_pos: (0.0, 0.0),
            active_resize: None,
            snap_to_grid: true,
            grid_size: 10.0,
            context_menu: None,
            next_id: 100,
            session_start: Some(Instant::now()),
        }
    }

    pub fn starter_model() -> Vec<CanvasElement> {
        vec![
            // 1. Main Reaction Vessel (Tank)
            CanvasElement {
                id: 1,
                label: "Reaction Vessel (T-101)".to_string(),
                element_type: ElementKind::Tank,
                position: (260.0, 160.0),
                size: (180.0, 240.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 1,
                binding: Some(VariableBinding {
                    variable_id: "Tank_Level".to_string(),
                    access_mode: AccessMode::ReadOnly,
                    active_color: Color32::from_rgb(0, 150, 255).to_array(),
                    inactive_color: Color32::from_rgb(20, 70, 120).to_array(),
                    blink: None,
                }),
                base_color: Color32::from_rgb(120, 140, 160).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 2. Chemical Inlet Valve
            CanvasElement {
                id: 2,
                label: "Fill Valve (V-1)".to_string(),
                element_type: ElementKind::Valve,
                position: (120.0, 120.0),
                size: (64.0, 48.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 2,
                binding: Some(VariableBinding::new_digital(
                    "Fill_Valve",
                    Color32::from_rgb(46, 204, 113),
                    Color32::from_rgb(108, 122, 137),
                )),
                base_color: Color32::from_rgb(180, 190, 200).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 3. Inlet Feed Pipe Line
            CanvasElement {
                id: 3,
                label: "Inlet Line".to_string(),
                element_type: ElementKind::Pipe,
                position: (184.0, 140.0),
                size: (120.0, 12.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 0,
                binding: Some(VariableBinding::new_digital(
                    "Fill_Valve",
                    Color32::from_rgb(52, 152, 219),
                    Color32::from_rgb(70, 80, 95),
                )),
                base_color: Color32::from_rgb(70, 80, 95).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 4. Secondary Reagent Dosing Valve
            CanvasElement {
                id: 4,
                label: "Dosing Valve (V-2)".to_string(),
                element_type: ElementKind::Valve,
                position: (380.0, 90.0),
                size: (60.0, 44.0),
                rotation: 90.0,
                flip_h: false,
                flip_v: false,
                z_index: 2,
                binding: Some(VariableBinding::new_digital(
                    "Dosing_Valve",
                    Color32::from_rgb(46, 204, 113),
                    Color32::from_rgb(108, 122, 137),
                )),
                base_color: Color32::from_rgb(180, 190, 200).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 5. Agitator Mixer Motor
            CanvasElement {
                id: 5,
                label: "Mixer Motor (M-101)".to_string(),
                element_type: ElementKind::Actuator(ActuatorKind::AgitatorMixer),
                position: (315.0, 240.0),
                size: (70.0, 60.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 3,
                binding: Some(VariableBinding::new_digital(
                    "Agitator_Fast",
                    Color32::from_rgb(241, 196, 15),
                    Color32::from_rgb(80, 90, 105),
                )),
                base_color: Color32::from_rgb(80, 90, 105).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 6. Tank Bottom Drain Valve
            CanvasElement {
                id: 6,
                label: "Drain Valve (V-3)".to_string(),
                element_type: ElementKind::Valve,
                position: (320.0, 420.0),
                size: (60.0, 44.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 2,
                binding: Some(VariableBinding::new_digital(
                    "Drain_Valve",
                    Color32::from_rgb(46, 204, 113),
                    Color32::from_rgb(108, 122, 137),
                )),
                base_color: Color32::from_rgb(180, 190, 200).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 7. Discharge Transfer Pump
            CanvasElement {
                id: 7,
                label: "Discharge Pump (P-101)".to_string(),
                element_type: ElementKind::Actuator(ActuatorKind::CentrifugalPump),
                position: (440.0, 410.0),
                size: (70.0, 70.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 3,
                binding: Some(VariableBinding::new_digital(
                    "Pump_Active",
                    Color32::from_rgb(46, 204, 113),
                    Color32::from_rgb(100, 110, 125),
                )),
                base_color: Color32::from_rgb(100, 110, 125).to_array(),
                button_mode: ButtonActionMode::default(),
            },
            // 8. Operator HMI Push Buttons
            CanvasElement {
                id: 8,
                label: "START (PB)".to_string(),
                element_type: ElementKind::PushButton,
                position: (480.0, 280.0),
                size: (90.0, 36.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 4,
                binding: Some(VariableBinding {
                    variable_id: "Start_Button".to_string(),
                    access_mode: AccessMode::ReadWrite,
                    active_color: Color32::from_rgb(39, 174, 96).to_array(),
                    inactive_color: Color32::from_rgb(46, 204, 113).to_array(),
                    blink: None,
                }),
                base_color: Color32::from_rgb(46, 204, 113).to_array(),
                button_mode: ButtonActionMode::Toggle,
            },
            CanvasElement {
                id: 9,
                label: "STOP (PB)".to_string(),
                element_type: ElementKind::PushButton,
                position: (480.0, 330.0),
                size: (90.0, 36.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 4,
                binding: Some(VariableBinding {
                    variable_id: "Stop_Button".to_string(),
                    access_mode: AccessMode::ReadWrite,
                    active_color: Color32::from_rgb(192, 57, 43).to_array(),
                    inactive_color: Color32::from_rgb(231, 76, 60).to_array(),
                    blink: None,
                }),
                base_color: Color32::from_rgb(231, 76, 60).to_array(),
                button_mode: ButtonActionMode::Toggle,
            },
            // 10. Alarm Annunciator Beacon
            CanvasElement {
                id: 10,
                label: "Alarm Beacon".to_string(),
                element_type: ElementKind::IndicatorLed,
                position: (500.0, 210.0),
                size: (48.0, 48.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: 4,
                binding: Some(VariableBinding {
                    variable_id: "High_Temp_Alarm".to_string(),
                    access_mode: AccessMode::ReadOnly,
                    active_color: Color32::from_rgb(255, 75, 75).to_array(),
                    inactive_color: Color32::from_rgb(80, 30, 30).to_array(),
                    blink: Some(BlinkConfig {
                        enabled: true,
                        trigger_state: true,
                        interval_ms: 300,
                    }),
                }),
                base_color: Color32::from_rgb(180, 50, 50).to_array(),
                button_mode: ButtonActionMode::default(),
            },
        ]
    }

    /// Primary render entry point for the Digital Twin window or viewport.
    pub fn render(&mut self, ui: &mut Ui, shared_io_handle: &SharedIoHandle) {
        if self.session_start.is_none() {
            self.session_start = Some(Instant::now());
        }
        let elapsed_ms = self
            .session_start
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        // Header Control Bar
        ui.horizontal(|ui| {
            TwinTheme::badge(ui, "TWIN", TwinTheme::BG_SURFACE, TwinTheme::ACCENT_CYAN);
            ui.heading(
                RichText::new("Plant Mimic & HMI Canvas")
                    .strong()
                    .color(TwinTheme::TEXT_PRIMARY),
            );
            ui.add_space(16.0);

            ui.checkbox(&mut self.snap_to_grid, "Snap (10px)");
            ui.add_space(8.0);

            if ui
                .button("× Clear")
                .on_hover_text("Remove all equipment from canvas")
                .clicked()
            {
                self.elements.clear();
                self.selected_id = None;
            }

            if ui
                .button("↺ Reset Plant")
                .on_hover_text("Load starter plant mimic")
                .clicked()
            {
                *self = Self::with_starter_plant();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                TwinTheme::badge(
                    ui,
                    &format!("{} Elements", self.elements.len()),
                    TwinTheme::BG_SURFACE,
                    TwinTheme::TEXT_SECONDARY,
                );
            });
        });

        ui.separator();

        // 3-Column Studio Layout
        egui::SidePanel::left("digital_twin_toolbox_panel")
            .resizable(false)
            .exact_width(190.0)
            .show_inside(ui, |ui| {
                self.render_toolbox(ui);
            });

        egui::SidePanel::right("digital_twin_inspector_panel")
            .resizable(true)
            .default_width(280.0)
            .show_inside(ui, |ui| {
                self.render_inspector(ui, shared_io_handle);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.render_canvas(ui, shared_io_handle, elapsed_ms);
        });
    }

    /// 1. Sidebar Component Toolbox (Creation Palette)
    fn render_toolbox(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            TwinTheme::badge(ui, "TOOLBOX", TwinTheme::BG_SURFACE, TwinTheme::ACCENT_CYAN);
            ui.strong(RichText::new("Palette").color(TwinTheme::TEXT_PRIMARY));
        });
        ui.add_space(4.0);
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);

            // Group 1: Operator Controls
            TwinTheme::card_frame().show(ui, |ui| {
                ui.strong(
                    RichText::new("Operator Controls")
                        .color(TwinTheme::ACCENT_CYAN)
                        .size(11.0),
                );
                ui.add_space(2.0);
                if ui
                    .button("[PB] Push Button")
                    .on_hover_text("Momentary pushbutton (ReadWrite)")
                    .clicked()
                {
                    self.spawn_element(ElementKind::PushButton);
                }
                if ui
                    .button("[SW] Toggle Switch")
                    .on_hover_text("Maintained selector switch (ReadWrite)")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Switch);
                }
                if ui
                    .button("[LED] Indicator Lamp")
                    .on_hover_text("Status beacon / pilot lamp")
                    .clicked()
                {
                    self.spawn_element(ElementKind::IndicatorLed);
                }
            });

            ui.add_space(4.0);

            // Group 2: Piping & Vessels
            TwinTheme::card_frame().show(ui, |ui| {
                ui.strong(
                    RichText::new("Piping & Vessels")
                        .color(TwinTheme::ACCENT_CYAN)
                        .size(11.0),
                );
                ui.add_space(2.0);
                if ui
                    .button("[TK] Storage Tank")
                    .on_hover_text("Liquid storage / process tank with level gauge")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Tank);
                }
                if ui
                    .button("[XV] Solenoid Valve")
                    .on_hover_text("Discrete open/closed flow valve")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Valve);
                }
                if ui
                    .button("[━] Process Pipe")
                    .on_hover_text("Fluid pipe line with flow animation")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Pipe);
                }
            });

            ui.add_space(4.0);

            // Group 3: Actuators & Motors
            TwinTheme::card_frame().show(ui, |ui| {
                ui.strong(
                    RichText::new("Actuators & Motors")
                        .color(TwinTheme::ACCENT_CYAN)
                        .size(11.0),
                );
                ui.add_space(2.0);
                if ui
                    .button("[P] Centrifugal Pump")
                    .on_hover_text("Impeller pump with rotation animation")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Actuator(ActuatorKind::CentrifugalPump));
                }
                if ui
                    .button("[M] Agitator Mixer")
                    .on_hover_text("Tank mixer with oscillation animation")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Actuator(ActuatorKind::AgitatorMixer));
                }
                if ui
                    .button("[MOT] Electric Motor")
                    .on_hover_text("AC induction motor drive")
                    .clicked()
                {
                    self.spawn_element(ElementKind::Actuator(ActuatorKind::ElectricMotor));
                }
            });

            ui.add_space(4.0);

            // Group 4: Basic Shapes
            TwinTheme::card_frame().show(ui, |ui| {
                ui.strong(
                    RichText::new("Geometric Primitives")
                        .color(TwinTheme::TEXT_MUTED)
                        .size(11.0),
                );
                ui.add_space(2.0);
                if ui.button("■ Rectangle").clicked() {
                    self.spawn_element(ElementKind::BasicShape(ShapeType::Rectangle));
                }
                if ui.button("▢ Rounded Box").clicked() {
                    self.spawn_element(ElementKind::BasicShape(ShapeType::RoundedRect));
                }
                if ui.button("● Circle").clicked() {
                    self.spawn_element(ElementKind::BasicShape(ShapeType::Circle));
                }
                if ui.button("▲ Triangle").clicked() {
                    self.spawn_element(ElementKind::BasicShape(ShapeType::Triangle));
                }
            });
        });
    }

    /// 2. Interactive Canvas with Mouse Selection, Dragging, Corner Resizing, and Context Menu
    fn render_canvas(&mut self, ui: &mut Ui, shared_io_handle: &SharedIoHandle, elapsed_ms: u64) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let canvas_rect = response.rect;
        let mouse_pos = response.hover_pos();

        // 1. Take a zero-lock snapshot of shared I/O. Read lock is dropped IMMEDIATELY.
        let io_snapshot = shared_io_handle
            .read()
            .map(|g| g.snapshot())
            .unwrap_or_default();

        // Background grid
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(18, 22, 28));
        if self.snap_to_grid {
            let grid_step = self.grid_size * 2.0;
            let mut x = canvas_rect.min.x + ((canvas_rect.min.x % grid_step).abs());
            while x < canvas_rect.max.x {
                painter.line_segment(
                    [pos2(x, canvas_rect.min.y), pos2(x, canvas_rect.max.y)],
                    egui::Stroke::new(0.5_f32, Color32::from_rgb(28, 34, 44)),
                );
                x += grid_step;
            }
            let mut y = canvas_rect.min.y + ((canvas_rect.min.y % grid_step).abs());
            while y < canvas_rect.max.y {
                painter.line_segment(
                    [pos2(canvas_rect.min.x, y), pos2(canvas_rect.max.x, y)],
                    egui::Stroke::new(0.5_f32, Color32::from_rgb(28, 34, 44)),
                );
                y += grid_step;
            }
        }

        let time = (elapsed_ms as f32) / 1000.0;
        let mut request_continuous_repaint = false;

        // Hover test on corner handles of currently selected element
        let mut hovered_handle: Option<ResizeHandle> = None;
        if let Some(pos) = mouse_pos
            && let Some(sel_id) = self.selected_id
            && let Some(elem) = self.elements.iter().find(|e| e.id == sel_id)
        {
            hovered_handle = elem.hit_test_handle(pos, HANDLE_HIT_RADIUS);
            if let Some(h) = hovered_handle {
                ui.ctx().set_cursor_icon(h.cursor());
            }
        }

        // Sort elements by z_index for rendering
        let mut render_order: Vec<usize> = (0..self.elements.len()).collect();
        render_order.sort_by_key(|&idx| self.elements[idx].z_index);

        for &idx in &render_order {
            let elem = &self.elements[idx];

            // Evaluate dynamic I/O state
            let mut is_active = false;
            let mut analog_val = 0.0f32;

            if let Some(ref binding) = elem.binding {
                if let Some(b) = io_snapshot.get_bool(&binding.variable_id) {
                    is_active = b;
                }
                if let Some(f) = io_snapshot.get_f32(&binding.variable_id) {
                    analog_val = f;
                    if f > 0.0 {
                        is_active = true;
                    }
                }

                // Check blinking rule
                if let Some(ref blink) = binding.blink
                    && blink.enabled
                    && is_active == blink.trigger_state
                {
                    let phase = (elapsed_ms / blink.interval_ms).is_multiple_of(2);
                    is_active = phase;
                    request_continuous_repaint = true;
                }
            }

            // Draw element
            draw_canvas_element(&painter, elem, is_active, analog_val, time);

            // If selected, draw accent bounding box and 4 draggable corner resize handles
            if self.selected_id == Some(elem.id) {
                draw_selection_overlay(&painter, elem, hovered_handle, 5.0);
            }
        }

        // --- Mouse Handling: Selection, Dragging, Corner Resizing, Context Menu ---

        // 1. Right-Click Context Menu Handling
        if response.secondary_clicked()
            && let Some(pos) = mouse_pos
        {
            let hit_elem = self.elements.iter().rev().find(|e| e.rect().contains(pos));
            if let Some(e) = hit_elem {
                self.selected_id = Some(e.id);
                self.context_menu = Some((pos, e.id));
            } else {
                self.context_menu = None;
            }
        }

        // Render Active Context Menu Popup
        let mut context_action: Option<(String, u64)> = None;
        if let Some((menu_pos, elem_id)) = self.context_menu {
            egui::Area::new(egui::Id::new("digital_twin_context_menu_area"))
                .fixed_pos(menu_pos)
                .show(ui.ctx(), |ui| {
                    TwinTheme::card_frame().show(ui, |ui| {
                        ui.set_min_width(160.0);
                        if ui
                            .button("× Delete")
                            .on_hover_text("Remove element from canvas")
                            .clicked()
                        {
                            context_action = Some(("delete".to_string(), elem_id));
                        }
                        if ui
                            .button("+ Duplicate")
                            .on_hover_text("Clone with new ID")
                            .clicked()
                        {
                            context_action = Some(("duplicate".to_string(), elem_id));
                        }
                        ui.separator();
                        if ui.button("↺ Rotate 90°").clicked() {
                            context_action = Some(("rotate".to_string(), elem_id));
                        }
                        if ui.button("⇄ Flip Horizontal").clicked() {
                            context_action = Some(("flip_h".to_string(), elem_id));
                        }
                        if ui.button("⇅ Flip Vertical").clicked() {
                            context_action = Some(("flip_v".to_string(), elem_id));
                        }
                        ui.separator();
                        if ui.button("▲ Bring to Front").clicked() {
                            context_action = Some(("front".to_string(), elem_id));
                        }
                        if ui.button("▼ Send to Back").clicked() {
                            context_action = Some(("back".to_string(), elem_id));
                        }
                    });
                });

            // Close context menu on click outside
            if response.clicked() || response.drag_started() {
                self.context_menu = None;
            }
        }

        if let Some((action, elem_id)) = context_action {
            self.execute_context_action(&action, elem_id);
            self.context_menu = None;
        }

        // 2. Left-Click (Selection, Corner Resize Start, or Drag Start)
        let is_primary_click = response.clicked_by(egui::PointerButton::Primary);
        let is_primary_drag_start = response.drag_started_by(egui::PointerButton::Primary);

        if (is_primary_click || is_primary_drag_start)
            && let Some(pos) = mouse_pos
        {
            // Check if clicking on a corner resize handle of currently selected element
            let mut handle_grabbed = false;
            if let Some(sel_id) = self.selected_id
                && let Some(elem) = self.elements.iter().find(|e| e.id == sel_id)
                && let Some(handle) = elem.hit_test_handle(pos, HANDLE_HIT_RADIUS)
            {
                self.active_resize = Some(ActiveResize {
                    elem_id: elem.id,
                    handle,
                    start_mouse: pos,
                    start_pos: elem.position,
                    start_size: elem.size,
                });
                self.drag_target_id = None;
                handle_grabbed = true;
            }

            if !handle_grabbed {
                let hit_elem = self.elements.iter().rev().find(|e| e.rect().contains(pos));

                if let Some(e) = hit_elem {
                    self.selected_id = Some(e.id);
                    if is_primary_drag_start {
                        self.drag_target_id = Some(e.id);
                        self.drag_start_mouse = Some(pos);
                        self.drag_start_elem_pos = e.position;
                    }

                    // Interactive ReadWrite Control Click (NO DEADLOCK: read lock is already dropped)
                    if let Some(ref binding) = e.binding
                        && binding.access_mode == AccessMode::ReadWrite
                    {
                        match e.element_type {
                            ElementKind::PushButton => {
                                if let Ok(mut io) = shared_io_handle.write() {
                                    io.request_write(&binding.variable_id, IecValue::Bool(true));
                                    if is_primary_click {
                                        // Momentary pulse on quick click-release
                                        io.request_write(
                                            &binding.variable_id,
                                            IecValue::Bool(false),
                                        );
                                    }
                                }
                            }
                            ElementKind::Switch => {
                                let curr =
                                    io_snapshot.get_bool(&binding.variable_id).unwrap_or(false);
                                if let Ok(mut io) = shared_io_handle.write() {
                                    io.request_write(&binding.variable_id, IecValue::Bool(!curr));
                                }
                            }
                            _ => {}
                        }
                    }
                } else if is_primary_click {
                    self.selected_id = None;
                    self.drag_target_id = None;
                    self.active_resize = None;
                }
            }
        }

        // 3. Dragging: Corner Resizing vs Element Position Movement
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(curr_m) = mouse_pos
        {
            if let Some(resize) = self.active_resize {
                // Resize mode
                ui.ctx().set_cursor_icon(resize.handle.cursor());
                let delta = curr_m - resize.start_mouse;
                let (dx, dy) = if self.snap_to_grid {
                    let sx = (delta.x / self.grid_size).round() * self.grid_size;
                    let sy = (delta.y / self.grid_size).round() * self.grid_size;
                    (sx, sy)
                } else {
                    (delta.x, delta.y)
                };

                if let Some(elem) = self.elements.iter_mut().find(|e| e.id == resize.elem_id) {
                    match resize.handle {
                        ResizeHandle::BottomRight => {
                            let new_w = (resize.start_size.0 + dx).max(MIN_ELEMENT_SIZE);
                            let new_h = (resize.start_size.1 + dy).max(MIN_ELEMENT_SIZE);
                            elem.size = (new_w, new_h);
                        }
                        ResizeHandle::BottomLeft => {
                            let new_w = (resize.start_size.0 - dx).max(MIN_ELEMENT_SIZE);
                            let new_x = resize.start_pos.0 + (resize.start_size.0 - new_w);
                            let new_h = (resize.start_size.1 + dy).max(MIN_ELEMENT_SIZE);
                            elem.position.0 = new_x;
                            elem.size = (new_w, new_h);
                        }
                        ResizeHandle::TopRight => {
                            let new_w = (resize.start_size.0 + dx).max(MIN_ELEMENT_SIZE);
                            let new_h = (resize.start_size.1 - dy).max(MIN_ELEMENT_SIZE);
                            let new_y = resize.start_pos.1 + (resize.start_size.1 - new_h);
                            elem.position.1 = new_y;
                            elem.size = (new_w, new_h);
                        }
                        ResizeHandle::TopLeft => {
                            let new_w = (resize.start_size.0 - dx).max(MIN_ELEMENT_SIZE);
                            let new_x = resize.start_pos.0 + (resize.start_size.0 - new_w);
                            let new_h = (resize.start_size.1 - dy).max(MIN_ELEMENT_SIZE);
                            let new_y = resize.start_pos.1 + (resize.start_size.1 - new_h);
                            elem.position = (new_x, new_y);
                            elem.size = (new_w, new_h);
                        }
                    }
                }
            } else if let (Some(drag_id), Some(start_m)) =
                (self.drag_target_id, self.drag_start_mouse)
            {
                // Element movement mode
                let delta = curr_m - start_m;
                if let Some(elem) = self.elements.iter_mut().find(|e| e.id == drag_id) {
                    let mut new_x = self.drag_start_elem_pos.0 + delta.x;
                    let mut new_y = self.drag_start_elem_pos.1 + delta.y;
                    if self.snap_to_grid {
                        new_x = (new_x / self.grid_size).round() * self.grid_size;
                        new_y = (new_y / self.grid_size).round() * self.grid_size;
                    }
                    elem.position = (new_x, new_y);
                }
            }
        }

        // 4. Drag End / Release
        if response.drag_stopped() {
            self.active_resize = None;

            // If dragging momentary push button, release to false
            if let Some(drag_id) = self.drag_target_id
                && let Some(elem) = self.elements.iter().find(|e| e.id == drag_id)
                && let Some(ref binding) = elem.binding
                && binding.access_mode == AccessMode::ReadWrite
                && elem.element_type == ElementKind::PushButton
                && let Ok(mut io) = shared_io_handle.write()
            {
                io.request_write(&binding.variable_id, IecValue::Bool(false));
            }

            self.drag_target_id = None;
            self.drag_start_mouse = None;
        }

        if request_continuous_repaint {
            ui.ctx().request_repaint();
        }
    }

    /// Executes a context menu action (delete, duplicate, rotate, flip, z-order).
    pub fn execute_context_action(&mut self, action: &str, elem_id: u64) {
        match action {
            "delete" => {
                self.elements.retain(|e| e.id != elem_id);
                if self.selected_id == Some(elem_id) {
                    self.selected_id = None;
                }
                if self.inspector_open_for == Some(elem_id) {
                    self.inspector_open_for = None;
                }
            }
            "inspect" => {
                self.selected_id = Some(elem_id);
                self.inspector_open_for = Some(elem_id);
                self.sync_color_picker_from_selected();
            }
            "duplicate" => {
                if let Some(elem) = self.elements.iter().find(|e| e.id == elem_id).cloned() {
                    let mut clone = elem;
                    self.next_id = self.next_id.saturating_add(1);
                    clone.id = self.next_id;
                    clone.position.0 += 20.0;
                    clone.position.1 += 20.0;
                    clone.label = format!("{} (Copy)", clone.label);
                    self.elements.push(clone);
                    self.selected_id = Some(self.next_id);
                }
            }
            "rotate" => {
                if let Some(elem) = self.elements.iter_mut().find(|e| e.id == elem_id) {
                    elem.rotation = (elem.rotation + 90.0).rem_euclid(360.0);
                }
            }
            "rotate_ccw" => {
                if let Some(elem) = self.elements.iter_mut().find(|e| e.id == elem_id) {
                    elem.rotation = (elem.rotation - 90.0).rem_euclid(360.0);
                }
            }
            "flip_h" => {
                if let Some(elem) = self.elements.iter_mut().find(|e| e.id == elem_id) {
                    elem.flip_h = !elem.flip_h;
                }
            }
            "flip_v" => {
                if let Some(elem) = self.elements.iter_mut().find(|e| e.id == elem_id) {
                    elem.flip_v = !elem.flip_v;
                }
            }
            "front" => {
                if let Some(pos) = self.elements.iter().position(|e| e.id == elem_id) {
                    let elem = self.elements.remove(pos);
                    let max_z = self.elements.iter().map(|e| e.z_index).max().unwrap_or(0);
                    let mut elem = elem;
                    elem.z_index = max_z + 1;
                    self.elements.push(elem);
                }
            }
            "back" => {
                if let Some(pos) = self.elements.iter().position(|e| e.id == elem_id) {
                    let elem = self.elements.remove(pos);
                    let min_z = self.elements.iter().map(|e| e.z_index).min().unwrap_or(0);
                    let mut elem = elem;
                    elem.z_index = min_z.saturating_sub(1);
                    self.elements.insert(0, elem);
                }
            }
            _ => {}
        }
    }

    /// Spawns a new element onto the canvas at the default position.
    fn spawn_element(&mut self, kind: ElementKind) {
        self.spawn_element_at(kind, None);
    }

    /// Spawns a new element onto the canvas at an optional coordinate and returns its ID.
    pub fn spawn_element_at(&mut self, kind: ElementKind, pos: Option<(f32, f32)>) -> u64 {
        let max_existing = self.elements.iter().map(|e| e.id).max().unwrap_or(0);
        if self.next_id <= max_existing {
            self.next_id = max_existing;
        }
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        let (label, size, color) = match kind {
            ElementKind::BasicShape(ShapeType::Line) => (
                "Line".to_string(),
                (140.0, 6.0),
                Color32::from_rgb(140, 160, 185),
            ),
            ElementKind::BasicShape(ShapeType::Square) => (
                "Square".to_string(),
                (80.0, 80.0),
                Color32::from_rgb(100, 110, 130),
            ),
            ElementKind::BasicShape(ShapeType::Rectangle) => (
                "Rectangle".to_string(),
                (110.0, 65.0),
                Color32::from_rgb(100, 110, 130),
            ),
            ElementKind::BasicShape(ShapeType::RoundedRect) => (
                "Rounded Box".to_string(),
                (110.0, 65.0),
                Color32::from_rgb(90, 120, 140),
            ),
            ElementKind::BasicShape(ShapeType::Circle) => (
                "Circle".to_string(),
                (70.0, 70.0),
                Color32::from_rgb(120, 100, 140),
            ),
            ElementKind::BasicShape(ShapeType::Triangle) => (
                "Triangle".to_string(),
                (80.0, 70.0),
                Color32::from_rgb(140, 110, 90),
            ),
            ElementKind::PushButton => (
                "Push Button".to_string(),
                (96.0, 38.0),
                Color32::from_rgb(52, 152, 219),
            ),
            ElementKind::Switch => (
                "Toggle Switch".to_string(),
                (64.0, 32.0),
                Color32::from_rgb(45, 55, 70),
            ),
            ElementKind::IndicatorLed => (
                "LED".to_string(),
                (44.0, 44.0),
                Color32::from_rgb(231, 76, 60),
            ),
            ElementKind::Valve => (
                "Valve".to_string(),
                (64.0, 48.0),
                Color32::from_rgb(180, 190, 200),
            ),
            ElementKind::Pipe => (
                "Pipe".to_string(),
                (140.0, 14.0),
                Color32::from_rgb(80, 90, 105),
            ),
            ElementKind::Tank => (
                "Tank".to_string(),
                (150.0, 210.0),
                Color32::from_rgb(120, 140, 160),
            ),
            ElementKind::Actuator(ActuatorKind::CentrifugalPump) => (
                "Pump".to_string(),
                (72.0, 72.0),
                Color32::from_rgb(100, 110, 125),
            ),
            ElementKind::Actuator(ActuatorKind::AgitatorMixer) => (
                "Mixer".to_string(),
                (72.0, 64.0),
                Color32::from_rgb(80, 90, 105),
            ),
            ElementKind::Actuator(ActuatorKind::ElectricMotor) => (
                "Motor".to_string(),
                (68.0, 68.0),
                Color32::from_rgb(90, 100, 115),
            ),
        };

        let offset = ((self.elements.len() % 8) as f32) * 16.0;
        let position = pos.unwrap_or((180.0 + offset, 160.0 + offset));

        let elem = CanvasElement {
            id,
            label,
            position,
            size,
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            z_index: self.elements.len(),
            element_type: kind,
            binding: None,
            base_color: color.to_array(),
            button_mode: ButtonActionMode::default(),
        };

        self.elements.push(elem);
        self.selected_id = Some(id);
        self.sync_color_picker_from_selected();
        id
    }

    /// Synchronizes `hsv_state` and `hex_input` from the currently selected element and target color slot.
    pub fn sync_color_picker_from_selected(&mut self) {
        let Some(sel_id) = self.selected_id.or(self.inspector_open_for) else {
            return;
        };
        let Some(elem) = self.elements.iter().find(|e| e.id == sel_id) else {
            return;
        };
        let rgba = match self.active_color_target {
            ColorEditTarget::BaseColor => elem.base_color,
            ColorEditTarget::ActiveColor => elem
                .binding
                .as_ref()
                .map(|b| b.active_color)
                .unwrap_or([46, 204, 113, 255]),
            ColorEditTarget::InactiveColor => elem
                .binding
                .as_ref()
                .map(|b| b.inactive_color)
                .unwrap_or([231, 76, 60, 255]),
        };
        self.hsv_state = rgb_to_hsv(rgba);
        self.hex_input = rgb_to_hex(rgba);
    }

    /// Applies an RGBA color to the currently selected element's active color target slot.
    pub fn apply_color_to_selected(&mut self, rgba: [u8; 4]) {
        let Some(sel_id) = self.inspector_open_for.or(self.selected_id) else {
            return;
        };
        if let Some(elem) = self.elements.iter_mut().find(|e| e.id == sel_id) {
            match self.active_color_target {
                ColorEditTarget::BaseColor => {
                    elem.base_color = rgba;
                }
                ColorEditTarget::ActiveColor => {
                    if let Some(ref mut b) = elem.binding {
                        b.active_color = rgba;
                    } else {
                        elem.base_color = rgba;
                    }
                }
                ColorEditTarget::InactiveColor => {
                    if let Some(ref mut b) = elem.binding {
                        b.inactive_color = rgba;
                    } else {
                        elem.base_color = rgba;
                    }
                }
            }
        }
        self.hsv_state = rgb_to_hsv(rgba);
        self.hex_input = rgb_to_hex(rgba);
    }

    /// Loads `<project_root>/sim/digital_twin.json` if present and valid; otherwise returns starter plant.
    pub fn load_or_default(project_root: &std::path::Path) -> Self {
        let path = project_root.join(TWIN_REL_PATH);
        if let Ok(raw) = std::fs::read_to_string(&path)
            && let Ok(mut ws) = serde_json::from_str::<Self>(&raw)
        {
            ws.mode = TwinMode::Edit;
            ws.session_start = Some(Instant::now());
            let max_id = ws.elements.iter().map(|e| e.id).max().unwrap_or(100);
            if ws.next_id <= max_id {
                ws.next_id = max_id;
            }
            return ws;
        }
        Self::with_starter_plant()
    }

    /// Atomically saves the Digital Twin workspace to `<project_root>/sim/digital_twin.json`.
    pub fn save_to_project(&self, project_root: &std::path::Path) -> Result<(), String> {
        let sim_dir = project_root.join("sim");
        std::fs::create_dir_all(&sim_dir).map_err(|e| e.to_string())?;
        let target = project_root.join(TWIN_REL_PATH);
        let temp = project_root.join("sim/digital_twin.json.tmp");
        let serialized = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&temp, serialized).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &target).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 3. Sidebar Properties Inspector for the selected element
    fn render_inspector(&mut self, ui: &mut Ui, shared_io_handle: &SharedIoHandle) {
        ui.horizontal(|ui| {
            TwinTheme::badge(
                ui,
                "INSPECTOR",
                TwinTheme::BG_SURFACE,
                TwinTheme::ACCENT_CYAN,
            );
            ui.strong(RichText::new("Properties").color(TwinTheme::TEXT_PRIMARY));
        });
        ui.add_space(4.0);
        ui.separator();

        let Some(sel_id) = self.selected_id else {
            ui.add_space(16.0);
            TwinTheme::card_frame().show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No Element Selected")
                            .strong()
                            .color(TwinTheme::TEXT_SECONDARY),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Click any equipment or shape on the canvas to inspect transforms and link PLC I/O tags.")
                            .size(11.0)
                            .color(TwinTheme::TEXT_MUTED),
                    );
                });
            });
            return;
        };

        let (available_vars, io_snapshot) = if let Ok(io) = shared_io_handle.read() {
            (io.list_variables(), io.snapshot())
        } else {
            (Vec::new(), crate::model::SharedIoSnapshot::default())
        };

        if let Some(elem) = self.elements.iter_mut().find(|e| e.id == sel_id) {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Identity Header Card
                TwinTheme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        TwinTheme::badge(
                            ui,
                            &format!("ID: {}", elem.id),
                            TwinTheme::BG_SURFACE,
                            TwinTheme::ACCENT_CYAN,
                        );
                        TwinTheme::badge(
                            ui,
                            &format!("{:?}", elem.element_type),
                            TwinTheme::BG_SURFACE,
                            TwinTheme::TEXT_SECONDARY,
                        );
                    });
                });

                ui.add_space(4.0);

                // --- Transform Controls ---
                TwinTheme::card_frame().show(ui, |ui| {
                    ui.strong(
                        RichText::new("Transform & Geometry")
                            .color(TwinTheme::TEXT_PRIMARY)
                            .size(11.5),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Label:").color(TwinTheme::TEXT_MUTED));
                        ui.text_edit_singleline(&mut elem.label);
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("X:").color(TwinTheme::TEXT_MUTED));
                        ui.add(egui::DragValue::new(&mut elem.position.0).speed(1.0));
                        ui.label(RichText::new("Y:").color(TwinTheme::TEXT_MUTED));
                        ui.add(egui::DragValue::new(&mut elem.position.1).speed(1.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("W:").color(TwinTheme::TEXT_MUTED));
                        ui.add(
                            egui::DragValue::new(&mut elem.size.0)
                                .speed(1.0)
                                .range(MIN_ELEMENT_SIZE..=2000.0),
                        );
                        ui.label(RichText::new("H:").color(TwinTheme::TEXT_MUTED));
                        ui.add(
                            egui::DragValue::new(&mut elem.size.1)
                                .speed(1.0)
                                .range(MIN_ELEMENT_SIZE..=2000.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Rotation:").color(TwinTheme::TEXT_MUTED));
                        if ui.button(format!("{:.0}° (+90°)", elem.rotation)).clicked() {
                            elem.rotation = (elem.rotation + 90.0) % 360.0;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut elem.flip_h, "Flip H");
                        ui.checkbox(&mut elem.flip_v, "Flip V");
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Base Color:").color(TwinTheme::TEXT_MUTED));
                        let mut c = elem.base_color32();
                        if egui::color_picker::color_edit_button_srgba(
                            ui,
                            &mut c,
                            egui::color_picker::Alpha::Opaque,
                        )
                        .changed()
                        {
                            elem.base_color = c.to_array();
                        }
                    });
                });

                ui.add_space(4.0);

                // --- I/O Variable Linker ---
                TwinTheme::card_frame().show(ui, |ui| {
                    ui.strong(
                        RichText::new("PLC Variable Binding")
                            .color(TwinTheme::TEXT_PRIMARY)
                            .size(11.5),
                    );
                    ui.add_space(2.0);

                    let mut is_bound = elem.binding.is_some();
                    if ui.checkbox(&mut is_bound, "Bind to I/O Tag").changed() {
                        if is_bound {
                            let default_var = available_vars
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "Tank_Level".to_string());
                            elem.binding = Some(VariableBinding {
                                variable_id: default_var,
                                access_mode: match elem.element_type {
                                    ElementKind::PushButton | ElementKind::Switch => {
                                        AccessMode::ReadWrite
                                    }
                                    _ => AccessMode::ReadOnly,
                                },
                                active_color: Color32::from_rgb(50, 210, 90).to_array(),
                                inactive_color: Color32::from_rgb(100, 110, 125).to_array(),
                                blink: None,
                            });
                        } else {
                            elem.binding = None;
                        }
                    }

                    if let Some(ref mut binding) = elem.binding {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Variable:").color(TwinTheme::TEXT_MUTED));
                            egui::ComboBox::from_id_salt(format!("io_var_sel_{}", elem.id))
                                .selected_text(&binding.variable_id)
                                .show_ui(ui, |ui| {
                                    for v in &available_vars {
                                        ui.selectable_value(&mut binding.variable_id, v.clone(), v);
                                    }
                                });
                        });

                        // Live Value Readout
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Live Value:").color(TwinTheme::TEXT_MUTED));
                            if let Some(b) = io_snapshot.get_bool(&binding.variable_id) {
                                let (txt, col) = if b {
                                    ("● TRUE (ON)", TwinTheme::ACCENT_EMERALD)
                                } else {
                                    ("○ FALSE", TwinTheme::TEXT_MUTED)
                                };
                                ui.colored_label(col, RichText::new(txt).monospace().strong());
                            } else if let Some(f) = io_snapshot.get_f32(&binding.variable_id) {
                                ui.monospace(
                                    RichText::new(format!("{f:.2}"))
                                        .color(TwinTheme::ACCENT_CYAN)
                                        .strong(),
                                );
                            } else {
                                ui.colored_label(TwinTheme::TEXT_MUTED, "(none)");
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Mode:").color(TwinTheme::TEXT_MUTED));
                            ui.selectable_value(
                                &mut binding.access_mode,
                                AccessMode::ReadOnly,
                                "Read-Only",
                            );
                            ui.selectable_value(
                                &mut binding.access_mode,
                                AccessMode::ReadWrite,
                                "Read-Write",
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Active Color:").color(TwinTheme::TEXT_MUTED));
                            let mut c = Color32::from_rgba_premultiplied(
                                binding.active_color[0],
                                binding.active_color[1],
                                binding.active_color[2],
                                binding.active_color[3],
                            );
                            if egui::color_picker::color_edit_button_srgba(
                                ui,
                                &mut c,
                                egui::color_picker::Alpha::Opaque,
                            )
                            .changed()
                            {
                                binding.active_color = c.to_array();
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Inactive Color:").color(TwinTheme::TEXT_MUTED));
                            let mut c = Color32::from_rgba_premultiplied(
                                binding.inactive_color[0],
                                binding.inactive_color[1],
                                binding.inactive_color[2],
                                binding.inactive_color[3],
                            );
                            if egui::color_picker::color_edit_button_srgba(
                                ui,
                                &mut c,
                                egui::color_picker::Alpha::Opaque,
                            )
                            .changed()
                            {
                                binding.inactive_color = c.to_array();
                            }
                        });

                        ui.separator();
                        ui.strong(
                            RichText::new("Blinking & Alarm Rule")
                                .color(TwinTheme::TEXT_PRIMARY)
                                .size(11.5),
                        );
                        let mut blink_enabled = binding.blink.as_ref().is_some_and(|b| b.enabled);
                        if ui.checkbox(&mut blink_enabled, "Enable Blinking").changed() {
                            if blink_enabled {
                                binding.blink = Some(BlinkConfig::default());
                            } else {
                                binding.blink = None;
                            }
                        }

                        if let Some(ref mut blink) = binding.blink {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Trigger:").color(TwinTheme::TEXT_MUTED));
                                ui.selectable_value(&mut blink.trigger_state, true, "When TRUE");
                                ui.selectable_value(&mut blink.trigger_state, false, "When FALSE");
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Interval:").color(TwinTheme::TEXT_MUTED));
                                ui.add(
                                    egui::Slider::new(&mut blink.interval_ms, 100..=2000)
                                        .suffix("ms"),
                                );
                            });
                        }
                    }
                });
            });
        }
    }
}
