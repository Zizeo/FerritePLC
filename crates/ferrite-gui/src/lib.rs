pub mod alarms_panel;
pub mod app;
pub mod canvas_fbd;
pub mod canvas_ld;
pub mod canvas_sfc;
pub mod digital_twin;
pub mod editor_st;
pub mod editor_symbols;
pub mod help_window;
pub mod i18n;
pub mod theme;
pub mod trend_plotter;

use std::path::{Path, PathBuf};

pub use app::{FerriteStudioApp, Message, UiTheme, WorkspaceTab};
use ferrite_project::{MANIFEST_FILE_NAME, Project, ProjectError};
pub use i18n::UiLanguage;
pub use theme::StudioTheme;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GuiError {
    #[error("Project error: {0}")]
    Project(#[from] ProjectError),

    #[error("Native GUI window error: {0}")]
    Iced(String),
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

/// Launches the native FerritePLC Studio desktop IDE & PLC Simulator window with a specific initial UI language (`En` or `Fr`).
pub fn run_gui_with_lang(initial_path: Option<PathBuf>, lang: UiLanguage) -> Result<(), GuiError> {
    let project = resolve_or_scaffold_project(initial_path.as_deref())?;
    let mut app = FerriteStudioApp::with_language(project, lang);

    let (main_id, main_open_task) = iced::window::open(iced::window::Settings {
        size: iced::Size::new(1440.0, 880.0),
        min_size: Some(iced::Size::new(1024.0, 680.0)),
        exit_on_close_request: false,
        ..Default::default()
    });
    app.main_window_id = Some(main_id);

    iced::daemon(
        FerriteStudioApp::title,
        FerriteStudioApp::update,
        FerriteStudioApp::view,
    )
    .subscription(FerriteStudioApp::subscription)
    .theme(FerriteStudioApp::theme)
    .run_with(move || (app, main_open_task.map(|_| Message::None)))
    .map_err(|e| GuiError::Iced(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_lang_st::parse_st_statements;
    use ferrite_project::PouSource;
    use std::env;
    use std::fs;

    #[test]
    fn test_studio_app_initialization_scenarios_and_i18n() -> Result<(), Box<dyn std::error::Error>>
    {
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
        assert!(app.show_left_explorer);
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

        let st_src = "PROGRAM test\n (* comment *)\n IF %IX0.0 AND X1.state THEN %MW10 := 42; END_IF;\nEND_PROGRAM";
        let parse_res = parse_st_statements(st_src, "test");
        assert!(parse_res.is_ok());

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
        assert!(app.ui_theme.is_dark());
        app.ui_theme = UiTheme::Light;
        assert!(!app.ui_theme.is_dark());

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
            let mut io = shared_io
                .write()
                .map_err(|e| format!("lock poisoned: {e}"))?;
            io.values
                .insert("Pump_Running".to_string(), IecValue::Bool(true));
            io.types.insert("Pump_Running".to_string(), IecType::Bool);
            io.pending_writes
                .push(("Manual_Start".to_string(), IecValue::Bool(true)));
        }

        {
            let mut io = shared_io
                .write()
                .map_err(|e| format!("lock poisoned: {e}"))?;
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
                    active_color: [0, 255, 0, 255],
                    inactive_color: [50, 50, 50, 255],
                    blink: Some(BlinkConfig {
                        enabled: true,
                        trigger_state: true,
                        interval_ms: 500,
                    }),
                }),
                base_color: [200, 200, 200, 255],
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
            let mut io = app
                .shared_io
                .write()
                .map_err(|e| format!("lock poisoned: {e}"))?;
            io.pending_writes
                .push(("%IX0.0".to_string(), IecValue::Bool(true)));
        }

        // Sync shared I/O
        app.sync_shared_io();

        // Step scan to execute pending writes and verify
        let _ = app.execute_plc_scan_tick()?;
        if let Some(rt) = &app.runtime {
            let addr: ferrite_core::address::DirectAddress = "%IX0.0".parse()?;
            let val = rt.process_image.read(addr, IecType::Bool)?;
            assert_eq!(
                val,
                IecValue::Bool(true),
                "%IX0.0 must be true after syncing pending writes"
            );
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
        let card = theme::StudioTheme::card_style(&iced::Theme::Dark);
        assert_eq!(
            card.background,
            Some(iced::Background::Color(theme::StudioTheme::BG_ELEVATED))
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_step_rows_and_digital_twin_example_sync() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = env::temp_dir().join("ferrite_gui_sfc_twin_sync_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // 1. Verify batch_seq SFC rows group AND divergence parallel branches properly
        if let Some(PouSource::Sfc(batch_sfc)) = app.project.pous.get("batch_seq") {
            let rows = canvas_sfc::compute_sfc_step_rows(batch_sfc);
            assert!(rows.len() >= 4);
            // Row 0 has Step 0
            assert_eq!(rows[0].len(), 1);
            // Row 1 has Step 1 and Step 2 in parallel columns (AND Divergence)
            assert_eq!(rows[1].len(), 2);
            let s1_name = &batch_sfc.steps[rows[1][0]].name;
            let s2_name = &batch_sfc.steps[rows[1][1]].name;
            assert!(s1_name.contains("Fill") || s1_name.contains("Dose"));
            assert!(s2_name.contains("Fill") || s2_name.contains("Dose"));
            let _element = canvas_sfc::render_sfc_editor(
                batch_sfc,
                app.runtime.as_ref(),
                app.ui_lang,
                None,
                &[],
            );
        }

        // 2. Test interactive ToggleForceTag for purely symbolic booleans
        let _ = app.update(Message::ToggleForceTag("High_Temp_Alarm".to_string()));
        if let Some(ref rt) = app.runtime {
            let val = rt.read_value("High_Temp_Alarm")?.as_bool()?;
            assert!(val);
        }
        // Toggle off
        let _ = app.update(Message::ToggleForceTag("High_Temp_Alarm".to_string()));
        if let Some(ref rt) = app.runtime {
            let val = rt.read_value("High_Temp_Alarm")?.as_bool()?;
            assert!(!val);
        }

        // 3. Test SFC Manual Divergence builder via Message (manual spawning without automatic dummy steps)
        let original_step_count =
            if let Some(PouSource::Sfc(sfc)) = app.project.pous.get("batch_seq") {
                sfc.steps.len()
            } else {
                0
            };
        let _ = app.update(Message::SfcAddAndDivergence("batch_seq".to_string()));
        if let Some(PouSource::Sfc(sfc)) = app.project.pous.get("batch_seq") {
            assert_eq!(sfc.steps.len(), original_step_count);
            let last_tr = sfc.transitions.last().ok_or("no transition found")?;
            assert_eq!(last_tr.divergence, ferrite_lang_sfc::DivergenceKind::And);
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_multi_window_detachment_and_schematic_canvas_rendering()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_gui_multi_window_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // 1. Verify Detach & Dock Lifecycle for Left Explorer
        assert!(!app.detach_left_explorer);
        assert!(app.show_left_explorer);
        assert!(app.detached_left_id.is_none());
        let _ = app.update(Message::DetachLeftExplorer);
        assert!(app.detach_left_explorer);
        assert!(app.detached_left_id.is_some());
        let left_id = app.detached_left_id.ok_or("no detached_left_id")?;
        // Check window close request docks back
        let _ = app.update(Message::WindowCloseRequested(left_id));
        assert!(!app.detach_left_explorer);
        assert!(app.show_left_explorer);
        assert!(app.detached_left_id.is_none());

        // 2. Verify Detach & Dock Lifecycle for Right I/O
        assert!(!app.detach_right_io);
        assert!(app.detached_right_id.is_none());
        let _ = app.update(Message::DetachRightIo);
        assert!(app.detach_right_io);
        assert!(app.detached_right_id.is_some());
        let _ = app.update(Message::DockRightIo);
        assert!(!app.detach_right_io);
        assert!(app.detached_right_id.is_none());

        // 3. Verify Detach & Dock Lifecycle for Bottom Dock
        assert!(!app.detach_bottom_dock);
        assert!(app.detached_bottom_id.is_none());
        let _ = app.update(Message::DetachBottomDock);
        assert!(app.detach_bottom_dock);
        assert!(app.detached_bottom_id.is_some());
        let _ = app.update(Message::DockBottomDock);
        assert!(!app.detach_bottom_dock);
        assert!(app.detached_bottom_id.is_none());

        // 4. Verify Ladder Quick Additions with ContactKind & CoilKind
        use ferrite_lang_ld::{CoilKind, ContactKind};
        let _ = app.update(Message::LdAddContactWithKind(
            "main_safety".to_string(),
            0,
            ContactKind::NormallyClosed,
        ));
        let _ = app.update(Message::LdAddCoilWithKind(
            "main_safety".to_string(),
            0,
            CoilKind::Set,
        ));
        if let Some(PouSource::Ld(ld)) = app.project.pous.get("main_safety") {
            let last_contact = ld.rungs[0].elements.last().ok_or("no contact")?;
            assert!(matches!(
                last_contact,
                ferrite_lang_ld::LdElement::Contact {
                    kind: ContactKind::NormallyClosed,
                    ..
                }
            ));
            let last_coil = ld.rungs[0].coils.last().ok_or("no coil")?;
            assert_eq!(last_coil.kind, CoilKind::Set);
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_multi_tree_editing_and_deferred_compile_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        use ferrite_lang_sfc::{
            ConvergenceKind, DivergenceKind, SfcProgram, SfcStep, SfcTransition, StepKind,
            validate_sfc_topology,
        };

        let temp_dir = env::temp_dir().join("ferrite_gui_sfc_multi_tree_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let mut proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        proj.add_pou("parallel_sfc", ferrite_core::LanguageKind::Sfc)?;
        if let Some(task) = proj.manifest.tasks.first_mut() {
            task.pou_order.push("parallel_sfc".to_string());
        }

        // Construct 2 independent parallel SFC trees in the same POU
        let mut sfc = SfcProgram {
            name: "parallel_sfc".to_string(),
            comment: "Dual-tree SFC".to_string(),
            steps: vec![
                // Tree 1
                SfcStep::new("X1".to_string(), "Init_1".to_string(), StepKind::Initial),
                SfcStep::new("X2".to_string(), "Step_2".to_string(), StepKind::Normal),
                // Tree 2
                SfcStep::new("X10".to_string(), "Init_2".to_string(), StepKind::Initial),
                SfcStep::new("X11".to_string(), "Step_11".to_string(), StepKind::Normal),
            ],
            transitions: vec![
                // Tree 1 transition
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["X1".to_string()],
                    to_steps: vec!["X2".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                // Tree 2 transition
                SfcTransition {
                    id: "T10".to_string(),
                    from_steps: vec!["X10".to_string()],
                    to_steps: vec!["X11".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
            ],
        };
        sfc.assign_default_grid_positions();
        proj.pous
            .insert("parallel_sfc".to_string(), PouSource::Sfc(sfc));
        let mut app = FerriteStudioApp::new(proj);

        // 1. Initial topology check: 2 trees, valid
        let report = if let Some(PouSource::Sfc(s)) = app.project.pous.get("parallel_sfc") {
            validate_sfc_topology(s)
        } else {
            panic!("parallel_sfc not found");
        };
        assert_eq!(report.trees.len(), 2);
        assert!(report.is_valid);

        // 2. Trigger SfcValidate message
        let _ = app.update(Message::SfcValidate("parallel_sfc".to_string()));
        assert!(app.status_banner.contains("2 independent parallel trees"));

        // 3. Deferred validation: User modifies Tree 2 step kind to Normal (missing initial step in tree 2)
        let _ = app.update(Message::SfcStepKindChanged(
            "parallel_sfc".to_string(),
            2,
            StepKind::Normal,
        ));

        let report_invalid = if let Some(PouSource::Sfc(s)) = app.project.pous.get("parallel_sfc") {
            validate_sfc_topology(s)
        } else {
            panic!("parallel_sfc not found");
        };
        assert!(!report_invalid.is_valid);
        assert_eq!(report_invalid.errors.len(), 1);

        let _ = app.update(Message::SfcValidate("parallel_sfc".to_string()));
        assert!(app.status_banner.contains("validation error"));

        // Recompile fails with topological error
        app.recompile_runtime();
        assert!(app.status_banner.contains("Compilation error"));

        // 4. User corrects Tree 2 by making X10 Initial again
        let _ = app.update(Message::SfcStepKindChanged(
            "parallel_sfc".to_string(),
            2,
            StepKind::Initial,
        ));
        app.recompile_runtime();
        assert!(app.runtime.is_some());
        assert!(app.status_banner.contains("recompiled successfully"));

        // 5. Verify render_sfc_editor renders without panic with both trees
        if let Some(PouSource::Sfc(s)) = app.project.pous.get("parallel_sfc") {
            let _elem =
                canvas_sfc::render_sfc_editor(s, app.runtime.as_ref(), app.ui_lang, None, &[]);
        }

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_load_project_from_path_and_plcopen_roundtrip() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = std::env::temp_dir().join("ferrite_test_load_path");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_load")?;
        scaffold.save(&temp_dir)?;

        let mut app = FerriteStudioApp::new(scaffold);
        let _ = app.update(Message::LoadProjectFromPath(temp_dir.clone()));
        assert_eq!(app.project.manifest.name, "test_load");
        assert!(app.status_banner.contains("test_load"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_keyboard_shortcut_mapping() {
        use crate::app::handle_keyboard_shortcut;

        let cmd_s = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("s".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(cmd_s, Some(Message::SaveProject)));

        let cmd_o = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("o".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(cmd_o, Some(Message::OpenProjectFolderDialog)));

        let cmd_z = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("z".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(cmd_z, Some(Message::Undo)));

        let cmd_shift_z = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("z".into()),
            iced::keyboard::Modifiers::COMMAND | iced::keyboard::Modifiers::SHIFT,
        );
        assert!(matches!(cmd_shift_z, Some(Message::Redo)));

        let cmd_y = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("y".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(cmd_y, Some(Message::Redo)));

        let del = handle_keyboard_shortcut(
            &iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete),
            iced::keyboard::Modifiers::empty(),
        );
        assert!(matches!(del, Some(Message::DeletePressed)));

        let backspace = handle_keyboard_shortcut(
            &iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace),
            iced::keyboard::Modifiers::empty(),
        );
        assert!(backspace.is_none());
    }

    #[test]
    fn test_active_menu_auto_dismissal() -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::ActiveMenu;
        let temp_dir = std::env::temp_dir().join("ferrite_test_menu_dismiss");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_menu")?;
        let mut app = FerriteStudioApp::new(scaffold);

        app.active_menu = Some(ActiveMenu::File);
        let _ = app.update(Message::SaveProject);
        assert_eq!(app.active_menu, None);

        app.active_menu = Some(ActiveMenu::Plc);
        let _ = app.update(Message::CheckProject);
        assert_eq!(app.active_menu, None);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_rpi_40pin_header_pin_styling() {
        let pin_1 = &ferrite_hal::RPI_40PIN_HEADER[0];
        assert_eq!(pin_1.pin_number, 1);
        assert!(pin_1.name.contains("3.3V") || pin_1.name.contains("3V3"));
        assert!(pin_1.bcm.is_none());

        let pin_11 = &ferrite_hal::RPI_40PIN_HEADER[10];
        assert_eq!(pin_11.pin_number, 11);
        assert_eq!(pin_11.bcm, Some(17));
    }

    #[test]
    fn test_sim_speed_display_and_options() {
        use crate::app::SimSpeed;
        assert_eq!(SimSpeed::ALL.len(), 6);
        assert_eq!(SimSpeed::Slow0_1x.to_string(), "0.1x");
        assert_eq!(SimSpeed::Realtime1x.to_string(), "1x");
        assert_eq!(SimSpeed::Fast2x.to_string(), "2x");
        assert_eq!(SimSpeed::Fast5x.to_string(), "5x");
        assert_eq!(SimSpeed::Fast10x.to_string(), "10x");
        assert_eq!(SimSpeed::Max.to_string(), "Max");
    }

    #[test]
    fn test_warm_restart_auto_pauses_and_syncs_shared_io() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_test_warm_restart_ui");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_warm")?;
        let mut app = FerriteStudioApp::new(scaffold);

        app.sim_running = true;
        let _ = app.update(Message::ResetSimWarm);
        assert!(!app.sim_running, "Warm restart must auto-pause the simulation");

        let shared = app
            .shared_io
            .read()
            .map_err(|e| format!("{e}"))?;
        assert_eq!(
            shared.values.get("EStop_OK"),
            Some(&ferrite_core::IecValue::Bool(true))
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_single_step_advances_exactly_one_scan_cycle() -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::SimSpeed;
        let temp_dir = std::env::temp_dir().join("ferrite_test_step_single");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_step")?;
        let mut app = FerriteStudioApp::new(scaffold);

        // Set speed to Max which normally executes batch of 50
        app.sim_speed = SimSpeed::Max;
        assert_eq!(app.sim_speed.batch_size(), 50);

        let initial_scans = app.runtime.as_ref().map(|rt| rt.scan_count).unwrap_or(0);
        let _ = app.update(Message::StepSimTick);
        let after_scans = app.runtime.as_ref().map(|rt| rt.scan_count).unwrap_or(0);

        assert_eq!(
            after_scans,
            initial_scans + 1,
            "Single-step must advance by exactly 1 scan cycle, not batch size"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_active_step_multi_pou_isolation() -> Result<(), Box<dyn std::error::Error>> {
        use crate::canvas_sfc::is_step_active;
        let temp_dir = std::env::temp_dir().join("ferrite_test_sfc_isolation");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_iso")?;
        let mut app = FerriteStudioApp::new(scaffold);

        // Advance simulation:
        // Set EStop_OK, Stop_Button, Guard_Door_Closed, Start_Button
        if let Some(ref mut rt) = app.runtime {
            rt.force("EStop_OK", ferrite_core::IecValue::Bool(true))?;
            rt.force("Stop_Button", ferrite_core::IecValue::Bool(true))?;
            rt.force("Guard_Door_Closed", ferrite_core::IecValue::Bool(true))?;
            rt.force("Start_Button", ferrite_core::IecValue::Bool(true))?;
        }
        let _ = app.execute_plc_scan_tick()?;
        let _ = app.execute_plc_scan_tick()?;

        let rt = app.runtime.as_ref().ok_or("runtime not initialized")?;
        // In batch_seq: Step 1 and Step 2 should be active, Step 0 should be inactive
        assert!(rt.read_value("batch_seq.X1.state")?.as_bool()?);
        assert!(rt.read_value("batch_seq.X2.state")?.as_bool()?);
        assert!(!rt.read_value("batch_seq.X0.state")?.as_bool()?);

        // In cip_clean: Step 0 should be active (Standby), Step 1 and 2 should be inactive
        assert!(rt.read_value("cip_clean.X0.state")?.as_bool()?);
        assert!(!rt.read_value("cip_clean.X1.state")?.as_bool()?);
        assert!(!rt.read_value("cip_clean.X2.state")?.as_bool()?);

        // Verify is_step_active correctly isolates cip_clean from batch_seq
        let (cip_s0, _) = is_step_active(rt, "cip_clean", "0");
        let (cip_s1, _) = is_step_active(rt, "cip_clean", "1");
        let (cip_s2, _) = is_step_active(rt, "cip_clean", "2");

        assert!(cip_s0, "cip_clean Step 0 (Standby) must be active");
        assert!(!cip_s1, "cip_clean Step 1 must NOT be active while batch_seq has step 1 active");
        assert!(!cip_s2, "cip_clean Step 2 must NOT be active while batch_seq has step 2 active");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_forces_active_banner_no_bugged_characters() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!("ferrite_forces_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // Force a tag to activate the forces banner
        if let Some(ref mut rt) = app.runtime {
            rt.force("Start_Button", ferrite_core::IecValue::Bool(true))?;
        }

        let count = app.runtime.as_ref().map(|rt| rt.active_forces_count()).unwrap_or(0);
        assert!(count >= 1, "There should be active forces present");

        // Format the banner text and assert it uses clean ASCII marker [!] and contains no emoji tofu
        let forces_text = format!(
            "[!] {} {}",
            count,
            app.ui_lang.tr("FORCES ACTIVE", "FORÇAGES ACTIFS")
        );
        assert!(forces_text.starts_with(&format!("[!] {count} FORCES ACTIVE")));
        assert!(!forces_text.contains("⚠️"));
        assert!(!forces_text.contains('\u{FE0F}'));

        // Render main view without panic
        let window_id = app.main_window_id.unwrap_or_else(iced::window::Id::unique);
        let _view = app.view(window_id);

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
