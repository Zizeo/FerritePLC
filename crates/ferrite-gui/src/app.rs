use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Stroke, Visuals};
use ferrite_core::{IecType, IecValue, LanguageKind, MemoryArea, ProcessImage, RuntimeError};
use ferrite_fieldbus_modbus::{
    ModbusRequest, ModbusResponse, ModbusTcpClient, ModbusTcpServer,
};
use ferrite_hal::{
    create_hardware_driver, GpioHalConfig, HardwareDriver, PinDirection, RPiPinInfo,
    RPI_40PIN_HEADER,
};
use ferrite_ir::ValidationDiagnostic;
use ferrite_project::{PouSource, Project};
use ferrite_runtime::{PlcRuntime, ScanTelemetry};

use crate::canvas_fbd::render_fbd_editor;
use crate::canvas_ld::render_ld_editor;
use crate::canvas_sfc::{SfcStepRef, render_sfc_editor};
use crate::editor_st::render_st_editor;
use crate::editor_symbols::render_symbols_editor;
use crate::digital_twin::{DigitalTwinWorkspace, SharedIoHandle, SharedIoState};
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;
use crate::trend_plotter::TrendPlotter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceTab {
    Pou(String),
    SymbolsAndConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BottomDockTab {
    #[default]
    Diagnostics,
    TrendPlotter,
    Alarms,
    Modbus,
    Gpio,
}

/// Visual theme of FerritePLC Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiTheme {
    #[default]
    Dark,
    Light,
}

impl UiTheme {
    pub fn visuals(self) -> Visuals {
        match self {
            UiTheme::Dark => {
                let mut v = Visuals::dark();
                // Deep obsidian background palette
                v.panel_fill = Color32::from_rgb(13, 17, 23); // #0D1117 (Obsidian slate)
                v.window_fill = Color32::from_rgb(22, 27, 34); // #161B22 (Card / Modal background)
                v.extreme_bg_color = Color32::from_rgb(9, 12, 16); // #090C10 (Canvas / Editor / Inputs)
                v.faint_bg_color = Color32::from_rgb(17, 22, 30);
                v.code_bg_color = Color32::from_rgb(15, 20, 28);

                // Crisp modern borders
                v.window_stroke = egui::Stroke::new(1.0_f32, Color32::from_rgb(48, 54, 61));
                v.widgets.noninteractive.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(33, 38, 45));
                v.widgets.noninteractive.fg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(139, 148, 158));
                v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(6);

                // Inactive widgets (buttons, pills, comboboxes)
                v.widgets.inactive.bg_fill = Color32::from_rgb(30, 36, 46);
                v.widgets.inactive.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(48, 56, 70));
                v.widgets.inactive.fg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(230, 237, 243));
                v.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

                // Hovered widgets with vibrant accent
                v.widgets.hovered.bg_fill = Color32::from_rgb(40, 48, 62);
                v.widgets.hovered.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248)); // #38BDF8 Sky cyan
                v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
                v.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

                // Active / Pressed widgets
                v.widgets.active.bg_fill = Color32::from_rgb(14, 116, 144); // #0E7490 Cyan-blue
                v.widgets.active.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248));
                v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
                v.widgets.active.corner_radius = egui::CornerRadius::same(6);

                // Open menus and combos
                v.widgets.open.bg_fill = Color32::from_rgb(26, 32, 42);
                v.widgets.open.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248));
                v.widgets.open.corner_radius = egui::CornerRadius::same(6);

                // Selection
                v.selection.bg_fill = Color32::from_rgba_premultiplied(14, 116, 144, 100);
                v.selection.stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(56, 189, 248));

                // Window & Menu Corner Radius
                v.window_corner_radius = egui::CornerRadius::same(8);
                v.menu_corner_radius = egui::CornerRadius::same(6);

                v
            }
            UiTheme::Light => {
                let mut v = Visuals::light();
                v.panel_fill = Color32::from_rgb(248, 250, 252);
                v.window_fill = Color32::from_rgb(255, 255, 255);
                v.extreme_bg_color = Color32::from_rgb(241, 245, 249);
                v.widgets.inactive.bg_fill = Color32::from_rgb(255, 255, 255);
                v.widgets.inactive.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(203, 213, 225));
                v.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
                v.widgets.hovered.bg_fill = Color32::from_rgb(241, 245, 249);
                v.widgets.hovered.bg_stroke =
                    egui::Stroke::new(1.0_f32, Color32::from_rgb(2, 132, 199));
                v.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
                v.widgets.active.bg_fill = Color32::from_rgb(2, 132, 199);
                v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
                v.widgets.active.corner_radius = egui::CornerRadius::same(6);
                v.window_corner_radius = egui::CornerRadius::same(8);
                v
            }
        }
    }
}

/// Maximum number of undo snapshots kept in memory.
const MAX_HISTORY: usize = 100;
/// Edits closer together than this are coalesced into a single undo step (e.g. typing).
const HISTORY_COALESCE: Duration = Duration::from_millis(800);

pub struct FerriteStudioApp {
    pub project: Project,
    pub runtime: Option<PlcRuntime>,
    pub diagnostics: Vec<ValidationDiagnostic>,
    pub active_tab: WorkspaceTab,
    pub sim_running: bool,
    pub modified: bool,
    pub last_tick: Instant,
    pub status_banner: String,
    pub scenario_logs: Vec<String>,
    pub new_pou_name: String,
    pub new_pou_lang: LanguageKind,
    pub open_path_input: String,
    pub ui_lang: UiLanguage,
    pub ui_theme: UiTheme,
    pub show_left_explorer: bool,
    pub show_right_io: bool,
    pub show_bottom_diagnostics: bool,
    pub detach_left_explorer: bool,
    pub detach_right_io: bool,
    pub detach_bottom_dock: bool,
    pub show_close_confirm_modal: bool,
    pub allowed_to_close: bool,
    pub undo_stack: Vec<Project>,
    pub redo_stack: Vec<Project>,
    last_committed_project: Project,
    last_edit_time: Option<Instant>,
    pub modbus_server: Option<ModbusTcpServer>,
    pub modbus_port: u16,
    pub modbus_shared_pi: Arc<RwLock<ProcessImage>>,
    pub modbus_last_test_result: Option<String>,
    pub trend_plotter: TrendPlotter,
    pub bottom_dock_tab: BottomDockTab,
    pub hal_config: GpioHalConfig,
    pub hal_driver: Box<dyn HardwareDriver>,
    pub hal_enabled: bool,
    pub show_digital_twin_window: bool,
    pub shared_io: SharedIoHandle,
    pub digital_twin_workspace: DigitalTwinWorkspace,
    pub io_filter_text: String,
    pub io_filter_area: Option<MemoryArea>,
    pub pou_filter_text: String,
}

impl FerriteStudioApp {
    pub fn new(initial_project: Project) -> Self {
        Self::with_language(initial_project, UiLanguage::En)
    }

    pub fn with_language(initial_project: Project, ui_lang: UiLanguage) -> Self {
        let first_pou = initial_project
            .manifest
            .pous
            .first()
            .map(|p| WorkspaceTab::Pou(p.name.clone()))
            .unwrap_or(WorkspaceTab::SymbolsAndConfig);

        let diagnostics = initial_project.check().unwrap_or_default();
        let runtime = PlcRuntime::from_project(&initial_project).ok();
        let open_path_input = initial_project.root_dir.display().to_string();

        let initial_pi = runtime
            .as_ref()
            .map(|rt| rt.process_image.clone())
            .unwrap_or_default();
        let modbus_shared_pi = Arc::new(RwLock::new(initial_pi));

        let hal_config = GpioHalConfig::default();
        let hal_driver = create_hardware_driver(hal_config.clone());

        let mut initial_shared_io = SharedIoState::new();
        if let Some(ref rt) = runtime {
            for tag in rt.tags.tags() {
                let val = if let Some(addr) = tag.address {
                    rt.process_image.read(addr, tag.data_type).unwrap_or_else(|_| tag.current_value().clone())
                } else {
                    tag.current_value().clone()
                };
                initial_shared_io.values.insert(tag.name.clone(), val);
                initial_shared_io.types.insert(tag.name.clone(), tag.data_type);
            }
        }
        let shared_io = Arc::new(RwLock::new(initial_shared_io));
        let digital_twin_workspace = DigitalTwinWorkspace::with_starter_plant();

        let status_banner = ui_lang
            .tr(
                "Ready — Compiled all POUs into unified ProcessImage & TagTable.",
                "Prêt — Tous les POU sont compilés dans la mémoire image et la table des symboles.",
            )
            .to_string();

        Self {
            last_committed_project: initial_project.clone(),
            project: initial_project,
            runtime,
            diagnostics,
            active_tab: first_pou,
            sim_running: false,
            modified: false,
            last_tick: Instant::now(),
            status_banner,
            scenario_logs: Vec::new(),
            new_pou_name: String::new(),
            new_pou_lang: LanguageKind::Ld,
            open_path_input,
            ui_lang,
            ui_theme: UiTheme::Dark,
            show_left_explorer: true,
            show_right_io: true,
            show_bottom_diagnostics: true,
            detach_left_explorer: false,
            detach_right_io: false,
            detach_bottom_dock: false,
            show_close_confirm_modal: false,
            allowed_to_close: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: None,
            modbus_server: None,
            modbus_port: 5020,
            modbus_shared_pi,
            modbus_last_test_result: None,
            trend_plotter: TrendPlotter::default(),
            bottom_dock_tab: BottomDockTab::Diagnostics,
            hal_config,
            hal_driver,
            hal_enabled: false,
            show_digital_twin_window: false,
            shared_io,
            digital_twin_workspace,
            io_filter_text: String::new(),
            io_filter_area: None,
            pou_filter_text: String::new(),
        }
    }

    /// Records a history snapshot whenever the project differs from the last committed state.
    /// Rapid successive edits (typing, dragging) are coalesced into a single undo step.
    pub fn track_history(&mut self) {
        if self.project == self.last_committed_project {
            return;
        }
        let now = Instant::now();
        let coalesce = self
            .last_edit_time
            .is_some_and(|t| now.duration_since(t) < HISTORY_COALESCE);
        if !coalesce {
            let prev = std::mem::replace(&mut self.last_committed_project, self.project.clone());
            self.undo_stack.push(prev);
            if self.undo_stack.len() > MAX_HISTORY {
                self.undo_stack.remove(0);
            }
        } else {
            self.last_committed_project = self.project.clone();
        }
        self.redo_stack.clear();
        self.last_edit_time = Some(now);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            let current = std::mem::replace(&mut self.project, prev);
            self.redo_stack.push(current);
            self.after_history_jump(true);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            let current = std::mem::replace(&mut self.project, next);
            self.undo_stack.push(current);
            self.after_history_jump(false);
        }
    }

    fn after_history_jump(&mut self, was_undo: bool) {
        self.last_committed_project = self.project.clone();
        self.last_edit_time = None;
        self.modified = true;
        if let WorkspaceTab::Pou(name) = &self.active_tab
            && !self.project.pous.contains_key(name)
        {
            self.active_tab = WorkspaceTab::SymbolsAndConfig;
        }
        self.recompile_and_reset();
        let lang = self.ui_lang;
        self.status_banner = if was_undo {
            lang.tr("Undo applied.", "Annulation appliquée.")
        } else {
            lang.tr("Redo applied.", "Rétablissement appliqué.")
        }
        .to_string();
    }

    pub fn recompile_and_reset(&mut self) {
        let lang = self.ui_lang;
        match self.project.check() {
            Ok(diags) => {
                self.diagnostics = diags;
                if self.diagnostics.is_empty() {
                    match PlcRuntime::from_project(&self.project) {
                        Ok(rt) => {
                            if let Ok(mut mb_pi) = self.modbus_shared_pi.write() {
                                *mb_pi = rt.process_image.clone();
                            }
                            self.runtime = Some(rt);
                            self.status_banner = match lang {
                                UiLanguage::En => format!(
                                    "CHECK OK: Compiled '{}' ({} POUs) — PLC Runtime reset.",
                                    self.project.manifest.name,
                                    self.project.pous.len()
                                ),
                                UiLanguage::Fr => format!(
                                    "VÉRIFICATION OK : '{}' compilé ({} POU) — Exécuteur API réinitialisé.",
                                    self.project.manifest.name,
                                    self.project.pous.len()
                                ),
                            };
                        }
                        Err(e) => {
                            self.status_banner = format!(
                                "{}: {e}",
                                lang.tr("Compile error", "Erreur de compilation")
                            );
                        }
                    }
                } else {
                    self.sim_running = false;
                    self.status_banner = match lang {
                        UiLanguage::En => format!(
                            "Validation failed with {} diagnostic error(s).",
                            self.diagnostics.len()
                        ),
                        UiLanguage::Fr => format!(
                            "Échec de validation avec {} erreur(s) de diagnostic.",
                            self.diagnostics.len()
                        ),
                    };
                }
            }
            Err(e) => {
                self.status_banner =
                    format!("{}: {e}", lang.tr("Check error", "Erreur de vérification"));
            }
        }
    }

    pub fn save_project(&mut self) {
        let root = self.project.root_dir.clone();
        let lang = self.ui_lang;
        match self.project.save(&root) {
            Ok(()) => {
                self.modified = false;
                self.status_banner = match lang {
                    UiLanguage::En => format!("Atomically saved project to {}", root.display()),
                    UiLanguage::Fr => {
                        format!("Projet enregistré de façon atomique dans {}", root.display())
                    }
                };
            }
            Err(e) => {
                self.status_banner = format!(
                    "{}: {e}",
                    lang.tr(
                        "Failed to save project",
                        "Échec de l'enregistrement du projet"
                    )
                );
            }
        }
    }

    pub fn save_project_as_dialog(&mut self) {
        let lang = self.ui_lang;
        let mut dialog = rfd::FileDialog::new().set_title(lang.tr(
            "Save FerritePLC Project to Folder",
            "Enregistrer le Projet FerritePLC dans un Dossier",
        ));
        if self.project.root_dir.exists() {
            dialog = dialog.set_directory(&self.project.root_dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.project.root_dir = folder.clone();
            self.open_path_input = folder.display().to_string();
            self.save_project();
        }
    }

    pub fn open_project_folder_dialog(&mut self) {
        let lang = self.ui_lang;
        let mut dialog = rfd::FileDialog::new().set_title(lang.tr(
            "Select FerritePLC Project Folder",
            "Sélectionner le Dossier du Projet FerritePLC",
        ));
        if let Some(parent) = self.project.root_dir.parent()
            && parent.exists()
        {
            dialog = dialog.set_directory(parent);
        } else if self.project.root_dir.exists() {
            dialog = dialog.set_directory(&self.project.root_dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.load_project_from_path(folder);
        }
    }

    pub fn open_project_manifest_dialog(&mut self) {
        let lang = self.ui_lang;
        let mut dialog = rfd::FileDialog::new()
            .set_title(lang.tr(
                "Open FerriteProject.fplc Manifest",
                "Ouvrir le Fichier Manifeste FerriteProject.fplc",
            ))
            .add_filter("FerritePLC Manifest (.fplc)", &["fplc"]);
        if let Some(parent) = self.project.root_dir.parent()
            && parent.exists()
        {
            dialog = dialog.set_directory(parent);
        } else if self.project.root_dir.exists() {
            dialog = dialog.set_directory(&self.project.root_dir);
        }
        if let Some(file_path) = dialog.pick_file() {
            let folder = file_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(file_path);
            self.load_project_from_path(folder);
        }
    }

    pub fn export_plcopen_xml_dialog(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("Export PLCopen TC6 XML")
            .add_filter("PLCopen XML (.xml)", &["xml"])
            .set_file_name("project_plcopen.xml")
            .save_file();
        if let Some(path) = file {
            match self.project.export_plcopen_xml() {
                Ok(xml) => {
                    if let Err(e) = std::fs::write(&path, xml) {
                        self.status_banner = format!("Export failed: {e}");
                    } else {
                        self.status_banner = format!("Exported PLCopen XML to {}", path.display());
                    }
                }
                Err(e) => {
                    self.status_banner = format!("Export error: {e}");
                }
            }
        }
    }

    pub fn import_plcopen_xml_dialog(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("Import PLCopen TC6 XML")
            .add_filter("PLCopen XML (.xml)", &["xml"])
            .pick_file();
        if let Some(xml_path) = file {
            let target_folder = rfd::FileDialog::new()
                .set_title("Select Target Directory for Imported Project")
                .pick_folder();
            if let Some(target_dir) = target_folder {
                match std::fs::read_to_string(&xml_path) {
                    Ok(xml_content) => {
                        match Project::import_plcopen_xml(&xml_content, &target_dir) {
                            Ok(proj) => {
                                self.project = proj;
                                self.open_path_input = target_dir.display().to_string();
                                self.status_banner = format!(
                                    "Successfully imported PLCopen project into {}",
                                    target_dir.display()
                                );
                                self.runtime = PlcRuntime::from_project(&self.project).ok();
                            }
                            Err(e) => {
                                self.status_banner = format!("Import parse error: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        self.status_banner = format!("Failed to read XML file: {e}");
                    }
                }
            }
        }
    }

    pub fn load_project_from_path(&mut self, raw_path: PathBuf) {
        let path = if raw_path.is_file() {
            raw_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(raw_path)
        } else {
            raw_path
        };
        let saved_lang = self.ui_lang;
        let show_l = self.show_left_explorer;
        let show_r = self.show_right_io;
        let show_b = self.show_bottom_diagnostics;
        let det_l = self.detach_left_explorer;
        let det_r = self.detach_right_io;
        let det_b = self.detach_bottom_dock;
        let saved_theme = self.ui_theme;
        match Project::load(&path) {
            Ok(proj) => {
                *self = Self::with_language(proj, saved_lang);
                self.ui_theme = saved_theme;
                self.show_left_explorer = show_l;
                self.show_right_io = show_r;
                self.show_bottom_diagnostics = show_b;
                self.detach_left_explorer = det_l;
                self.detach_right_io = det_r;
                self.detach_bottom_dock = det_b;
                self.status_banner = match saved_lang {
                    UiLanguage::En => format!("Loaded project from {}", path.display()),
                    UiLanguage::Fr => format!("Projet chargé depuis {}", path.display()),
                };
            }
            Err(e) => {
                self.status_banner = format!(
                    "{}: {e}",
                    saved_lang.tr("Load failed", "Échec du chargement")
                );
            }
        }
    }

    pub fn execute_all_scenarios(&mut self) {
        let lang = self.ui_lang;
        self.scenario_logs.clear();
        for scenario in &self.project.scenarios.scenarios {
            match PlcRuntime::from_project(&self.project) {
                Ok(mut rt) => match rt.run_scenario(scenario) {
                    Ok(rep) => {
                        let line = match lang {
                            UiLanguage::En => format!(
                                "[PASS] Scenario '{}': {} scans executed, {} assertions verified (max scan: {} us)",
                                rep.scenario_name,
                                rep.scans_executed,
                                rep.assertions_verified,
                                rep.max_scan_duration_us
                            ),
                            UiLanguage::Fr => format!(
                                "[SUCCÈS] Scénario '{}' : {} cycles exécutés, {} assertions vérifiées (cycle max : {} us)",
                                rep.scenario_name,
                                rep.scans_executed,
                                rep.assertions_verified,
                                rep.max_scan_duration_us
                            ),
                        };
                        self.scenario_logs.push(line);
                        self.runtime = Some(rt);
                    }
                    Err(err) => {
                        self.scenario_logs.push(format!("[FAIL] {err}"));
                    }
                },
                Err(e) => {
                    self.scenario_logs
                        .push(format!("[ERROR] Could not compile runtime: {e}"));
                }
            }
        }
        self.status_banner = match lang {
            UiLanguage::En => format!(
                "Executed {} scenario(s) from sim/scenarios.toml.",
                self.project.scenarios.scenarios.len()
            ),
            UiLanguage::Fr => format!(
                "{} scénario(s) exécuté(s) depuis sim/scenarios.toml.",
                self.project.scenarios.scenarios.len()
            ),
        };
    }

    /// Synchronizes shared I/O state between Main Window runtime and Digital Twin viewport.
    pub fn sync_shared_io(&mut self) {
        if let Ok(mut io) = self.shared_io.write()
            && let Some(ref mut rt) = self.runtime
        {
            // 1. Drain pending interactive writes requested by the Digital Twin canvas HMI
            for (var_name, val) in io.take_pending_writes() {
                let _ = rt.set_stimulus(&var_name, val);
            }

            // 2. Publish current tag values and data types to the shared state
            for tag in rt.tags.tags() {
                let val = if let Some(addr) = tag.address {
                    rt.process_image.read(addr, tag.data_type).unwrap_or_else(|_| tag.current_value().clone())
                } else {
                    tag.current_value().clone()
                };
                io.values.insert(tag.name.clone(), val);
                io.types.insert(tag.name.clone(), tag.data_type);
            }
        }
    }

    pub fn execute_plc_scan_tick(&mut self) -> Result<ScanTelemetry, RuntimeError> {
        // Apply pending Digital Twin HMI interactions before scan execution
        if let Ok(mut io) = self.shared_io.write()
            && let Some(ref mut rt) = self.runtime
        {
            for (var_name, val) in io.take_pending_writes() {
                let _ = rt.set_stimulus(&var_name, val);
            }
        }

        // Phase 1 (Physical Hardware Inputs): read physical pins into process image
        if self.hal_enabled
            && let Some(ref mut rt) = self.runtime
        {
            let _ = self.hal_driver.read_inputs(&mut rt.process_image);
        }

        // Sync external Modbus mutations into runtime process image
        if self.modbus_server.is_some()
            && let Ok(mb_pi) = self.modbus_shared_pi.read()
            && let Some(ref mut rt) = self.runtime
        {
            for tag in rt.tags.tags() {
                if let Some(addr) = tag.address
                    && let Ok(val) = mb_pi.read(addr, tag.data_type)
                {
                    let _ = rt.process_image.write(addr, &val);
                }
            }
        }

        let res = if let Some(ref mut rt) = self.runtime {
            rt.step_scan()
        } else {
            return Err(RuntimeError::ExecutionError {
                pou: "Runtime".into(),
                message: "PLC Runtime is offline".into(),
            });
        };

        if let Some(ref rt) = self.runtime {
            // Phase 3 (Physical Hardware Outputs): write committed outputs to physical pins
            if self.hal_enabled {
                let _ = self.hal_driver.write_outputs(&rt.process_image);
            }

            if self.modbus_server.is_some()
                && let Ok(mut mb_pi) = self.modbus_shared_pi.write()
            {
                *mb_pi = rt.process_image.clone();
            }
            self.trend_plotter.sample(rt);
        }

        // Update shared I/O state for Digital Twin viewport
        self.sync_shared_io();

        res
    }

    pub fn toggle_modbus_server(&mut self) {
        let lang = self.ui_lang;
        if let Some(mut s) = self.modbus_server.take() {
            s.stop();
            self.status_banner = lang
                .tr("Modbus TCP Server stopped.", "Serveur Modbus TCP arrêté.")
                .to_string();
        } else {
            let addr_str = format!("127.0.0.1:{}", self.modbus_port);
            match addr_str.parse::<std::net::SocketAddr>() {
                Ok(addr) => {
                    match ModbusTcpServer::start(addr, Arc::clone(&self.modbus_shared_pi)) {
                        Ok(srv) => {
                            self.modbus_server = Some(srv);
                            self.status_banner = match lang {
                                UiLanguage::En => format!("Modbus TCP Server active on {addr}"),
                                UiLanguage::Fr => format!("Serveur Modbus TCP actif sur {addr}"),
                            };
                        }
                        Err(e) => {
                            self.status_banner = format!(
                                "{}: {e}",
                                lang.tr(
                                    "Could not start Modbus server",
                                    "Impossible de démarrer le serveur Modbus"
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    self.status_banner = format!(
                        "{}: {e}",
                        lang.tr("Invalid bind address", "Adresse de liaison invalide")
                    );
                }
            }
        }
    }

    pub fn test_modbus_ping(&mut self) {
        let lang = self.ui_lang;
        let addr_str = format!("127.0.0.1:{}", self.modbus_port);
        match addr_str.parse::<std::net::SocketAddr>() {
            Ok(addr) => {
                let start = Instant::now();
                match ModbusTcpClient::connect(addr) {
                    Ok(mut client) => {
                        match client.send_request(
                            1,
                            &ModbusRequest::ReadCoils {
                                address: 0,
                                count: 1,
                            },
                        ) {
                            Ok(ModbusResponse::ReadBits(bits)) => {
                                let el = start.elapsed().as_micros();
                                let val_str = bits.first().copied().unwrap_or(false);
                                self.modbus_last_test_result = Some(format!(
                                    "PING OK ({} us) — Read Coil #0 (%QX0.0) = {}",
                                    el, val_str
                                ));
                            }
                            Ok(other) => {
                                self.modbus_last_test_result =
                                    Some(format!("PING OK but unexpected response: {other:?}"));
                            }
                            Err(e) => {
                                self.modbus_last_test_result = Some(format!("PING ERR: {e}"));
                            }
                        }
                    }
                    Err(e) => {
                        self.modbus_last_test_result = Some(format!(
                            "{}: {e}",
                            lang.tr("Connection failed", "Connexion échouée")
                        ));
                    }
                }
            }
            Err(e) => {
                self.modbus_last_test_result = Some(format!("Invalid address: {e}"));
            }
        }
    }

    fn trigger_single_step(&mut self) {
        let lang = self.ui_lang;
        if self.modified {
            self.recompile_and_reset();
            self.modified = false;
        }
        match self.execute_plc_scan_tick() {
            Ok(tel) => {
                self.status_banner = match lang {
                    UiLanguage::En => format!(
                        "Completed Scan #{} ({} us, virtual time = {} ms)",
                        tel.scan_count, tel.scan_duration_us, tel.virtual_time_ms
                    ),
                    UiLanguage::Fr => format!(
                        "Cycle #{} terminé ({} us, horloge virtuelle = {} ms)",
                        tel.scan_count, tel.scan_duration_us, tel.virtual_time_ms
                    ),
                };
            }
            Err(e) => {
                self.status_banner = format!("{}: {e}", lang.tr("Scan error", "Erreur de cycle"));
            }
        }
    }

    pub fn render_left_explorer_body(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;

        // Project Summary Card
        StudioTheme::card_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                StudioTheme::badge(ui, "PROJ", StudioTheme::BG_SURFACE, StudioTheme::ACCENT_CYAN);
                ui.vertical(|ui| {
                    ui.strong(RichText::new(&self.project.manifest.name).color(StudioTheme::TEXT_PRIMARY));
                    ui.monospace(
                        RichText::new(format!(
                            "{}ms cycle | {} POUs",
                            self.project.manifest.scan_cycle_ms,
                            self.project.manifest.pous.len()
                        ))
                        .color(StudioTheme::TEXT_MUTED)
                        .size(10.5),
                    );
                });
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .button(lang.tr("Open...", "Ouvrir..."))
                    .on_hover_text(lang.tr(
                        "Open project folder via OS file explorer",
                        "Ouvrir un dossier projet via l'explorateur de fichiers OS",
                    ))
                    .clicked()
                {
                    self.open_project_folder_dialog();
                }
                if ui
                    .button(lang.tr("Save", "Enregistrer"))
                    .on_hover_text(lang.tr(
                        "Save project to current folder",
                        "Enregistrer le projet dans le dossier actuel",
                    ))
                    .clicked()
                {
                    self.save_project();
                }
                if ui
                    .button(lang.tr("Save As...", "Enregistrer sous..."))
                    .on_hover_text(lang.tr(
                        "Save project to another folder via OS file explorer",
                        "Enregistrer le projet dans un autre dossier via l'explorateur OS",
                    ))
                    .clicked()
                {
                    self.save_project_as_dialog();
                }
            });
        });

        ui.add_space(6.0);

        // Core Workspace Resources
        let is_sym_tab = self.active_tab == WorkspaceTab::SymbolsAndConfig;
        if StudioTheme::document_tab(
            ui,
            lang.tr("Global Tags & FB Registry", "Variables Globales & FB"),
            "SYM",
            StudioTheme::ACCENT_CYAN,
            is_sym_tab,
            false,
        )
        .clicked()
        {
            self.active_tab = WorkspaceTab::SymbolsAndConfig;
        }

        ui.add_space(2.0);
        if ui
            .button(
                RichText::new(lang.tr(
                    "Digital Twin Window",
                    "Jumeau Numérique (Fenêtre OS)",
                ))
                .color(StudioTheme::ACCENT_CYAN),
            )
            .clicked()
        {
            self.show_digital_twin_window = true;
            self.sync_shared_io();
        }

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Filter / Search POUs
        ui.horizontal(|ui| {
            ui.strong(RichText::new(lang.tr("POUs", "POU")).color(StudioTheme::TEXT_PRIMARY));
            ui.colored_label(
                StudioTheme::TEXT_MUTED,
                format!("({})", self.project.manifest.pous.len()),
            );
        });

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.pou_filter_text)
                    .hint_text(lang.tr("Filter POUs...", "Filtrer les POU..."))
                    .desired_width(ui.available_width() - 24.0),
            );
            if !self.pou_filter_text.is_empty() && ui.small_button("✖").clicked() {
                self.pou_filter_text.clear();
            }
        });

        ui.add_space(4.0);

        let query = self.pou_filter_text.trim().to_lowercase();
        let entries = self.project.manifest.pous.clone();
        for entry in entries {
            if !query.is_empty()
                && !entry.name.to_lowercase().contains(&query)
                && !entry.language.to_string().to_lowercase().contains(&query)
            {
                continue;
            }

            let selected = self.active_tab == WorkspaceTab::Pou(entry.name.clone());
            let badge_col = match entry.language {
                LanguageKind::Ld => StudioTheme::ACCENT_CYAN,
                LanguageKind::Sfc => StudioTheme::ACCENT_AMBER,
                LanguageKind::St => StudioTheme::ACCENT_PURPLE,
                LanguageKind::Fbd => Color32::from_rgb(97, 175, 239),
            };
            let badge_txt = match entry.language {
                LanguageKind::Ld => "LD",
                LanguageKind::Sfc => "SFC",
                LanguageKind::St => "ST",
                LanguageKind::Fbd => "FBD",
            };

            if StudioTheme::document_tab(
                ui,
                &entry.name,
                badge_txt,
                badge_col,
                selected,
                self.modified && selected,
            )
            .clicked()
            {
                self.active_tab = WorkspaceTab::Pou(entry.name.clone());
            }
            ui.add_space(1.0);
        }

        ui.add_space(10.0);

        // New POU Creation Card
        StudioTheme::card_frame().show(ui, |ui| {
            ui.strong(RichText::new(lang.tr("+ New POU", "+ Nouveau POU")).color(StudioTheme::TEXT_PRIMARY));
            ui.add_space(2.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.new_pou_name)
                    .hint_text(lang.tr("POU name...", "Nom du POU..."))
                    .desired_width(ui.available_width()),
            );
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                for l in [
                    LanguageKind::Ld,
                    LanguageKind::Sfc,
                    LanguageKind::St,
                    LanguageKind::Fbd,
                ] {
                    ui.selectable_value(&mut self.new_pou_lang, l, l.to_string());
                }
            });
            ui.add_space(4.0);
            if ui
                .button(RichText::new(lang.tr("Create POU", "Créer POU")).strong())
                .clicked()
                && !self.new_pou_name.trim().is_empty()
            {
                let name = self.new_pou_name.trim().to_string();
                match self.project.add_pou(&name, self.new_pou_lang) {
                    Ok(_) => {
                        self.new_pou_name.clear();
                        self.active_tab = WorkspaceTab::Pou(name);
                        self.recompile_and_reset();
                    }
                    Err(e) => {
                        self.status_banner = format!(
                            "{}: {e}",
                            lang.tr("Could not add POU", "Impossible d'ajouter le POU")
                        );
                    }
                }
            }
        });
    }

    pub fn render_right_io_body(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        if let Some(ref mut rt) = self.runtime {
            egui::ScrollArea::vertical()
                .id_salt("right_watch_scroll")
                .show(ui, |ui| {
                    ui.strong(RichText::new(lang.tr(
                        "Live Process Image & Symbolic Tags:",
                        "Mémoire Image Temps Réel & Variables :",
                    )).color(StudioTheme::TEXT_PRIMARY));
                    ui.add_space(4.0);

                    // Filter search bar
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.io_filter_text)
                                .hint_text(lang.tr("Filter symbols...", "Filtrer les symboles..."))
                                .desired_width(ui.available_width() - 24.0),
                        );
                        if !self.io_filter_text.is_empty() && ui.small_button("✖").clicked() {
                            self.io_filter_text.clear();
                        }
                    });

                    ui.add_space(2.0);

                    // Memory Area Filter Pills
                    ui.horizontal_wrapped(|ui| {
                        let is_all = self.io_filter_area.is_none();
                        if ui.selectable_label(is_all, lang.tr("All", "Tous")).clicked() {
                            self.io_filter_area = None;
                        }
                        let is_in = self.io_filter_area == Some(MemoryArea::Input);
                        if ui.selectable_label(is_in, "%I (In)").clicked() {
                            self.io_filter_area = if is_in { None } else { Some(MemoryArea::Input) };
                        }
                        let is_out = self.io_filter_area == Some(MemoryArea::Output);
                        if ui.selectable_label(is_out, "%Q (Out)").clicked() {
                            self.io_filter_area = if is_out { None } else { Some(MemoryArea::Output) };
                        }
                        let is_mem = self.io_filter_area == Some(MemoryArea::Marker);
                        if ui.selectable_label(is_mem, "%M (Mem)").clicked() {
                            self.io_filter_area = if is_mem { None } else { Some(MemoryArea::Marker) };
                        }
                    });

                    ui.add_space(4.0);

                    let search_lower = self.io_filter_text.trim().to_lowercase();
                    let tag_snapshots: Vec<_> = rt.tags.tags().to_vec();
                    let filtered_tags: Vec<_> = tag_snapshots
                        .into_iter()
                        .filter(|tag| {
                            if let Some(target_area) = self.io_filter_area
                                && tag.address.map(|a| a.area) != Some(target_area)
                            {
                                return false;
                            }
                            if !search_lower.is_empty() {
                                let name_matches = tag.name.to_lowercase().contains(&search_lower);
                                let addr_matches = tag
                                    .address
                                    .map(|a| a.to_string().to_lowercase().contains(&search_lower))
                                    .unwrap_or(false);
                                if !name_matches && !addr_matches {
                                    return false;
                                }
                            }
                            true
                        })
                        .collect();

                    let mut pending_stimulus: Option<(String, IecValue)> = None;
                    let mut pending_force: Option<(String, IecValue)> = None;
                    let mut pending_unforce: Option<String> = None;

                    egui::Grid::new("live_io_watch_grid")
                        .striped(true)
                        .min_col_width(55.0)
                        .show(ui, |ui| {
                            ui.strong(RichText::new(lang.tr("Symbol", "Symbole")).color(StudioTheme::TEXT_SECONDARY));
                            ui.strong(RichText::new(lang.tr("Addr", "Adr")).color(StudioTheme::TEXT_SECONDARY));
                            ui.strong(RichText::new(
                                lang.tr("Live Value / Control", "Valeur / Commande"),
                            ).color(StudioTheme::TEXT_SECONDARY));
                            ui.strong(RichText::new(lang.tr("Force", "Forçage")).color(StudioTheme::TEXT_SECONDARY));
                            ui.end_row();

                            for tag in &filtered_tags {
                                ui.monospace(RichText::new(&tag.name).color(StudioTheme::TEXT_PRIMARY));
                                let addr_str = tag
                                    .address
                                    .map(|a| a.to_string())
                                    .unwrap_or_else(|| "-".to_string());
                                ui.monospace(RichText::new(addr_str).color(StudioTheme::TEXT_MUTED));

                                let current_val = rt
                                    .read_value(&tag.name)
                                    .unwrap_or_else(|_| tag.initial_value.clone());

                                let is_sfc_state = tag.name.ends_with(".state");
                                if tag.read_only_outside_sfc && !is_sfc_state {
                                    let col = match current_val {
                                        IecValue::Bool(true) => StudioTheme::ACCENT_EMERALD,
                                        _ => StudioTheme::TEXT_MUTED,
                                    };
                                    ui.colored_label(col, current_val.to_string());
                                } else {
                                    match tag.data_type {
                                        IecType::Bool => {
                                            let b = current_val.as_bool().unwrap_or(false);
                                            let (btn_fill, btn_stroke, text_color, btn_txt) = if b {
                                                (
                                                    StudioTheme::ACCENT_EMERALD,
                                                    Stroke::new(1.0_f32, Color32::from_rgb(52, 211, 153)),
                                                    Color32::WHITE,
                                                    "● TRUE (ON)",
                                                )
                                            } else {
                                                (
                                                    StudioTheme::BG_SURFACE,
                                                    Stroke::new(1.0_f32, StudioTheme::BORDER_SUBTLE),
                                                    StudioTheme::TEXT_MUTED,
                                                    "○ FALSE",
                                                )
                                            };
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(btn_txt)
                                                            .monospace()
                                                            .size(11.0)
                                                            .strong()
                                                            .color(text_color),
                                                    )
                                                    .fill(btn_fill)
                                                    .stroke(btn_stroke)
                                                    .corner_radius(egui::CornerRadius::same(4)),
                                                )
                                                .clicked()
                                            {
                                                pending_stimulus = Some((
                                                    tag.name.clone(),
                                                    IecValue::Bool(!b),
                                                ));
                                            }
                                        }
                                        IecType::Int => {
                                            let mut v = current_val.as_int().unwrap_or(0);
                                            if ui
                                                .add(egui::DragValue::new(&mut v))
                                                .changed()
                                            {
                                                pending_stimulus = Some((
                                                    tag.name.clone(),
                                                    IecValue::Int(v),
                                                ));
                                            }
                                        }
                                        IecType::Dint => {
                                            let mut v = current_val.as_dint().unwrap_or(0);
                                            if ui
                                                .add(egui::DragValue::new(&mut v))
                                                .changed()
                                            {
                                                pending_stimulus = Some((
                                                    tag.name.clone(),
                                                    IecValue::Dint(v),
                                                ));
                                            }
                                        }
                                        IecType::Real => {
                                            let mut v = current_val.as_real().unwrap_or(0.0);
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut v)
                                                        .speed(0.5),
                                                )
                                                .changed()
                                            {
                                                pending_stimulus = Some((
                                                    tag.name.clone(),
                                                    IecValue::Real(v),
                                                ));
                                            }
                                        }
                                        _ => {
                                            ui.monospace(current_val.to_string());
                                        }
                                    }
                                }

                                if tag.forced.is_some() {
                                    if ui
                                        .small_button(
                                            RichText::new(
                                                lang.tr("Unforce", "Libérer"),
                                            )
                                            .color(StudioTheme::ACCENT_AMBER)
                                            .strong(),
                                        )
                                        .clicked()
                                    {
                                        pending_unforce = Some(tag.name.clone());
                                    }
                                } else if (!tag.read_only_outside_sfc || is_sfc_state)
                                    && ui
                                        .small_button(lang.tr("Force", "Forcer"))
                                        .clicked()
                                {
                                    pending_force =
                                        Some((tag.name.clone(), current_val.clone()));
                                } else {
                                    ui.label("-");
                                }

                                ui.end_row();
                            }
                        });

                    if let Some((sym, val)) = pending_stimulus {
                        let _ = rt.set_stimulus(&sym, val);
                    }
                    if let Some((sym, val)) = pending_force {
                        let _ = rt.force(&sym, val);
                    }
                    if let Some(sym) = pending_unforce {
                        let _ = rt.unforce(&sym);
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.strong(RichText::new(lang.tr(
                        "Shared Function Block Instances:",
                        "Instances de Blocs Fonctionnels Partagés :",
                    )).color(StudioTheme::TEXT_PRIMARY));
                    ui.add_space(4.0);

                    for fb in rt.tags.fbs_mut() {
                        StudioTheme::card_frame().show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.strong(RichText::new(&fb.name).color(StudioTheme::TEXT_PRIMARY));
                                ui.monospace(RichText::new(format!("({})", fb.kind.label())).color(StudioTheme::ACCENT_CYAN));
                                let q_col = if fb.q {
                                    StudioTheme::ACCENT_EMERALD
                                } else {
                                    StudioTheme::TEXT_MUTED
                                };
                                ui.colored_label(
                                    q_col,
                                    format!("IN={} | Q={}", fb.in_bool, fb.q),
                                );
                            });
                            if fb.kind.is_corrector() {
                                ui.monospace(format!(
                                    "SP={:.2} | PV={:.2} | ERR={:.2} => OUT={:.2}",
                                    fb.sp_real, fb.pv_real, fb.err_real, fb.out_real
                                ));
                                let mut param_changed = false;
                                ui.horizontal_wrapped(|ui| {
                                    ui.label("Kp:");
                                    param_changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut fb.kp)
                                                .speed(0.1)
                                                .range(-10000.0..=10000.0),
                                        )
                                        .changed();
                                    if fb.kind.has_integral() {
                                        ui.label("Ti(s):");
                                        param_changed |= ui
                                            .add(
                                                egui::DragValue::new(&mut fb.ti_s)
                                                    .speed(0.1)
                                                    .range(0.001..=10000.0),
                                            )
                                            .changed();
                                    }
                                    if fb.kind.has_derivative() {
                                        ui.label("Td(s):");
                                        param_changed |= ui
                                            .add(
                                                egui::DragValue::new(&mut fb.td_s)
                                                    .speed(0.05)
                                                    .range(0.0..=10000.0),
                                            )
                                            .changed();
                                    }
                                    if ui
                                        .small_button(lang.tr("Reset ∫", "RAZ ∫"))
                                        .clicked()
                                    {
                                        fb.reset_corrector();
                                    }
                                });
                                if param_changed
                                    && let Some(p_fb) = self
                                        .project
                                        .fb_instances
                                        .instances
                                        .iter_mut()
                                        .find(|f| f.name == fb.name)
                                {
                                    p_fb.kp = fb.kp;
                                    p_fb.ti_s = fb.ti_s;
                                    p_fb.td_s = fb.td_s;
                                }
                            } else {
                                let ratio = if fb.pt_ms > 0 {
                                    (fb.et_ms as f32 / fb.pt_ms as f32).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                ui.horizontal(|ui| {
                                    ui.monospace(format!("{}/{}ms", fb.et_ms, fb.pt_ms));
                                    ui.add(egui::ProgressBar::new(ratio).desired_width(100.0));
                                });
                            }
                        });
                        ui.add_space(2.0);
                    }
                });
        } else {
            ui.colored_label(
                StudioTheme::ACCENT_ROSE,
                lang.tr(
                    "PLC Runtime offline due to validation error. Fix diagnostics and click 'Check & Compile'.",
                    "Exécuteur API hors ligne suite à une erreur de validation. Corrigez les erreurs puis cliquez sur 'Vérifier & Compiler'.",
                ),
            );
        }
    }

    pub fn render_modbus_panel_body(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        ui.horizontal_wrapped(|ui| {
            let is_running = self.modbus_server.is_some();
            let state_badge_col = if is_running {
                Color32::from_rgb(46, 213, 115)
            } else {
                Color32::from_rgb(255, 107, 107)
            };
            let state_txt = if is_running {
                format!("[RUNNING on 127.0.0.1:{}]", self.modbus_port)
            } else {
                "[STOPPED]".to_string()
            };
            ui.colored_label(state_badge_col, RichText::new(state_txt).strong());

            let toggle_txt = if is_running {
                lang.tr("⏹ Stop Modbus Server", "⏹ Arrêter Serveur Modbus")
            } else {
                lang.tr("▶ Start Modbus Server", "▶ Démarrer Serveur Modbus")
            };
            if ui.button(toggle_txt).clicked() {
                self.toggle_modbus_server();
            }

            ui.separator();
            ui.label(lang.tr("Port:", "Port :"));
            if !is_running {
                ui.add(egui::DragValue::new(&mut self.modbus_port).range(1024..=65535));
            } else {
                ui.monospace(self.modbus_port.to_string());
            }

            ui.separator();
            if let Some(ref srv) = self.modbus_server {
                let telem = srv.telemetry();
                ui.label(format!(
                    "{} {} | {} {} | {} {}",
                    lang.tr("Requests:", "Requêtes :"),
                    telem.requests_total,
                    lang.tr("Clients:", "Clients :"),
                    telem.active_clients,
                    lang.tr("Errors:", "Erreurs :"),
                    telem.exceptions_total,
                ));
            }

            ui.separator();
            if ui
                .button(lang.tr("⚡ Ping Test Client", "⚡ Tester Ping Client"))
                .on_hover_text(lang.tr(
                    "Connect as internal Modbus client and read coil #0 (%QX0.0)",
                    "Se connecter en client Modbus interne et lire le bit #0 (%QX0.0)",
                ))
                .clicked()
            {
                self.test_modbus_ping();
            }

            if let Some(ref res) = self.modbus_last_test_result {
                ui.colored_label(Color32::from_rgb(72, 202, 228), res);
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.strong(lang.tr(
            "IEC 61131-3 Standard Modbus Address Mapping:",
            "Mappage des Adresses Modbus Standard IEC 61131-3 :",
        ));

        egui::Grid::new("modbus_mapping_grid")
            .striped(true)
            .min_col_width(120.0)
            .show(ui, |ui| {
                ui.strong(lang.tr("Modbus Register Range", "Plage Registres Modbus"));
                ui.strong(lang.tr("Function Code", "Code Fonction"));
                ui.strong(lang.tr("FerritePLC Address", "Adresse FerritePLC"));
                ui.strong(lang.tr("Access Mode", "Mode d'Accès"));
                ui.end_row();

                ui.monospace("00001 - 01000 (0..999)");
                ui.label("FC01, FC05, FC15 (Coils)");
                ui.colored_label(Color32::from_rgb(86, 182, 194), "%QX0.0 .. %QX124.7");
                ui.label(lang.tr("Read / Write", "Lecture / Écriture"));
                ui.end_row();

                ui.monospace("01001 - 02000 (1000..1999)");
                ui.label("FC01, FC05, FC15 (Coils)");
                ui.colored_label(Color32::from_rgb(229, 192, 123), "%MX0.0 .. %MX124.7");
                ui.label(lang.tr("Read / Write", "Lecture / Écriture"));
                ui.end_row();

                ui.monospace("10001 - 11000 (0..999)");
                ui.label("FC02 (Discrete Inputs)");
                ui.colored_label(Color32::from_rgb(46, 213, 115), "%IX0.0 .. %IX124.7");
                ui.label(lang.tr("Read Only", "Lecture Seule"));
                ui.end_row();

                ui.monospace("30001 - 30512 (0..511)");
                ui.label("FC04 (Input Registers)");
                ui.colored_label(Color32::from_rgb(72, 202, 228), "%IW0 .. %IW511");
                ui.label(lang.tr("Read Only (Words)", "Lecture Seule (Mots)"));
                ui.end_row();

                ui.monospace("40001 - 40512 (0..511)");
                ui.label("FC03, FC06, FC16 (Holding Regs)");
                ui.colored_label(Color32::from_rgb(198, 120, 221), "%MW0 .. %MW511");
                ui.label(lang.tr("Read / Write (Words)", "Lecture / Écriture (Mots)"));
                ui.end_row();

                ui.monospace("41001 - 41512 (1000..1511)");
                ui.label("FC03, FC06, FC16 (Holding Regs)");
                ui.colored_label(Color32::from_rgb(86, 182, 194), "%QW0 .. %QW511");
                ui.label(lang.tr("Read / Write (Words)", "Lecture / Écriture (Mots)"));
                ui.end_row();
            });
    }

    pub fn render_gpio_panel_body(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                Color32::from_rgb(72, 202, 228),
                RichText::new("Raspberry Pi 40-Pin GPIO Hardware Interface").strong(),
            );
            ui.separator();
            ui.checkbox(&mut self.hal_enabled, lang.tr("HAL Enabled", "HAL Activé"));
            ui.separator();
            ui.label(lang.tr("Active Driver:", "Pilote Actif :"));
            ui.monospace(self.hal_driver.name());
        });

        ui.add_space(4.0);
        ui.separator();

        let live_levels = self.hal_driver.pin_levels();
        let mut toggle_pin: Option<(u32, bool)> = None;

        egui::ScrollArea::vertical()
            .id_salt("gpio_panel_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left side: Visual 40-pin header (2 columns of 20 pins)
                    ui.group(|ui| {
                        ui.strong(lang.tr(
                            "Raspberry Pi 40-Pin Header Map",
                            "Connecteur GPIO 40 Broches Raspberry Pi",
                        ));
                        ui.add_space(2.0);

                        let hal_cfg = &self.hal_config;
                        let render_pin_widget =
                            |ui: &mut egui::Ui, pin: &RPiPinInfo| -> Option<(u32, bool)> {
                                let (txt, col, is_gpio) =
                                    Self::rpi_pin_style(pin, hal_cfg, &live_levels);
                                if is_gpio
                                    && ui
                                        .button(RichText::new(&txt).color(col).size(10.5))
                                        .clicked()
                                {
                                    if let Some(bcm) = pin.bcm {
                                        let curr = live_levels.get(&bcm).copied().unwrap_or(false);
                                        return Some((bcm, !curr));
                                    }
                                } else if !is_gpio {
                                    ui.colored_label(col, RichText::new(&txt).size(10.5));
                                }
                                None
                            };

                        egui::Grid::new("rpi_40pin_grid")
                            .striped(true)
                            .spacing([8.0, 3.0])
                            .show(ui, |ui| {
                                for pair_idx in 0..20 {
                                    let odd_pin_num = pair_idx * 2 + 1;
                                    let even_pin_num = pair_idx * 2 + 2;
                                    let odd = &RPI_40PIN_HEADER[(odd_pin_num - 1) as usize];
                                    let even = &RPI_40PIN_HEADER[(even_pin_num - 1) as usize];

                                    // Odd pin (Left)
                                    if let Some(t) = render_pin_widget(ui, odd) {
                                        toggle_pin = Some(t);
                                    }

                                    // Pin numbers in middle
                                    ui.monospace(format!("{:2} | {:2}", odd_pin_num, even_pin_num));

                                    // Even pin (Right)
                                    if let Some(t) = render_pin_widget(ui, even) {
                                        toggle_pin = Some(t);
                                    }

                                    ui.end_row();
                                }
                            });
                    });

                    ui.add_space(8.0);

                    // Right side: Active PLC Mappings Table
                    ui.vertical(|ui| {
                        ui.strong(lang.tr(
                            "Configured PLC I/O Pin Mappings",
                            "Mappages E/S API Configurés",
                        ));
                        ui.add_space(2.0);

                        egui::Grid::new("gpio_mappings_summary_grid")
                            .striped(true)
                            .min_col_width(90.0)
                            .show(ui, |ui| {
                                ui.strong("BCM");
                                ui.strong(lang.tr("Label", "Libellé"));
                                ui.strong(lang.tr("Dir", "Sens"));
                                ui.strong(lang.tr("PLC Addr", "Adr API"));
                                ui.strong(lang.tr("Physical Level", "Niveau Physique"));
                                ui.end_row();

                                for m in &self.hal_config.mappings {
                                    ui.monospace(format!("BCM {}", m.bcm_pin));
                                    ui.label(&m.label);
                                    let (dir_txt, dir_col) = match m.direction {
                                        PinDirection::Input => {
                                            ("IN", Color32::from_rgb(86, 182, 194))
                                        }
                                        PinDirection::Output => {
                                            ("OUT", Color32::from_rgb(46, 213, 115))
                                        }
                                    };
                                    ui.colored_label(dir_col, dir_txt);
                                    ui.monospace(m.address.to_string());

                                    let is_high =
                                        live_levels.get(&m.bcm_pin).copied().unwrap_or(false);
                                    let (lvl_txt, lvl_col) = if is_high {
                                        ("HIGH (3.3V)", Color32::from_rgb(46, 213, 115))
                                    } else {
                                        ("LOW (0.0V)", Color32::from_rgb(140, 150, 170))
                                    };
                                    ui.colored_label(lvl_col, lvl_txt);
                                    ui.end_row();
                                }
                            });

                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(lang.tr(
                            "Tip: Click on any Input GPIO pin in the 40-pin header above to simulate physical button presses / sensor trips in real-time.",
                            "Astuce : Cliquez sur n'importe quelle broche GPIO d'entrée dans le connecteur ci-dessus pour simuler l'appui sur un bouton ou le déclenchement d'un capteur.",
                        ))
                            .italics()
                            .size(11.0),
                        );
                    });
                });
            });

        if let Some((bcm, new_lvl)) = toggle_pin {
            let _ = self.hal_driver.set_mock_input(bcm, new_lvl);
        }
    }

    fn rpi_pin_style(
        info: &RPiPinInfo,
        cfg: &GpioHalConfig,
        levels: &HashMap<u32, bool>,
    ) -> (String, Color32, bool) {
        if info.name.starts_with("3V3") {
            return (
                info.name.to_string(),
                Color32::from_rgb(255, 107, 107),
                false,
            );
        }
        if info.name.starts_with("5V") {
            return (
                info.name.to_string(),
                Color32::from_rgb(255, 184, 77),
                false,
            );
        }
        if info.name.starts_with("Ground") {
            return ("GND".to_string(), Color32::from_rgb(120, 130, 140), false);
        }

        if let Some(bcm) = info.bcm {
            let is_high = levels.get(&bcm).copied().unwrap_or(false);
            let state_sym = if is_high { "●" } else { "○" };
            if let Some(m) = cfg.mappings.iter().find(|m| m.bcm_pin == bcm) {
                let col = match m.direction {
                    PinDirection::Input => Color32::from_rgb(86, 182, 194),
                    PinDirection::Output => Color32::from_rgb(46, 213, 115),
                };
                let txt = format!("{} {} [{}]", state_sym, info.name, m.address);
                (txt, col, m.direction == PinDirection::Input)
            } else {
                let txt = format!("{} {}", state_sym, info.name);
                (txt, Color32::from_rgb(180, 190, 205), false)
            }
        } else {
            (
                info.name.to_string(),
                Color32::from_rgb(150, 160, 175),
                false,
            )
        }
    }

    pub fn render_bottom_dock_tab_selectors(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        let is_diag = self.bottom_dock_tab == BottomDockTab::Diagnostics;
        let is_trend = self.bottom_dock_tab == BottomDockTab::TrendPlotter;
        let is_alarms = self.bottom_dock_tab == BottomDockTab::Alarms;
        let is_mb = self.bottom_dock_tab == BottomDockTab::Modbus;
        let is_gpio = self.bottom_dock_tab == BottomDockTab::Gpio;

        let err_count = self.diagnostics.len();
        let diag_badge_col = if err_count > 0 {
            StudioTheme::ACCENT_ROSE
        } else {
            StudioTheme::ACCENT_EMERALD
        };
        let diag_badge_txt = if err_count > 0 {
            format!("{err_count} ERR")
        } else {
            "0 ERR".to_string()
        };

        if StudioTheme::document_tab(
            ui,
            lang.tr("Diagnostics", "Diagnostics"),
            &diag_badge_txt,
            diag_badge_col,
            is_diag,
            false,
        )
        .clicked()
        {
            self.bottom_dock_tab = BottomDockTab::Diagnostics;
        }

        let active_alarms = self.runtime.as_ref().map(|rt| {
            rt.alarms.iter().filter(|a| a.state.is_active()).count()
        }).unwrap_or(0);
        let alarm_badge_col = if active_alarms > 0 {
            StudioTheme::ACCENT_ROSE
        } else {
            StudioTheme::TEXT_MUTED
        };
        let alarm_badge_txt = if active_alarms > 0 {
            format!("{active_alarms} ACT")
        } else {
            "0".to_string()
        };

        if StudioTheme::document_tab(
            ui,
            lang.tr("Alarms", "Alarmes"),
            &alarm_badge_txt,
            alarm_badge_col,
            is_alarms,
            false,
        )
        .clicked()
        {
            self.bottom_dock_tab = BottomDockTab::Alarms;
        }

        if StudioTheme::document_tab(
            ui,
            lang.tr("Trend Oscilloscope", "Oscilloscope"),
            "TRD",
            StudioTheme::ACCENT_CYAN,
            is_trend,
            false,
        )
        .clicked()
        {
            self.bottom_dock_tab = BottomDockTab::TrendPlotter;
        }

        let mb_online = self.modbus_server.is_some();
        let mb_badge_col = if mb_online {
            StudioTheme::ACCENT_EMERALD
        } else {
            StudioTheme::TEXT_MUTED
        };
        let mb_badge_txt = if mb_online {
            format!(":{}", self.modbus_port)
        } else {
            "OFF".to_string()
        };

        if StudioTheme::document_tab(
            ui,
            "Modbus TCP",
            &mb_badge_txt,
            mb_badge_col,
            is_mb,
            false,
        )
        .clicked()
        {
            self.bottom_dock_tab = BottomDockTab::Modbus;
        }

        let gpio_badge_col = if self.hal_enabled {
            StudioTheme::ACCENT_EMERALD
        } else {
            StudioTheme::TEXT_MUTED
        };
        let gpio_badge_txt = if self.hal_enabled {
            "40-PIN"
        } else {
            "OFF"
        };

        if StudioTheme::document_tab(
            ui,
            lang.tr("RPi GPIO", "GPIO RPi"),
            gpio_badge_txt,
            gpio_badge_col,
            is_gpio,
            false,
        )
        .clicked()
        {
            self.bottom_dock_tab = BottomDockTab::Gpio;
        }

        ui.separator();
        ui.label(
            RichText::new(&self.status_banner)
                .color(StudioTheme::TEXT_SECONDARY)
                .italics()
                .size(11.0),
        );
    }

    pub fn render_bottom_dock_content(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        match self.bottom_dock_tab {
            BottomDockTab::Alarms => {
                crate::alarms_panel::render_alarms_panel(ui, self.runtime.as_mut());
            }
            BottomDockTab::TrendPlotter => {
                self.trend_plotter.render(ui, self.runtime.as_ref(), lang);
            }
            BottomDockTab::Modbus => {
                self.render_modbus_panel_body(ui);
            }
            BottomDockTab::Gpio => {
                self.render_gpio_panel_body(ui);
            }
            BottomDockTab::Diagnostics => {
                egui::ScrollArea::vertical()
                    .id_salt("bottom_diag_scroll")
                    .show(ui, |ui| {
                        if self.diagnostics.is_empty() {
                            ui.colored_label(
                                Color32::from_rgb(46, 213, 115),
                                lang.tr(
                                    "0 compiler/validation errors across all LD, SFC, ST, and FBD POUs.",
                                    "0 erreur de compilation ou de validation sur l'ensemble des POU (LD, SFC, ST, FBD).",
                                ),
                            );
                        } else {
                            for diag in &self.diagnostics {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        Color32::from_rgb(255, 107, 107),
                                        format!(
                                            "[{:?}] {} ({}): {}",
                                            diag.severity,
                                            diag.pou,
                                            diag.language,
                                            diag.message
                                        ),
                                    );
                                    if ui
                                        .small_button(lang.tr("Jump to POU", "Ouvrir POU"))
                                        .clicked()
                                    {
                                        self.active_tab =
                                            WorkspaceTab::Pou(diag.pou.clone());
                                    }
                                });
                            }
                        }

                        if !self.scenario_logs.is_empty() {
                            ui.separator();
                            ui.strong(lang.tr(
                                "Scenario Verification Results (sim/scenarios.toml):",
                                "Résultats de Vérification des Scénarios (sim/scenarios.toml) :",
                            ));
                            for line in &self.scenario_logs {
                                let col = if line.starts_with("[PASS]")
                                    || line.starts_with("[SUCCÈS]")
                                {
                                    Color32::from_rgb(46, 213, 115)
                                } else {
                                    Color32::from_rgb(255, 107, 107)
                                };
                                ui.colored_label(col, line);
                            }
                        }
                    });
            }
        }
    }

    pub fn render_bottom_dock_body(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.render_bottom_dock_tab_selectors(ui);
        });
        ui.separator();
        self.render_bottom_dock_content(ui);
    }
}

impl eframe::App for FerriteStudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(self.ui_theme.visuals());
        let lang = self.ui_lang;

        // Undo / Redo keyboard shortcuts (skipped while a text field owns the keyboard,
        // so TextEdit keeps its own native undo).
        if !ctx.wants_keyboard_input() {
            let undo_sc = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            let redo_sc = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Y);
            let redo_sc2 = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            // Check the more specific Ctrl+Shift+Z first so it isn't consumed as Ctrl+Z.
            if ctx.input_mut(|i| i.consume_shortcut(&redo_sc2) || i.consume_shortcut(&redo_sc)) {
                self.redo();
            } else if ctx.input_mut(|i| i.consume_shortcut(&undo_sc)) {
                self.undo();
            }
        }

        // Intercept OS window close event to show the Save / Don't Save / Cancel confirmation popup
        if ctx.input(|i| i.viewport().close_requested()) && !self.allowed_to_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.show_close_confirm_modal = true;
        }

        // Render Close Confirmation Popup Modal when requested
        if self.show_close_confirm_modal {
            egui::Window::new(lang.tr(
                "Close FerritePLC Studio?",
                "Fermer FerritePLC Studio ?",
            ))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_min_width(360.0);
                ui.add_space(4.0);
                if self.modified {
                    ui.colored_label(
                        Color32::from_rgb(255, 184, 77),
                        lang.tr(
                            "You have unsaved changes in the current project.",
                            "Vous avez des modifications non enregistrées dans le projet actuel.",
                        ),
                    );
                } else {
                    ui.label(lang.tr(
                        "Do you want to save your project before closing?",
                        "Voulez-vous enregistrer votre projet avant de quitter ?",
                    ));
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(lang.tr(
                                    "Save and Close",
                                    "Enregistrer et Quitter",
                                ))
                                .color(Color32::WHITE)
                                .strong(),
                            )
                            .fill(Color32::from_rgb(38, 130, 82)),
                        )
                        .clicked()
                    {
                        self.save_project();
                        self.allowed_to_close = true;
                        self.show_close_confirm_modal = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(lang.tr(
                                    "Close Without Saving",
                                    "Quitter sans Enregistrer",
                                ))
                                .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(165, 55, 55)),
                        )
                        .clicked()
                    {
                        self.allowed_to_close = true;
                        self.show_close_confirm_modal = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    if ui
                        .button(lang.tr("Cancel", "Annuler"))
                        .clicked()
                    {
                        self.show_close_confirm_modal = false;
                    }
                });
            });
        }

        // Real-time deterministic PLC scan loop driver when sim_running == true
        let scan_interval = Duration::from_millis(self.project.manifest.scan_cycle_ms.max(10));
        if self.sim_running {
            if self.last_tick.elapsed() >= scan_interval {
                if self.modified {
                    self.recompile_and_reset();
                    self.modified = false;
                }
                if let Err(err) = self.execute_plc_scan_tick() {
                    self.sim_running = false;
                    self.status_banner = format!(
                        "{}: {err}",
                        lang.tr("PLC Runtime halted", "Exécuteur API arrêté")
                    );
                }
                self.last_tick = Instant::now();
            }
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // 1. Top Collapsible Menu Bar (File/Project, PLC Run/Control, View/Panels, Language)
        egui::TopBottomPanel::top("top_plc_menubar")
            .exact_height(38.0)
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    // Quick toggle button for Left Project Explorer
                    let left_toggle_txt = if self.show_left_explorer {
                        lang.tr("▤ Explorer", "▤ Explorateur")
                    } else {
                        lang.tr("▷ Explorer", "▷ Explorateur")
                    };
                    if ui
                        .selectable_label(self.show_left_explorer, left_toggle_txt)
                        .on_hover_text(lang.tr(
                            "Show/Collapse Project Explorer Side Panel",
                            "Afficher/Réduire le panneau Explorateur de Projet",
                        ))
                        .clicked()
                    {
                        self.show_left_explorer = !self.show_left_explorer;
                    }

                    ui.separator();

                    // Brand Pill
                    ui.horizontal(|ui| {
                        ui.colored_label(
                            StudioTheme::ACCENT_CYAN,
                            RichText::new("FERRITE").monospace().strong().size(13.0),
                        );
                        ui.colored_label(
                            StudioTheme::TEXT_PRIMARY,
                            RichText::new("STUDIO").strong().size(13.0),
                        );
                        StudioTheme::badge(ui, "v0.1.0", StudioTheme::BG_SURFACE, StudioTheme::TEXT_MUTED);
                    });

                    ui.separator();

                    // Collapsible FILE / PROJECT Menu
                    ui.menu_button(lang.tr("File v", "Fichier v"), |ui| {
                        ui.set_min_width(260.0);
                        ui.strong(lang.tr("Project Management", "Gestion du Projet"));
                        if ui
                            .button(lang.tr("Save Project", "Enregistrer le Projet"))
                            .clicked()
                        {
                            self.save_project();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "Save Project As... (OS Explorer)",
                                "Enregistrer sous... (Explorateur OS)",
                            ))
                            .clicked()
                        {
                            self.save_project_as_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "Open Project Folder... (OS Explorer)",
                                "Ouvrir Dossier Projet... (Explorateur OS)",
                            ))
                            .clicked()
                        {
                            self.open_project_folder_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "Open FerriteProject.fplc... (OS Explorer)",
                                "Ouvrir FerriteProject.fplc... (Explorateur OS)",
                            ))
                            .clicked()
                            {
                            self.open_project_manifest_dialog();
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.strong("PLCopen TC6 XML (IEC 61131-3)");
                        if ui
                            .button(lang.tr(
                                "Export PLCopen TC6 XML...",
                                "Exporter PLCopen TC6 XML...",
                            ))
                            .clicked()
                        {
                            self.export_plcopen_xml_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "Import PLCopen TC6 XML...",
                                "Importer PLCopen TC6 XML...",
                            ))
                            .clicked()
                        {
                            self.import_plcopen_xml_dialog();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .button(lang.tr(
                                "Global Tags & FB Registry",
                                "Variables Globales & Blocs FB",
                            ))
                            .clicked()
                        {
                            self.active_tab = WorkspaceTab::SymbolsAndConfig;
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "Open Digital Twin Window...",
                                "Ouvrir Jumeau Numérique (Fenêtre OS)...",
                            ))
                            .clicked()
                        {
                            self.show_digital_twin_window = true;
                            self.sync_shared_io();
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.label(lang.tr("Project Folder Path:", "Chemin du Dossier Projet :"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.open_path_input)
                                    .hint_text(lang.tr("Project folder path", "Chemin du projet"))
                                    .desired_width(185.0),
                            );
                            if ui
                                .small_button("Browse")
                                .on_hover_text(lang.tr(
                                    "Browse folder in OS Explorer",
                                    "Parcourir dans l'Explorateur OS",
                                ))
                                .clicked()
                            {
                                self.open_project_folder_dialog();
                                ui.close_menu();
                            }
                        });
                        if ui
                            .button(lang.tr("Load Path", "Charger ce Chemin"))
                            .clicked()
                        {
                            let path = PathBuf::from(self.open_path_input.trim());
                            self.load_project_from_path(path);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .button(lang.tr("Quit FerritePLC Studio", "Quitter FerritePLC Studio"))
                            .clicked()
                        {
                            self.show_close_confirm_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Collapsible PLC RUN & SIMULATION Menu
                    ui.menu_button(lang.tr("⚡ PLC v", "⚡ API v"), |ui| {
                        ui.set_min_width(220.0);
                        let run_label = if self.sim_running {
                            lang.tr("Pause PLC", "Mettre l'API en Pause")
                        } else {
                            lang.tr("Run PLC", "Démarrer l'API")
                        };
                        if ui.button(run_label).clicked() {
                            if !self.sim_running && self.modified {
                                self.recompile_and_reset();
                                self.modified = false;
                            }
                            self.sim_running = !self.sim_running;
                            self.last_tick = Instant::now();
                            ui.close_menu();
                        }

                        if ui
                            .add_enabled(
                                !self.sim_running,
                                egui::Button::new(
                                    lang.tr("Single Step Scan", "Cycle Pas-à-Pas"),
                                ),
                            )
                            .clicked()
                        {
                            self.trigger_single_step();
                            ui.close_menu();
                        }

                        if ui
                            .button(lang.tr("Reset PLC", "Réinitialiser l'API"))
                            .clicked()
                        {
                            self.sim_running = false;
                            self.recompile_and_reset();
                            ui.close_menu();
                        }

                        ui.separator();
                        if ui
                            .button(lang.tr("Check & Compile", "Vérifier & Compiler"))
                            .clicked()
                        {
                            self.recompile_and_reset();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "Run Automated Scenarios",
                                "Exécuter Scénarios Automatisés",
                            ))
                            .clicked()
                        {
                            self.execute_all_scenarios();
                            self.show_bottom_diagnostics = true;
                            ui.close_menu();
                        }
                    });

                    // Collapsible VIEW / PANELS Menu
                    ui.menu_button(lang.tr("⊞ Panels v", "⊞ Panneaux v"), |ui| {
                        ui.set_min_width(240.0);
                        ui.checkbox(
                            &mut self.show_left_explorer,
                            lang.tr(
                                "Project Explorer (Left)",
                                "Explorateur de Projet (Gauche)",
                            ),
                        );
                        ui.checkbox(
                            &mut self.show_right_io,
                            lang.tr("Virtual I/O Rack (Right)", "Baie E/S Virtuelle (Droite)"),
                        );
                        ui.checkbox(
                            &mut self.show_bottom_diagnostics,
                            lang.tr(
                                "Bottom Dock (Diagnostics & Hardware)",
                                "Panneau Inférieur (Diagnostics & Matériel)",
                            ),
                        );
                        ui.separator();
                        ui.strong(lang.tr("Panel Mode", "Mode des Panneaux"));
                        ui.checkbox(
                            &mut self.detach_left_explorer,
                            lang.tr(
                                "Detach Explorer (Floating Window)",
                                "Détacher l'Explorateur (Fenêtre)",
                            ),
                        );
                        ui.checkbox(
                            &mut self.detach_right_io,
                            lang.tr(
                                "Detach Virtual I/O (Floating Window)",
                                "Détacher Baie E/S (Fenêtre)",
                            ),
                        );
                        ui.checkbox(
                            &mut self.detach_bottom_dock,
                            lang.tr(
                                "Detach Bottom Dock (Floating Window)",
                                "Détacher Panneau Inférieur (Fenêtre)",
                            ),
                        );
                    });

                    // Undo / Redo
                    if ui
                        .add_enabled(self.can_undo(), egui::Button::new(lang.tr("↶ Undo", "↶ Annuler")))
                        .on_hover_text(lang.tr("Undo (Ctrl+Z)", "Annuler (Ctrl+Z)"))
                        .clicked()
                    {
                        self.undo();
                    }
                    if ui
                        .add_enabled(self.can_redo(), egui::Button::new(lang.tr("↷ Redo", "↷ Rétablir")))
                        .on_hover_text(lang.tr("Redo (Ctrl+Y / Ctrl+Shift+Z)", "Rétablir (Ctrl+Y / Ctrl+Maj+Z)"))
                        .clicked()
                    {
                        self.redo();
                    }

                    // Collapsible OPTIONS Menu (UI Language + Theme)
                    ui.menu_button(lang.tr("⚙ Options v", "⚙ Options v"), |ui| {
                        ui.set_min_width(200.0);
                        ui.strong(format!(
                            "{} ({})",
                            lang.tr("UI Language", "Langue de l'interface"),
                            self.ui_lang.code()
                        ));
                        if ui
                            .selectable_value(&mut self.ui_lang, UiLanguage::En, "EN - English")
                            .clicked()
                        {
                            self.status_banner = "Language switched to English.".to_string();
                        }
                        if ui
                            .selectable_value(&mut self.ui_lang, UiLanguage::Fr, "FR - Français")
                            .clicked()
                        {
                            self.status_banner = "Langue basculée en Français.".to_string();
                        }
                        ui.separator();
                        ui.strong(lang.tr("Theme", "Thème"));
                        ui.selectable_value(
                            &mut self.ui_theme,
                            UiTheme::Dark,
                            lang.tr("Dark", "Sombre"),
                        );
                        ui.selectable_value(
                            &mut self.ui_theme,
                            UiTheme::Light,
                            lang.tr("Light", "Clair"),
                        );
                    });

                    ui.separator();

                    // Project Title & Modified Indicator
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&self.project.manifest.name)
                                .color(StudioTheme::ACCENT_CYAN)
                                .strong(),
                        );
                        if self.modified {
                            ui.colored_label(StudioTheme::ACCENT_AMBER, "●")
                                .on_hover_text(lang.tr("Project has unsaved changes", "Le projet a des modifications non enregistrées"));
                        }
                    });

                    ui.separator();

                    // Modern Execution Pod
                    ui.horizontal(|ui| {
                        let (run_txt, run_bg, run_fg) = if self.sim_running {
                            (lang.tr("▶ RUNNING", "▶ EN MARCHE"), StudioTheme::ACCENT_EMERALD, Color32::WHITE)
                        } else {
                            (lang.tr("⏸ PAUSED", "⏸ EN PAUSE"), StudioTheme::ACCENT_AMBER, Color32::from_rgb(20, 20, 20))
                        };

                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(run_txt).color(run_fg).strong().size(11.0),
                                )
                                .fill(run_bg)
                                .corner_radius(egui::CornerRadius::same(4)),
                            )
                            .on_hover_text(lang.tr("Toggle PLC Run / Pause", "Basculer Démarrer / Pause API"))
                            .clicked()
                        {
                            if !self.sim_running && self.modified {
                                self.recompile_and_reset();
                                self.modified = false;
                            }
                            self.sim_running = !self.sim_running;
                            self.last_tick = Instant::now();
                        }

                        if ui
                            .add_enabled(
                                !self.sim_running,
                                egui::Button::new(RichText::new(lang.tr("⚡ Step", "⚡ Pas")).size(11.0))
                                    .corner_radius(egui::CornerRadius::same(4)),
                            )
                            .on_hover_text(lang.tr("Single-step one scan cycle", "Exécuter un cycle pas-à-pas"))
                            .clicked()
                        {
                            self.trigger_single_step();
                        }

                        if ui
                            .button(RichText::new(lang.tr("↺ Reset", "↺ RAZ")).size(11.0))
                            .on_hover_text(lang.tr("Reset PLC memory and timers", "Réinitialiser la mémoire et les temporisateurs"))
                            .clicked()
                        {
                            self.sim_running = false;
                            self.recompile_and_reset();
                        }
                    });

                    // Live Telemetry Capsule
                    if let Some(ref rt) = self.runtime {
                        StudioTheme::telemetry_pill(
                            ui,
                            self.sim_running,
                            rt.scan_count,
                            rt.virtual_time_ms,
                            rt.last_scan_duration_us,
                        );
                    }

                    ui.separator();

                    // Hardware & Feature Launchers
                    let twin_btn_col = if self.show_digital_twin_window {
                        StudioTheme::ACCENT_CYAN
                    } else {
                        StudioTheme::TEXT_MUTED
                    };
                    if ui
                        .small_button(RichText::new("Twin").color(twin_btn_col).strong())
                        .on_hover_text(lang.tr(
                            "Open Digital Twin Window (OS Viewport)",
                            "Ouvrir la Fenêtre du Jumeau Numérique (Viewport OS)",
                        ))
                        .clicked()
                    {
                        self.show_digital_twin_window = !self.show_digital_twin_window;
                        self.sync_shared_io();
                    }

                    let mb_running = self.modbus_server.is_some();
                    let mb_btn_col = if mb_running {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::TEXT_MUTED
                    };
                    let mb_txt = if mb_running {
                        format!("Modbus:{}", self.modbus_port)
                    } else {
                        "Modbus".to_string()
                    };
                    if ui
                        .small_button(RichText::new(mb_txt).color(mb_btn_col))
                        .on_hover_text(lang.tr("Open Modbus TCP Panel", "Ouvrir le Panneau Modbus TCP"))
                        .clicked()
                    {
                        self.show_bottom_diagnostics = true;
                        self.bottom_dock_tab = BottomDockTab::Modbus;
                    }

                    if ui
                        .small_button(RichText::new(lang.tr("Trend", "Oscillo")).color(StudioTheme::ACCENT_CYAN))
                        .on_hover_text(lang.tr("Open Trend Oscilloscope", "Ouvrir l'oscilloscope temps réel"))
                        .clicked()
                    {
                        self.show_bottom_diagnostics = true;
                        self.bottom_dock_tab = BottomDockTab::TrendPlotter;
                    }

                    let gpio_btn_col = if self.hal_enabled {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::TEXT_MUTED
                    };
                    if ui
                        .small_button(RichText::new("GPIO").color(gpio_btn_col))
                        .on_hover_text(lang.tr("Open Raspberry Pi GPIO Panel", "Ouvrir le Panneau GPIO Raspberry Pi"))
                        .clicked()
                    {
                        self.show_bottom_diagnostics = true;
                        self.bottom_dock_tab = BottomDockTab::Gpio;
                    }

                    let active_alarms = self.runtime.as_ref().map(|rt| {
                        rt.alarms.iter().filter(|a| a.state.is_active()).count()
                    }).unwrap_or(0);
                    let alarm_btn_col = if active_alarms > 0 {
                        StudioTheme::ACCENT_ROSE
                    } else {
                        StudioTheme::TEXT_MUTED
                    };
                    let alarm_txt = if active_alarms > 0 {
                        format!("! ({active_alarms})")
                    } else {
                        "Alarms".to_string()
                    };
                    if ui
                        .small_button(RichText::new(alarm_txt).color(alarm_btn_col).strong())
                        .on_hover_text(lang.tr("Open Alarms Panel", "Ouvrir le Panneau des Alarmes"))
                        .clicked()
                    {
                        self.show_bottom_diagnostics = true;
                        self.bottom_dock_tab = BottomDockTab::Alarms;
                    }

                    // Quick toggle button for Bottom Dock
                    let bottom_toggle_txt = if self.show_bottom_diagnostics {
                        lang.tr("▾ Dock", "▾ Panneau")
                    } else {
                        lang.tr("▴ Dock", "▴ Panneau")
                    };
                    if ui
                        .selectable_label(self.show_bottom_diagnostics, bottom_toggle_txt)
                        .on_hover_text(lang.tr(
                            "Show/Collapse Bottom Dock",
                            "Afficher/Réduire le Panneau Inférieur",
                        ))
                        .clicked()
                    {
                        self.show_bottom_diagnostics = !self.show_bottom_diagnostics;
                    }

                    // Right-aligned quick toggle button for Virtual I/O Panel
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let right_toggle_txt = if self.show_right_io {
                            lang.tr("Virtual I/O ◂", "Baie E/S ◂")
                        } else {
                            lang.tr("Virtual I/O ▸", "Baie E/S ▸")
                        };
                        if ui
                            .selectable_label(self.show_right_io, right_toggle_txt)
                            .on_hover_text(lang.tr(
                                "Show/Collapse Virtual I/O & Watch Side Panel",
                                "Afficher/Réduire le panneau Baie E/S Virtuelle",
                            ))
                            .clicked()
                        {
                            self.show_right_io = !self.show_right_io;
                        }
                    });
                });
            });

        // 2. Collapsible / Detachable Bottom Dock (Diagnostics, Real-Time Trend Plotter, Modbus TCP Fieldbus, RPi GPIO)
        if self.show_bottom_diagnostics {
            if self.detach_bottom_dock {
                let title = lang.tr(
                    "FerritePLC — Diagnostics, Oscilloscope & Hardware Dock",
                    "FerritePLC — Panneau Diagnostics, Oscilloscope & Matériel",
                );
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("ferrite_bottom_dock_os_viewport"),
                    egui::ViewportBuilder::default()
                        .with_title(title)
                        .with_inner_size([760.0, 400.0])
                        .with_min_inner_size([380.0, 200.0]),
                    |vp_ctx, class| {
                        vp_ctx.set_visuals(self.ui_theme.visuals());
                        crate::configure_system_fonts(vp_ctx);
                        if class == egui::ViewportClass::Embedded {
                            let mut is_open = true;
                            egui::Window::new(title)
                                .id(egui::Id::new("detached_bottom_dock_window"))
                                .open(&mut is_open)
                                .default_size([740.0, 380.0])
                                .resizable(true)
                                .show(vp_ctx, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .small_button(lang.tr("⬇ Reattach", "⬇ Réattacher"))
                                            .on_hover_text(lang.tr(
                                                "Dock back into main window",
                                                "Réattacher à la fenêtre principale",
                                            ))
                                            .clicked()
                                        {
                                            self.detach_bottom_dock = false;
                                        }
                                    });
                                    ui.separator();
                                    self.render_bottom_dock_body(ui);
                                });
                            if !is_open {
                                self.detach_bottom_dock = false;
                            }
                        } else {
                            egui::CentralPanel::default().show(vp_ctx, |ui| {
                                ui.horizontal(|ui| {
                                    ui.heading(lang.tr(
                                        "Diagnostics, Oscilloscope & Hardware",
                                        "Diagnostics, Oscilloscope & Matériel",
                                    ));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button(lang.tr("⬇ Reattach", "⬇ Réattacher"))
                                                .on_hover_text(lang.tr(
                                                    "Dock back into main window",
                                                    "Réattacher à la fenêtre principale",
                                                ))
                                                .clicked()
                                            {
                                                self.detach_bottom_dock = false;
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                                self.render_bottom_dock_body(ui);
                            });
                            if vp_ctx.input(|i| i.viewport().close_requested()) {
                                self.detach_bottom_dock = false;
                            }
                        }
                    },
                );
            } else {
                let default_h = match self.bottom_dock_tab {
                    BottomDockTab::TrendPlotter => 230.0,
                    BottomDockTab::Alarms => 240.0,
                    BottomDockTab::Modbus => 195.0,
                    BottomDockTab::Gpio => 260.0,
                    BottomDockTab::Diagnostics => 135.0,
                };
                egui::TopBottomPanel::bottom("bottom_diagnostics_dock")
                    .resizable(true)
                    .default_height(default_h)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            self.render_bottom_dock_tab_selectors(ui);

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button("v")
                                        .on_hover_text(lang.tr(
                                            "Collapse Bottom Dock",
                                            "Réduire le Panneau Inférieur",
                                        ))
                                        .clicked()
                                    {
                                        self.show_bottom_diagnostics = false;
                                    }
                                    if ui
                                        .small_button(lang.tr("⧉ Detach", "⧉ Détacher"))
                                        .on_hover_text(lang.tr(
                                            "Detach into native OS window (movable across screens)",
                                            "Détacher dans une fenêtre OS native (déplaçable sur un autre écran)",
                                        ))
                                        .clicked()
                                    {
                                        self.detach_bottom_dock = true;
                                    }
                                },
                            );
                        });
                        ui.separator();

                        self.render_bottom_dock_content(ui);
                    });
            }
        }

        // 3. Collapsible / Detachable Left Project Explorer & POU Navigator (Native OS Window when detached)
        if self.show_left_explorer {
            if self.detach_left_explorer {
                let title = lang.tr(
                    "FerritePLC — Project Explorer",
                    "FerritePLC — Explorateur de Projet",
                );
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("ferrite_left_explorer_os_viewport"),
                    egui::ViewportBuilder::default()
                        .with_title(title)
                        .with_inner_size([300.0, 560.0])
                        .with_min_inner_size([220.0, 260.0]),
                    |vp_ctx, class| {
                        vp_ctx.set_visuals(self.ui_theme.visuals());
                        crate::configure_system_fonts(vp_ctx);
                        if class == egui::ViewportClass::Embedded {
                            let mut is_open = true;
                            egui::Window::new(title)
                                .id(egui::Id::new("detached_left_explorer_window"))
                                .open(&mut is_open)
                                .default_size([280.0, 520.0])
                                .resizable(true)
                                .show(vp_ctx, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .small_button(lang.tr("⬇ Reattach", "⬇ Réattacher"))
                                            .clicked()
                                        {
                                            self.detach_left_explorer = false;
                                        }
                                    });
                                    ui.separator();
                                    self.render_left_explorer_body(ui);
                                });
                            if !is_open {
                                self.detach_left_explorer = false;
                            }
                        } else {
                            egui::CentralPanel::default().show(vp_ctx, |ui| {
                                ui.horizontal(|ui| {
                                    ui.heading(
                                        lang.tr("Project Explorer", "Explorateur de Projet"),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button(lang.tr("⬇ Reattach", "⬇ Réattacher"))
                                                .on_hover_text(lang.tr(
                                                    "Dock back into main window",
                                                    "Réattacher à la fenêtre principale",
                                                ))
                                                .clicked()
                                            {
                                                self.detach_left_explorer = false;
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                                self.render_left_explorer_body(ui);
                            });
                            if vp_ctx.input(|i| i.viewport().close_requested()) {
                                self.detach_left_explorer = false;
                            }
                        }
                    },
                );
            } else {
                egui::SidePanel::left("left_project_explorer")
                    .resizable(true)
                    .default_width(240.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(lang.tr("Project Explorer", "Explorateur de Projet"));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button("<")
                                        .on_hover_text(lang.tr(
                                            "Collapse Project Explorer",
                                            "Réduire l'Explorateur de Projet",
                                        ))
                                        .clicked()
                                    {
                                        self.show_left_explorer = false;
                                    }
                                    if ui
                                        .small_button(lang.tr("⧉ Detach", "⧉ Détacher"))
                                        .on_hover_text(lang.tr(
                                            "Detach into native OS window (movable across screens)",
                                            "Détacher dans une fenêtre OS native (déplaçable sur un autre écran)",
                                        ))
                                        .clicked()
                                    {
                                        self.detach_left_explorer = true;
                                    }
                                },
                            );
                        });
                        ui.separator();
                        self.render_left_explorer_body(ui);
                    });
            }
        }

        // 4. Collapsible / Detachable Right Virtual Hardware I/O Rack, Force/Watch Table & Live FB Monitor (Native OS Window when detached)
        if self.show_right_io {
            if self.detach_right_io {
                let title = lang.tr(
                    "FerritePLC — Virtual I/O Rack & Watch Table",
                    "FerritePLC — Baie E/S Virtuelle & Table de Forçage",
                );
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of("ferrite_right_io_os_viewport"),
                    egui::ViewportBuilder::default()
                        .with_title(title)
                        .with_inner_size([400.0, 640.0])
                        .with_min_inner_size([260.0, 300.0]),
                    |vp_ctx, class| {
                        vp_ctx.set_visuals(self.ui_theme.visuals());
                        crate::configure_system_fonts(vp_ctx);
                        if class == egui::ViewportClass::Embedded {
                            let mut is_open = true;
                            egui::Window::new(title)
                                .id(egui::Id::new("detached_right_io_window"))
                                .open(&mut is_open)
                                .default_size([380.0, 600.0])
                                .resizable(true)
                                .show(vp_ctx, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .small_button(lang.tr("⬇ Reattach", "⬇ Réattacher"))
                                            .clicked()
                                        {
                                            self.detach_right_io = false;
                                        }
                                    });
                                    ui.separator();
                                    self.render_right_io_body(ui);
                                });
                            if !is_open {
                                self.detach_right_io = false;
                            }
                        } else {
                            egui::CentralPanel::default().show(vp_ctx, |ui| {
                                ui.horizontal(|ui| {
                                    ui.heading(lang.tr(
                                        "Virtual I/O Rack & Watch Table",
                                        "Baie E/S Virtuelle & Table de Forçage",
                                    ));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button(lang.tr("⬇ Reattach", "⬇ Réattacher"))
                                                .on_hover_text(lang.tr(
                                                    "Dock back into main window",
                                                    "Réattacher à la fenêtre principale",
                                                ))
                                                .clicked()
                                            {
                                                self.detach_right_io = false;
                                            }
                                        },
                                    );
                                });
                                ui.separator();
                                self.render_right_io_body(ui);
                            });
                            if vp_ctx.input(|i| i.viewport().close_requested()) {
                                self.detach_right_io = false;
                            }
                        }
                    },
                );
            } else {
                egui::SidePanel::right("right_io_and_watch_panel")
                    .resizable(true)
                    .default_width(355.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(lang.tr(
                                "Virtual I/O Rack & Watch Table",
                                "Baie E/S Virtuelle & Table de Forçage",
                            ));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button(">")
                                        .on_hover_text(lang.tr(
                                            "Collapse Virtual I/O Panel",
                                            "Réduire la Baie E/S Virtuelle",
                                        ))
                                        .clicked()
                                    {
                                        self.show_right_io = false;
                                    }
                                    if ui
                                        .small_button(lang.tr("⧉ Detach", "⧉ Détacher"))
                                        .on_hover_text(lang.tr(
                                            "Detach into native OS window (movable across screens)",
                                            "Détacher dans une fenêtre OS native (déplaçable sur un autre écran)",
                                        ))
                                        .clicked()
                                    {
                                        self.detach_right_io = true;
                                    }
                                },
                            );
                        });
                        ui.separator();
                        self.render_right_io_body(ui);
                    });
            }
        }

        // Independent OS-Level Viewport for the 2D Visual Digital Twin Plant Mimic & HMI
        if self.show_digital_twin_window {
            self.sync_shared_io();
            let twin_title = lang.tr(
                "FerritePLC — 2D Visual Digital Twin Plant Mimic & HMI",
                "FerritePLC — Jumeau Numérique 2D & IHM",
            );
            let shared_io_handle = Arc::clone(&self.shared_io);

            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("ferrite_digital_twin_os_viewport"),
                egui::ViewportBuilder::default()
                    .with_title(twin_title)
                    .with_inner_size([1180.0, 740.0])
                    .with_min_inner_size([760.0, 500.0]),
                |vp_ctx, class| {
                    vp_ctx.set_visuals(self.ui_theme.visuals());
                    crate::configure_system_fonts(vp_ctx);

                    if class == egui::ViewportClass::Embedded {
                        let mut is_open = true;
                        egui::Window::new(twin_title)
                            .id(egui::Id::new("digital_twin_window_embedded"))
                            .open(&mut is_open)
                            .default_size([1100.0, 700.0])
                            .resizable(true)
                            .show(vp_ctx, |ui| {
                                self.digital_twin_workspace.render(ui, &shared_io_handle);
                            });
                        if !is_open {
                            self.show_digital_twin_window = false;
                        }
                    } else {
                        egui::CentralPanel::default().show(vp_ctx, |ui| {
                            self.digital_twin_workspace.render(ui, &shared_io_handle);
                        });
                        if vp_ctx.input(|i| i.viewport().close_requested()) {
                            self.show_digital_twin_window = false;
                        }
                    }
                },
            );
        }

        // 5. Center Main Editor / Visual Canvas Area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Document Multi-Tab Bar
            egui::ScrollArea::horizontal()
                .id_salt("document_tab_bar_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let is_sym_active = self.active_tab == WorkspaceTab::SymbolsAndConfig;
                        if StudioTheme::document_tab(
                            ui,
                            lang.tr("Global Tags & FBs", "Variables Globales & FB"),
                            "SYM",
                            StudioTheme::ACCENT_CYAN,
                            is_sym_active,
                            false,
                        )
                        .clicked()
                        {
                            self.active_tab = WorkspaceTab::SymbolsAndConfig;
                        }

                        for entry in &self.project.manifest.pous {
                            let is_pou_active = self.active_tab == WorkspaceTab::Pou(entry.name.clone());
                            let (badge_txt, badge_col) = match entry.language {
                                LanguageKind::Ld => ("LD", StudioTheme::ACCENT_CYAN),
                                LanguageKind::Sfc => ("SFC", StudioTheme::ACCENT_AMBER),
                                LanguageKind::St => ("ST", StudioTheme::ACCENT_PURPLE),
                                LanguageKind::Fbd => ("FBD", Color32::from_rgb(97, 175, 239)),
                            };
                            if StudioTheme::document_tab(
                                ui,
                                &entry.name,
                                badge_txt,
                                badge_col,
                                is_pou_active,
                                self.modified && is_pou_active,
                            )
                            .clicked()
                            {
                                self.active_tab = WorkspaceTab::Pou(entry.name.clone());
                            }
                        }
                    });
                });

            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);

            match &self.active_tab.clone() {
                WorkspaceTab::SymbolsAndConfig => {
                    render_symbols_editor(ui, &mut self.project, &mut self.modified, lang);
                }
                WorkspaceTab::Pou(pou_name) => {
                    // Collect all SFC steps across all SFC POUs in the project for cross-SFC transitions & actions
                    let mut all_sfc_steps: Vec<SfcStepRef> = Vec::new();
                    for entry in &self.project.manifest.pous {
                        if let Some(PouSource::Sfc(sfc)) = self.project.pous.get(&entry.name) {
                            for step in &sfc.steps {
                                let prefix = if step.id.starts_with('X') || step.id.starts_with('x') {
                                    step.id.clone()
                                } else {
                                    format!("X{}", step.id)
                                };
                                all_sfc_steps.push(SfcStepRef {
                                    pou_name: sfc.name.clone(),
                                    step_id: step.id.clone(),
                                    step_name: step.name.clone(),
                                    state_sym: format!("{}.{prefix}.state", sfc.name),
                                });
                            }
                        }
                    }

                    if let Some(pou_src) = self.project.pous.get_mut(pou_name) {
                        match pou_src {
                            PouSource::Ld(ld_prog) => {
                                render_ld_editor(
                                    ui,
                                    ld_prog,
                                    self.runtime.as_mut(),
                                    &mut self.modified,
                                    lang,
                                );
                            }
                            PouSource::Sfc(sfc_prog) => {
                                render_sfc_editor(
                                    ui,
                                    sfc_prog,
                                    self.runtime.as_mut(),
                                    &all_sfc_steps,
                                    &mut self.modified,
                                    lang,
                                );
                            }
                            PouSource::St(st_prog) => {
                                render_st_editor(
                                    ui,
                                    st_prog,
                                    self.runtime.as_ref(),
                                    &mut self.modified,
                                    lang,
                                );
                            }
                            PouSource::Fbd(fbd_prog) => {
                                render_fbd_editor(
                                    ui,
                                    fbd_prog,
                                    &mut self.project.fb_instances.instances,
                                    self.runtime.as_mut(),
                                    &mut self.modified,
                                    lang,
                                );
                            }
                        }
                    } else {
                        ui.label(format!(
                            "POU '{pou_name}' {}",
                            lang.tr("not found in project.", "introuvable dans le projet.")
                        ));
                    }
                }
            }
        });

        // Snapshot any project edit made this frame into the undo history
        self.track_history();
    }
}
