use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::model::{
    AccessMode, ActiveResize, ActuatorKind, BlinkConfig, ButtonActionMode, CanvasElement,
    ColorEditTarget, ElementKind, ShapeType, TwinMode, VariableBinding, rgb_to_hex, rgb_to_hsv,
};

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
    pub drag_start_mouse: Option<(f32, f32)>,
    #[serde(skip)]
    pub drag_start_elem_pos: (f32, f32),
    #[serde(skip)]
    pub active_resize: Option<ActiveResize>,
    pub snap_to_grid: bool,
    pub grid_size: f32,
    #[serde(skip)]
    pub context_menu: Option<((f32, f32), u64)>,
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
            grid_size: 20.0,
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
                    active_color: [0, 150, 255, 255],
                    inactive_color: [20, 70, 120, 255],
                    blink: None,
                }),
                base_color: [120, 140, 160, 255],
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
                    [46, 204, 113, 255],
                    [108, 122, 137, 255],
                )),
                base_color: [180, 190, 200, 255],
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
                    [52, 152, 219, 255],
                    [70, 80, 95, 255],
                )),
                base_color: [70, 80, 95, 255],
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
                    [46, 204, 113, 255],
                    [108, 122, 137, 255],
                )),
                base_color: [180, 190, 200, 255],
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
                    [241, 196, 15, 255],
                    [80, 90, 105, 255],
                )),
                base_color: [80, 90, 105, 255],
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
                    [46, 204, 113, 255],
                    [108, 122, 137, 255],
                )),
                base_color: [180, 190, 200, 255],
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
                    [46, 204, 113, 255],
                    [100, 110, 125, 255],
                )),
                base_color: [100, 110, 125, 255],
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
                    active_color: [39, 174, 96, 255],
                    inactive_color: [46, 204, 113, 255],
                    blink: None,
                }),
                base_color: [46, 204, 113, 255],
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
                    active_color: [192, 57, 43, 255],
                    inactive_color: [231, 76, 60, 255],
                    blink: None,
                }),
                base_color: [231, 76, 60, 255],
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
                    active_color: [255, 75, 75, 255],
                    inactive_color: [80, 30, 30, 255],
                    blink: Some(BlinkConfig {
                        enabled: true,
                        trigger_state: true,
                        interval_ms: 300,
                    }),
                }),
                base_color: [180, 50, 50, 255],
                button_mode: ButtonActionMode::default(),
            },
        ]
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

    /// Spawns a new element onto the canvas at an optional coordinate and returns its ID.
    pub fn spawn_element_at(&mut self, kind: ElementKind, pos: Option<(f32, f32)>) -> u64 {
        let max_existing = self.elements.iter().map(|e| e.id).max().unwrap_or(0);
        if self.next_id <= max_existing {
            self.next_id = max_existing;
        }
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        let (label, size, color) = match kind {
            ElementKind::BasicShape(ShapeType::Line) => {
                ("Line".to_string(), (140.0, 6.0), [140, 160, 185, 255])
            }
            ElementKind::BasicShape(ShapeType::Square) => {
                ("Square".to_string(), (80.0, 80.0), [100, 110, 130, 255])
            }
            ElementKind::BasicShape(ShapeType::Rectangle) => {
                ("Rectangle".to_string(), (110.0, 65.0), [100, 110, 130, 255])
            }
            ElementKind::BasicShape(ShapeType::RoundedRect) => (
                "Rounded Box".to_string(),
                (110.0, 65.0),
                [90, 120, 140, 255],
            ),
            ElementKind::BasicShape(ShapeType::Circle) => {
                ("Circle".to_string(), (70.0, 70.0), [120, 100, 140, 255])
            }
            ElementKind::BasicShape(ShapeType::Triangle) => {
                ("Triangle".to_string(), (80.0, 70.0), [140, 110, 90, 255])
            }
            ElementKind::PushButton => {
                ("Push Button".to_string(), (96.0, 38.0), [52, 152, 219, 255])
            }
            ElementKind::Switch => {
                ("Toggle Switch".to_string(), (64.0, 32.0), [45, 55, 70, 255])
            }
            ElementKind::IndicatorLed => ("LED".to_string(), (44.0, 44.0), [231, 76, 60, 255]),
            ElementKind::Valve => ("Valve".to_string(), (64.0, 48.0), [180, 190, 200, 255]),
            ElementKind::Pipe => ("Pipe".to_string(), (140.0, 14.0), [80, 90, 105, 255]),
            ElementKind::Tank => ("Tank".to_string(), (150.0, 210.0), [120, 140, 160, 255]),
            ElementKind::Actuator(ActuatorKind::CentrifugalPump) => {
                ("Pump".to_string(), (72.0, 72.0), [100, 110, 125, 255])
            }
            ElementKind::Actuator(ActuatorKind::AgitatorMixer) => {
                ("Mixer".to_string(), (72.0, 64.0), [80, 90, 105, 255])
            }
            ElementKind::Actuator(ActuatorKind::ElectricMotor) => {
                ("Motor".to_string(), (68.0, 68.0), [90, 100, 115, 255])
            }
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
            base_color: color,
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

    /// Applies an RGBA color to the currently selected element's active color target slot
    /// without recomputing `hsv_state` from RGB (preserves hue when S=0 or V=0).
    pub fn apply_color_to_selected_keep_hsv(&mut self, rgba: [u8; 4]) {
        let Some(sel_id) = self.selected_id.or(self.inspector_open_for) else {
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
        self.hex_input = rgb_to_hex(rgba);
    }

    /// Applies an RGBA color to the currently selected element's active color target slot.
    pub fn apply_color_to_selected(&mut self, rgba: [u8; 4]) {
        self.apply_color_to_selected_keep_hsv(rgba);
        self.hsv_state = rgb_to_hsv(rgba);
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
}
