use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use egui::{Color32, Pos2, Rect, pos2, vec2};
use ferrite_core::types::{IecType, IecValue};
use serde::{Deserialize, Serialize};

/// Lightweight read-only snapshot of shared process I/O.
/// Allows zero-lock rendering so UI interactions can write without deadlocks.
#[derive(Debug, Clone, Default)]
pub struct SharedIoSnapshot {
    pub values: HashMap<String, IecValue>,
    pub types: HashMap<String, IecType>,
}

impl SharedIoSnapshot {
    pub fn get_bool(&self, var_name: &str) -> Option<bool> {
        self.values.get(var_name).and_then(|v| v.as_bool().ok())
    }

    pub fn get_f32(&self, var_name: &str) -> Option<f32> {
        self.values.get(var_name).and_then(|v| match v {
            IecValue::Real(r) => Some(*r),
            IecValue::Int(i) => Some(*i as f32),
            IecValue::Dint(d) => Some(*d as f32),
            IecValue::Time(t) => Some(*t as f32),
            IecValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        })
    }
}

/// Thread-safe shared process I/O state synchronized between Main PLC Runtime and Digital Twin.
#[derive(Debug, Clone, Default)]
pub struct SharedIoState {
    pub values: HashMap<String, IecValue>,
    pub types: HashMap<String, IecType>,
    pub pending_writes: Vec<(String, IecValue)>,
}

impl SharedIoState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_write(&mut self, variable: impl Into<String>, val: IecValue) {
        self.pending_writes.push((variable.into(), val));
    }

    pub fn take_pending_writes(&mut self) -> Vec<(String, IecValue)> {
        std::mem::take(&mut self.pending_writes)
    }

    pub fn list_variables(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.values.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn snapshot(&self) -> SharedIoSnapshot {
        SharedIoSnapshot {
            values: self.values.clone(),
            types: self.types.clone(),
        }
    }
}

pub type SharedIoHandle = Arc<RwLock<SharedIoState>>;

/// Geometric basic shape types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeType {
    Rectangle,
    Circle,
    Triangle,
    RoundedRect,
}

/// Industrial actuator equipment types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActuatorKind {
    ElectricMotor,
    CentrifugalPump,
    AgitatorMixer,
}

/// Equipment kinds supported by the Digital Twin canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementKind {
    BasicShape(ShapeType),
    Actuator(ActuatorKind),
    PushButton,
    Switch,
    IndicatorLed,
    Valve,
    Pipe,
    Tank,
}

/// Access mode: Read-Only (monitoring) vs Read-Write (interactive HMI control).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

/// Blinking behavior rule for alarms and indicator signals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlinkConfig {
    pub enabled: bool,
    pub trigger_state: bool,
    pub interval_ms: u64,
}

impl Default for BlinkConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trigger_state: true,
            interval_ms: 500,
        }
    }
}

/// I/O variable binding rules for dynamic visual animation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableBinding {
    pub variable_id: String,
    pub access_mode: AccessMode,
    pub active_color: [u8; 4],
    pub inactive_color: [u8; 4],
    pub blink: Option<BlinkConfig>,
}

impl VariableBinding {
    pub fn new_digital(variable_id: impl Into<String>, active: Color32, inactive: Color32) -> Self {
        Self {
            variable_id: variable_id.into(),
            access_mode: AccessMode::ReadOnly,
            active_color: active.to_array(),
            inactive_color: inactive.to_array(),
            blink: None,
        }
    }
}

/// 4 Interactive Corner Resize Handles for CAD / HMI elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeHandle {
    pub fn cursor(self) -> egui::CursorIcon {
        match self {
            Self::TopLeft | Self::BottomRight => egui::CursorIcon::ResizeNwSe,
            Self::TopRight | Self::BottomLeft => egui::CursorIcon::ResizeNeSw,
        }
    }
}

/// State tracking an ongoing corner resize drag operation.
#[derive(Debug, Clone, Copy)]
pub struct ActiveResize {
    pub elem_id: u64,
    pub handle: ResizeHandle,
    pub start_mouse: Pos2,
    pub start_pos: (f32, f32),
    pub start_size: (f32, f32),
}

/// An individual interactive plant equipment element on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasElement {
    pub id: u64,
    pub label: String,
    pub position: (f32, f32),
    pub size: (f32, f32),
    pub rotation: f32, // 0°, 90°, 180°, 270°
    pub flip_h: bool,
    pub flip_v: bool,
    pub z_index: usize,
    pub element_type: ElementKind,
    pub binding: Option<VariableBinding>,
    pub base_color: [u8; 4],
}

impl CanvasElement {
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(
            pos2(self.position.0, self.position.1),
            vec2(self.size.0, self.size.1),
        )
    }

    pub fn base_color32(&self) -> Color32 {
        Color32::from_rgba_premultiplied(
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            self.base_color[3],
        )
    }

    /// Returns the center positions and bounding rects of the 4 corner resize handles.
    pub fn corner_handles(&self, handle_radius: f32) -> [(ResizeHandle, Pos2, Rect); 4] {
        let r = self.rect();
        let pts = [
            (ResizeHandle::TopLeft, r.left_top()),
            (ResizeHandle::TopRight, r.right_top()),
            (ResizeHandle::BottomLeft, r.left_bottom()),
            (ResizeHandle::BottomRight, r.right_bottom()),
        ];
        pts.map(|(handle, pt)| {
            let handle_rect =
                Rect::from_center_size(pt, vec2(handle_radius * 2.0, handle_radius * 2.0));
            (handle, pt, handle_rect)
        })
    }

    /// Hit-tests whether a given mouse position is over one of the 4 corner resize handles.
    pub fn hit_test_handle(&self, pos: Pos2, handle_radius: f32) -> Option<ResizeHandle> {
        let handles = self.corner_handles(handle_radius);
        for (handle, _, rect) in handles {
            if rect.contains(pos) {
                return Some(handle);
            }
        }
        None
    }
}
