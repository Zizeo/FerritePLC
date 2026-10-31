pub mod draw;
pub mod model;
pub mod theme;
pub mod workspace;

pub use draw::{draw_canvas_element, draw_selection_overlay};
pub use model::{
    AccessMode, ActiveResize, ActuatorKind, BlinkConfig, ButtonActionMode, CanvasElement,
    ColorEditTarget, ElementKind, ResizeHandle, ShapeType, SharedIoHandle, SharedIoSnapshot,
    SharedIoState, TwinMode, VariableBinding, hex_to_rgb, hsv_to_rgb, rgb_to_hex, rgb_to_hsv,
};
pub use theme::TwinTheme;
pub use workspace::DigitalTwinWorkspace;

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, pos2};
    use ferrite_core::types::{IecType, IecValue};
    use std::sync::{Arc, RwLock};

    #[test]
    fn test_zero_deadlock_write_and_snapshot() -> Result<(), Box<dyn std::error::Error>> {
        let shared: SharedIoHandle = Arc::new(RwLock::new(SharedIoState::default()));

        // Populate state
        {
            let mut io = shared.write().map_err(|e| e.to_string())?;
            io.values
                .insert("Pump_Running".to_string(), IecValue::Bool(true));
            io.values
                .insert("Tank_Level".to_string(), IecValue::Real(78.5));
            io.types.insert("Pump_Running".to_string(), IecType::Bool);
            io.types.insert("Tank_Level".to_string(), IecType::Real);
        }

        // Take snapshot (drops read guard immediately)
        let snap = shared.read().map_err(|e| e.to_string())?.snapshot();
        assert_eq!(snap.get_bool("Pump_Running"), Some(true));
        assert_eq!(snap.get_f32("Tank_Level"), Some(78.5));

        // Simultaneous write during what would be render loop - MUST NOT DEADLOCK
        {
            let mut io = shared.write().map_err(|e| e.to_string())?;
            io.request_write("Start_Button", IecValue::Bool(true));
            let writes = io.take_pending_writes();
            assert_eq!(writes.len(), 1);
            assert_eq!(writes[0].0, "Start_Button");
            assert_eq!(writes[0].1, IecValue::Bool(true));
        }
        Ok(())
    }

    #[test]
    fn test_corner_handle_hit_test_and_resizing() {
        let elem = CanvasElement {
            id: 42,
            label: "Test Vessel".to_string(),
            position: (100.0, 100.0),
            size: (200.0, 150.0),
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            z_index: 0,
            element_type: ElementKind::Tank,
            binding: None,
            base_color: Color32::GRAY.to_array(),
            button_mode: ButtonActionMode::default(),
        };

        // Top-Left corner is at (100.0, 100.0)
        assert_eq!(
            elem.hit_test_handle(pos2(100.0, 100.0), 8.0),
            Some(ResizeHandle::TopLeft)
        );
        // Top-Right corner is at (300.0, 100.0)
        assert_eq!(
            elem.hit_test_handle(pos2(300.0, 100.0), 8.0),
            Some(ResizeHandle::TopRight)
        );
        // Bottom-Left corner is at (100.0, 250.0)
        assert_eq!(
            elem.hit_test_handle(pos2(100.0, 250.0), 8.0),
            Some(ResizeHandle::BottomLeft)
        );
        // Bottom-Right corner is at (300.0, 250.0)
        assert_eq!(
            elem.hit_test_handle(pos2(300.0, 250.0), 8.0),
            Some(ResizeHandle::BottomRight)
        );
        // Center of element should NOT hit any corner handle
        assert_eq!(elem.hit_test_handle(pos2(200.0, 175.0), 8.0), None);

        // Verify cursor mappings
        assert_eq!(ResizeHandle::TopLeft.cursor(), egui::CursorIcon::ResizeNwSe);
        assert_eq!(
            ResizeHandle::BottomRight.cursor(),
            egui::CursorIcon::ResizeNwSe
        );
        assert_eq!(
            ResizeHandle::TopRight.cursor(),
            egui::CursorIcon::ResizeNeSw
        );
        assert_eq!(
            ResizeHandle::BottomLeft.cursor(),
            egui::CursorIcon::ResizeNeSw
        );
    }

    #[test]
    fn test_starter_plant_model_and_actions() -> Result<(), Box<dyn std::error::Error>> {
        let mut ws = DigitalTwinWorkspace::with_starter_plant();
        assert!(ws.elements.len() >= 10);

        // Verify START push button exists and has read-write access
        let start_pb = ws
            .elements
            .iter()
            .find(|e| e.label.starts_with("START"))
            .ok_or("START push button must exist")?;
        assert_eq!(start_pb.element_type, ElementKind::PushButton);
        assert_eq!(
            start_pb.binding.as_ref().map(|b| b.access_mode),
            Some(AccessMode::ReadWrite)
        );

        // Duplicate action
        let pre_len = ws.elements.len();
        ws.execute_context_action("duplicate", start_pb.id);
        assert_eq!(ws.elements.len(), pre_len + 1);

        // Rotate action
        let dup_id = ws.next_id;
        ws.execute_context_action("rotate", dup_id);
        let dup_elem = ws
            .elements
            .iter()
            .find(|e| e.id == dup_id)
            .ok_or("duplicated elem")?;
        assert_eq!(dup_elem.rotation, 90.0);

        // Delete action
        ws.execute_context_action("delete", dup_id);
        assert_eq!(ws.elements.len(), pre_len);
        Ok(())
    }

    #[test]
    fn test_twin_rotated_hit_test_and_line_square_shapes() -> Result<(), Box<dyn std::error::Error>> {
        let mut ws = DigitalTwinWorkspace::default();
        let line_id = ws.spawn_element_at(
            ElementKind::BasicShape(ShapeType::Line),
            Some((100.0, 200.0)),
        );
        let sq_id = ws.spawn_element_at(
            ElementKind::BasicShape(ShapeType::Square),
            Some((300.0, 300.0)),
        );
        assert!(ws.elements.iter().any(|e| e.id == line_id));
        assert!(ws.elements.iter().any(|e| e.id == sq_id));

        // Create a 120x12 horizontal line centered at (200, 200) -> position (140, 194)
        let mut line_elem = CanvasElement {
            id: 999,
            label: "Process Line".to_string(),
            position: (140.0, 194.0),
            size: (120.0, 12.0),
            rotation: 0.0,
            flip_h: false,
            flip_v: false,
            z_index: 1,
            element_type: ElementKind::BasicShape(ShapeType::Line),
            binding: None,
            base_color: [255, 255, 255, 255],
            button_mode: ButtonActionMode::Toggle,
        };

        // Unrotated: hits (250, 200), misses (200, 250)
        assert!(line_elem.contains_point(250.0, 200.0));
        assert!(!line_elem.contains_point(200.0, 250.0));

        // Rotate 90 degrees around center (200, 200) -> now vertical!
        line_elem.rotation = 90.0;
        assert!(!line_elem.contains_point(250.0, 200.0));
        assert!(line_elem.contains_point(200.0, 250.0));

        // Rotation handle hit test
        let (rh_x, rh_y) = line_elem.rotation_handle_pos();
        assert!(line_elem.hit_test_rotation_handle(rh_x, rh_y, 8.0));
        assert!(!line_elem.hit_test_rotation_handle(0.0, 0.0, 8.0));

        Ok(())
    }

    #[test]
    fn test_hsv_and_hex_color_conversions() {
        let orig = [255, 0, 128, 255];
        let hex = rgb_to_hex(orig);
        assert_eq!(hex, "#FF0080");
        assert_eq!(hex_to_rgb("#FF0080", 255), Some(orig));
        assert_eq!(hex_to_rgb("00FF7F", 255), Some([0, 255, 127, 255]));
        // Invalid hex inputs must return None safely
        assert_eq!(hex_to_rgb("#GG0011", 255), None);
        assert_eq!(hex_to_rgb("123", 255), None);
        assert_eq!(hex_to_rgb("", 255), None);

        let (h, s, v) = rgb_to_hsv([0, 255, 0, 255]);
        assert!((h - 120.0).abs() < 1.0);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);
        let back = hsv_to_rgb(h, s, v, 255);
        assert_eq!(back, [0, 255, 0, 255]);
    }

    #[test]
    fn test_twin_project_disk_persistence_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ferrite_twin_test_{now_nanos}"));
        std::fs::create_dir_all(&temp_dir)?;

        let mut ws = DigitalTwinWorkspace::with_starter_plant();
        let id = ws.spawn_element_at(
            ElementKind::BasicShape(ShapeType::Line),
            Some((420.0, 180.0)),
        );
        ws.selected_id = Some(id);
        ws.active_color_target = ColorEditTarget::ActiveColor;
        if let Some(elem) = ws.elements.iter_mut().find(|e| e.id == id) {
            elem.binding = Some(VariableBinding {
                variable_id: "Alarm_Out".to_string(),
                access_mode: AccessMode::ReadOnly,
                active_color: [0, 255, 0, 255],
                inactive_color: [255, 0, 0, 255],
                blink: Some(BlinkConfig {
                    enabled: true,
                    trigger_state: true,
                    interval_ms: 250,
                }),
            });
        }
        ws.apply_color_to_selected([10, 240, 80, 255]);

        ws.save_to_project(&temp_dir)?;
        let loaded = DigitalTwinWorkspace::load_or_default(&temp_dir);
        assert_eq!(loaded.elements.len(), ws.elements.len());
        let loaded_line = loaded
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or("saved line element missing")?;
        let binding = loaded_line.binding.as_ref().ok_or("binding missing")?;
        assert_eq!(binding.variable_id, "Alarm_Out");
        assert_eq!(binding.active_color, [10, 240, 80, 255]);
        assert_eq!(binding.inactive_color, [255, 0, 0, 255]);
        assert_eq!(
            binding.blink.as_ref().map(|b| b.interval_ms),
            Some(250)
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
