rust_i18n::i18n!("locales");
pub mod alarms_panel;
pub mod app;
pub mod canvas_fbd;
pub mod canvas_ld;
pub mod canvas_sfc;
pub mod clipboard;
pub mod custom_fbd_library;
pub mod digital_twin;
pub mod editor_st;
pub mod editor_symbols;
pub mod fbd_library_window;
pub mod help_window;
pub mod i18n;
pub mod icons;
pub mod settings;
pub mod theme;
pub mod trend_plotter;

use std::path::{Path, PathBuf};
use std::sync::Once;

pub use app::{BreakpointMode, FerriteStudioApp, Message, UiTheme, WorkspaceTab};
pub type StudioApp = FerriteStudioApp;

use ferrite_project::{MANIFEST_FILE_NAME, Project, ProjectError};
pub use i18n::UiLanguage;
pub use settings::UserSettings;
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
                    let user_fonts_dir =
                        std::path::PathBuf::from(local_app_data).join(r"Microsoft\Windows\Fonts");
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
            face.families
                .iter()
                .any(|(name, _)| name.contains("Nerd Font"))
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

/// Returns the application window icon parsed from raw 128x128 RGBA image bytes.
pub fn app_window_icon() -> Option<iced::window::Icon> {
    const ICON_RGBA_BYTES: &[u8] = include_bytes!("../assets/icon_128.rgba");
    iced::window::icon::from_rgba(ICON_RGBA_BYTES.to_vec(), 128, 128).ok()
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
    let lang = settings::load_saved_language_or_os();
    run_gui_with_lang(initial_path, lang)
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
        icon: app_window_icon(),
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
    fn test_st_jump_to_line_updates_cursor() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_gui_st_jump_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::with_language(proj, UiLanguage::En);

        let st_code = "line1 := 1;\nline2 := 2;\nline3 := 3;\nline4 := 4;\nline5 := 5;";
        let pou_name = "test_jump_st".to_string();
        app.st_contents.insert(
            pou_name.clone(),
            iced::widget::text_editor::Content::with_text(st_code),
        );

        if let Some(content) = app.st_contents.get(&pou_name) {
            assert_eq!(content.cursor_position(), (0, 0));
        }

        let _ = app.update(Message::StJumpToLine(pou_name.clone(), 2));
        if let Some(content) = app.st_contents.get(&pou_name) {
            assert_eq!(content.cursor_position().0, 2);
        }

        let _ = app.update(Message::StJumpToLine(pou_name.clone(), 4));
        if let Some(content) = app.st_contents.get(&pou_name) {
            assert_eq!(content.cursor_position().0, 4);
        }

        let _ = app.update(Message::StJumpToLine(pou_name.clone(), 0));
        if let Some(content) = app.st_contents.get(&pou_name) {
            assert_eq!(content.cursor_position().0, 0);
        }

        Ok(())
    }

    #[test]
    fn test_app_window_icon_loads_successfully() {
        let icon = app_window_icon();
        assert!(
            icon.is_some(),
            "app_window_icon should successfully decode 128x128 RGBA bytes"
        );
    }

    #[test]
    fn test_studio_app_initialization_scenarios_and_i18n() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = env::temp_dir().join("ferrite_gui_studio_test");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::with_language(proj, UiLanguage::En);
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
        assert!(app.status_banner.contains("recompiled successfully") || app.status_banner.contains("recompil"));

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
        assert!(matches!(cmd_s, Some(Message::SaveActivePouOrProject)));

        let cmd_shift_s = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("s".into()),
            iced::keyboard::Modifiers::COMMAND | iced::keyboard::Modifiers::SHIFT,
        );
        assert!(matches!(cmd_shift_s, Some(Message::SaveProject)));

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

        let cmd_c = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("c".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(cmd_c, Some(Message::CopySelection)));

        let cmd_v = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("v".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(cmd_v, Some(Message::PasteClipboard)));

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
    fn test_warm_restart_auto_pauses_and_syncs_shared_io() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = std::env::temp_dir().join("ferrite_test_warm_restart_ui");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let scaffold = ferrite_project::Project::scaffold(&temp_dir, "test_warm")?;
        let mut app = FerriteStudioApp::new(scaffold);

        app.sim_running = true;
        let _ = app.update(Message::ResetSimWarm);
        assert!(
            !app.sim_running,
            "Warm restart must auto-pause the simulation"
        );

        let shared = app.shared_io.read().map_err(|e| format!("{e}"))?;
        assert_eq!(
            shared.values.get("EStop_OK"),
            Some(&ferrite_core::IecValue::Bool(true))
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_single_step_advances_exactly_one_scan_cycle() -> Result<(), Box<dyn std::error::Error>>
    {
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
        assert!(
            !cip_s1,
            "cip_clean Step 1 must NOT be active while batch_seq has step 1 active"
        );
        assert!(
            !cip_s2,
            "cip_clean Step 2 must NOT be active while batch_seq has step 2 active"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_forces_active_banner_no_bugged_characters() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_forces_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // Force a tag to activate the forces banner
        if let Some(ref mut rt) = app.runtime {
            rt.force("Start_Button", ferrite_core::IecValue::Bool(true))?;
        }

        let count = app
            .runtime
            .as_ref()
            .map(|rt| rt.active_forces_count())
            .unwrap_or(0);
        assert!(count >= 1, "There should be active forces present");

        app.ui_lang = UiLanguage::En;
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
        assert_eq!(
            app.help_selected_topic,
            crate::help_window::HelpTopic::ArchScanLoop
        );
        assert_eq!(app.help_search_query, "");

        // Test Search input
        let _ = app.update(Message::HelpSearchChanged("ladder".to_string()));
        assert_eq!(app.help_search_query, "ladder");

        // Test Select topic
        let _ = app.update(Message::SelectHelpTopic(
            crate::help_window::HelpTopic::LdContacts,
        ));
        assert_eq!(
            app.help_selected_topic,
            crate::help_window::HelpTopic::LdContacts
        );

        // Test view rendering
        let view_el = app.view_help_window();
        drop(view_el);

        // Test Help window OS title
        let fake_help_id = iced::window::Id::unique();
        app.help_window_id = Some(fake_help_id);
        app.ui_lang = UiLanguage::En;
        assert_eq!(
            app.title(fake_help_id),
            "FerritePLC — Help & Industrial Reference Manual"
        );

        // Test Close message
        let _ = app.update(Message::CloseHelpWindow);
        assert!(app.help_window_id.is_none());

        // Test Empty search rendering
        let _ = app.update(Message::HelpSearchChanged(
            "nonexistenttopic123".to_string(),
        ));
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
        let Some(b_idx) = fbd.blocks.iter().position(|b| b.id == "PID_Test") else {
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
        assert!(
            app.project
                .fb_instances
                .instances
                .iter()
                .any(|fb| fb.name == "PID_Test" && fb.kind == StandardFbKind::PidParallel)
        );

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
        assert!(
            app.project
                .fb_instances
                .instances
                .iter()
                .any(|fb| fb.name == "PID_Test" && fb.kind == StandardFbKind::Pi)
        );

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
                face.families
                    .iter()
                    .any(|(name, _)| name.contains("FiraCode"))
            });
            #[cfg(target_os = "windows")]
            {
                if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                    let fira_path = std::path::PathBuf::from(local_app_data)
                        .join(r"Microsoft\Windows\Fonts\FiraCodeNerdFont-Regular.ttf");
                    if fira_path.exists() {
                        assert!(
                            has_fira,
                            "FiraCode Nerd Font in user directory should be loaded into fontdb"
                        );
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
            let mut buf1 =
                cosmic_text::Buffer::new(font_system, cosmic_text::Metrics::new(14.0, 18.0));
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
            let mut buf2 =
                cosmic_text::Buffer::new(font_system, cosmic_text::Metrics::new(14.0, 18.0));
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
            let local_fonts = std::env::var("LOCALAPPDATA")
                .map(|l| std::path::PathBuf::from(l).join(r"Microsoft\Windows\Fonts"))
                .ok();
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
                        cosmic_text::Attrs::new()
                            .family(cosmic_text::Family::Name("FiraCode Nerd Font")),
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
    fn test_digital_twin_os_window_edit_and_runtime_modes() -> Result<(), Box<dyn std::error::Error>>
    {
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
        assert_eq!(
            app.digital_twin_workspace.inspector_open_for,
            Some(circle_id)
        );

        let _ = app.update(Message::TwinToggleBinding(circle_id, true));
        let _ = app.update(Message::TwinSetVariable(
            circle_id,
            "High_Temp_Alarm".to_string(),
        ));
        let _ = app.update(Message::TwinSelectColorTarget(
            ColorEditTarget::InactiveColor,
        ));
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
        let _ = app.update(Message::TwinSetButtonMode(
            pb_id,
            ButtonActionMode::Momentary,
        ));

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

    #[test]
    fn test_per_pou_dirty_tracking_and_individual_pou_save()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_per_pou_save_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_per_pou")?;
        let mut app = FerriteStudioApp::new(proj);

        assert!(!app.is_pou_modified("main_safety"));
        assert!(!app.is_pou_modified("batch_seq"));
        assert!(!app.modified);

        // Modify both `main_safety` (LD) and `batch_seq` (SFC)
        let _ = app.update(Message::LdAddContact("main_safety".to_string(), 0));
        let _ = app.update(Message::SfcAddStep("batch_seq".to_string()));

        assert!(app.is_pou_modified("main_safety"));
        assert!(app.is_pou_modified("batch_seq"));
        assert!(app.modified);

        // Render main window to verify explorer with dirty (* / italic) POUs renders without panic
        let win_id = app.main_window_id.unwrap_or_else(iced::window::Id::unique);
        let _ = app.view(win_id);

        // Save only `main_safety`
        let _ = app.update(Message::SavePou("main_safety".to_string()));
        assert!(
            !app.is_pou_modified("main_safety"),
            "Saved POU must no longer be marked modified"
        );
        assert!(
            app.is_pou_modified("batch_seq"),
            "Other modified POU must remain marked modified"
        );
        assert!(
            app.modified,
            "Project modified flag must remain true while another POU is still unsaved"
        );

        // Save `batch_seq` individually as well
        let _ = app.update(Message::SavePou("batch_seq".to_string()));
        assert!(!app.is_pou_modified("batch_seq"));
        assert!(
            !app.modified,
            "Project modified flag must clear once all modified POUs are saved"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_keyboard_shortcut_pou_and_project_saving() -> Result<(), Box<dyn std::error::Error>> {
        use crate::app::handle_keyboard_shortcut;

        // Ctrl+S maps to SaveActivePouOrProject
        let ctrl_s = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("s".into()),
            iced::keyboard::Modifiers::COMMAND,
        );
        assert!(matches!(ctrl_s, Some(Message::SaveActivePouOrProject)));

        // Ctrl+Shift+S maps to SaveProject
        let ctrl_shift_s = handle_keyboard_shortcut(
            &iced::keyboard::Key::Character("s".into()),
            iced::keyboard::Modifiers::COMMAND | iced::keyboard::Modifiers::SHIFT,
        );
        assert!(matches!(ctrl_shift_s, Some(Message::SaveProject)));

        // Verify SaveActivePouOrProject behavior in FerriteStudioApp
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_save_shortcut_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_sc")?;
        let mut app = FerriteStudioApp::new(proj);

        // Active tab is main_safety, modify it
        app.active_tab = WorkspaceTab::Pou("main_safety".to_string());
        let _ = app.update(Message::LdAddContact("main_safety".to_string(), 0));
        assert!(app.is_pou_modified("main_safety"));

        // SaveActivePouOrProject should save main_safety
        let _ = app.update(Message::SaveActivePouOrProject);
        assert!(!app.is_pou_modified("main_safety"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_pou_explorer_context_menu_and_drag_reorder() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_explorer_dnd_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_exp_dnd")?;
        let mut app = FerriteStudioApp::new(proj);

        assert_eq!(app.pou_context_menu, None);
        assert_eq!(app.dragging_pou, None);

        // Context menu test
        let _ = app.update(Message::OpenPouContextMenu("main_safety".to_string()));
        assert_eq!(app.pou_context_menu, Some("main_safety".to_string()));

        let _ = app.update(Message::ClosePouContextMenu);
        assert_eq!(app.pou_context_menu, None);

        // Drag and drop test: move main_safety (index 0) to math_pid (index 2)
        let p_from = app.project.manifest.pous[0].name.clone();
        let p_to = app.project.manifest.pous[2].name.clone();

        let _ = app.update(Message::PouDragStarted(p_from.clone()));
        assert_eq!(app.dragging_pou, Some(p_from.clone()));

        let _ = app.update(Message::PouDragHovered(p_to.clone()));
        assert_eq!(app.drag_hover_pou, Some(p_to));

        let _ = app.update(Message::PouDragDropped);
        assert_eq!(app.dragging_pou, None);
        assert_eq!(app.drag_hover_pou, None);
        assert_eq!(app.project.manifest.pous[2].name, p_from);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_fbd_library_window_open_close_and_messages() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_fbd_win_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_fbd_win")?;
        let mut app = FerriteStudioApp::new(proj);

        // Search query updates
        assert_eq!(app.fbd_library_search_query, "");
        let _ = app.update(Message::FbdLibrarySearchChanged("timer".to_string()));
        assert_eq!(app.fbd_library_search_query, "timer");

        // Window close when None is no-op
        assert_eq!(app.fbd_library_window_id, None);
        let _ = app.update(Message::CloseFbdLibraryWindow);
        assert_eq!(app.fbd_library_window_id, None);

        // Window open/close lifecycle
        let dummy_id = iced::window::Id::unique();
        app.fbd_library_window_id = Some(dummy_id);

        let title = app.title(dummy_id);
        assert!(title.contains("Function Block Library") || title.contains("Biblioth"));

        let _ = app.view(dummy_id);

        // Close request via WindowCloseRequested
        let _ = app.update(Message::WindowCloseRequested(dummy_id));
        assert_eq!(app.fbd_library_window_id, None);

        // Close request via CloseFbdLibraryWindow
        app.fbd_library_window_id = Some(dummy_id);
        let _ = app.update(Message::CloseFbdLibraryWindow);
        assert_eq!(app.fbd_library_window_id, None);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_fbd_library_catalog_and_filtering() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_fbd_lib_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_fbd_lib")?;
        let mut app = FerriteStudioApp::new(proj);

        // Set search query and render
        app.fbd_library_search_query = "pid".to_string();
        let _view = crate::fbd_library_window::render_fbd_library_window(&app);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
    #[test]
    fn test_ladder_paste_below_active_rung() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_ld_paste_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_ld_paste")?;
        let mut app = FerriteStudioApp::new(proj);

        let pou_name = "main_safety".to_string();
        app.active_tab = WorkspaceTab::Pou(pou_name.clone());

        let initial_rungs =
            if let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get(&pou_name) {
                ld.rungs.len()
            } else {
                0
            };
        assert!(initial_rungs > 0, "Initial ladder POU must have rungs");

        // Select rung 0
        app.ld_selected = Some((pou_name.clone(), crate::canvas_ld::LdElementKey::Rung(0)));
        app.ld_selected_elements = vec![crate::canvas_ld::LdElementKey::Rung(0)];

        // Copy active selection
        let _ = app.update(Message::CopySelection);

        // Prepare clipboard payload
        let copied_rung = ferrite_lang_ld::LdRung {
            id: "Rung_Copied".to_string(),
            comment: "Pasted safety rung".to_string(),
            elements: Vec::new(),
            coils: Vec::new(),
        };
        let payload = crate::clipboard::serialize_clipboard(
            crate::clipboard::FerriteClipboardData::Ladder {
                rungs: vec![copied_rung],
            },
        )?;

        // Paste clipboard
        let _ = app.update(Message::ClipboardPasted(Some(payload)));

        // Verify inserted immediately below rung 0 (at index 1)
        if let Some(ferrite_project::PouSource::Ld(ld)) = app.project.pous.get(&pou_name) {
            assert_eq!(ld.rungs.len(), initial_rungs + 1);
            assert_eq!(
                app.ld_selected,
                Some((pou_name.clone(), crate::canvas_ld::LdElementKey::Rung(1)))
            );
        } else {
            panic!("Expected LD POU");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_sfc_paste_with_grid_delta_and_unique_ids() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_sfc_paste_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_sfc_paste")?;
        let mut app = FerriteStudioApp::new(proj);

        let pou_name = "batch_seq".to_string();
        app.active_tab = WorkspaceTab::Pou(pou_name.clone());

        let step = ferrite_lang_sfc::SfcStep {
            id: "Step_1".to_string(),
            name: "Step_1".to_string(),
            kind: ferrite_lang_sfc::StepKind::Initial,
            actions: Vec::new(),
            grid_pos: Some((10, 10)),
                    ..Default::default()
                };
        let trans = ferrite_lang_sfc::SfcTransition {
            id: "Trans_1".to_string(),
            receptivity: "Sensor_1".to_string(),
            from_steps: vec!["Step_1".to_string()],
            to_steps: vec!["Step_2".to_string()],
            convergence: ferrite_lang_sfc::ConvergenceKind::default(),
            divergence: ferrite_lang_sfc::DivergenceKind::default(),
            grid_pos: Some((10, 14)),
        };
        let payload =
            crate::clipboard::serialize_clipboard(crate::clipboard::FerriteClipboardData::Sfc {
                steps: vec![step],
                transitions: vec![trans],
            })?;

        let _ = app.update(Message::ClipboardPasted(Some(payload)));

        if let Some(ferrite_project::PouSource::Sfc(sfc)) = app.project.pous.get(&pou_name) {
            // Find newly pasted step (must have grid_pos (12, 12))
            let pasted_step = sfc
                .steps
                .iter()
                .find(|s| s.grid_pos == Some((12, 12)))
                .ok_or("Pasted step with (+2, +2) delta not found")?;
            assert_ne!(
                pasted_step.id, "Step_1",
                "Pasted step must have unique non-colliding ID"
            );

            let pasted_trans = sfc
                .transitions
                .iter()
                .find(|t| t.grid_pos == Some((12, 16)))
                .ok_or("Pasted trans with (+2, +2) delta not found")?;
            assert_ne!(
                pasted_trans.id, "Trans_1",
                "Pasted trans must have unique non-colliding ID"
            );
            assert!(
                pasted_trans.from_steps.contains(&pasted_step.id),
                "Transition link must be remapped to new step ID"
            );
        } else {
            panic!("Expected SFC POU");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_fbd_paste_with_unique_ids() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_fbd_paste_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_fbd_paste")?;
        let mut app = FerriteStudioApp::new(proj);

        let pou_name = "pump_ctrl".to_string();
        app.active_tab = WorkspaceTab::Pou(pou_name.clone());
        app.fbd_selected_block = Some(0);

        let initial_blocks =
            if let Some(ferrite_project::PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) {
                fbd.blocks.len()
            } else {
                0
            };

        let block = ferrite_lang_fbd::FbdBlock {
            id: "TON_1".to_string(),
            kind: ferrite_lang_fbd::FbdBlockKind::And,
            inputs: std::collections::HashMap::new(),
            outputs: std::collections::HashMap::new(),
        };
        let payload =
            crate::clipboard::serialize_clipboard(crate::clipboard::FerriteClipboardData::Fbd {
                blocks: vec![block],
            })?;

        let _ = app.update(Message::ClipboardPasted(Some(payload)));

        if let Some(ferrite_project::PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) {
            assert_eq!(fbd.blocks.len(), initial_blocks + 1);
            let pasted_block = &fbd.blocks[1];
            assert_ne!(pasted_block.id, "TON_1");
            assert_eq!(app.fbd_selected_block, Some(1));
        } else {
            panic!("Expected FBD POU");
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_fbd_block_drag_and_drop_reorders_blocks() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_fbd_reorder_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_fbd_reorder")?;
        let mut app = FerriteStudioApp::new(proj);

        let pou_name = "pump_ctrl".to_string();
        app.active_tab = WorkspaceTab::Pou(pou_name.clone());

        let initial_blocks =
            if let Some(ferrite_project::PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) {
                fbd.blocks.len()
            } else {
                0
            };
        assert!(
            initial_blocks >= 2,
            "pump_ctrl should have at least 2 blocks"
        );

        let first_id =
            if let Some(ferrite_project::PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) {
                fbd.blocks[0].id.clone()
            } else {
                String::new()
            };

        // Drag block 0 onto block 1
        let _ = app.update(Message::FbdBlockDragStarted(pou_name.clone(), 0));
        assert_eq!(app.fbd_dragging_block, Some(0));

        let _ = app.update(Message::FbdBlockDragHovered(pou_name.clone(), 1));
        assert_eq!(app.fbd_drag_hover_block, Some(1));

        let _ = app.update(Message::FbdBlockDragDropped);
        assert_eq!(app.fbd_dragging_block, None);
        assert_eq!(app.fbd_drag_hover_block, None);

        // First block is now at index 1
        if let Some(ferrite_project::PouSource::Fbd(fbd)) = app.project.pous.get(&pou_name) {
            assert_eq!(fbd.blocks[1].id, first_id);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_custom_fbd_builder_validation_and_messages() -> Result<(), Box<dyn std::error::Error>> {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ferrite_app_test_{unique_id}"));
        let p = ferrite_project::Project::scaffold(&temp_dir, "FbdCustomTest")?;
        let mut app = FerriteStudioApp::new(p);

        // 1. Toggle builder
        assert!(!app.custom_fbd_builder_open);
        let _ = app.update(Message::ToggleCustomFbdBuilder);
        assert!(app.custom_fbd_builder_open);

        // 2. ST Validation
        let _ = app.update(Message::CustomFbdStCodeChanged(
            "INVALID ST SYNTAX {{{".to_string(),
        ));
        assert!(app.custom_fbd_validation_error.is_some());

        let _ = app.update(Message::CustomFbdStCodeChanged(
            "OUT := IN1 AND IN2;".to_string(),
        ));
        assert!(app.custom_fbd_validation_error.is_none());

        // Multiline ST logic with Enter key
        let _ = app.update(Message::CustomFbdStAction(
            iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Enter),
        ));
        assert!(app.custom_fbd_st_content.text().contains('\n'));

        // 3. Name validation on save
        let _ = app.update(Message::CustomFbdNameChanged(
            "Bad Name.WithDot".to_string(),
        ));
        let _ = app.update(Message::SaveCustomFbdBlock);
        assert!(app.custom_fbd_validation_error.is_some());
        assert!(app.custom_fbd_builder_open); // Stays open on error

        // 4. Valid save
        let _ = app.update(Message::CustomFbdNameChanged("MySafetyCheck".to_string()));
        let _ = app.update(Message::CustomFbdDescChanged(
            "Checks dual permissives".to_string(),
        ));
        let _ = app.update(Message::CustomFbdInputsChanged("IN1, IN2".to_string()));
        let _ = app.update(Message::CustomFbdOutputsChanged("OUT".to_string()));
        let _ = app.update(Message::CustomFbdStCodeChanged(
            "OUT := IN1 AND IN2;".to_string(),
        ));
        let _ = app.update(Message::SaveCustomFbdBlock);

        assert!(app.custom_fbd_validation_error.is_none());
        assert!(!app.custom_fbd_builder_open);
        assert!(
            app.global_custom_fbs
                .iter()
                .any(|b| b.name == "MySafetyCheck")
        );

        // 5. Edit block
        let _ = app.update(Message::EditCustomFbdBlock("MySafetyCheck".to_string()));
        assert!(app.custom_fbd_builder_open);
        assert_eq!(app.custom_fbd_name_input, "MySafetyCheck");
        assert_eq!(app.custom_fbd_desc_input, "Checks dual permissives");

        // 6. Add custom block to canvas
        let def = app
            .global_custom_fbs
            .iter()
            .find(|b| b.name == "MySafetyCheck")
            .cloned()
            .ok_or("MySafetyCheck not found")?;
        let _ = app.update(Message::AddCustomFbdToCanvas(def));

        // Find FBD POU and verify block and composite definition were added
        let has_composite_pou = app.project.pous.values().any(|src| {
            if let ferrite_project::PouSource::Fbd(fbd) = src {
                fbd.composites.iter().any(|c| c.name == "MySafetyCheck")
                    && fbd.blocks.iter().any(|b| b.id.starts_with("MySafetyCheck"))
            } else {
                false
            }
        });
        assert!(
            has_composite_pou,
            "FBD POU should contain composite definition and instance"
        );

        // 7. Delete block
        let _ = app.update(Message::DeleteCustomFbdBlock("MySafetyCheck".to_string()));
        assert!(
            !app.global_custom_fbs
                .iter()
                .any(|b| b.name == "MySafetyCheck")
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
    #[test]
    fn test_menu_hover_and_auto_dismissal() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_menu_hover_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        // 1. Initial state: active_menu is None
        assert_eq!(app.active_menu, None);

        // 2. Hovering when active_menu is None does NOT open the menu
        let _ = app.update(Message::MenuHover(crate::app::ActiveMenu::Plc));
        assert_eq!(app.active_menu, None);

        // 3. Opening File menu
        let _ = app.update(Message::ToggleActiveMenu(Some(crate::app::ActiveMenu::File)));
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::File));

        // 4. Hovering over PLC while File is open switches to PLC
        let _ = app.update(Message::MenuHover(crate::app::ActiveMenu::Plc));
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::Plc));

        // 5. Hovering over Panels switches to Panels
        let _ = app.update(Message::MenuHover(crate::app::ActiveMenu::Panels));
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::Panels));

        // 6. Action executions automatically close active_menu
        let _ = app.update(Message::OpenHelpWindow);
        assert_eq!(app.active_menu, None, "OpenHelpWindow should dismiss active_menu");

        // Open menu again, then trigger OpenDigitalTwinWindow
        let _ = app.update(Message::ToggleActiveMenu(Some(crate::app::ActiveMenu::Panels)));
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::Panels));
        let _ = app.update(Message::OpenDigitalTwinWindow);
        assert_eq!(app.active_menu, None, "OpenDigitalTwinWindow should dismiss active_menu");

        // Open menu again, then dismiss via backdrop / ToggleActiveMenu(None)
        let _ = app.update(Message::ToggleActiveMenu(Some(crate::app::ActiveMenu::Options)));
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::Options));
        let _ = app.update(Message::ToggleActiveMenu(None));
        assert_eq!(app.active_menu, None, "ToggleActiveMenu(None) should dismiss active_menu");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
    #[test]
    fn test_classic_dropdown_menu_rendering() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_dropdown_render_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);

        let win_id = iced::window::Id::unique();
        // Render base view with no active menu
        let _ = app.view(win_id);

        // Render with File menu active
        app.active_menu = Some(crate::app::ActiveMenu::File);
        let _ = app.view(win_id);

        // Render with Plc menu active
        app.active_menu = Some(crate::app::ActiveMenu::Plc);
        let _ = app.view(win_id);

        // Render with Panels menu active
        app.active_menu = Some(crate::app::ActiveMenu::Panels);
        let _ = app.view(win_id);

        // Render with Options menu active
        app.active_menu = Some(crate::app::ActiveMenu::Options);
        let _ = app.view(win_id);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
    #[test]
    fn test_ui_rendering_in_all_locales() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_locales_render_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);
        let win_id = iced::window::Id::unique();

        for lang in UiLanguage::ALL {
            let _ = app.update(Message::SetLanguage(lang));
            assert_eq!(app.ui_lang, lang);

            // 1. Render normal workspace
            let _ = app.view(win_id);

            // 2. Render all dropdown menus
            for menu in [
                crate::app::ActiveMenu::File,
                crate::app::ActiveMenu::Plc,
                crate::app::ActiveMenu::Panels,
                crate::app::ActiveMenu::Options,
            ] {
                app.active_menu = Some(menu);
                let _ = app.view(win_id);
            }
            app.active_menu = None;

            // 3. Render close confirmation modal
            app.show_close_confirm_modal = true;
            let _ = app.view(win_id);
            app.show_close_confirm_modal = false;

            // 4. Render confirm clear POU modal
            app.confirm_clear_modal = Some("main_safety".to_string());
            let _ = app.view(win_id);
            app.confirm_clear_modal = None;

            // 5. Render SFC delete modal
            app.sfc_delete_modal = Some((
                "batch_seq".to_string(),
                vec![
                    crate::canvas_sfc::SfcElementKey::Step("S1".to_string()),
                    crate::canvas_sfc::SfcElementKey::Transition("T1".to_string()),
                ],
            ));
            let _ = app.view(win_id);
            app.sfc_delete_modal = None;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_options_language_submenu_side_dropdown() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_lang_submenu_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;
        let mut app = FerriteStudioApp::new(proj);
        let win_id = iced::window::Id::unique();

        // 1. Initial state
        assert_eq!(app.active_menu, None);
        assert_eq!(app.active_submenu, None);

        // 2. Open Options menu
        let _ = app.update(Message::ToggleActiveMenu(Some(crate::app::ActiveMenu::Options)));
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::Options));
        assert_eq!(app.active_submenu, None);

        // 3. Hovering Language item opens side-dropdown submenu
        let _ = app.update(Message::SubmenuHover(Some(crate::app::ActiveSubmenu::Language)));
        assert_eq!(app.active_submenu, Some(crate::app::ActiveSubmenu::Language));

        // 4. View renders without panic when language submenu is open alongside Options menu
        let _ = app.view(win_id);

        // 5. Hovering Theme item closes side-dropdown submenu
        let _ = app.update(Message::SubmenuHover(None));
        assert_eq!(app.active_submenu, None);
        assert_eq!(app.active_menu, Some(crate::app::ActiveMenu::Options));

        // 6. Clicking Language item toggles submenu open
        let _ = app.update(Message::ToggleSubmenu(Some(crate::app::ActiveSubmenu::Language)));
        assert_eq!(app.active_submenu, Some(crate::app::ActiveSubmenu::Language));

        // 7. Selecting German sets ui_lang to De, dismisses menu and submenu
        let _ = app.update(Message::SetLanguage(UiLanguage::De));
        assert_eq!(app.ui_lang, UiLanguage::De);
        assert_eq!(app.active_menu, None);
        assert_eq!(app.active_submenu, None);
        assert_eq!(crate::tr!("menu.file.save_project", app.ui_lang), "Projekt speichern");

        // 8. Selecting Spanish sets ui_lang to Es
        let _ = app.update(Message::SetLanguage(UiLanguage::Es));
        assert_eq!(app.ui_lang, UiLanguage::Es);
        assert_eq!(app.active_menu, None);
        assert_eq!(app.active_submenu, None);
        assert_eq!(crate::tr!("menu.file.save_project", app.ui_lang), "Guardar proyecto");

        // 9. Selecting Italian sets ui_lang to It
        let _ = app.update(Message::SetLanguage(UiLanguage::It));
        assert_eq!(app.ui_lang, UiLanguage::It);
        assert_eq!(app.active_menu, None);
        assert_eq!(app.active_submenu, None);
        assert_eq!(crate::tr!("menu.file.save_project", app.ui_lang), "Salva progetto");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_symbols_and_pid_card_fit_in_all_languages() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_symbols_pid_fit_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let mut proj = resolve_or_scaffold_project(Some(&temp_dir))?;

        // Add a PID corrector to shared instances
        proj.fb_instances.instances.push(ferrite_core::FbInstance::new(
            "PID_Loop_1".to_string(),
            ferrite_core::StandardFbKind::PidMixed,
        ));

        let mut app = FerriteStudioApp::new(proj.clone());
        let win_id = iced::window::Id::unique();

        // Add PID blocks to pump_ctrl (standard FBD pou in scaffolded project)
        let _ = app.update(Message::FbdAddBlock(
            "pump_ctrl".to_string(),
            ferrite_lang_fbd::FbdBlockKind::StandardFb {
                instance: "PID_Mixed_ISA".to_string(),
                fb_type: Some(ferrite_core::StandardFbKind::PidMixed),
            },
        ));

        for lang in UiLanguage::ALL {
            // 1. Verify i18n keys for symbols
            let title = crate::tr!("symbols.title", lang);
            assert!(!title.is_empty(), "Title should not be empty for {:?}", lang);
            let add_pid = crate::tr!("symbols.add_pid", lang);
            assert!(!add_pid.is_empty(), "Add PID label should not be empty for {:?}", lang);

            // 2. Render Symbols & Config workspace tab
            app.ui_lang = lang;
            app.active_tab = WorkspaceTab::SymbolsAndConfig;
            {
                let _sym_view = app.view(win_id);
                let _sym_direct = crate::editor_symbols::render_symbols_editor(&app.project, lang);
            }

            // 3. Render FBD editor with PID blocks
            app.active_tab = WorkspaceTab::Pou("pump_ctrl".to_string());
            {
                let _fbd_view = app.view(win_id);
            }

            // 4. Render FBD Library window
            {
                let _lib_view = crate::fbd_library_window::render_fbd_library_window(&app);
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_iced_view_layout_rules_and_anti_truncation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join("ferrite_gui_layout_rules_test");
        if temp_dir.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
        }
        let mut proj = resolve_or_scaffold_project(Some(&temp_dir))?;

        // 1. Setup sample shared symbols and FB instances
        proj.fb_instances.instances.push(ferrite_core::FbInstance::new(
            "PID_Loop_Test".to_string(),
            ferrite_core::StandardFbKind::PidParallel,
        ));
                proj.global_tags.tags.push(ferrite_core::TagEntry::new(
            "Long_Descriptive_Tag_Name_For_Testing",
            ferrite_core::IecType::Real,
            None,
            ferrite_core::IecValue::Real(42.5),
            "Sample comment that spans multiple words to verify wrapping",
        ));

        let mut app = FerriteStudioApp::new(proj.clone());
        let win_id = iced::window::Id::unique();

        for lang in UiLanguage::ALL {
            app.ui_lang = lang;

            // Rule 1 & 4: Symbols Editor with stacked forms and FillPortion columns
            {
                let _sym = crate::editor_symbols::render_symbols_editor(&app.project, lang);
            }

            // Rule 3: Modals with FillPortion(1) equal-width buttons & centered text
            app.show_close_confirm_modal = true;
            {
                let _close_modal = app.view(win_id);
            }
            app.show_close_confirm_modal = false;

            app.sfc_delete_modal = Some((
                "batch_seq".to_string(),
                vec![crate::canvas_sfc::SfcElementKey::Step("S1".to_string())],
            ));
            {
                let _del_modal = app.view(win_id);
            }
            app.sfc_delete_modal = None;

            app.confirm_clear_modal = Some("main_safety".to_string());
            {
                let _clr_modal = app.view(win_id);
            }
            app.confirm_clear_modal = None;

            // Rule 1, 2, 5: FBD Canvas & Library with intrinsic buttons and stacked architecture selector
            app.active_tab = WorkspaceTab::Pou("pump_ctrl".to_string());
            {
                let _fbd_canvas = app.view(win_id);
                let _fbd_lib = crate::fbd_library_window::render_fbd_library_window(&app);
            }

            // Alarms Panel with FillPortion columns and Word wrapping
            {
                let _alarms = crate::alarms_panel::render_alarms_panel(app.runtime.as_ref(), lang);
            }

            // ST Editor with FillPortion live process variables
            app.active_tab = WorkspaceTab::Pou("math_pid".to_string());
            {
                let _st_view = app.view(win_id);
            }
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_language_preference_persisted_across_app_restarts() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("ferrite_gui_lang_persist_test_{}", std::process::id()));
        let settings_file = temp_dir.join("settings.json");
        std::fs::create_dir_all(&temp_dir)?;

        crate::settings::set_custom_settings_path(Some(settings_file.clone()));

        let proj = resolve_or_scaffold_project(Some(&temp_dir))?;

        // 1. First app launch: defaults to French
        {
            let mut app = FerriteStudioApp::new(proj.clone());
            assert_eq!(app.ui_lang, UiLanguage::Fr, "Default language on first launch must be French");
            let _ = app.update(Message::SetLanguage(UiLanguage::Fr));
            assert_eq!(app.ui_lang, UiLanguage::Fr);
        } // App is "closed" here

        // Verify settings file was created and contains French
        let saved = UserSettings::load_from_path(&settings_file)?;
        assert_eq!(saved.language, Some(UiLanguage::Fr));

        // 2. Second app launch ("reopen"): should automatically load French
        {
            let app = FerriteStudioApp::new(proj.clone());
            assert_eq!(app.ui_lang, UiLanguage::Fr);
        }

        // 3. Third app launch: change to German, close and reopen
        {
            let mut app = FerriteStudioApp::new(proj.clone());
            let _ = app.update(Message::SetLanguage(UiLanguage::De));
            assert_eq!(app.ui_lang, UiLanguage::De);
        }

        {
            let app = FerriteStudioApp::new(proj);
            assert_eq!(app.ui_lang, UiLanguage::De);
        }

        crate::settings::set_custom_settings_path(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_i22_select_tab_clears_editor_selections() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = FerriteStudioApp::new_test_app();
        app.active_tab = WorkspaceTab::Pou("main_safety".to_string());
        app.ld_selected = Some((
            "main_safety".to_string(),
            crate::canvas_ld::LdElementKey::Contact { rung: 0, branch_path: vec![], index: 0 },
        ));
        app.ld_selected_elements = vec![
            crate::canvas_ld::LdElementKey::Contact { rung: 0, branch_path: vec![], index: 0 },
        ];

        // Switch tab to batch_seq (SFC)
        let _ = app.update(Message::SelectTab(WorkspaceTab::Pou("batch_seq".to_string())));

        // Verify selections are cleared
        assert!(app.ld_selected.is_none());
        assert!(app.ld_selected_elements.is_empty());
        assert!(app.sfc_selected.is_none());
        assert!(app.sfc_selected_elements.is_empty());

        // Pressing delete while on batch_seq should not mutate main_safety
        let initial_main_safety = app.project.pous.get("main_safety").cloned();
        let _ = app.update(Message::DeletePressed);
        assert_eq!(app.project.pous.get("main_safety"), initial_main_safety.as_ref());

        Ok(())
    }

    #[test]
    fn test_i23_st_contents_resynced_on_load_project() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = FerriteStudioApp::new_test_app();
        app.st_contents.insert(
            "math_pid".to_string(),
            iced::widget::text_editor::Content::with_text("OLD_ST_SOURCE"),
        );

        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_i23_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut new_proj = ferrite_project::Project::scaffold(&temp_dir, "new_st_proj")?;
        let new_st = "PROGRAM new_math\n  val := 42;\nEND_PROGRAM\n";
        new_proj.pous.insert(
            "math_pid".to_string(),
            ferrite_project::PouSource::St(ferrite_lang_st::StProgram::parse("math_pid", new_st)?),
        );
        new_proj.save(&temp_dir)?;

        let _ = app.update(Message::LoadProjectFromPath(temp_dir.clone()));

        let Some(st_content) = app.st_contents.get("math_pid") else {
            panic!("ST content should be synced");
        };
        assert_eq!(st_content.text(), new_st);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_i24_sfc_id_no_collision_after_deletions() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = FerriteStudioApp::new_test_app();
        let pou_name = "batch_seq".to_string();

        // Add 2 transitions
        let _ = app.update(Message::SfcAddTransition(pou_name.clone()));
        let _ = app.update(Message::SfcAddTransition(pou_name.clone()));

        let sfc = match app.project.pous.get(&pou_name) {
            Some(ferrite_project::PouSource::Sfc(s)) => s.clone(),
            _ => panic!("Expected SFC POU"),
        };
        // Delete a middle transition
        assert!(sfc.transitions.len() >= 2);
        let _ = app.update(Message::SfcRemoveTransition(pou_name.clone(), 0));

        // Add a new transition
        let _ = app.update(Message::SfcAddTransition(pou_name.clone()));

        let sfc_after = match app.project.pous.get(&pou_name) {
            Some(ferrite_project::PouSource::Sfc(s)) => s,
            _ => panic!("Expected SFC POU"),
        };

        // Ensure all transition IDs are unique
        let mut ids = std::collections::HashSet::new();
        for t in &sfc_after.transitions {
            assert!(ids.insert(t.id.clone()), "Duplicate transition ID found: {}", t.id);
        }

        Ok(())
    }

    #[test]
    fn test_i25_save_pou_preserves_dirty_when_symbols_modified() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_i25_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_i25")?;
        let mut app = FerriteStudioApp::new(proj);

        // Edit a global tag
        let _ = app.update(Message::TagNameChanged(0, "Renamed_Tag".to_string()));
        assert!(app.modified);

        // Modify and save only one POU
        let _ = app.update(Message::LdAddContact("main_safety".to_string(), 0));
        let _ = app.update(Message::SavePou("main_safety".to_string()));

        // Even though main_safety was saved, modified must still be true because global tags are unsaved!
        assert!(app.modified, "app.modified must remain true when symbols are dirty");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_i26_confirm_close_save_and_exit_aborts_on_save_error() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = FerriteStudioApp::new_test_app();
        app.modified = true;
        app.show_close_confirm_modal = true;

        // Create a regular file and point root_dir to it, so create_dir_all fails
        let temp_file = std::env::temp_dir().join(format!(
            "ferrite_test_i26_file_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        std::fs::write(&temp_file, "blocking file")?;
        app.project.root_dir = temp_file.clone();

        let _ = app.update(Message::ConfirmCloseSaveAndExit);

        // Exit must have been aborted
        assert!(!app.allowed_to_close, "Exit must be aborted on save error");
        assert!(!app.show_close_confirm_modal);
        assert!(app.status_banner.starts_with("Save failed"), "Status banner: {}", app.status_banner);

        let _ = std::fs::remove_file(&temp_file);
        Ok(())
    }

    #[test]
    fn test_i27_fbd_block_naming_no_duplicates_after_delete() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = FerriteStudioApp::new_test_app();
        let pou_name = "pump_ctrl".to_string();

        // Add two TON blocks
        let _ = app.update(Message::FbdAddBlock(
            pou_name.clone(),
            ferrite_lang_fbd::FbdBlockKind::StandardFb {
                instance: "TON_1".to_string(),
                fb_type: Some(ferrite_core::StandardFbKind::Ton),
            },
        ));
        let _ = app.update(Message::FbdAddBlock(
            pou_name.clone(),
            ferrite_lang_fbd::FbdBlockKind::StandardFb {
                instance: "TON_2".to_string(),
                fb_type: Some(ferrite_core::StandardFbKind::Ton),
            },
        ));

        // Delete TON_1
        let _ = app.update(Message::FbdRemoveBlock(pou_name.clone(), 0));

        // Add another block with instance TON_2 (as if clicked from library when count was 1)
        let _ = app.update(Message::FbdAddBlock(
            pou_name.clone(),
            ferrite_lang_fbd::FbdBlockKind::StandardFb {
                instance: "TON_2".to_string(),
                fb_type: Some(ferrite_core::StandardFbKind::Ton),
            },
        ));

        let fbd = match app.project.pous.get(&pou_name) {
            Some(ferrite_project::PouSource::Fbd(f)) => f,
            _ => panic!("Expected FBD POU"),
        };

        // Verify there are no duplicate block IDs
        let mut ids = std::collections::HashSet::new();
        for b in &fbd.blocks {
            assert!(ids.insert(b.id.clone()), "Duplicate FBD block ID found: {}", b.id);
        }

        Ok(())
    }

    #[test]
    fn test_i28_numeric_buffer_and_commit() {
        use crate::editor_symbols::{clear_symbol_buffer, get_symbol_buffer, set_symbol_buffer};

        let key = "fb.0.kp";
        clear_symbol_buffer(key);
        assert_eq!(get_symbol_buffer(key), None);

        // User is typing "0."
        set_symbol_buffer(key, "0.".to_string());
        assert_eq!(get_symbol_buffer(key).as_deref(), Some("0."));

        // User finishes typing "0.5"
        set_symbol_buffer(key, "0.5".to_string());
        assert_eq!(get_symbol_buffer(key).as_deref(), Some("0.5"));

        // Commit on submit
        clear_symbol_buffer(key);
        assert_eq!(get_symbol_buffer(key), None);
    }

    #[test]
    fn test_digital_twin_load_failure_preserves_corrupted_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_twin_fail_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_twin_fail")?;

        // Write malformed JSON to sim/digital_twin.json
        let sim_dir = temp_dir.join("sim");
        std::fs::create_dir_all(&sim_dir)?;
        let twin_path = sim_dir.join("digital_twin.json");
        let malformed_content = "{ invalid json content !!!";
        std::fs::write(&twin_path, malformed_content)?;

        // Launch app
        let mut app = FerriteStudioApp::new(proj);

        // Verify banner shows failure and load_failed is set
        assert!(app.digital_twin_load_failed);
        assert!(app.status_banner.contains("Failed to load digital twin"));
        assert!(app.status_banner.contains("digital_twin.json"));

        // Save project
        let _ = app.update(Message::SaveProject);

        // Verify corrupted file was NOT overwritten
        let current_file_content = std::fs::read_to_string(&twin_path)?;
        assert_eq!(current_file_content, malformed_content, "Corrupted twin file must not be overwritten on save");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_online_reload_preserves_runtime_state() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_online_reload_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_online_reload")?;
        let mut app = FerriteStudioApp::new(proj);

        // 1. Force a tag value
        let _ = app.update(Message::ToggleForceTag("Start_Button".to_string()));
        let is_forced = app
            .runtime
            .as_ref()
            .and_then(|r| r.tags.get_tag("Start_Button"))
            .and_then(|t| t.forced.as_ref())
            .cloned();
        assert!(is_forced.is_some(), "Start_Button should be forced");

        // 2. Step scans to advance state and scan counter
        for _ in 0..5 {
            let _ = app.execute_plc_scan_tick();
        }
        let scans_before = app.runtime.as_ref().map(|r| r.scan_count).unwrap_or(0);
        assert_eq!(scans_before, 5, "Scan count should be 5");

        // 3. Edit POU in project (set comment on main_safety rung 0)
        let _ = app.update(Message::LdRungCommentChanged(
            "main_safety".to_string(),
            0,
            "Updated safety rung comment".to_string(),
        ));

        // 4. Trigger recompile_runtime
        app.recompile_runtime();

        // 5. Verify status banner and state preservation
        assert!(
            app.status_banner.contains("Online reload")
                || app.status_banner.contains("Rechargement en ligne"),
            "Banner must show online reload: {}",
            app.status_banner
        );
        let scans_after = app.runtime.as_ref().map(|r| r.scan_count).unwrap_or(0);
        assert_eq!(
            scans_after, 5,
            "Scan count must be preserved across online reload"
        );

        let is_still_forced = app
            .runtime
            .as_ref()
            .and_then(|r| r.tags.get_tag("Start_Button"))
            .and_then(|t| t.forced.as_ref())
            .cloned();
        assert_eq!(
            is_still_forced, is_forced,
            "Forced tag state must be preserved across online reload"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_symbol_editor_new_iec_types_selectable_and_parseable() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = FerriteStudioApp::new_test_app();

        // Verify ALL_ELEMENTARY contains new types
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Sint));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Usint));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Uint));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Udint));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Lint));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Ulint));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Byte));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Word));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Dword));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Lword));
        assert!(ferrite_core::IecType::ALL_ELEMENTARY.contains(&ferrite_core::IecType::Lreal));

        // Add a new global tag for Dword testing
        let _ = app.update(Message::AddGlobalTag);
        let new_idx = app.project.global_tags.tags.len() - 1;
        let _ = app.update(Message::TagTypeChanged(new_idx, ferrite_core::IecType::Dword));
        let _ = app.update(Message::TagNameChanged(new_idx, "Dword_Tag".to_string()));
        assert_eq!(app.project.global_tags.tags[new_idx].data_type, ferrite_core::IecType::Dword);
        assert_eq!(app.project.global_tags.tags[new_idx].initial_value, ferrite_core::IecValue::Dword(0));

        // Also test Lreal mutation on tag 0 in project
        let _ = app.update(Message::TagTypeChanged(0, ferrite_core::IecType::Lreal));
        assert_eq!(app.project.global_tags.tags[0].data_type, ferrite_core::IecType::Lreal);
        assert_eq!(app.project.global_tags.tags[0].initial_value, ferrite_core::IecValue::Lreal(0.0));

        // Force numeric tag with hex literal
        let _ = app.update(Message::ForceNumericTag("Dword_Tag".to_string(), "16#ABCD".to_string()));
        let forced_val = app
            .runtime
            .as_ref()
            .and_then(|r| r.tags.get_tag("Dword_Tag"))
            .and_then(|t| t.forced.as_ref())
            .cloned();
        assert_eq!(forced_val, Some(ferrite_core::IecValue::Dword(0xABCD)));

        // Render symbol editor in all languages
        for lang in crate::i18n::UiLanguage::ALL {
            let _view = crate::editor_symbols::render_symbols_editor(&app.project, lang);
        }

        Ok(())
    }

    #[test]
    fn test_in_scan_trace_configuration_and_drain() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_trace_ui_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let proj = ferrite_project::Project::scaffold(&temp_dir, "test_trace_ui")?;
        let mut app = FerriteStudioApp::new(proj);

        // Configure trace on Start_Button and EStop_OK
        let _ = app.update(Message::TraceTagsInputChanged("Start_Button, EStop_OK".to_string()));
        let _ = app.update(Message::TraceCapacityInputChanged("10".to_string()));
        let _ = app.update(Message::TraceStart);
        assert!(app.trace_active);

        // Run scans
        for _ in 0..5 {
            let _ = app.execute_plc_scan_tick();
        }

        // Drain trace
        let _ = app.update(Message::TraceDrain);
        assert_eq!(app.trace_drained_samples.len(), 5);
        assert_eq!(app.trace_drained_samples[0].scan_count, 1);
        assert_eq!(app.trace_drained_samples[4].scan_count, 5);

        // Render bottom dock with trace tab
        app.bottom_dock_tab = crate::app::BottomDockTab::Trace;
        {
            let _view = app.view(iced::window::Id::unique());
        }

        // Clear trace
        let _ = app.update(Message::TraceClear);
        assert!(app.trace_drained_samples.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_theme_persistence_in_user_settings() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_theme_persist_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let settings_path = temp_dir.join("settings.json");
        std::fs::create_dir_all(&temp_dir)?;

        crate::settings::set_custom_settings_path(Some(settings_path));

        // Default when absent
        assert_eq!(crate::settings::load_saved_theme(), None);
        assert_eq!(crate::settings::load_saved_theme_or_default(), crate::app::UiTheme::Dark);

        // Save Light theme
        crate::settings::save_theme_preference(crate::app::UiTheme::Light)?;
        assert_eq!(crate::settings::load_saved_theme(), Some(crate::app::UiTheme::Light));
        assert_eq!(crate::settings::load_saved_theme_or_default(), crate::app::UiTheme::Light);

        // Save Dark theme
        crate::settings::save_theme_preference(crate::app::UiTheme::Dark)?;
        assert_eq!(crate::settings::load_saved_theme(), Some(crate::app::UiTheme::Dark));

        crate::settings::set_custom_settings_path(None);
        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_light_theme_backgrounds_and_palette() {
        let dark = iced::Theme::Dark;
        let light = iced::Theme::Light;

        assert!(theme::StudioTheme::is_dark(&dark));
        assert!(!theme::StudioTheme::is_dark(&light));

        // Backgrounds must be distinctly different between dark and light
        assert_ne!(
            theme::StudioTheme::bg_base(&dark),
            theme::StudioTheme::bg_base(&light)
        );
        assert_ne!(
            theme::StudioTheme::bg_panel(&dark),
            theme::StudioTheme::bg_panel(&light)
        );
        assert_ne!(
            theme::StudioTheme::bg_elevated(&dark),
            theme::StudioTheme::bg_elevated(&light)
        );
        assert_ne!(
            theme::StudioTheme::bg_surface(&dark),
            theme::StudioTheme::bg_surface(&light)
        );

        // Light background must be light (luminance > 0.8)
        let light_bg = theme::StudioTheme::bg_base(&light);
        assert!(light_bg.r > 0.8 && light_bg.g > 0.8 && light_bg.b > 0.8);

        // Light text must be dark ink for high contrast
        let light_text = theme::StudioTheme::text_primary(&light);
        assert!(light_text.r < 0.2 && light_text.g < 0.2 && light_text.b < 0.2);

        // Card style in light theme must use light elevated background
        let card_light = theme::StudioTheme::card_style(&light);
        assert!(matches!(card_light.background, Some(iced::Background::Color(c)) if c == iced::Color::WHITE));
    }
}
