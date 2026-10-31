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
pub mod icons;
pub mod theme;
pub mod trend_plotter;

use std::path::{Path, PathBuf};
use std::sync::Once;

pub use app::{FerriteStudioApp, Message, UiTheme, WorkspaceTab};
pub type StudioApp = FerriteStudioApp;
use ferrite_project::{MANIFEST_FILE_NAME, Project, ProjectError};
pub use i18n::UiLanguage;
pub use theme::StudioTheme;
use thiserror::Error;

static INIT_FONTS: Once = Once::new();

/// Initializes system fonts and emoji fallbacks into Iced's cosmic-text font system.
pub fn init_system_and_emoji_fonts() {
    INIT_FONTS.call_once(|| {
        if let Ok(mut fs_lock) = iced::advanced::graphics::text::font_system().write() {
            let db = fs_lock.raw().db_mut();
            db.load_system_fonts();

            #[cfg(target_os = "windows")]
            {
                let win_fonts = [
                    r"C:\Windows\Fonts\seguiemj.ttf",
                    r"C:\Windows\Fonts\seguisym.ttf",
                ];
                for path in win_fonts {
                    if std::path::Path::new(path).exists() {
                        let _ = db.load_font_file(path);
                    }
                }

                // Load user-installed fonts (%LOCALAPPDATA%\Microsoft\Windows\Fonts)
                // Windows 10/11 installs per-user fonts (e.g. Nerd Fonts) here by default
                if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                    let user_fonts_dir = std::path::PathBuf::from(local_app_data)
                        .join(r"Microsoft\Windows\Fonts");
                    if user_fonts_dir.exists() {
                        db.load_fonts_dir(&user_fonts_dir);
                    }
                }
            }
        }
    });
}

/// Returns true if a Nerd Font (such as FiraCode Nerd Font) is loaded in the font system.
pub fn is_nerd_font_available() -> bool {
    init_system_and_emoji_fonts();
    if let Ok(mut fs_lock) = iced::advanced::graphics::text::font_system().write() {
        let db = fs_lock.raw().db();
        db.faces().any(|face| {
            face.families.iter().any(|(name, _)| {
                name.contains("Nerd Font")
            })
        })
    } else {
        false
    }
}

/// Resolves the preferred default application font for FerritePLC Studio.
/// If a Nerd Font (e.g. FiraCode Nerd Font) is installed, it is used for superior
/// code typography and icon glyphs. Otherwise, falls back to the system default font.
pub fn resolve_app_font() -> iced::Font {
    if is_nerd_font_available() {
        iced::Font::with_name("FiraCode Nerd Font")
    } else {
        iced::Font::DEFAULT
    }
}

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
    init_system_and_emoji_fonts();
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
    .default_font(resolve_app_font())
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
                button_mode: ferrite_twin::ButtonActionMode::default(),
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

    #[test]
    fn test_help_window_messages_and_state() {
        let mut app = StudioApp::new_test_app();
        assert!(app.help_window_id.is_none());
        assert_eq!(app.help_selected_topic, crate::help_window::HelpTopic::ArchScanLoop);
        assert_eq!(app.help_search_query, "");

        // Test Search input
        let _ = app.update(Message::HelpSearchChanged("ladder".to_string()));
        assert_eq!(app.help_search_query, "ladder");

        // Test Select topic
        let _ = app.update(Message::SelectHelpTopic(crate::help_window::HelpTopic::LdContacts));
        assert_eq!(app.help_selected_topic, crate::help_window::HelpTopic::LdContacts);

        // Test view rendering
        let view_el = app.view_help_window();
        drop(view_el);

        // Test Help window OS title
        let fake_help_id = iced::window::Id::unique();
        app.help_window_id = Some(fake_help_id);
        assert_eq!(
            app.title(fake_help_id),
            "FerritePLC — Help & Industrial Reference Manual"
        );

        // Test Close message
        let _ = app.update(Message::CloseHelpWindow);
        assert!(app.help_window_id.is_none());

        // Test Empty search rendering
        let _ = app.update(Message::HelpSearchChanged("nonexistenttopic123".to_string()));
        let empty_view = app.view_help_window();
        let _ = empty_view;
    }

    #[test]
    fn test_f1_key_maps_to_open_help_window() {
        let key_event = iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::F1),
            modifiers: iced::keyboard::Modifiers::default(),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::F1),
            location: iced::keyboard::Location::Standard,
            text: None,
        };
        let msg = StudioApp::handle_key_event(key_event);
        assert!(matches!(msg, Some(Message::OpenHelpWindow)));
    }

    #[test]
    fn test_fbd_corrector_architecture_switching() {
        use ferrite_core::StandardFbKind;
        use ferrite_lang_fbd::{FbdBlockKind, FbdInputSource};

        let mut app = StudioApp::new_test_app();
        let pou_name = "pump_ctrl".to_string();

        // Add a PID Parallel block
        let _ = app.update(Message::FbdAddBlock(
            pou_name.clone(),
            FbdBlockKind::StandardFb {
                instance: "PID_Test".to_string(),
                fb_type: Some(StandardFbKind::PidParallel),
            },
        ));

        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU pump_ctrl");
        };
        let Some(b_idx) = fbd
            .blocks
            .iter()
            .position(|b| b.id == "PID_Test")
        else {
            panic!("PID_Test block not found");
        };
        let blk = &fbd.blocks[b_idx];

        assert_eq!(
            blk.kind,
            FbdBlockKind::StandardFb {
                instance: "PID_Test".to_string(),
                fb_type: Some(StandardFbKind::PidParallel),
            }
        );
        assert!(blk.inputs.contains_key("SP"));
        assert!(blk.inputs.contains_key("PV"));
        assert!(blk.inputs.contains_key("KP"));
        assert!(blk.inputs.contains_key("TI"));
        assert!(blk.inputs.contains_key("TD"));
        assert!(blk.outputs.is_empty()); // Outputs start unbound

        // Verify fb_instances registry
        assert!(app
            .project
            .fb_instances
            .instances
            .iter()
            .any(|fb| fb.name == "PID_Test" && fb.kind == StandardFbKind::PidParallel));

        // Switch to PI: TI is retained, TD must be removed
        let _ = app.update(Message::FbdChangeCorrectorKind(
            pou_name.clone(),
            b_idx,
            StandardFbKind::Pi,
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        let blk = &fbd.blocks[b_idx];
        assert_eq!(
            blk.kind,
            FbdBlockKind::StandardFb {
                instance: "PID_Test".to_string(),
                fb_type: Some(StandardFbKind::Pi),
            }
        );
        assert!(blk.inputs.contains_key("TI"));
        assert!(!blk.inputs.contains_key("TD"));
        assert!(app
            .project
            .fb_instances
            .instances
            .iter()
            .any(|fb| fb.name == "PID_Test" && fb.kind == StandardFbKind::Pi));

        // Switch to P: TI must be removed
        let _ = app.update(Message::FbdChangeCorrectorKind(
            pou_name.clone(),
            b_idx,
            StandardFbKind::P,
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        let blk = &fbd.blocks[b_idx];
        assert_eq!(
            blk.kind,
            FbdBlockKind::StandardFb {
                instance: "PID_Test".to_string(),
                fb_type: Some(StandardFbKind::P),
            }
        );
        assert!(!blk.inputs.contains_key("TI"));
        assert!(!blk.inputs.contains_key("TD"));

        // Switch to PID Series: TI and TD are added back
        let _ = app.update(Message::FbdChangeCorrectorKind(
            pou_name.clone(),
            b_idx,
            StandardFbKind::PidSeries,
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        let blk = &fbd.blocks[b_idx];
        assert_eq!(
            blk.kind,
            FbdBlockKind::StandardFb {
                instance: "PID_Test".to_string(),
                fb_type: Some(StandardFbKind::PidSeries),
            }
        );
        assert!(blk.inputs.contains_key("TI"));
        assert!(blk.inputs.contains_key("TD"));
        assert_eq!(
            blk.inputs["TI"],
            FbdInputSource::Value {
                expr: "5.0".to_string()
            }
        );
        assert_eq!(
            blk.inputs["TD"],
            FbdInputSource::Value {
                expr: "0.5".to_string()
            }
        );
    }

    #[test]
    fn test_fbd_block_header_actions() {
        let mut app = StudioApp::new_test_app();
        let pou_name = "pump_ctrl".to_string();

        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU pump_ctrl");
        };
        let initial_b0 = fbd.blocks[0].id.clone();
        let initial_b1 = fbd.blocks[1].id.clone();

        // Test Reordering: Move block 0 down
        let _ = app.update(Message::FbdMoveBlockDown(pou_name.clone(), 0));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        assert_eq!(fbd.blocks[0].id, initial_b1);
        assert_eq!(fbd.blocks[1].id, initial_b0);

        // Move block 1 up -> restores order
        let _ = app.update(Message::FbdMoveBlockUp(pou_name.clone(), 1));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        assert_eq!(fbd.blocks[0].id, initial_b0);
        assert_eq!(fbd.blocks[1].id, initial_b1);

        // Test Renaming
        let _ = app.update(Message::FbdBlockIdChanged(
            pou_name.clone(),
            0,
            "Block_Renamed_Test".to_string(),
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        assert_eq!(fbd.blocks[0].id, "Block_Renamed_Test");
    }

    #[test]
    fn test_pou_explorer_rename_and_reorder_messages() {
        let mut app = StudioApp::new_test_app();
        assert!(app.project.manifest.pous.len() >= 2);

        let p0 = app.project.manifest.pous[0].name.clone();
        let p1 = app.project.manifest.pous[1].name.clone();

        // 1. Reordering
        let _ = app.update(Message::MovePouDown(p0.clone()));
        assert_eq!(app.project.manifest.pous[0].name, p1);
        assert_eq!(app.project.manifest.pous[1].name, p0);

        let _ = app.update(Message::MovePouUp(p0.clone()));
        assert_eq!(app.project.manifest.pous[0].name, p0);
        assert_eq!(app.project.manifest.pous[1].name, p1);

        // 2. Renaming: start and cancel
        let _ = app.update(Message::StartRenamePou(p0.clone()));
        assert_eq!(app.renaming_pou, Some((p0.clone(), p0.clone())));

        let _ = app.update(Message::PouRenameInputChanged("buffer_name".to_string()));
        assert_eq!(
            app.renaming_pou,
            Some((p0.clone(), "buffer_name".to_string()))
        );

        let _ = app.update(Message::CancelRenamePou);
        assert!(app.renaming_pou.is_none());

        // 3. Renaming: confirm with active tab synchronization
        app.active_tab = WorkspaceTab::Pou(p0.clone());
        let _ = app.update(Message::StartRenamePou(p0.clone()));
        let new_name = format!("{}_renamed", p0);
        let _ = app.update(Message::PouRenameInputChanged(new_name.clone()));
        let _ = app.update(Message::ConfirmRenamePou);

        assert!(app.renaming_pou.is_none());
        assert_eq!(app.active_tab, WorkspaceTab::Pou(new_name.clone()));
        assert!(app.project.manifest.pous.iter().any(|p| p.name == new_name));
        assert!(app.project.pous.contains_key(&new_name));
        assert!(!app.project.pous.contains_key(&p0));

        // 4. Deleting a POU
        let _ = app.update(Message::DeletePou(new_name.clone()));
        assert!(!app.project.manifest.pous.iter().any(|p| p.name == new_name));
        assert!(!app.project.pous.contains_key(&new_name));
        assert_ne!(app.active_tab, WorkspaceTab::Pou(new_name));
    }

    #[test]
    fn test_fbd_input_source_changed_float_vs_link() {
        use ferrite_lang_fbd::FbdInputSource;

        let mut app = StudioApp::new_test_app();
        let pou_name = "pump_ctrl".to_string();

        // 1. Typing a float like "5.0" should become a Value, NOT a Link
        let _ = app.update(Message::FbdInputSourceChanged(
            pou_name.clone(),
            0,
            "IN1".to_string(),
            "5.0".to_string(),
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        assert_eq!(
            fbd.blocks[0].inputs["IN1"],
            FbdInputSource::Value {
                expr: "5.0".to_string()
            }
        );

        // 2. Typing a direct address "%IX0.0" should become a Value
        let _ = app.update(Message::FbdInputSourceChanged(
            pou_name.clone(),
            0,
            "IN1".to_string(),
            "%IX0.0".to_string(),
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        assert_eq!(
            fbd.blocks[0].inputs["IN1"],
            FbdInputSource::Value {
                expr: "%IX0.0".to_string()
            }
        );

        // 3. Typing an existing block and pin "Block_1.OUT" should become a Link
        let target_block_id = fbd.blocks[1].id.clone();
        let _ = app.update(Message::FbdInputSourceChanged(
            pou_name.clone(),
            0,
            "IN1".to_string(),
            format!("{target_block_id}.OUT"),
        ));
        let Some(PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) else {
            panic!("Expected FBD POU");
        };
        assert_eq!(
            fbd.blocks[0].inputs["IN1"],
            FbdInputSource::Link {
                from_block: target_block_id,
                from_pin: "OUT".to_string()
            }
        );
    }

    #[test]
    fn test_font_system_can_load_system_fonts() {
        init_system_and_emoji_fonts();
        if let Ok(mut fs_lock) = iced::advanced::graphics::text::font_system().write() {
            let db = fs_lock.raw().db();
            assert!(!db.is_empty(), "System fonts should be loaded into fontdb");

            let has_fira = db.faces().any(|face| {
                face.families.iter().any(|(name, _)| name.contains("FiraCode"))
            });
            #[cfg(target_os = "windows")]
            {
                if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                    let fira_path = std::path::PathBuf::from(local_app_data)
                        .join(r"Microsoft\Windows\Fonts\FiraCodeNerdFont-Regular.ttf");
                    if fira_path.exists() {
                        assert!(has_fira, "FiraCode Nerd Font in user directory should be loaded into fontdb");
                    }
                }
            }
        }
    }

    #[test]
    fn test_emoji_glyph_resolution() {
        use iced::advanced::graphics::text::cosmic_text;

        init_system_and_emoji_fonts();

        if let Ok(mut fs_lock) = iced::advanced::graphics::text::font_system().write() {
            let font_system = fs_lock.raw();

            // Case 1: Primary font is Segoe UI Emoji with Basic shaping
            let mut buf1 = cosmic_text::Buffer::new(
                font_system,
                cosmic_text::Metrics::new(14.0, 18.0),
            );
            buf1.set_text(
                font_system,
                "📖 🔒 ⚠️ ✏️ ⚙️",
                cosmic_text::Attrs::new().family(cosmic_text::Family::Name("Segoe UI Emoji")),
                cosmic_text::Shaping::Basic,
            );
            buf1.shape_until_scroll(font_system, false);
            // Test if SwashCache can rasterize Segoe UI Emoji
            let mut swash_cache = cosmic_text::SwashCache::new();
            for run in buf1.layout_runs() {
                for glyph in run.glyphs {
                    let phys = glyph.physical((0.0, 0.0), 1.0);
                    let image = swash_cache.get_image_uncached(font_system, phys.cache_key);
                    assert!(
                        image.is_some(),
                        "SwashCache should rasterize Segoe UI Emoji glyph"
                    );
                }
            }

            // Case 2: SansSerif with Advanced shaping (fallback to Segoe UI Emoji)
            let mut buf2 = cosmic_text::Buffer::new(
                font_system,
                cosmic_text::Metrics::new(14.0, 18.0),
            );
            buf2.set_text(
                font_system,
                "📖 🔒 ⚠️ ✏️ ⚙️",
                cosmic_text::Attrs::new().family(cosmic_text::Family::SansSerif),
                cosmic_text::Shaping::Advanced,
            );
            buf2.shape_until_scroll(font_system, false);
            for run in buf2.layout_runs() {
                for glyph in run.glyphs {
                    assert_ne!(
                        glyph.glyph_id, 0,
                        "SansSerif with Advanced shaping should resolve emoji glyphs via fallback!"
                    );
                }
            }

            // Case 3: FiraCode Nerd Font with Nerd Font icon codepoints
            let local_fonts = std::env::var("LOCALAPPDATA").map(|l| {
                std::path::PathBuf::from(l).join(r"Microsoft\Windows\Fonts")
            }).ok();
            if let Some(dir) = local_fonts {
                let fira_path = dir.join("FiraCodeNerdFont-Regular.ttf");
                if fira_path.exists() {
                    let _ = font_system.db_mut().load_font_file(&fira_path);
                    let mut buf3 = cosmic_text::Buffer::new(
                        font_system,
                        cosmic_text::Metrics::new(14.0, 18.0),
                    );
                    // \u{f02d} = book, \u{f023} = lock, \u{f071} = warning, \u{f040} = pencil, \u{f013} = gear
                    buf3.set_text(
                        font_system,
                        "\u{f02d} \u{f023} \u{f071} \u{f040} \u{f013}",
                        cosmic_text::Attrs::new().family(cosmic_text::Family::Name("FiraCode Nerd Font")),
                        cosmic_text::Shaping::Basic,
                    );
                    buf3.shape_until_scroll(font_system, false);
                    for run in buf3.layout_runs() {
                        for glyph in run.glyphs {
                            assert_ne!(
                                glyph.glyph_id, 0,
                                "FiraCode Nerd Font should resolve Nerd Font codepoints!"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn test_digital_twin_os_window_edit_and_runtime_modes() -> Result<(), Box<dyn std::error::Error>> {
        use ferrite_twin::{
            ActuatorKind, ButtonActionMode, ColorEditTarget, ElementKind, ShapeType, TwinMode,
        };

        let now_nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ferrite_test_twin_rework_{now_nanos}"));
        std::fs::create_dir_all(&temp_dir)?;
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_twin_rework")?;
        let mut app = FerriteStudioApp::new(scaffold);

        // 1. Open Digital Twin OS Window
        assert!(app.digital_twin_window_id.is_none());
        let _ = app.update(Message::OpenDigitalTwinWindow);
        let twin_win_id = app
            .digital_twin_window_id
            .ok_or("Digital Twin OS window must be opened")?;
        drop(app.view(twin_win_id));

        // 2. Spawn industrial equipment and geometric shapes in Edit Mode
        assert_eq!(app.digital_twin_workspace.mode, TwinMode::Edit);
        let initial_len = app.digital_twin_workspace.elements.len();
        let _ = app.update(Message::TwinSpawnElement(ElementKind::PushButton));
        let pb_id = app
            .digital_twin_workspace
            .selected_id
            .ok_or("spawned PB must be selected")?;
        let _ = app.update(Message::TwinSpawnElement(ElementKind::Valve));
        let _ = app.update(Message::TwinSpawnElement(ElementKind::Tank));
        let _ = app.update(Message::TwinSpawnElement(ElementKind::Actuator(
            ActuatorKind::ElectricMotor,
        )));
        let _ = app.update(Message::TwinSpawnElement(ElementKind::BasicShape(
            ShapeType::Line,
        )));
        let _ = app.update(Message::TwinSpawnElement(ElementKind::BasicShape(
            ShapeType::Square,
        )));
        let _ = app.update(Message::TwinSpawnElement(ElementKind::BasicShape(
            ShapeType::Circle,
        )));
        let circle_id = app
            .digital_twin_workspace
            .selected_id
            .ok_or("spawned Circle must be selected")?;
        assert_eq!(app.digital_twin_workspace.elements.len(), initial_len + 7);

        // 3. Move, resize, and rotate the Circle in Edit Mode
        let orig_pos = app
            .digital_twin_workspace
            .elements
            .iter()
            .find(|e| e.id == circle_id)
            .ok_or("circle exists")?
            .position;
        let click_pt = (orig_pos.0 + 10.0, orig_pos.1 + 10.0);
        let _ = app.update(Message::TwinCanvasMouseDown {
            pos: click_pt,
            right_button: false,
        });
        let _ = app.update(Message::TwinCanvasMouseMove((
            click_pt.0 + 50.0,
            click_pt.1 + 40.0,
        )));
        let _ = app.update(Message::TwinCanvasMouseUp);
        let moved_pos = app
            .digital_twin_workspace
            .elements
            .iter()
            .find(|e| e.id == circle_id)
            .ok_or("circle exists")?
            .position;
        assert!(moved_pos.0 > orig_pos.0 && moved_pos.1 > orig_pos.1);

        let _ = app.update(Message::TwinSetWidth(circle_id, "120".to_string()));
        let _ = app.update(Message::TwinSetHeight(circle_id, "120".to_string()));
        let _ = app.update(Message::TwinSetRotation(circle_id, 45.0));

        // 4. Right-click context menu -> Open Parameters -> Bind variable, pick Red OFF / Green ON colors, enable blinking
        let center_pt = (moved_pos.0 + 60.0, moved_pos.1 + 60.0);
        let _ = app.update(Message::TwinCanvasMouseDown {
            pos: center_pt,
            right_button: true,
        });
        assert!(app.digital_twin_workspace.context_menu.is_some());
        let _ = app.update(Message::TwinOpenInspector(circle_id));
        assert_eq!(app.digital_twin_workspace.inspector_open_for, Some(circle_id));

        let _ = app.update(Message::TwinToggleBinding(circle_id, true));
        let _ = app.update(Message::TwinSetVariable(
            circle_id,
            "High_Temp_Alarm".to_string(),
        ));
        let _ = app.update(Message::TwinSelectColorTarget(ColorEditTarget::InactiveColor));
        let _ = app.update(Message::TwinSetColorHex("#FF0000".to_string()));
        // Invalid hex must be ignored safely
        let _ = app.update(Message::TwinSetColorHex("#GG0011".to_string()));
        let _ = app.update(Message::TwinSelectColorTarget(ColorEditTarget::ActiveColor));
        let _ = app.update(Message::TwinSetColorHex("#00FF00".to_string()));
        let _ = app.update(Message::TwinToggleBlink(circle_id, true));
        let _ = app.update(Message::TwinSetBlinkInterval(circle_id, 250));

        let circle = app
            .digital_twin_workspace
            .elements
            .iter()
            .find(|e| e.id == circle_id)
            .ok_or("circle exists")?;
        assert_eq!(circle.rotation, 45.0);
        let c_bind = circle.binding.as_ref().ok_or("circle binding")?;
        assert_eq!(c_bind.inactive_color, [255, 0, 0, 255]);
        assert_eq!(c_bind.active_color, [0, 255, 0, 255]);
        assert_eq!(c_bind.blink.as_ref().map(|b| b.interval_ms), Some(250));

        // Verify Hue is preserved when dragging SV to S=0, V=0
        let _ = app.update(Message::TwinPickColorHue(210.0));
        let _ = app.update(Message::TwinPickColorSv(0.0, 0.0));
        assert!((app.digital_twin_workspace.hsv_state.0 - 210.0).abs() < 1e-3);
        // Restore Green ON color
        let _ = app.update(Message::TwinSetColorHex("#00FF00".to_string()));

        // Verify duplicating after spawning assigns a strictly unique ID
        let _ = app.update(Message::TwinContextAction {
            action: "duplicate".to_string(),
            id: circle_id,
        });
        let ids: std::collections::HashSet<u64> = app
            .digital_twin_workspace
            .elements
            .iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(
            ids.len(),
            app.digital_twin_workspace.elements.len(),
            "All element IDs must be strictly unique after duplicate"
        );

        // Bind spawned PushButton to Start_Button in Momentary mode
        let _ = app.update(Message::TwinOpenInspector(pb_id));
        let _ = app.update(Message::TwinToggleBinding(pb_id, true));
        let _ = app.update(Message::TwinSetVariable(pb_id, "Start_Button".to_string()));
        let _ = app.update(Message::TwinSetButtonMode(pb_id, ButtonActionMode::Momentary));

        // 5. Save Digital Twin layout to project disk
        let _ = app.update(Message::TwinSaveToProject);
        assert!(temp_dir.join("sim/digital_twin.json").exists());

        // 6. Switch to Runtime Mode -> PLC starts running, elements are fixed, button click modifies variable
        app.sim_running = false;
        let _ = app.update(Message::TwinSetMode(TwinMode::Runtime));
        assert_eq!(app.digital_twin_workspace.mode, TwinMode::Runtime);
        assert!(
            app.sim_running,
            "Switching Digital Twin to Runtime mode must start PLC runtime"
        );
        assert!(app.digital_twin_workspace.inspector_open_for.is_none());

        // Attempting to drag in Runtime mode must NOT move the circle
        let pos_before_rt_drag = app
            .digital_twin_workspace
            .elements
            .iter()
            .find(|e| e.id == circle_id)
            .ok_or("circle")?
            .position;
        let _ = app.update(Message::TwinCanvasMouseDown {
            pos: center_pt,
            right_button: false,
        });
        let _ = app.update(Message::TwinCanvasMouseMove((
            center_pt.0 + 100.0,
            center_pt.1 + 100.0,
        )));
        let _ = app.update(Message::TwinCanvasMouseUp);
        let pos_after_rt_drag = app
            .digital_twin_workspace
            .elements
            .iter()
            .find(|e| e.id == circle_id)
            .ok_or("circle")?
            .position;
        assert_eq!(
            pos_before_rt_drag, pos_after_rt_drag,
            "Objects must remain fixed in Runtime mode"
        );

        // Press and release the Momentary PushButton in Runtime mode
        let _ = app.update(Message::TwinRuntimePress(pb_id));
        assert_eq!(
            app.shared_io
                .read()
                .map_err(|e| e.to_string())?
                .snapshot()
                .get_bool("Start_Button"),
            Some(true)
        );
        // Switching back to Edit mode must release held momentary button
        let _ = app.update(Message::TwinSetMode(TwinMode::Edit));
        assert_eq!(
            app.shared_io
                .read()
                .map_err(|e| e.to_string())?
                .snapshot()
                .get_bool("Start_Button"),
            Some(false)
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
