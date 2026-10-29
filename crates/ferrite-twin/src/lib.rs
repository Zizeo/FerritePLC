pub mod draw;
pub mod model;
pub mod theme;
pub mod workspace;

pub use draw::{draw_canvas_element, draw_selection_overlay};
pub use model::{
    AccessMode, ActiveResize, ActuatorKind, BlinkConfig, CanvasElement, ElementKind, ResizeHandle,
    ShapeType, SharedIoHandle, SharedIoSnapshot, SharedIoState, VariableBinding,
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
}
