pub mod alarms_panel;
pub mod app;
pub mod canvas_fbd;
pub mod canvas_ld;
pub mod canvas_sfc;
pub mod digital_twin;
pub mod editor_st;
pub mod editor_symbols;
pub mod i18n;
pub mod theme;
pub mod trend_plotter;

use std::path::{Path, PathBuf};

pub use app::{FerriteStudioApp, UiTheme, WorkspaceTab};
use ferrite_project::{MANIFEST_FILE_NAME, Project, ProjectError};
pub use i18n::UiLanguage;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("Project error: {0}")]
    Project(#[from] ProjectError),

    #[error("Native GUI window error: {0}")]
    Eframe(String),
}

/// Resolves or scaffolds an initial `Project` for FerritePLC Studio.
pub fn resolve_or_scaffold_project(initial_path: Option<&Path>) -> Result<Project, ProjectError> {
    if let Some(p) = initial_path {
        if p.exists() && (p.is_file() || p.join(MANIFEST_FILE_NAME).exists()) {
            return Project::load(p);
        }
        return Project::scaffold(p, "my_automation_plant");
    }

    let cwd_manifest = PathBuf::from(MANIFEST_FILE_NAME);
    if cwd_manifest.exists() {
        return Project::load(Path::new("."));
    }

    let example_dir = PathBuf::from("examples").join("my_automation_plant");
    if example_dir.join(MANIFEST_FILE_NAME).exists() {
        return Project::load(&example_dir);
    }

    Project::scaffold(&example_dir, "my_automation_plant")
}

/// Launches the native FerritePLC Studio desktop IDE & PLC Simulator window.
pub fn run_gui(initial_path: Option<PathBuf>) -> Result<(), GuiError> {
    run_gui_with_lang(initial_path, UiLanguage::En)
}

static SYSTEM_FONTS: std::sync::OnceLock<Option<egui::FontDefinitions>> = std::sync::OnceLock::new();

/// Configures OS fallback fonts (`Segoe UI`, `Segoe UI Symbol`, `Segoe UI Emoji`, `Consolas` on Windows)
/// so Unicode symbols and box-drawing glyphs never render as missing-glyph squares (`□`).
pub fn configure_system_fonts(ctx: &egui::Context) {
    let font_id = egui::Id::new("ferrite_fonts_configured");
    let already_configured = ctx.data(|d| d.get_temp::<bool>(font_id).unwrap_or(false));
    if already_configured {
        return;
    }

    let defs = SYSTEM_FONTS.get_or_init(|| {
        let mut fonts = egui::FontDefinitions::default();
        let candidate_fonts = [
            ("segoeui", r"C:\Windows\Fonts\segoeui.ttf"),
            ("seguisym", r"C:\Windows\Fonts\seguisym.ttf"),
            ("seguiemj", r"C:\Windows\Fonts\seguiemj.ttf"),
            ("consola", r"C:\Windows\Fonts\consola.ttf"),
        ];

        let mut loaded_any = false;
        for (name, path) in candidate_fonts {
            if let Ok(bytes) = std::fs::read(path) {
                fonts.font_data.insert(
                    name.to_string(),
                    std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                );
                if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                    prop.push(name.to_string());
                }
                if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                    mono.push(name.to_string());
                }
                loaded_any = true;
            }
        }

        if loaded_any {
            Some(fonts)
        } else {
            None
        }
    });

    if let Some(ref fonts) = *defs {
        ctx.set_fonts(fonts.clone());
    }
    ctx.data_mut(|d| d.insert_temp(font_id, true));
}

/// Launches the native FerritePLC Studio desktop IDE & PLC Simulator window with a specific initial UI language (`En` or `Fr`).
pub fn run_gui_with_lang(initial_path: Option<PathBuf>, lang: UiLanguage) -> Result<(), GuiError> {
    let project = resolve_or_scaffold_project(initial_path.as_deref())?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("FerritePLC Studio — {}", project.manifest.name))
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([1024.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "FerritePLC Studio",
        options,
        Box::new(move |cc| {
            configure_system_fonts(&cc.egui_ctx);
            Ok(Box::new(FerriteStudioApp::with_language(project, lang)))
        }),
    )
    .map_err(|e| GuiError::Eframe(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_st::highlight_st_source;
    use std::env;
    use std::fs;

    #[test]
    fn test_studio_app_initialization_scenarios_and_i18n() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_gui_studio_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);
        assert_eq!(app.ui_lang, UiLanguage::En);
        assert!(app.diagnostics.is_empty());
        assert!(app.runtime.is_some());
        assert!(!app.detach_left_explorer);
        assert!(!app.detach_right_io);
        assert!(!app.detach_bottom_dock);
        app.detach_left_explorer = true;
        app.detach_right_io = true;
        app.detach_bottom_dock = true;
        assert!(app.detach_left_explorer && app.detach_right_io && app.detach_bottom_dock);

        app.execute_all_scenarios();
        assert_eq!(app.scenario_logs.len(), 1);
        assert!(app.scenario_logs[0].starts_with("[PASS]"));

        // Switch UI language to French and verify localized scenario and status output
        app.ui_lang = UiLanguage::Fr;
        assert_eq!(app.ui_lang.code(), "FR");
        assert_eq!(app.ui_lang.display_name(), "Français");
        app.execute_all_scenarios();
        assert_eq!(app.scenario_logs.len(), 1);
        assert!(app.scenario_logs[0].starts_with("[SUCCÈS]"));

        let job = highlight_st_source(
            "PROGRAM test\n (* comment *)\n IF %IX0.0 AND X1.state THEN %MW10 := 42; END_IF;\nEND_PROGRAM",
        );
        assert!(!job.sections.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_undo_redo_and_theme() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_gui_undo_redo_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);
        assert_eq!(app.ui_theme, UiTheme::Dark);
        app.ui_theme = UiTheme::Light;
        assert!(!app.ui_theme.visuals().dark_mode);

        assert!(!app.can_undo() && !app.can_redo());
        let original_name = app.project.manifest.name.clone();

        // Edit 1
        app.project.manifest.name = "edited_once".to_string();
        app.track_history();
        assert!(app.can_undo());

        app.undo();
        assert_eq!(app.project.manifest.name, original_name);
        assert!(app.can_redo());
        // Undo must not itself be recorded as a new edit
        app.track_history();
        assert!(app.can_redo());

        app.redo();
        assert_eq!(app.project.manifest.name, "edited_once");
        assert!(!app.can_redo());

        // A fresh edit after undo clears the redo stack
        app.undo();
        app.project.manifest.name = "branch".to_string();
        app.track_history();
        assert!(!app.can_redo());
        app.undo();
        assert_eq!(app.project.manifest.name, original_name);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_studio_app_modbus_and_trend() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_gui_modbus_trend_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // Trend plotter initial state and sampling
        assert!(!app.trend_plotter.channels.is_empty());
        assert!(app.trend_plotter.channels.contains_key("SP"));
        assert!(app.trend_plotter.channels.contains_key("PV"));
        assert!(app.trend_plotter.channels.contains_key("OUT"));

        // Step scan should sample trend plotter
        let tel = app.execute_plc_scan_tick()?;
        assert_eq!(tel.scan_count, 1);
        let sp_pts = app
            .trend_plotter
            .channels
            .get("SP")
            .map(|c| c.points.len())
            .unwrap_or(0);
        assert!(sp_pts >= 1);

        // Modbus TCP Server lifecycle
        app.modbus_port = 5099;
        assert!(app.modbus_server.is_none());
        app.toggle_modbus_server();
        assert!(app.modbus_server.is_some());

        // Test internal ping
        app.test_modbus_ping();
        assert!(
            app.modbus_last_test_result
                .as_ref()
                .is_some_and(|r| r.starts_with("PING OK"))
        );

        // Stop server
        app.toggle_modbus_server();
        assert!(app.modbus_server.is_none());

        // HAL GPIO Driver lifecycle (deactivated by default)
        assert!(!app.hal_enabled);
        app.hal_enabled = true;
        // Set simulated button on BCM 17 (maps to %IX0.0)
        app.hal_driver.set_mock_input(17, true)?;
        let _ = app.execute_plc_scan_tick()?;
        let pin_levels = app.hal_driver.pin_levels();
        assert_eq!(pin_levels.get(&17).copied(), Some(true));

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_digital_twin_workspace_and_shared_io() -> Result<(), Box<dyn std::error::Error>> {
        use crate::digital_twin::{
            AccessMode, ActuatorKind, BlinkConfig, CanvasElement, DigitalTwinWorkspace,
            ElementKind, ShapeType, SharedIoHandle, SharedIoState, VariableBinding,
        };
        use ferrite_core::types::{IecType, IecValue};
        use std::sync::{Arc, RwLock};

        // 1. Thread-safe SharedIoState operations
        let shared_io: SharedIoHandle = Arc::new(RwLock::new(SharedIoState::default()));
        {
            let mut io = shared_io.write().map_err(|e| format!("lock poisoned: {e}"))?;
            io.values.insert("Pump_Running".to_string(), IecValue::Bool(true));
            io.types.insert("Pump_Running".to_string(), IecType::Bool);
            io.pending_writes.push(("Manual_Start".to_string(), IecValue::Bool(true)));
        }

        {
            let mut io = shared_io.write().map_err(|e| format!("lock poisoned: {e}"))?;
            assert_eq!(io.values.get("Pump_Running"), Some(&IecValue::Bool(true)));
            let pending = io.take_pending_writes();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].0, "Manual_Start");
            assert_eq!(pending[0].1, IecValue::Bool(true));
        }

        // 2. DigitalTwinWorkspace element management
        let mut ws = DigitalTwinWorkspace::default();
        let initial_count = ws.elements.len();
        assert!(initial_count > 0, "Starter plant model must have elements");

        // Add each ElementKind variant
        let kinds = vec![
            ElementKind::BasicShape(ShapeType::Rectangle),
            ElementKind::BasicShape(ShapeType::Circle),
            ElementKind::Actuator(ActuatorKind::CentrifugalPump),
            ElementKind::Actuator(ActuatorKind::AgitatorMixer),
            ElementKind::PushButton,
            ElementKind::Switch,
            ElementKind::IndicatorLed,
            ElementKind::Valve,
            ElementKind::Pipe,
            ElementKind::Tank,
        ];

        for (idx, kind) in kinds.into_iter().enumerate() {
            let elem = CanvasElement {
                id: (1000 + idx) as u64,
                label: format!("Elem {idx}"),
                element_type: kind,
                position: (100.0 + (idx as f32) * 20.0, 100.0),
                size: (60.0, 60.0),
                rotation: 0.0,
                flip_h: false,
                flip_v: false,
                z_index: idx,
                binding: Some(VariableBinding {
                    variable_id: "Pump_Running".to_string(),
                    access_mode: AccessMode::ReadWrite,
                    active_color: egui::Color32::GREEN.to_array(),
                    inactive_color: egui::Color32::DARK_GRAY.to_array(),
                    blink: Some(BlinkConfig {
                        enabled: true,
                        trigger_state: true,
                        interval_ms: 500,
                    }),
                }),
                base_color: egui::Color32::LIGHT_GRAY.to_array(),
            };
            ws.elements.push(elem);
        }

        assert_eq!(ws.elements.len(), initial_count + 10);

        // Test element selection and hit testing
        ws.selected_id = Some(1000);
        assert_eq!(ws.selected_id, Some(1000));

        // Duplicate selected element
        let pre_dup_len = ws.elements.len();
        if let Some(pos) = ws.elements.iter().position(|e| e.id == 1000) {
            let mut clone = ws.elements[pos].clone();
            clone.id = 9999;
            clone.position.0 += 20.0;
            clone.position.1 += 20.0;
            ws.elements.push(clone);
            ws.selected_id = Some(9999);
        }
        assert_eq!(ws.elements.len(), pre_dup_len + 1);

        // Rotate selected element
        if let Some(pos) = ws.elements.iter().position(|e| e.id == 9999) {
            ws.elements[pos].rotation = (ws.elements[pos].rotation + 90.0) % 360.0;
            assert_eq!(ws.elements[pos].rotation, 90.0);
        }

        // Delete selected element
        ws.elements.retain(|e| e.id != 9999);
        assert_eq!(ws.elements.len(), pre_dup_len);

        // Bring to front
        if let Some(pos) = ws.elements.iter().position(|e| e.id == 1001) {
            let elem = ws.elements.remove(pos);
            ws.elements.push(elem);
            assert_eq!(ws.elements.last().map(|e| e.id), Some(1001));
        }

        // Send to back
        if let Some(pos) = ws.elements.iter().position(|e| e.id == 1001) {
            let elem = ws.elements.remove(pos);
            ws.elements.insert(0, elem);
            assert_eq!(ws.elements.first().map(|e| e.id), Some(1001));
        }

        // 3. Integration with FerriteStudioApp
        let temp_dir = std::env::temp_dir().join("ferrite_gui_twin_sync_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // Verify window toggle state
        assert!(!app.show_digital_twin_window);
        app.show_digital_twin_window = true;
        assert!(app.show_digital_twin_window);

        // Queue write from twin to runtime
        {
            let mut io = app.shared_io.write().map_err(|e| format!("lock poisoned: {e}"))?;
            io.pending_writes.push(("%IX0.0".to_string(), IecValue::Bool(true)));
        }

        // Sync shared I/O
        app.sync_shared_io();

        // Step scan to execute pending writes and verify
        let _ = app.execute_plc_scan_tick()?;
        if let Some(rt) = &app.runtime {
            let addr: ferrite_core::address::DirectAddress = "%IX0.0".parse()?;
            let val = rt.process_image.read(addr, IecType::Bool)?;
            assert_eq!(val, IecValue::Bool(true), "%IX0.0 must be true after syncing pending writes");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_ui_modernization_and_filtering() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_modern_ui_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // Verify initial filter states
        assert!(app.pou_filter_text.is_empty());
        assert!(app.io_filter_text.is_empty());
        assert_eq!(app.io_filter_area, None);

        // Filter POUs
        app.pou_filter_text = "safety".to_string();
        let query = app.pou_filter_text.to_lowercase();
        let matching_pous: Vec<_> = app
            .project
            .manifest
            .pous
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&query))
            .collect();
        assert!(!matching_pous.is_empty());

        // Filter I/O tags by area
        app.io_filter_area = Some(ferrite_core::MemoryArea::Input);
        if let Some(ref rt) = app.runtime {
            let inputs: Vec<_> = rt
                .tags
                .tags()
                .iter()
                .filter(|t| t.address.map(|a| a.area) == Some(ferrite_core::MemoryArea::Input))
                .collect();
            assert!(!inputs.is_empty());
        }

        // Verify theme tokens
        let card = theme::StudioTheme::card_frame();
        assert_eq!(card.fill, theme::StudioTheme::BG_ELEVATED);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
