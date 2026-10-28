use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use iced::widget::{
    button, column, container, pick_list, row, scrollable, text, text_editor, text_input,
};
use iced::border::Radius;
use iced::{Alignment, Background, Border, Color, Element, Length, Task, Theme};

use ferrite_core::{DirectAddress, FbInstance, IecType, IecValue, LanguageKind, MemoryArea, ProcessImage, RuntimeError, StandardFbKind, TagEntry};
use ferrite_fieldbus_modbus::{ModbusRequest, ModbusResponse, ModbusTcpClient, ModbusTcpServer};
use ferrite_hal::{create_hardware_driver, GpioHalConfig, HardwareDriver};
use ferrite_ir::ValidationDiagnostic;
use ferrite_lang_fbd::{FbdBlock, FbdBlockKind, FbdInputSource};
use ferrite_lang_ld::{CoilKind, ContactKind, LdCoil, LdElement, LdRung};
use ferrite_lang_sfc::{ActionQualifier, ConvergenceKind, DivergenceKind, SfcAction, SfcStep, SfcTransition, StepKind};
use ferrite_lang_st::parse_st_statements;
use ferrite_project::{PouSource, Project};
use ferrite_runtime::{PlcRuntime, ScanTelemetry};

use crate::alarms_panel::render_alarms_panel;
use crate::canvas_fbd::{delete_fbd_block, render_fbd_editor};
use crate::canvas_ld::render_ld_editor;
use crate::canvas_sfc::render_sfc_editor;
use crate::digital_twin::{render_digital_twin, DigitalTwinWorkspace, SharedIoHandle, SharedIoState};
use crate::editor_st::render_st_editor;
use crate::editor_symbols::render_symbols_editor;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;
use crate::trend_plotter::{TrendCanvas, TrendPlotter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceTab {
    Pou(String),
    SymbolsAndConfig,
    DigitalTwin,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiTheme {
    #[default]
    Dark,
    Light,
}

impl UiTheme {
    pub fn is_dark(self) -> bool {
        matches!(self, UiTheme::Dark)
    }
}

/// Strongly typed Elm Architecture interaction messages.
#[derive(Debug, Clone)]
pub enum Message {
    None,
    // Navigation & Layout
    SelectTab(WorkspaceTab),
    SelectBottomTab(BottomDockTab),
    ToggleLeftExplorer,
    ToggleRightIo,
    ToggleBottomDock,
    DetachLeftExplorer,
    DetachRightIo,
    DetachBottomDock,
    DockLeftExplorer,
    DockRightIo,
    DockBottomDock,
    WindowCloseRequested(iced::window::Id),
    ConfirmCloseSaveAndExit,
    ConfirmCloseDiscardAndExit,
    ConfirmCloseCancel,
    WindowCreated(iced::window::Id),
    SetLanguage(UiLanguage),
    SetTheme(UiTheme),

    // Simulation controls
    ToggleRunSim,
    StepSimTick,
    ResetSim,
    PlcTickTimer,
    RunScenarios,

    // Project & Files
    SaveProject,
    CheckProject,
    Undo,
    Redo,
    AddPouClicked,
    NewPouNameChanged(String),
    NewPouLangSelected(LanguageKind),
    ConfirmClearPou(String),
    CancelClearPou,
    ExecuteClearPou(String),

    // Filtering & Tag Force
    PouFilterTextChanged(String),
    IoFilterTextChanged(String),
    IoFilterAreaSelected(Option<MemoryArea>),
    IoFilterSfcSelected,
    ToggleForceBool(String, DirectAddress),
    ForceValInputChanged(String, String),

    // Hardware GPIO & Modbus
    ToggleHalEnabled,
    MockGpioPinToggled(u32),
    ToggleModbusServer,
    ModbusPortChanged(String),
    TestModbusPing,

    // Alarms
    AcknowledgeAlarm(String),
    AcknowledgeAllAlarms,

    // Trend Plotter
    TrendTogglePause,
    TrendClear,
    TrendToggleChannel(String),

    // Symbols & Config
    ScanCycleMsChanged(u64),
    WatchdogUsChanged(u64),
    AddGlobalTag,
    RemoveGlobalTag(usize),
    TagNameChanged(usize, String),
    TagTypeChanged(usize, IecType),
    TagAddressChanged(usize, String),
    TagInitialValueChanged(usize, String),
    TagCommentChanged(usize, String),
    AddFbInstance(StandardFbKind),
    RemoveFbInstance(usize),
    FbNameChanged(usize, String),
    FbKindChanged(usize, StandardFbKind),
    FbGainChanged(usize, f32),
    FbTiChanged(usize, f32),
    FbTdChanged(usize, f32),
    FbPtChanged(usize, u64),
    FbPvChanged(usize, i16),

    // Structured Text Editor
    StEditorAction(String, text_editor::Action),

    // Ladder Diagram
    LdAddRung(String),
    LdRemoveRung(String, usize),
    LdRungCommentChanged(String, usize, String),
    LdAddContact(String, usize),
    LdAddContactWithKind(String, usize, ContactKind),
    LdRemoveContact(String, usize, usize),
    LdContactSymbolChanged(String, usize, usize, String),
    LdContactKindChanged(String, usize, usize, ContactKind),
    LdAddCoil(String, usize),
    LdAddCoilWithKind(String, usize, CoilKind),
    LdRemoveCoil(String, usize, usize),
    LdCoilSymbolChanged(String, usize, usize, String),
    LdCoilKindChanged(String, usize, usize, CoilKind),

    // Sequential Function Chart (SFC / Grafcet)
    SfcAddStep(String),
    SfcRemoveStep(String, usize),
    SfcStepNameChanged(String, usize, String),
    SfcStepKindChanged(String, usize, StepKind),
    SfcAddAction(String, usize),
    SfcRemoveAction(String, usize, usize),
    SfcActionQualifierChanged(String, usize, usize, ActionQualifier),
    SfcActionDurationChanged(String, usize, usize, String),
    SfcActionTargetChanged(String, usize, usize, String),
    SfcAddTransition(String),
    SfcRemoveTransition(String, usize),
    SfcTransitionReceptivityChanged(String, usize, String),
    SfcTransitionFromChanged(String, usize, String),
    SfcTransitionToChanged(String, usize, String),
    SfcTransitionConvergenceChanged(String, usize, ConvergenceKind),
    SfcTransitionDivergenceChanged(String, usize, DivergenceKind),
    SfcAddAndDivergence(String),
    SfcAddAndConvergence(String),
    SfcAddOrDivergence(String),
    SfcAddOrConvergence(String),
    SfcSelectElement(String, Option<crate::canvas_sfc::SfcElementKey>),
    SfcSelectElements(String, Vec<crate::canvas_sfc::SfcElementKey>),
    SfcRequestDeleteSelected(String),
    SfcConfirmDeleteSelected,
    SfcCancelDeleteSelected,
    SfcDisconnectStepInput(String, String),
    SfcDisconnectStepOutput(String, String),
    SfcDisconnectTransitionInput(String, String),
    SfcDisconnectTransitionOutput(String, String),
    SfcDisconnectAll(String, crate::canvas_sfc::SfcElementKey),
    SfcMoveElement(String, crate::canvas_sfc::SfcElementKey, (i32, i32)),
    SfcConnectPorts {
        pou_name: String,
        source: crate::canvas_sfc::SfcPortSource,
        target: crate::canvas_sfc::SfcPortTarget,
    },
    SfcConnectSteps {
        pou_name: String,
        from_step: String,
        to_step: String,
    },
    SfcConnectTransitions {
        pou_name: String,
        from_trans: String,
        to_trans: String,
    },
    SfcAutoLayout(String),
    SfcValidate(String),
    ToggleForceTag(String),

    // Function Block Diagram (FBD)
    FbdAddBlock(String, FbdBlockKind),
    FbdRemoveBlock(String, usize),
    FbdInputSourceChanged(String, usize, String, String),
    FbdOutputTargetChanged(String, usize, String, String),

    // Digital Twin
    DigitalTwinToggleElement(u64),
}

const MAX_HISTORY: usize = 100;
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
    pub show_bottom_dock: bool,
    pub detach_left_explorer: bool,
    pub detach_right_io: bool,
    pub detach_bottom_dock: bool,
    pub main_window_id: Option<iced::window::Id>,
    pub detached_left_id: Option<iced::window::Id>,
    pub detached_right_id: Option<iced::window::Id>,
    pub detached_bottom_id: Option<iced::window::Id>,
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
    pub io_filter_sfc: bool,
    pub pou_filter_text: String,
    pub st_contents: HashMap<String, text_editor::Content>,
    pub confirm_clear_modal: Option<String>,
    pub sfc_selected: Option<(String, crate::canvas_sfc::SfcElementKey)>,
    pub sfc_selected_elements: Vec<crate::canvas_sfc::SfcElementKey>,
    pub sfc_delete_modal: Option<(String, Vec<crate::canvas_sfc::SfcElementKey>)>,
}

impl FerriteStudioApp {
    pub fn new(initial_project: Project) -> Self {
        Self::with_language(initial_project, UiLanguage::En)
    }

    pub fn with_language(mut initial_project: Project, ui_lang: UiLanguage) -> Self {
        for pou_src in initial_project.pous.values_mut() {
            if let PouSource::Sfc(sfc) = pou_src
                && (sfc.steps.iter().any(|s| s.grid_pos.is_none())
                    || sfc.transitions.iter().any(|t| t.grid_pos.is_none()))
            {
                sfc.assign_default_grid_positions();
            }
        }

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

        // Initialize ST editor contents from project
        let mut st_contents = HashMap::new();
        for pou_meta in &initial_project.manifest.pous {
            if pou_meta.language == LanguageKind::St
                && let Some(PouSource::St(st_prog)) = initial_project.pous.get(&pou_meta.name) {
                    st_contents.insert(pou_meta.name.clone(), text_editor::Content::with_text(&st_prog.source));
                }
        }

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
            show_bottom_dock: true,
            detach_left_explorer: false,
            detach_right_io: false,
            detach_bottom_dock: false,
            main_window_id: None,
            detached_left_id: None,
            detached_right_id: None,
            detached_bottom_id: None,
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
            io_filter_sfc: false,
            pou_filter_text: String::new(),
            st_contents,
            confirm_clear_modal: None,
            sfc_selected: None,
            sfc_selected_elements: Vec::new(),
            sfc_delete_modal: None,
        }
    }

    pub fn title(&self, window_id: iced::window::Id) -> String {
        if Some(window_id) == self.detached_left_id {
            return self.ui_lang.tr("FerritePLC — Project Explorer", "FerritePLC — Explorateur de Projet").to_string();
        }
        if Some(window_id) == self.detached_right_id {
            return self.ui_lang.tr("FerritePLC — Virtual I/O Rack & Watch Table", "FerritePLC — Baie E/S Virtuelle & Forçage").to_string();
        }
        if Some(window_id) == self.detached_bottom_id {
            return self.ui_lang.tr("FerritePLC — Diagnostics & Telemetry Dock", "FerritePLC — Panneau Diagnostics & Télémétrie").to_string();
        }
        format!("FerritePLC Studio — {}", self.project.manifest.name)
    }

    pub fn theme(&self, _window_id: iced::window::Id) -> Theme {
        match self.ui_theme {
            UiTheme::Dark => Theme::Dark,
            UiTheme::Light => Theme::Light,
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        let close_sub = iced::window::close_requests().map(Message::WindowCloseRequested);
        let tick_sub = if self.sim_running {
            let period = Duration::from_millis(self.project.manifest.scan_cycle_ms.max(5));
            iced::time::every(period).map(|_| Message::PlcTickTimer)
        } else {
            iced::Subscription::none()
        };
        let key_sub = iced::keyboard::on_key_press(|key, _modifiers| {
            if matches!(
                key,
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete | iced::keyboard::key::Named::Backspace)
            ) {
                Some(Message::SfcRequestDeleteSelected(String::new()))
            } else {
                None
            }
        });
        iced::Subscription::batch([close_sub, tick_sub, key_sub])
    }

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
            let curr = std::mem::replace(&mut self.project, prev);
            self.redo_stack.push(curr);
            self.last_committed_project = self.project.clone();
            self.last_edit_time = None;
            self.recompile_runtime();
            self.sync_st_contents();
            self.modified = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            let curr = std::mem::replace(&mut self.project, next);
            self.undo_stack.push(curr);
            self.last_committed_project = self.project.clone();
            self.last_edit_time = None;
            self.recompile_runtime();
            self.sync_st_contents();
            self.modified = true;
        }
    }

    pub fn sync_st_contents(&mut self) {
        for pou_meta in &self.project.manifest.pous {
            if pou_meta.language == LanguageKind::St
                && let Some(PouSource::St(st_prog)) = self.project.pous.get(&pou_meta.name) {
                    self.st_contents.insert(pou_meta.name.clone(), text_editor::Content::with_text(&st_prog.source));
                }
        }
    }

    pub fn recompile_runtime(&mut self) {
        self.diagnostics = self.project.check().unwrap_or_default();
        match PlcRuntime::from_project(&self.project) {
            Ok(rt) => {
                self.runtime = Some(rt);
                self.status_banner = self
                    .ui_lang
                    .tr(
                        "Runtime recompiled successfully.",
                        "API recompilée avec succès.",
                    )
                    .to_string();
            }
            Err(e) => {
                self.status_banner = format!("Compilation error: {e}");
            }
        }
    }

    pub fn execute_plc_scan_tick(&mut self) -> Result<ScanTelemetry, RuntimeError> {
        self.sync_shared_io();

        let Some(rt) = &mut self.runtime else {
            return Err(RuntimeError::ExecutionError {
                pou: "system".into(),
                message: "Runtime not initialized".into(),
            });
        };

        if self.hal_enabled {
            let _ = self.hal_driver.read_inputs(&mut rt.process_image);
        }

        let tel = rt.step_scan()?;
        self.trend_plotter.sample(rt);

        if self.hal_enabled {
            let _ = self.hal_driver.write_outputs(&rt.process_image);
        }

        if let Ok(mut snap) = self.shared_io.write() {
            for tag in rt.tags.tags() {
                if let Ok(v) = rt.read_value(&tag.name) {
                    snap.values.insert(tag.name.clone(), v);
                }
            }
        }

        if self.modbus_server.is_some()
            && let Ok(mut pi_lock) = self.modbus_shared_pi.write() {
                *pi_lock = rt.process_image.clone();
            }

        Ok(tel)
    }

    pub fn sync_shared_io(&mut self) {
        let pending = if let Ok(mut lock) = self.shared_io.write() {
            lock.take_pending_writes()
        } else {
            Vec::new()
        };

        if let Some(rt) = &mut self.runtime {
            for (var_name, val) in pending {
                let _ = rt.force(&var_name, val);
            }
        }
    }

    pub fn execute_all_scenarios(&mut self) {
        self.scenario_logs.clear();
        let mut rt = match PlcRuntime::from_project(&self.project) {
            Ok(r) => r,
            Err(e) => {
                self.scenario_logs.push(format!("[ERROR] Runtime build failed: {e}"));
                return;
            }
        };

        for sc in &self.project.scenarios.scenarios {
            match rt.run_scenario(sc) {
                Ok(rep) => {
                    let pass_tag = self.ui_lang.tr("[PASS]", "[SUCCÈS]");
                    self.scenario_logs.push(format!(
                        "{pass_tag} Scenario '{}': verified ({} assertions, {} scans, max {}µs)",
                        sc.name, rep.assertions_verified, rep.scans_executed, rep.max_scan_duration_us
                    ));
                }
                Err(e) => {
                    self.scenario_logs.push(format!("[ERROR] Scenario '{}': {e}", sc.name));
                }
            }
        }
    }

    pub fn toggle_modbus_server(&mut self) {
        if self.modbus_server.is_some() {
            self.modbus_server = None;
            self.modbus_last_test_result = Some(self.ui_lang.tr("Modbus TCP Server stopped.", "Serveur Modbus TCP arrêté.").to_string());
        } else {
            if let Some(rt) = &self.runtime
                && let Ok(mut pi) = self.modbus_shared_pi.write() {
                    *pi = rt.process_image.clone();
                }
            let addr_str = format!("127.0.0.1:{}", self.modbus_port);
            if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
                match ModbusTcpServer::start(addr, self.modbus_shared_pi.clone()) {
                    Ok(srv) => {
                        self.modbus_server = Some(srv);
                        self.modbus_last_test_result = Some(format!(
                            "{} {addr}",
                            self.ui_lang.tr("Modbus TCP Server active on", "Serveur Modbus TCP actif sur")
                        ));
                    }
                    Err(e) => {
                        self.modbus_last_test_result = Some(format!("Failed to bind Modbus TCP server: {e}"));
                    }
                }
            } else {
                self.modbus_last_test_result = Some(format!("Invalid socket address: {addr_str}"));
            }
        }
    }

    pub fn test_modbus_ping(&mut self) {
        let addr_str = format!("127.0.0.1:{}", self.modbus_port);
        let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() else {
            self.modbus_last_test_result = Some(format!("Invalid socket address: {addr_str}"));
            return;
        };

        let client = ModbusTcpClient::connect(addr);
        match client {
            Ok(mut c) => {
                let req = ModbusRequest::ReadDiscreteInputs { address: 0, count: 8 };
                match c.send_request(1, &req) {
                    Ok(ModbusResponse::ReadBits(bits)) => {
                        self.modbus_last_test_result = Some(format!("PING OK — Received {} inputs: {:?}", bits.len(), bits));
                    }
                    Ok(other) => {
                        self.modbus_last_test_result = Some(format!("PING OK — Unexpected response: {:?}", other));
                    }
                    Err(e) => {
                        self.modbus_last_test_result = Some(format!("PING FAILED — Error sending request: {e}"));
                    }
                }
            }
            Err(e) => {
                self.modbus_last_test_result = Some(format!("PING FAILED — Connection error: {e}"));
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::None => Task::none(),
            Message::SelectTab(tab) => {
                self.active_tab = tab;
                Task::none()
            }
            Message::SelectBottomTab(tab) => {
                self.bottom_dock_tab = tab;
                Task::none()
            }
            Message::ToggleLeftExplorer => {
                self.show_left_explorer = !self.show_left_explorer;
                Task::none()
            }
            Message::ToggleRightIo => {
                self.show_right_io = !self.show_right_io;
                Task::none()
            }
            Message::ToggleBottomDock => {
                self.show_bottom_dock = !self.show_bottom_dock;
                Task::none()
            }
            Message::DetachLeftExplorer => {
                self.detach_left_explorer = true;
                let (id, open_task) = iced::window::open(iced::window::Settings {
                    size: iced::Size::new(380.0, 800.0),
                    min_size: Some(iced::Size::new(260.0, 400.0)),
                    exit_on_close_request: false,
                    ..Default::default()
                });
                self.detached_left_id = Some(id);
                open_task.map(|_| Message::None)
            }
            Message::DockLeftExplorer => {
                self.show_left_explorer = true;
                self.detach_left_explorer = false;
                if let Some(id) = self.detached_left_id.take() {
                    iced::window::close(id)
                } else {
                    Task::none()
                }
            }
            Message::DetachRightIo => {
                self.detach_right_io = true;
                let (id, open_task) = iced::window::open(iced::window::Settings {
                    size: iced::Size::new(460.0, 800.0),
                    min_size: Some(iced::Size::new(320.0, 400.0)),
                    exit_on_close_request: false,
                    ..Default::default()
                });
                self.detached_right_id = Some(id);
                open_task.map(|_| Message::None)
            }
            Message::DockRightIo => {
                self.show_right_io = true;
                self.detach_right_io = false;
                if let Some(id) = self.detached_right_id.take() {
                    iced::window::close(id)
                } else {
                    Task::none()
                }
            }
            Message::DetachBottomDock => {
                self.detach_bottom_dock = true;
                let (id, open_task) = iced::window::open(iced::window::Settings {
                    size: iced::Size::new(1100.0, 420.0),
                    min_size: Some(iced::Size::new(600.0, 250.0)),
                    exit_on_close_request: false,
                    ..Default::default()
                });
                self.detached_bottom_id = Some(id);
                open_task.map(|_| Message::None)
            }
            Message::DockBottomDock => {
                self.show_bottom_dock = true;
                self.detach_bottom_dock = false;
                if let Some(id) = self.detached_bottom_id.take() {
                    iced::window::close(id)
                } else {
                    Task::none()
                }
            }
            Message::WindowCloseRequested(id) => {
                if Some(id) == self.detached_left_id {
                    self.show_left_explorer = true;
                    self.detach_left_explorer = false;
                    self.detached_left_id = None;
                    iced::window::close(id)
                } else if Some(id) == self.detached_right_id {
                    self.show_right_io = true;
                    self.detach_right_io = false;
                    self.detached_right_id = None;
                    iced::window::close(id)
                } else if Some(id) == self.detached_bottom_id {
                    self.show_bottom_dock = true;
                    self.detach_bottom_dock = false;
                    self.detached_bottom_id = None;
                    iced::window::close(id)
                } else if Some(id) == self.main_window_id || self.main_window_id.is_none() {
                    if self.modified && !self.allowed_to_close {
                        self.show_close_confirm_modal = true;
                        Task::none()
                    } else {
                        iced::exit()
                    }
                } else {
                    iced::window::close(id)
                }
            }
            Message::ConfirmCloseSaveAndExit => {
                let root = self.project.root_dir.clone();
                let _ = self.project.save(&root);
                self.modified = false;
                self.allowed_to_close = true;
                self.show_close_confirm_modal = false;
                iced::exit()
            }
            Message::ConfirmCloseDiscardAndExit => {
                self.allowed_to_close = true;
                self.show_close_confirm_modal = false;
                iced::exit()
            }
            Message::ConfirmCloseCancel => {
                self.show_close_confirm_modal = false;
                Task::none()
            }
            Message::WindowCreated(_id) => Task::none(),
            Message::SetLanguage(lang) => {
                self.ui_lang = lang;
                Task::none()
            }
            Message::SetTheme(theme) => {
                self.ui_theme = theme;
                Task::none()
            }
            Message::ToggleRunSim => {
                self.sim_running = !self.sim_running;
                Task::none()
            }
            Message::StepSimTick => {
                let _ = self.execute_plc_scan_tick();
                Task::none()
            }
            Message::ResetSim => {
                self.recompile_runtime();
                self.trend_plotter.clear_all();
                Task::none()
            }
            Message::PlcTickTimer => {
                let _ = self.execute_plc_scan_tick();
                Task::none()
            }
            Message::RunScenarios => {
                self.execute_all_scenarios();
                self.bottom_dock_tab = BottomDockTab::Diagnostics;
                Task::none()
            }
            Message::SaveProject => {
                let root = self.project.root_dir.clone();
                if let Err(e) = self.project.save(&root) {
                    self.status_banner = format!("Save failed: {e}");
                } else {
                    self.modified = false;
                    self.status_banner = self.ui_lang.tr("Project saved successfully.", "Projet enregistré avec succès.").to_string();
                }
                Task::none()
            }
            Message::CheckProject => {
                self.recompile_runtime();
                self.bottom_dock_tab = BottomDockTab::Diagnostics;
                Task::none()
            }
            Message::Undo => {
                self.undo();
                Task::none()
            }
            Message::Redo => {
                self.redo();
                Task::none()
            }
            Message::AddPouClicked => {
                if !self.new_pou_name.trim().is_empty() {
                    let name = self.new_pou_name.trim().to_string();
                    let kind = self.new_pou_lang;
                    if self.project.add_pou(&name, kind).is_ok() {
                        self.track_history();
                        self.active_tab = WorkspaceTab::Pou(name.clone());
                        if kind == LanguageKind::St {
                            self.st_contents.insert(name.clone(), text_editor::Content::with_text("PROGRAM main\n  \nEND_PROGRAM\n"));
                        }
                        self.new_pou_name.clear();
                        self.recompile_runtime();
                        self.modified = true;
                    }
                }
                Task::none()
            }
            Message::NewPouNameChanged(name) => {
                self.new_pou_name = name;
                Task::none()
            }
            Message::NewPouLangSelected(kind) => {
                self.new_pou_lang = kind;
                Task::none()
            }
            Message::ConfirmClearPou(name) => {
                self.confirm_clear_modal = Some(name);
                Task::none()
            }
            Message::CancelClearPou => {
                self.confirm_clear_modal = None;
                Task::none()
            }
            Message::ExecuteClearPou(name) => {
                if let Some(pou) = self.project.pous.get_mut(&name) {
                    match pou {
                        PouSource::Ld(ld) => ld.rungs.clear(),
                        PouSource::Sfc(sfc) => {
                            sfc.steps.clear();
                            sfc.transitions.clear();
                        }
                        PouSource::St(st) => {
                            st.source.clear();
                            st.statements.clear();
                            if let Some(content) = self.st_contents.get_mut(&name) {
                                *content = text_editor::Content::new();
                            }
                        }
                        PouSource::Fbd(fbd) => fbd.blocks.clear(),
                    }
                    self.track_history();
                    self.recompile_runtime();
                    self.modified = true;
                }
                self.confirm_clear_modal = None;
                Task::none()
            }
            Message::PouFilterTextChanged(txt) => {
                self.pou_filter_text = txt;
                Task::none()
            }
            Message::IoFilterTextChanged(txt) => {
                self.io_filter_text = txt;
                Task::none()
            }
            Message::IoFilterAreaSelected(area) => {
                self.io_filter_sfc = false;
                self.io_filter_area = area;
                Task::none()
            }
            Message::IoFilterSfcSelected => {
                self.io_filter_sfc = true;
                self.io_filter_area = None;
                Task::none()
            }
            Message::ToggleForceBool(tag_name, addr) => {
                if let Some(rt) = &mut self.runtime {
                    let cur = rt.process_image.read(addr, IecType::Bool).unwrap_or(IecValue::Bool(false));
                    let next = match cur {
                        IecValue::Bool(b) => IecValue::Bool(!b),
                        _ => IecValue::Bool(true),
                    };
                    let _ = rt.force(&tag_name, next);
                }
                Task::none()
            }
            Message::ForceValInputChanged(tag_name, val_str) => {
                if let Some(rt) = &mut self.runtime
                    && let Ok(parsed) = IecValue::parse_literal(&val_str, None) {
                        let _ = rt.force(&tag_name, parsed);
                    }
                Task::none()
            }
            Message::ToggleHalEnabled => {
                self.hal_enabled = !self.hal_enabled;
                Task::none()
            }
            Message::MockGpioPinToggled(bcm) => {
                let cur = self.hal_driver.pin_levels().get(&bcm).copied().unwrap_or(false);
                let _ = self.hal_driver.set_mock_input(bcm, !cur);
                Task::none()
            }
            Message::ToggleModbusServer => {
                self.toggle_modbus_server();
                Task::none()
            }
            Message::ModbusPortChanged(p_str) => {
                if let Ok(p) = p_str.parse::<u16>() {
                    self.modbus_port = p;
                }
                Task::none()
            }
            Message::TestModbusPing => {
                self.test_modbus_ping();
                Task::none()
            }
            Message::AcknowledgeAlarm(id) => {
                if let Some(rt) = &mut self.runtime {
                    rt.acknowledge_alarm(&id);
                }
                Task::none()
            }
            Message::AcknowledgeAllAlarms => {
                if let Some(rt) = &mut self.runtime {
                    rt.acknowledge_all_alarms();
                }
                Task::none()
            }
            Message::TrendTogglePause => {
                self.trend_plotter.paused = !self.trend_plotter.paused;
                Task::none()
            }
            Message::TrendClear => {
                self.trend_plotter.clear_all();
                Task::none()
            }
            Message::TrendToggleChannel(ch) => {
                if let Some(c) = self.trend_plotter.channels.get_mut(&ch) {
                    c.enabled = !c.enabled;
                }
                Task::none()
            }
            Message::ScanCycleMsChanged(ms) => {
                self.project.manifest.scan_cycle_ms = ms;
                self.track_history();
                self.modified = true;
                Task::none()
            }
            Message::WatchdogUsChanged(us) => {
                self.project.manifest.watchdog_us = us;
                self.track_history();
                self.modified = true;
                Task::none()
            }
            Message::AddGlobalTag => {
                let n = self.project.global_tags.tags.len() + 1;
                self.project.global_tags.tags.push(TagEntry::new(
                    format!("New_Tag_{n}"),
                    IecType::Bool,
                    None,
                    IecValue::Bool(false),
                    self.ui_lang.tr("User variable", "Variable utilisateur"),
                ));
                self.track_history();
                self.modified = true;
                Task::none()
            }
            Message::RemoveGlobalTag(idx) => {
                if idx < self.project.global_tags.tags.len() {
                    self.project.global_tags.tags.remove(idx);
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::TagNameChanged(idx, name) => {
                if let Some(tag) = self.project.global_tags.tags.get_mut(idx) {
                    tag.name = name;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::TagTypeChanged(idx, ty) => {
                if let Some(tag) = self.project.global_tags.tags.get_mut(idx) {
                    tag.data_type = ty;
                    tag.initial_value = ty.default_value();
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::TagAddressChanged(idx, addr_str) => {
                if let Some(tag) = self.project.global_tags.tags.get_mut(idx) {
                    if addr_str.trim().is_empty() {
                        tag.address = None;
                    } else if let Ok(parsed) = addr_str.trim().parse::<DirectAddress>() {
                        tag.address = Some(parsed);
                    }
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::TagInitialValueChanged(idx, init_str) => {
                if let Some(tag) = self.project.global_tags.tags.get_mut(idx)
                    && let Ok(v) = IecValue::parse_literal(&init_str, Some(tag.data_type)) {
                        tag.initial_value = v;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::TagCommentChanged(idx, c) => {
                if let Some(tag) = self.project.global_tags.tags.get_mut(idx) {
                    tag.comment = c;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::AddFbInstance(kind) => {
                let n = self.project.fb_instances.instances.len() + 1;
                let name = if kind.is_corrector() { format!("PID_{n}") } else { format!("Timer_{n}") };
                self.project.fb_instances.instances.push(FbInstance::new(name, kind));
                self.track_history();
                self.modified = true;
                Task::none()
            }
            Message::RemoveFbInstance(idx) => {
                if idx < self.project.fb_instances.instances.len() {
                    self.project.fb_instances.instances.remove(idx);
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbNameChanged(idx, name) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.name = name;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbKindChanged(idx, kind) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.kind = kind;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbGainChanged(idx, kp) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.kp = kp;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbTiChanged(idx, ti) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.ti_s = ti;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbTdChanged(idx, td) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.td_s = td;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbPtChanged(idx, pt) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.pt_ms = pt;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbPvChanged(idx, pv) => {
                if let Some(fb) = self.project.fb_instances.instances.get_mut(idx) {
                    fb.pv = pv;
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::StEditorAction(name, action) => {
                if let Some(content) = self.st_contents.get_mut(&name) {
                    content.perform(action);
                    let new_src = content.text();
                    if let Some(PouSource::St(st_prog)) = self.project.pous.get_mut(&name) {
                        st_prog.source = new_src.clone();
                        if let Ok(stmts) = parse_st_statements(&new_src, &name) {
                            st_prog.statements = stmts;
                        }
                        self.track_history();
                        self.modified = true;
                    }
                }
                Task::none()
            }
            Message::LdAddRung(pou_name) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name) {
                    let next = ld.rungs.len() + 1;
                    ld.rungs.push(LdRung {
                        id: format!("Rung_{next}"),
                        comment: String::new(),
                        elements: vec![LdElement::Contact {
                            symbol: "Start_Button".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                        coils: vec![LdCoil {
                            symbol: "Conveyor_Ready".to_string(),
                            kind: CoilKind::Normal,
                        }],
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::LdRemoveRung(pou_name, r_idx) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && r_idx < ld.rungs.len() {
                        ld.rungs.remove(r_idx);
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::LdRungCommentChanged(pou_name, r_idx, c) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx) {
                        rung.comment = c;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::LdAddContact(pou_name, r_idx) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx) {
                        rung.elements.push(LdElement::Contact {
                            symbol: "Sensor_1".to_string(),
                            kind: ContactKind::NormallyOpen,
                        });
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::LdAddContactWithKind(pou_name, r_idx, kind) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx) {
                        let sym = match kind {
                            ContactKind::NormallyOpen => "Sensor_1".to_string(),
                            ContactKind::NormallyClosed => "High_Temp_Alarm".to_string(),
                            ContactKind::RisingEdge => "Start_Button".to_string(),
                            ContactKind::FallingEdge => "Stop_Button".to_string(),
                        };
                        rung.elements.push(LdElement::Contact {
                            symbol: sym,
                            kind,
                        });
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::LdRemoveContact(pou_name, r_idx, e_idx) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx)
                        && e_idx < rung.elements.len() {
                            rung.elements.remove(e_idx);
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::LdContactSymbolChanged(pou_name, r_idx, e_idx, s) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx)
                        && let Some(LdElement::Contact { symbol, .. }) = rung.elements.get_mut(e_idx) {
                            *symbol = s;
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::LdContactKindChanged(pou_name, r_idx, e_idx, k) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx)
                        && let Some(LdElement::Contact { kind, .. }) = rung.elements.get_mut(e_idx) {
                            *kind = k;
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::LdAddCoil(pou_name, r_idx) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx) {
                        rung.coils.push(LdCoil {
                            symbol: "Output_1".to_string(),
                            kind: CoilKind::Normal,
                        });
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::LdAddCoilWithKind(pou_name, r_idx, kind) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx) {
                        rung.coils.push(LdCoil {
                            symbol: "Permissive_OK".to_string(),
                            kind,
                        });
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::LdRemoveCoil(pou_name, r_idx, c_idx) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx)
                        && c_idx < rung.coils.len() {
                            rung.coils.remove(c_idx);
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::LdCoilSymbolChanged(pou_name, r_idx, c_idx, s) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx)
                        && let Some(coil) = rung.coils.get_mut(c_idx) {
                            coil.symbol = s;
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::LdCoilKindChanged(pou_name, r_idx, c_idx, k) => {
                if let Some(PouSource::Ld(ld)) = self.project.pous.get_mut(&pou_name)
                    && let Some(rung) = ld.rungs.get_mut(r_idx)
                        && let Some(coil) = rung.coils.get_mut(c_idx) {
                            coil.kind = k;
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::SfcAddStep(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let next = sfc
                        .steps
                        .iter()
                        .filter_map(|s| s.id.trim_start_matches(['X', 'x']).parse::<usize>().ok())
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let step_id = format!("{next}");
                    let step_name = format!("Step_{next}");

                    let pos = if let Some((ref p, ref key)) = self.sfc_selected
                        && p == &pou_name
                    {
                        match key {
                            crate::canvas_sfc::SfcElementKey::Step(sid) => {
                                sfc.steps
                                    .iter()
                                    .find(|s| &s.id == sid)
                                    .and_then(|s| s.grid_pos)
                                    .map(|(sx, sy)| (sx, sy + 160))
                            }
                            crate::canvas_sfc::SfcElementKey::Transition(tid) => {
                                sfc.transitions
                                    .iter()
                                    .find(|t| &t.id == tid)
                                    .and_then(|t| t.grid_pos)
                                    .map(|(tx, ty)| ((tx - 50).max(60), ty + 80))
                            }
                        }
                    } else {
                        None
                    };

                    let grid_pos = pos.or_else(|| {
                        let lowest_y = sfc
                            .steps
                            .iter()
                            .filter_map(|s| s.grid_pos.map(|(_, y)| y + 60))
                            .chain(sfc.transitions.iter().filter_map(|t| t.grid_pos.map(|(_, y)| y + 20)))
                            .max();
                        let base_x = sfc.steps.first().and_then(|s| s.grid_pos.map(|(x, _)| x)).unwrap_or(60);
                        Some((base_x, lowest_y.map(|y| y + 40).unwrap_or(40)))
                    });

                    // If a step was selected, insert an intermediate transition between them
                    if let Some((ref p, crate::canvas_sfc::SfcElementKey::Step(ref sid))) = self.sfc_selected
                        && p == &pou_name
                    {
                        let trans_id = format!("T{}", sfc.transitions.len() + 1);
                        let (sx, sy) = sfc.steps.iter().find(|s| &s.id == sid).and_then(|s| s.grid_pos).unwrap_or((60, 40));
                        sfc.transitions.push(SfcTransition {
                            id: trans_id,
                            from_steps: vec![sid.clone()],
                            to_steps: vec![step_id.clone()],
                            receptivity: "TRUE".to_string(),
                            convergence: Default::default(),
                            divergence: Default::default(),
                            grid_pos: Some((sx + 50, sy + 80)),
                        });
                    } else if let Some((ref p, crate::canvas_sfc::SfcElementKey::Transition(ref tid))) = self.sfc_selected
                        && p == &pou_name
                        && let Some(tr) = sfc.transitions.iter_mut().find(|t| &t.id == tid)
                        && !tr.to_steps.contains(&step_id)
                    {
                        tr.to_steps.push(step_id.clone());
                    }

                    sfc.steps.push(SfcStep {
                        id: step_id,
                        name: step_name,
                        kind: StepKind::Normal,
                        actions: Vec::new(),
                        grid_pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcRemoveStep(pou_name, s_idx) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && s_idx < sfc.steps.len() {
                        sfc.steps.remove(s_idx);
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcStepNameChanged(pou_name, s_idx, name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx) {
                        step.name = name;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcStepKindChanged(pou_name, s_idx, kind) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx) {
                        step.kind = kind;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcAddAction(pou_name, s_idx) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx) {
                        step.actions.push(SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Motor_Run".to_string(),
                            duration: None,
                        });
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcRemoveAction(pou_name, s_idx, a_idx) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx)
                        && a_idx < step.actions.len() {
                            step.actions.remove(a_idx);
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::SfcActionQualifierChanged(pou_name, s_idx, a_idx, q) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx)
                        && let Some(act) = step.actions.get_mut(a_idx) {
                            act.qualifier = q;
                            if matches!(q, ActionQualifier::L | ActionQualifier::D) && act.duration.is_none() {
                                act.duration = Some("T#2s".to_string());
                            }
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::SfcActionTargetChanged(pou_name, s_idx, a_idx, t) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx)
                        && let Some(act) = step.actions.get_mut(a_idx) {
                            act.target = t;
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::SfcActionDurationChanged(pou_name, s_idx, a_idx, d) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(step) = sfc.steps.get_mut(s_idx)
                        && let Some(act) = step.actions.get_mut(a_idx) {
                            act.duration = Some(d);
                            self.track_history();
                            self.modified = true;
                        }
                Task::none()
            }
            Message::SfcAddTransition(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let next = sfc.transitions.len() + 1;
                    let trans_id = format!("T{next}");

                    let (pos, from_steps) = if let Some((ref p, ref key)) = self.sfc_selected
                        && p == &pou_name
                    {
                        match key {
                            crate::canvas_sfc::SfcElementKey::Step(sid) => {
                                let step_pos = sfc.steps.iter().find(|s| &s.id == sid).and_then(|s| s.grid_pos);
                                let p = step_pos.map(|(sx, sy)| (sx + 50, sy + 100));
                                (p, vec![sid.clone()])
                            }
                            crate::canvas_sfc::SfcElementKey::Transition(tid) => {
                                let (tx, ty) = sfc.transitions.iter().find(|t| &t.id == tid).and_then(|t| t.grid_pos).unwrap_or((110, 140));
                                let inter_step_id = format!("X{}", sfc.steps.len() + 1);
                                let inter_step_name = format!("Step_{}", sfc.steps.len() + 1);
                                if let Some(tr) = sfc.transitions.iter_mut().find(|t| &t.id == tid)
                                    && !tr.to_steps.contains(&inter_step_id) {
                                        tr.to_steps.push(inter_step_id.clone());
                                    }
                                sfc.steps.push(SfcStep {
                                    id: inter_step_id.clone(),
                                    name: inter_step_name,
                                    kind: StepKind::Normal,
                                    actions: Vec::new(),
                                    grid_pos: Some(((tx - 50).max(60), ty + 80)),
                                });
                                (Some((tx, ty + 180)), vec![inter_step_id])
                            }
                        }
                    } else {
                        (None, Vec::new())
                    };

                    let grid_pos = pos.or_else(|| {
                        let lowest_y = sfc
                            .steps
                            .iter()
                            .filter_map(|s| s.grid_pos.map(|(_, y)| y + 60))
                            .chain(sfc.transitions.iter().filter_map(|t| t.grid_pos.map(|(_, y)| y + 20)))
                            .max();
                        let base_x = sfc.transitions.first().and_then(|t| t.grid_pos.map(|(x, _)| x)).unwrap_or(110);
                        Some((base_x, lowest_y.map(|y| y + 40).unwrap_or(140)))
                    });

                    sfc.transitions.push(SfcTransition {
                        id: trans_id,
                        from_steps,
                        to_steps: Vec::new(),
                        receptivity: "TRUE".to_string(),
                        convergence: Default::default(),
                        divergence: Default::default(),
                        grid_pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcRemoveTransition(pou_name, t_idx) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && t_idx < sfc.transitions.len() {
                        sfc.transitions.remove(t_idx);
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcTransitionReceptivityChanged(pou_name, t_idx, r) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.get_mut(t_idx) {
                        tr.receptivity = r;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcTransitionFromChanged(pou_name, t_idx, s) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.get_mut(t_idx) {
                        tr.from_steps = s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcTransitionToChanged(pou_name, t_idx, s) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.get_mut(t_idx) {
                        tr.to_steps = s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcTransitionConvergenceChanged(pou_name, t_idx, conv) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.get_mut(t_idx) {
                        tr.convergence = conv;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcTransitionDivergenceChanged(pou_name, t_idx, div) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.get_mut(t_idx) {
                        tr.divergence = div;
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcAddAndDivergence(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let next_t = sfc.transitions.len() + 1;
                    let trans_id = format!("T{next_t}_AndDiv");

                    let mut from_steps = Vec::new();
                    let mut to_steps = Vec::new();
                    let mut pos = None;

                    let selected_steps: Vec<String> = self.sfc_selected_elements.iter().filter_map(|e| {
                        if let crate::canvas_sfc::SfcElementKey::Step(id) = e { Some(id.clone()) } else { None }
                    }).collect();

                    if selected_steps.len() >= 2 {
                        to_steps = selected_steps.clone();
                        let step_positions: Vec<(i32, i32)> = sfc.steps.iter()
                            .filter(|s| selected_steps.contains(&s.id))
                            .filter_map(|s| s.grid_pos)
                            .collect();
                        if !step_positions.is_empty() {
                            let avg_x = step_positions.iter().map(|(x, _)| x).sum::<i32>() / step_positions.len() as i32;
                            let min_y = step_positions.iter().map(|(_, y)| *y).min().unwrap_or(140);
                            pos = Some((avg_x + 50, (min_y - 80).max(40)));
                        }
                    } else if let Some((ref p, crate::canvas_sfc::SfcElementKey::Step(ref sid))) = self.sfc_selected
                        && p == &pou_name {
                            from_steps.push(sid.clone());
                            if let Some(s) = sfc.steps.iter().find(|s| &s.id == sid)
                                && let Some((sx, sy)) = s.grid_pos {
                                    pos = Some((sx + 50, sy + 100));
                                }
                    }

                    let grid_pos = pos.or_else(|| {
                        let lowest_y = sfc.steps.iter().filter_map(|s| s.grid_pos.map(|(_, y)| y + 60))
                            .chain(sfc.transitions.iter().filter_map(|t| t.grid_pos.map(|(_, y)| y + 20)))
                            .max();
                        let base_x = sfc.transitions.first().and_then(|t| t.grid_pos.map(|(x, _)| x)).unwrap_or(110);
                        Some((base_x, lowest_y.map(|y| y + 40).unwrap_or(140)))
                    });

                    sfc.transitions.push(SfcTransition {
                        id: trans_id,
                        from_steps,
                        to_steps,
                        receptivity: "TRUE".to_string(),
                        convergence: ConvergenceKind::And,
                        divergence: DivergenceKind::And,
                        grid_pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcAddAndConvergence(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let next_t = sfc.transitions.len() + 1;
                    let trans_id = format!("T{next_t}_AndConv");

                    let mut from_steps = Vec::new();
                    let mut pos = None;

                    let selected_steps: Vec<String> = self.sfc_selected_elements.iter().filter_map(|e| {
                        if let crate::canvas_sfc::SfcElementKey::Step(id) = e { Some(id.clone()) } else { None }
                    }).collect();

                    if selected_steps.len() >= 2 {
                        from_steps = selected_steps.clone();
                        let step_positions: Vec<(i32, i32)> = sfc.steps.iter()
                            .filter(|s| selected_steps.contains(&s.id))
                            .filter_map(|s| s.grid_pos)
                            .collect();
                        if !step_positions.is_empty() {
                            let avg_x = step_positions.iter().map(|(x, _)| x).sum::<i32>() / step_positions.len() as i32;
                            let max_y = step_positions.iter().map(|(_, y)| *y).max().unwrap_or(100);
                            pos = Some((avg_x + 50, max_y + 100));
                        }
                    } else if let Some((ref p, crate::canvas_sfc::SfcElementKey::Step(ref sid))) = self.sfc_selected
                        && p == &pou_name {
                            from_steps.push(sid.clone());
                            if let Some(s) = sfc.steps.iter().find(|s| &s.id == sid)
                                && let Some((sx, sy)) = s.grid_pos {
                                    pos = Some((sx + 50, sy + 100));
                                }
                    }

                    let grid_pos = pos.or_else(|| {
                        let lowest_y = sfc.steps.iter().filter_map(|s| s.grid_pos.map(|(_, y)| y + 60))
                            .chain(sfc.transitions.iter().filter_map(|t| t.grid_pos.map(|(_, y)| y + 20)))
                            .max();
                        let base_x = sfc.transitions.first().and_then(|t| t.grid_pos.map(|(x, _)| x)).unwrap_or(110);
                        Some((base_x, lowest_y.map(|y| y + 40).unwrap_or(140)))
                    });

                    sfc.transitions.push(SfcTransition {
                        id: trans_id,
                        from_steps,
                        to_steps: Vec::new(),
                        receptivity: "TRUE".to_string(),
                        convergence: ConvergenceKind::And,
                        divergence: DivergenceKind::And,
                        grid_pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcAddOrDivergence(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let next_t = sfc.transitions.len() + 1;
                    let trans_id = format!("T{next_t}_OrDiv");

                    let mut from_steps = Vec::new();
                    let mut to_steps = Vec::new();
                    let mut pos = None;

                    let selected_steps: Vec<String> = self.sfc_selected_elements.iter().filter_map(|e| {
                        if let crate::canvas_sfc::SfcElementKey::Step(id) = e { Some(id.clone()) } else { None }
                    }).collect();

                    if selected_steps.len() >= 2 {
                        to_steps = selected_steps.clone();
                        let step_positions: Vec<(i32, i32)> = sfc.steps.iter()
                            .filter(|s| selected_steps.contains(&s.id))
                            .filter_map(|s| s.grid_pos)
                            .collect();
                        if !step_positions.is_empty() {
                            let avg_x = step_positions.iter().map(|(x, _)| x).sum::<i32>() / step_positions.len() as i32;
                            let min_y = step_positions.iter().map(|(_, y)| *y).min().unwrap_or(140);
                            pos = Some((avg_x + 50, (min_y - 80).max(40)));
                        }
                    } else if let Some((ref p, crate::canvas_sfc::SfcElementKey::Step(ref sid))) = self.sfc_selected
                        && p == &pou_name {
                            from_steps.push(sid.clone());
                            if let Some(s) = sfc.steps.iter().find(|s| &s.id == sid)
                                && let Some((sx, sy)) = s.grid_pos {
                                    pos = Some((sx + 50, sy + 100));
                                }
                    }

                    let grid_pos = pos.or_else(|| {
                        let lowest_y = sfc.steps.iter().filter_map(|s| s.grid_pos.map(|(_, y)| y + 60))
                            .chain(sfc.transitions.iter().filter_map(|t| t.grid_pos.map(|(_, y)| y + 20)))
                            .max();
                        let base_x = sfc.transitions.first().and_then(|t| t.grid_pos.map(|(x, _)| x)).unwrap_or(110);
                        Some((base_x, lowest_y.map(|y| y + 40).unwrap_or(140)))
                    });

                    sfc.transitions.push(SfcTransition {
                        id: trans_id,
                        from_steps,
                        to_steps,
                        receptivity: "TRUE".to_string(),
                        convergence: ConvergenceKind::And,
                        divergence: DivergenceKind::Or,
                        grid_pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcAddOrConvergence(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let next_t = sfc.transitions.len() + 1;
                    let trans_id = format!("T{next_t}_OrConv");

                    let mut from_steps = Vec::new();
                    let mut pos = None;

                    let selected_steps: Vec<String> = self.sfc_selected_elements.iter().filter_map(|e| {
                        if let crate::canvas_sfc::SfcElementKey::Step(id) = e { Some(id.clone()) } else { None }
                    }).collect();

                    if selected_steps.len() >= 2 {
                        from_steps = selected_steps.clone();
                        let step_positions: Vec<(i32, i32)> = sfc.steps.iter()
                            .filter(|s| selected_steps.contains(&s.id))
                            .filter_map(|s| s.grid_pos)
                            .collect();
                        if !step_positions.is_empty() {
                            let avg_x = step_positions.iter().map(|(x, _)| x).sum::<i32>() / step_positions.len() as i32;
                            let max_y = step_positions.iter().map(|(_, y)| *y).max().unwrap_or(100);
                            pos = Some((avg_x + 50, max_y + 100));
                        }
                    } else if let Some((ref p, crate::canvas_sfc::SfcElementKey::Step(ref sid))) = self.sfc_selected
                        && p == &pou_name {
                            from_steps.push(sid.clone());
                            if let Some(s) = sfc.steps.iter().find(|s| &s.id == sid)
                                && let Some((sx, sy)) = s.grid_pos {
                                    pos = Some((sx + 50, sy + 100));
                                }
                    }

                    let grid_pos = pos.or_else(|| {
                        let lowest_y = sfc.steps.iter().filter_map(|s| s.grid_pos.map(|(_, y)| y + 60))
                            .chain(sfc.transitions.iter().filter_map(|t| t.grid_pos.map(|(_, y)| y + 20)))
                            .max();
                        let base_x = sfc.transitions.first().and_then(|t| t.grid_pos.map(|(x, _)| x)).unwrap_or(110);
                        Some((base_x, lowest_y.map(|y| y + 40).unwrap_or(140)))
                    });

                    sfc.transitions.push(SfcTransition {
                        id: trans_id,
                        from_steps,
                        to_steps: Vec::new(),
                        receptivity: "TRUE".to_string(),
                        convergence: ConvergenceKind::Or,
                        divergence: DivergenceKind::And,
                        grid_pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcSelectElement(pou_name, key) => {
                self.sfc_selected = key.as_ref().map(|k| (pou_name, k.clone()));
                self.sfc_selected_elements = key.map(|k| vec![k]).unwrap_or_default();
                Task::none()
            }
            Message::SfcSelectElements(pou_name, keys) => {
                if keys.len() == 1 {
                    self.sfc_selected = Some((pou_name, keys[0].clone()));
                } else {
                    self.sfc_selected = None;
                }
                self.sfc_selected_elements = keys;
                Task::none()
            }
            Message::SfcRequestDeleteSelected(pou_name) => {
                let target_pou = if pou_name.is_empty() {
                    if let WorkspaceTab::Pou(ref p) = self.active_tab {
                        p.clone()
                    } else {
                        return Task::none();
                    }
                } else {
                    pou_name
                };

                let mut elements = self.sfc_selected_elements.clone();
                if elements.is_empty()
                    && let Some((ref p, ref k)) = self.sfc_selected
                        && p == &target_pou {
                            elements.push(k.clone());
                        }

                if elements.len() > 1 {
                    self.sfc_delete_modal = Some((target_pou, elements));
                } else if elements.len() == 1
                    && let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&target_pou) {
                        let mut del_steps = std::collections::HashSet::new();
                        let mut del_trans = std::collections::HashSet::new();
                        for el in &elements {
                            match el {
                                crate::canvas_sfc::SfcElementKey::Step(id) => {
                                    del_steps.insert(id.clone());
                                }
                                crate::canvas_sfc::SfcElementKey::Transition(id) => {
                                    del_trans.insert(id.clone());
                                }
                            }
                        }

                        sfc.steps.retain(|s| !del_steps.contains(&s.id));
                        sfc.transitions.retain(|t| !del_trans.contains(&t.id));

                        for tr in &mut sfc.transitions {
                            tr.from_steps.retain(|sid| !del_steps.contains(sid));
                            tr.to_steps.retain(|sid| !del_steps.contains(sid));
                        }

                        self.sfc_selected = None;
                        self.sfc_selected_elements.clear();
                        self.track_history();
                        self.recompile_runtime();
                        self.modified = true;
                }
                Task::none()
            }
            Message::SfcCancelDeleteSelected => {
                self.sfc_delete_modal = None;
                Task::none()
            }
            Message::SfcConfirmDeleteSelected => {
                if let Some((pou_name, elements)) = self.sfc_delete_modal.take()
                    && let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                        let mut del_steps = std::collections::HashSet::new();
                        let mut del_trans = std::collections::HashSet::new();
                        for el in &elements {
                            match el {
                                crate::canvas_sfc::SfcElementKey::Step(id) => {
                                    del_steps.insert(id.clone());
                                }
                                crate::canvas_sfc::SfcElementKey::Transition(id) => {
                                    del_trans.insert(id.clone());
                                }
                            }
                        }

                        sfc.steps.retain(|s| !del_steps.contains(&s.id));
                        sfc.transitions.retain(|t| !del_trans.contains(&t.id));

                        for tr in &mut sfc.transitions {
                            tr.from_steps.retain(|sid| !del_steps.contains(sid));
                            tr.to_steps.retain(|sid| !del_steps.contains(sid));
                        }

                        self.sfc_selected = None;
                        self.sfc_selected_elements.clear();
                        self.track_history();
                        self.recompile_runtime();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcDisconnectStepInput(pou_name, step_id) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    for tr in &mut sfc.transitions {
                        tr.to_steps.retain(|sid| sid != &step_id);
                    }
                    self.track_history();
                    self.recompile_runtime();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcDisconnectStepOutput(pou_name, step_id) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    for tr in &mut sfc.transitions {
                        tr.from_steps.retain(|sid| sid != &step_id);
                    }
                    self.track_history();
                    self.recompile_runtime();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcDisconnectTransitionInput(pou_name, trans_id) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == trans_id) {
                        tr.from_steps.clear();
                        self.track_history();
                        self.recompile_runtime();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcDisconnectTransitionOutput(pou_name, trans_id) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name)
                    && let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == trans_id) {
                        tr.to_steps.clear();
                        self.track_history();
                        self.recompile_runtime();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::SfcDisconnectAll(pou_name, key) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    match key {
                        crate::canvas_sfc::SfcElementKey::Step(step_id) => {
                            for tr in &mut sfc.transitions {
                                tr.from_steps.retain(|sid| sid != &step_id);
                                tr.to_steps.retain(|sid| sid != &step_id);
                            }
                        }
                        crate::canvas_sfc::SfcElementKey::Transition(trans_id) => {
                            if let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == trans_id) {
                                tr.from_steps.clear();
                                tr.to_steps.clear();
                            }
                        }
                    }
                    self.track_history();
                    self.recompile_runtime();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcMoveElement(pou_name, key, (gx, gy)) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    match key {
                        crate::canvas_sfc::SfcElementKey::Step(id) => {
                            if let Some(step) = sfc.steps.iter_mut().find(|s| s.id == id) {
                                step.grid_pos = Some((gx, gy));
                                self.track_history();
                                self.modified = true;
                            }
                        }
                        crate::canvas_sfc::SfcElementKey::Transition(id) => {
                            if let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == id) {
                                tr.grid_pos = Some((gx, gy));
                                self.track_history();
                                self.modified = true;
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::SfcConnectPorts { pou_name, source, target } => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    match (source, target) {
                        (
                            crate::canvas_sfc::SfcPortSource::StepOutput(step_id),
                            crate::canvas_sfc::SfcPortTarget::TransitionInput(trans_id),
                        )
                        | (
                            crate::canvas_sfc::SfcPortSource::TransitionInput(trans_id),
                            crate::canvas_sfc::SfcPortTarget::StepOutput(step_id),
                        )
                        | (
                            crate::canvas_sfc::SfcPortSource::StepOutput(step_id),
                            crate::canvas_sfc::SfcPortTarget::TransitionOutput(trans_id),
                        )
                        | (
                            crate::canvas_sfc::SfcPortSource::TransitionInput(trans_id),
                            crate::canvas_sfc::SfcPortTarget::StepInput(step_id),
                        ) => {
                            if let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == trans_id)
                                && !tr.from_steps.contains(&step_id)
                            {
                                tr.from_steps.push(step_id);
                                self.track_history();
                                self.modified = true;
                            }
                        }
                        (
                            crate::canvas_sfc::SfcPortSource::TransitionOutput(trans_id),
                            crate::canvas_sfc::SfcPortTarget::StepInput(step_id),
                        )
                        | (
                            crate::canvas_sfc::SfcPortSource::StepInput(step_id),
                            crate::canvas_sfc::SfcPortTarget::TransitionOutput(trans_id),
                        )
                        | (
                            crate::canvas_sfc::SfcPortSource::TransitionOutput(trans_id),
                            crate::canvas_sfc::SfcPortTarget::StepOutput(step_id),
                        )
                        | (
                            crate::canvas_sfc::SfcPortSource::StepInput(step_id),
                            crate::canvas_sfc::SfcPortTarget::TransitionInput(trans_id),
                        ) => {
                            if let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == trans_id)
                                && !tr.to_steps.contains(&step_id)
                            {
                                tr.to_steps.push(step_id);
                                self.track_history();
                                self.modified = true;
                            }
                        }
                        _ => {}
                    }
                }
                Task::none()
            }
            Message::SfcConnectSteps { pou_name, from_step, to_step } => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let trans_id = format!("T{}", sfc.transitions.len() + 1);
                    let pos = {
                        let p1 = sfc.steps.iter().find(|s| s.id == from_step).and_then(|s| s.grid_pos);
                        let p2 = sfc.steps.iter().find(|s| s.id == to_step).and_then(|s| s.grid_pos);
                        match (p1, p2) {
                            (Some((x1, y1)), Some((x2, y2))) => Some(((x1 + x2) / 2 + 50, (y1 + y2) / 2)),
                            (Some((x, y)), None) => Some((x + 50, y + 80)),
                            _ => None,
                        }
                    };
                    sfc.transitions.push(SfcTransition {
                        id: trans_id,
                        from_steps: vec![from_step],
                        to_steps: vec![to_step],
                        receptivity: "TRUE".to_string(),
                        convergence: Default::default(),
                        divergence: Default::default(),
                        grid_pos: pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcConnectTransitions { pou_name, from_trans, to_trans } => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    let step_id = format!("X{}", sfc.steps.len() + 1);
                    let step_name = format!("Step_{}", sfc.steps.len() + 1);
                    let pos = {
                        let p1 = sfc.transitions.iter().find(|t| t.id == from_trans).and_then(|t| t.grid_pos);
                        let p2 = sfc.transitions.iter().find(|t| t.id == to_trans).and_then(|t| t.grid_pos);
                        match (p1, p2) {
                            (Some((x1, y1)), Some((x2, y2))) => Some((((x1 + x2) / 2 - 50).max(60), (y1 + y2) / 2)),
                            (Some((x, y)), None) => Some(((x - 50).max(60), y + 80)),
                            _ => None,
                        }
                    };
                    if let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == from_trans)
                        && !tr.to_steps.contains(&step_id) {
                            tr.to_steps.push(step_id.clone());
                        }
                    if let Some(tr) = sfc.transitions.iter_mut().find(|t| t.id == to_trans)
                        && !tr.from_steps.contains(&step_id) {
                            tr.from_steps.push(step_id.clone());
                        }
                    sfc.steps.push(SfcStep {
                        id: step_id,
                        name: step_name,
                        kind: StepKind::Normal,
                        actions: Vec::new(),
                        grid_pos: pos,
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcAutoLayout(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get_mut(&pou_name) {
                    for s in &mut sfc.steps {
                        s.grid_pos = None;
                    }
                    for t in &mut sfc.transitions {
                        t.grid_pos = None;
                    }
                    sfc.assign_default_grid_positions();
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::SfcValidate(pou_name) => {
                if let Some(PouSource::Sfc(sfc)) = self.project.pous.get(&pou_name) {
                    let report = ferrite_lang_sfc::validate_sfc_topology(sfc);
                    self.status_banner = if report.is_valid {
                        format!(
                            "SFC '{pou_name}': Topology valid ({} independent parallel tree{})",
                            report.trees.len(),
                            if report.trees.len() == 1 { "" } else { "s" }
                        )
                    } else {
                        format!("SFC '{pou_name}': {} validation error(s) detected", report.errors.len())
                    };
                }
                Task::none()
            }
            Message::ToggleForceTag(tag_name) => {
                if let Some(rt) = &mut self.runtime {
                    let cur = rt.read_value(&tag_name).ok().and_then(|v| v.as_bool().ok()).unwrap_or(false);
                    let next = IecValue::Bool(!cur);
                    let _ = rt.force(&tag_name, next.clone());
                    if let Ok(mut snap) = self.shared_io.write() {
                        snap.values.insert(tag_name.clone(), next.clone());
                        snap.request_write(&tag_name, next);
                    }
                }
                Task::none()
            }
            Message::FbdAddBlock(pou_name, kind) => {
                if let Some(PouSource::Fbd(fbd)) = self.project.pous.get_mut(&pou_name) {
                    let next = fbd.blocks.len() + 1;
                    let mut inputs = HashMap::new();
                    inputs.insert("IN1".to_string(), FbdInputSource::Value { expr: "TRUE".to_string() });
                    inputs.insert("IN2".to_string(), FbdInputSource::Value { expr: "TRUE".to_string() });
                    fbd.blocks.push(FbdBlock {
                        id: format!("Block_{next}"),
                        kind,
                        inputs,
                        outputs: HashMap::new(),
                    });
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbdRemoveBlock(pou_name, b_idx) => {
                if let Some(PouSource::Fbd(fbd)) = self.project.pous.get_mut(&pou_name) {
                    delete_fbd_block(fbd, b_idx);
                    self.track_history();
                    self.modified = true;
                }
                Task::none()
            }
            Message::FbdInputSourceChanged(pou_name, b_idx, pin, val_str) => {
                if let Some(PouSource::Fbd(fbd)) = self.project.pous.get_mut(&pou_name)
                    && let Some(blk) = fbd.blocks.get_mut(b_idx) {
                        if val_str.contains('.') {
                            let parts: Vec<&str> = val_str.split('.').collect();
                            if parts.len() == 2 {
                                blk.inputs.insert(pin, FbdInputSource::Link {
                                    from_block: parts[0].trim().to_string(),
                                    from_pin: parts[1].trim().to_string(),
                                });
                            } else {
                                blk.inputs.insert(pin, FbdInputSource::Value { expr: val_str });
                            }
                        } else {
                            blk.inputs.insert(pin, FbdInputSource::Value { expr: val_str });
                        }
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::FbdOutputTargetChanged(pou_name, b_idx, pin, target_str) => {
                if let Some(PouSource::Fbd(fbd)) = self.project.pous.get_mut(&pou_name)
                    && let Some(blk) = fbd.blocks.get_mut(b_idx) {
                        if target_str.trim().is_empty() {
                            blk.outputs.remove(&pin);
                        } else {
                            blk.outputs.insert(pin, target_str.trim().to_string());
                        }
                        self.track_history();
                        self.modified = true;
                    }
                Task::none()
            }
            Message::DigitalTwinToggleElement(id) => {
                if let Some(elem) = self.digital_twin_workspace.elements.iter().find(|e| e.id == id)
                    && let Some(ref binding) = elem.binding {
                        let cur_val = if let Ok(lock) = self.shared_io.read() {
                            lock.values.get(&binding.variable_id).and_then(|v| v.as_bool().ok()).unwrap_or(false)
                        } else {
                            false
                        };
                        let next_val = IecValue::Bool(!cur_val);
                        if let Ok(mut lock) = self.shared_io.write() {
                            lock.request_write(&binding.variable_id, next_val.clone());
                            lock.values.insert(binding.variable_id.clone(), next_val.clone());
                        }
                        if let Some(rt) = &mut self.runtime {
                            let _ = rt.force(&binding.variable_id, next_val);
                        }
                    }
                Task::none()
            }
        }
    }

    pub fn view<'a>(&'a self, window_id: iced::window::Id) -> Element<'a, Message> {
        if Some(window_id) == self.detached_left_id {
            return self.view_detached_left();
        }
        if Some(window_id) == self.detached_right_id {
            return self.view_detached_right();
        }
        if Some(window_id) == self.detached_bottom_id {
            return self.view_detached_bottom();
        }
        self.view_main()
    }

    pub fn view_main(&self) -> Element<'_, Message> {
        let header = self.view_header();

        let mut body_row = row![];

        if self.show_left_explorer && !self.detach_left_explorer {
            body_row = body_row.push(self.view_left_explorer());
        }

        body_row = body_row.push(self.view_center_workspace());

        if self.show_right_io && !self.detach_right_io {
            body_row = body_row.push(self.view_right_io());
        }

        let main_area = container(body_row)
            .width(Length::Fill)
            .height(Length::Fill);

        let mut root_col = column![header, main_area];

        if self.show_bottom_dock && !self.detach_bottom_dock {
            root_col = root_col.push(self.view_bottom_dock());
        }

        let base_content: Element<'_, Message> = container(root_col)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_BASE)),
                ..Default::default()
            })
            .into();

        if self.show_close_confirm_modal {
            let modal_content = container(
                column![
                    text(self.ui_lang.tr("Unsaved Changes", "Modifications non enregistrées")).size(16),
                    text(self.ui_lang.tr(
                        "You have unsaved changes in your project. Do you want to save before closing?",
                        "Vous avez des modifications non enregistrées. Voulez-vous enregistrer avant de quitter ?"
                    ))
                    .size(13)
                    .color(StudioTheme::TEXT_SECONDARY),
                    row![
                        button(text(self.ui_lang.tr("Save & Exit", "Enregistrer & Quitter")).size(12))
                            .on_press(Message::ConfirmCloseSaveAndExit)
                            .style(StudioTheme::primary_button)
                            .padding([6, 12]),
                        button(text(self.ui_lang.tr("Discard", "Ignorer")).size(12))
                            .on_press(Message::ConfirmCloseDiscardAndExit)
                            .style(StudioTheme::danger_button)
                            .padding([6, 12]),
                        button(text(self.ui_lang.tr("Cancel", "Annuler")).size(12))
                            .on_press(Message::ConfirmCloseCancel)
                            .style(StudioTheme::secondary_button)
                            .padding([6, 12]),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                ]
                .spacing(16),
            )
            .padding(20)
            .style(StudioTheme::modal_card_style)
            .width(460);

            return container(
                container(modal_content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
                ..Default::default()
            })
            .into();
        }

        if let Some((ref _pou_name, ref elements)) = self.sfc_delete_modal {
            let step_count = elements.iter().filter(|e| matches!(e, crate::canvas_sfc::SfcElementKey::Step(_))).count();
            let trans_count = elements.iter().filter(|e| matches!(e, crate::canvas_sfc::SfcElementKey::Transition(_))).count();
            let total = elements.len();

            let modal_content = container(
                column![
                    text(self.ui_lang.tr("Delete Selected SFC Elements", "Supprimer les éléments SFC sélectionnés")).size(16),
                    text(format!(
                        "{} {} {} ({} {}, {} {})? {}",
                        self.ui_lang.tr("Are you sure you want to delete", "Voulez-vous vraiment supprimer"),
                        total,
                        self.ui_lang.tr("selected object(s)", "objet(s) sélectionné(s)"),
                        step_count,
                        self.ui_lang.tr("step(s)", "étape(s)"),
                        trans_count,
                        self.ui_lang.tr("transition(s)", "transition(s)"),
                        self.ui_lang.tr("All attached wires and links will be removed.", "Toutes les liaisons associées seront supprimées.")
                    ))
                    .size(13)
                    .color(StudioTheme::TEXT_SECONDARY),
                    row![
                        button(text(self.ui_lang.tr("Delete", "Supprimer")).size(12))
                            .on_press(Message::SfcConfirmDeleteSelected)
                            .style(StudioTheme::danger_button)
                            .padding([6, 12]),
                        button(text(self.ui_lang.tr("Cancel", "Annuler")).size(12))
                            .on_press(Message::SfcCancelDeleteSelected)
                            .style(StudioTheme::secondary_button)
                            .padding([6, 12]),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                ]
                .spacing(16),
            )
            .padding(20)
            .style(StudioTheme::modal_card_style)
            .width(460);

            return container(
                container(modal_content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
                ..Default::default()
            })
            .into();
        }

        if let Some(ref pou_name) = self.confirm_clear_modal {
            let p_name = pou_name.clone();
            let modal_content = container(
                column![
                    text(self.ui_lang.tr("Confirm Clear POU", "Confirmer le vidage du POU")).size(16),
                    text(format!(
                        "{} '{}'?",
                        self.ui_lang.tr("Are you sure you want to clear all contents of", "Voulez-vous vraiment vider tout le contenu de"),
                        p_name
                    ))
                    .size(13)
                    .color(StudioTheme::TEXT_SECONDARY),
                    row![
                        button(text(self.ui_lang.tr("Yes, Clear POU", "Oui, vider le POU")).size(12))
                            .on_press(Message::ExecuteClearPou(p_name))
                            .style(StudioTheme::danger_button)
                            .padding([6, 12]),
                        button(text(self.ui_lang.tr("Cancel", "Annuler")).size(12))
                            .on_press(Message::CancelClearPou)
                            .style(StudioTheme::secondary_button)
                            .padding([6, 12]),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                ]
                .spacing(16),
            )
            .padding(20)
            .style(StudioTheme::modal_card_style)
            .width(420);

            container(
                container(modal_content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x(Length::Fill)
                    .center_y(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
                ..Default::default()
            })
            .into()
        } else {
            base_content
        }
    }

    fn view_header(&self) -> Element<'_, Message> {
        let (scan_count, time_ms, duration_us) = if let Some(ref rt) = self.runtime {
            (rt.scan_count, rt.virtual_time_ms, rt.last_scan_duration_us)
        } else {
            (0, 0, 0)
        };

        let status_dot_color = if self.sim_running {
            StudioTheme::ACCENT_EMERALD
        } else {
            StudioTheme::ACCENT_AMBER
        };

        let telemetry_pill = container(
            row![
                container(iced::widget::Space::with_width(Length::Fixed(8.0))).height(Length::Fixed(8.0)).style(move |_theme: &Theme| container::Style { background: Some(Background::Color(status_dot_color)), border: Border { radius: Radius::from(4.0), ..Default::default() }, ..Default::default() }),
                text(if self.sim_running { "RUN" } else { "PAUSED" })
                    .size(11)
                    .color(status_dot_color),
                text("|").size(11).color(StudioTheme::BORDER_CARD),
                text(format!("#{scan_count}  {time_ms}ms  {duration_us}us"))
                    .size(11)
                    .color(StudioTheme::TEXT_SECONDARY),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([4, 10])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05)));

        let run_btn = button(
            text(if self.sim_running {
                self.ui_lang.tr("Pause", "Pause")
            } else {
                self.ui_lang.tr("Run", "Démarrer")
            })
            .size(12),
        )
        .on_press(Message::ToggleRunSim)
        .style(if self.sim_running {
            StudioTheme::success_button
        } else {
            StudioTheme::primary_button
        })
        .padding([6, 12]);

        let step_btn = button(text(self.ui_lang.tr("Step", "Pas")).size(12))
            .on_press(Message::StepSimTick)
            .style(StudioTheme::secondary_button)
            .padding([6, 12]);

        let reset_btn = button(text(self.ui_lang.tr("Reset", "Réinit")).size(12))
            .on_press(Message::ResetSim)
            .style(StudioTheme::secondary_button)
            .padding([6, 12]);

        let save_btn = button(text(self.ui_lang.tr("Save", "Enregistrer")).size(12))
            .on_press(Message::SaveProject)
            .style(StudioTheme::secondary_button)
            .padding([6, 12]);

        let check_btn = button(text(self.ui_lang.tr("Check", "Vérifier")).size(12))
            .on_press(Message::CheckProject)
            .style(StudioTheme::secondary_button)
            .padding([6, 12]);

        let undo_btn = button(text("< Undo").size(11))
            .on_press_maybe(if self.can_undo() { Some(Message::Undo) } else { None })
            .style(StudioTheme::ghost_button)
            .padding([6, 10]);

        let redo_btn = button(text("Redo >").size(11))
            .on_press_maybe(if self.can_redo() { Some(Message::Redo) } else { None })
            .style(StudioTheme::ghost_button)
            .padding([6, 10]);

        let scenarios_btn = button(text(self.ui_lang.tr("Scenarios", "Scénarios")).size(12))
            .on_press(Message::RunScenarios)
            .style(StudioTheme::secondary_button)
            .padding([6, 12]);

        let lang_btn = button(text(self.ui_lang.code()).size(11))
            .on_press(Message::SetLanguage(match self.ui_lang {
                UiLanguage::En => UiLanguage::Fr,
                UiLanguage::Fr => UiLanguage::En,
            }))
            .style(StudioTheme::ghost_button)
            .padding([6, 10]);

        let explorer_toggle = button(text(if self.show_left_explorer { "<<" } else { ">>" }).size(11))
            .on_press(Message::ToggleLeftExplorer)
            .style(StudioTheme::ghost_button)
            .padding([6, 8]);

        let io_toggle = button(text(if self.show_right_io { ">>" } else { "<<" }).size(11))
            .on_press(Message::ToggleRightIo)
            .style(StudioTheme::ghost_button)
            .padding([6, 8]);

        let dock_toggle = button(text(if self.show_bottom_dock { "v" } else { "^" }).size(11))
            .on_press(Message::ToggleBottomDock)
            .style(StudioTheme::ghost_button)
            .padding([6, 8]);

        container(
            row![
                text("FerritePLC").size(15),
                telemetry_pill,
                iced::widget::Space::with_width(12),
                run_btn,
                step_btn,
                reset_btn,
                iced::widget::Space::with_width(8),
                save_btn,
                check_btn,
                undo_btn,
                redo_btn,
                scenarios_btn,
                iced::widget::Space::with_width(Length::Fill),
                lang_btn,
                explorer_toggle,
                dock_toggle,
                io_toggle,
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([8, 16])
        .style(StudioTheme::header_container_style)
        .width(Length::Fill)
        .into()
    }

    fn view_left_explorer(&self) -> Element<'_, Message> {
        let header = row![
            text(self.ui_lang.tr("EXPLORER", "EXPLORATEUR"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
            iced::widget::Space::with_width(Length::Fill),
            button(text(self.ui_lang.tr("Detach", "Détacher")).size(10.5))
                .on_press(Message::DetachLeftExplorer)
                .style(StudioTheme::ghost_button)
                .padding([1, 4]),
        ]
        .align_y(Alignment::Center);

        container(
            column![
                header,
                self.view_left_explorer_body(),
            ]
            .spacing(10)
            .padding(14),
        )
        .width(260)
        .height(Length::Fill)
        .style(StudioTheme::sidebar_container_style)
        .into()
    }

    fn view_detached_left(&self) -> Element<'_, Message> {
        let header = row![
            text(self.ui_lang.tr("Project Explorer", "Explorateur de Projet")).size(15),
            iced::widget::Space::with_width(Length::Fill),
            button(text(self.ui_lang.tr("Reattach", "Réattacher")).size(11))
                .on_press(Message::DockLeftExplorer)
                .style(StudioTheme::secondary_button)
                .padding([4, 8]),
        ]
        .padding([8, 14])
        .align_y(Alignment::Center);

        let content = container(self.view_left_explorer_body())
            .padding(14)
            .width(Length::Fill)
            .height(Length::Fill);

        container(column![header, content].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_BASE)),
                ..Default::default()
            })
            .into()
    }

    fn view_left_explorer_body(&self) -> Element<'_, Message> {
        let filter_input = text_input(
            self.ui_lang.tr("Filter POUs...", "Filtrer POU..."),
            &self.pou_filter_text,
        )
        .on_input(Message::PouFilterTextChanged)
        .padding(8)
        .style(StudioTheme::styled_input);

        let query = self.pou_filter_text.to_lowercase();
        let mut pou_list = column![].spacing(4);

        for p in &self.project.manifest.pous {
            if !query.is_empty() && !p.name.to_lowercase().contains(&query) {
                continue;
            }
            let is_active = self.active_tab == WorkspaceTab::Pou(p.name.clone());
            let (badge_txt, badge_color) = match p.language {
                LanguageKind::Ld => ("LD", StudioTheme::ACCENT_CYAN),
                LanguageKind::Sfc => ("SFC", StudioTheme::ACCENT_AMBER),
                LanguageKind::St => ("ST", StudioTheme::ACCENT_PURPLE),
                LanguageKind::Fbd => ("FBD", StudioTheme::ACCENT_EMERALD),
            };

            let p_name = p.name.clone();
            let btn = button(
                row![
                    container(text(badge_txt).size(10).color(badge_color))
                        .padding([2, 6])
                        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                    text(&p.name).size(12),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press(Message::SelectTab(WorkspaceTab::Pou(p_name)))
            .style(StudioTheme::tab_button(is_active))
            .width(Length::Fill)
            .padding([6, 10]);

            pou_list = pou_list.push(btn);
        }

        // Add POU Section
        let add_pou_row = row![
            text_input("Name", &self.new_pou_name)
                .on_input(Message::NewPouNameChanged)
                .width(Length::Fill)
                .padding([4, 6])
                .style(StudioTheme::styled_input),
            pick_list(
                &[LanguageKind::Ld, LanguageKind::Sfc, LanguageKind::St, LanguageKind::Fbd][..],
                Some(self.new_pou_lang),
                Message::NewPouLangSelected,
            )
            .width(75)
            .style(StudioTheme::styled_pick_list),
            button(text("+").size(13))
                .on_press(Message::AddPouClicked)
                .style(StudioTheme::primary_button)
                .padding([4, 8]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        // System Configuration Tabs
        let is_sym_active = self.active_tab == WorkspaceTab::SymbolsAndConfig;
        let sym_btn = button(
            row![
                container(text("CFG").size(9).color(StudioTheme::ACCENT_CYAN))
                    .padding([2, 5])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.12))),
                text(self.ui_lang.tr("Symbols & Config", "Symboles & Config")).size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(Message::SelectTab(WorkspaceTab::SymbolsAndConfig))
        .style(StudioTheme::tab_button(is_sym_active))
        .width(Length::Fill)
        .padding([6, 10]);

        let is_twin_active = self.active_tab == WorkspaceTab::DigitalTwin;
        let twin_btn = button(
            row![
                container(text("2D").size(9).color(StudioTheme::ACCENT_EMERALD))
                    .padding([2, 5])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15))),
                text(self.ui_lang.tr("Digital Twin 2D", "Jumeau Numérique 2D")).size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .on_press(Message::SelectTab(WorkspaceTab::DigitalTwin))
        .style(StudioTheme::tab_button(is_twin_active))
        .width(Length::Fill)
        .padding([6, 10]);

        column![
            filter_input,
            scrollable(pou_list).height(Length::Fill),
            text(self.ui_lang.tr("+ Add POU", "+ Ajouter POU"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
            add_pou_row,
            iced::widget::Space::with_height(8),
            sym_btn,
            twin_btn,
        ]
        .spacing(8)
        .into()
    }

    fn view_right_io(&self) -> Element<'_, Message> {
        let header = row![
            text(self.ui_lang.tr("Memory & I/O", "Mémoire & E/S"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
            iced::widget::Space::with_width(Length::Fill),
            button(text(self.ui_lang.tr("Detach", "Détacher")).size(10.5))
                .on_press(Message::DetachRightIo)
                .style(StudioTheme::ghost_button)
                .padding([1, 4]),
        ]
        .align_y(Alignment::Center);

        container(
            column![
                header,
                self.view_right_io_body(),
            ]
            .spacing(10)
            .padding([12, 12]),
        )
        .width(295)
        .height(Length::Fill)
        .style(StudioTheme::sidebar_container_style)
        .into()
    }

    fn view_detached_right(&self) -> Element<'_, Message> {
        let header = row![
            text(self.ui_lang.tr("Virtual I/O Rack & Watch Table", "Baie E/S Virtuelle & Table de Forçage")).size(15),
            iced::widget::Space::with_width(Length::Fill),
            button(text(self.ui_lang.tr("Reattach", "Réattacher")).size(11))
                .on_press(Message::DockRightIo)
                .style(StudioTheme::secondary_button)
                .padding([4, 8]),
        ]
        .padding([8, 14])
        .align_y(Alignment::Center);

        let content = container(self.view_right_io_body())
            .padding(14)
            .width(Length::Fill)
            .height(Length::Fill);

        container(column![header, content].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_BASE)),
                ..Default::default()
            })
            .into()
    }

    fn view_right_io_body(&self) -> Element<'_, Message> {
        let filter_input = text_input(
            self.ui_lang.tr("Filter tags...", "Filtrer variables..."),
            &self.io_filter_text,
        )
        .on_input(Message::IoFilterTextChanged)
        .padding(8)
        .style(StudioTheme::styled_input);

        let area_selectors = row![
            button(text("ALL").size(10))
                .on_press(Message::IoFilterAreaSelected(None))
                .style(StudioTheme::tab_button(self.io_filter_area.is_none() && !self.io_filter_sfc))
                .padding([4, 6]),
            button(text("%I").size(10))
                .on_press(Message::IoFilterAreaSelected(Some(MemoryArea::Input)))
                .style(StudioTheme::tab_button(self.io_filter_area == Some(MemoryArea::Input) && !self.io_filter_sfc))
                .padding([4, 6]),
            button(text("%Q").size(10))
                .on_press(Message::IoFilterAreaSelected(Some(MemoryArea::Output)))
                .style(StudioTheme::tab_button(self.io_filter_area == Some(MemoryArea::Output) && !self.io_filter_sfc))
                .padding([4, 6]),
            button(text("%M").size(10))
                .on_press(Message::IoFilterAreaSelected(Some(MemoryArea::Marker)))
                .style(StudioTheme::tab_button(self.io_filter_area == Some(MemoryArea::Marker) && !self.io_filter_sfc))
                .padding([4, 6]),
            button(text("SFC").size(10))
                .on_press(Message::IoFilterSfcSelected)
                .style(StudioTheme::tab_button(self.io_filter_sfc))
                .padding([4, 6]),
        ]
        .spacing(4);

        let mut tags_list = column![].spacing(4);

        if let Some(ref rt) = self.runtime {
            let query = self.io_filter_text.to_lowercase();
            for tag in rt.tags.tags() {
                if self.io_filter_sfc {
                    let is_sfc_var = tag.name.ends_with(".state") || tag.name.ends_with(".t") || tag.name.contains(".state");
                    if !is_sfc_var {
                        continue;
                    }
                } else if let Some(selected_area) = self.io_filter_area
                    && tag.address.map(|a| a.area) != Some(selected_area) {
                        continue;
                    }
                if !query.is_empty() && !tag.name.to_lowercase().contains(&query) {
                    continue;
                }

                let val = if let Some(addr) = tag.address {
                    rt.process_image.read(addr, tag.data_type).unwrap_or_else(|_| tag.current_value().clone())
                } else {
                    tag.current_value().clone()
                };

                let addr_str = tag.address.map(|a| a.to_string()).unwrap_or_else(|| "-".to_string());
                let t_name = tag.name.clone();

                let interactive_val: Element<'_, Message> = match &val {
                    IecValue::Bool(b) => {
                        button(text(if *b { "TRUE" } else { "FALSE" }).size(10.5))
                            .on_press(Message::ToggleForceTag(t_name))
                            .style(if *b { StudioTheme::success_button } else { StudioTheme::secondary_button })
                            .padding([2, 6])
                            .into()
                    }
                    _ => {
                        text(val.to_string()).size(11).color(StudioTheme::ACCENT_CYAN).into()
                    }
                };

                let tag_row = container(
                    row![
                        column![
                            text(&tag.name).size(11.5),
                            text(addr_str).size(9.5).color(StudioTheme::TEXT_MUTED),
                        ]
                        .spacing(1)
                        .width(Length::Fill),
                        interactive_val,
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([4, 6])
                .style(StudioTheme::card_style)
                .width(Length::Fill);

                tags_list = tags_list.push(tag_row);
            }
        }

        column![
            filter_input,
            area_selectors,
            scrollable(container(tags_list).padding(iced::Padding { top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 }).width(Length::Fill)).height(Length::Fill),
        ]
        .spacing(8)
        .into()
    }

    fn view_center_workspace(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match &self.active_tab {
            WorkspaceTab::Pou(name) => {
                if let Some(pou) = self.project.pous.get(name) {
                    match pou {
                        PouSource::Ld(ld) => render_ld_editor(ld, self.runtime.as_ref(), self.ui_lang),
                        PouSource::Sfc(sfc) => {
                            let sel = self
                                .sfc_selected
                                .as_ref()
                                .and_then(|(p, k)| if p == name { Some(k) } else { None });
                            render_sfc_editor(sfc, self.runtime.as_ref(), self.ui_lang, sel, &self.sfc_selected_elements)
                        }
                        PouSource::St(_) => {
                            if let Some(content) = self.st_contents.get(name) {
                                render_st_editor(name, content, self.runtime.as_ref(), self.ui_lang)
                            } else {
                                container(text("ST source loading...").size(13)).into()
                            }
                        }
                        PouSource::Fbd(fbd) => render_fbd_editor(fbd, self.runtime.as_ref(), self.ui_lang),
                    }
                } else {
                    container(text("POU not found").size(14)).into()
                }
            }
            WorkspaceTab::SymbolsAndConfig => {
                render_symbols_editor(&self.project, self.ui_lang)
            }
            WorkspaceTab::DigitalTwin => {
                let snapshot = if let Ok(lock) = self.shared_io.read() {
                    lock.snapshot()
                } else {
                    Default::default()
                };
                render_digital_twin(&self.digital_twin_workspace, snapshot, self.ui_lang)
            }
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_bottom_dock_tabs(&self, is_detached: bool) -> Element<'_, Message> {
        let detach_btn = if is_detached {
            button(text(self.ui_lang.tr("Reattach", "Réattacher")).size(11))
                .on_press(Message::DockBottomDock)
                .style(StudioTheme::secondary_button)
                .padding([4, 8])
        } else {
            button(text(self.ui_lang.tr("Detach", "Détacher")).size(10.5))
                .on_press(Message::DetachBottomDock)
                .style(StudioTheme::ghost_button)
                .padding([2, 5])
        };

        row![
            button(text(self.ui_lang.tr("Diagnostics", "Diagnostics")).size(11))
                .on_press(Message::SelectBottomTab(BottomDockTab::Diagnostics))
                .style(StudioTheme::tab_button(self.bottom_dock_tab == BottomDockTab::Diagnostics))
                .padding([4, 10]),
            button(text(self.ui_lang.tr("Trend Oscilloscope", "Oscilloscope")).size(11))
                .on_press(Message::SelectBottomTab(BottomDockTab::TrendPlotter))
                .style(StudioTheme::tab_button(self.bottom_dock_tab == BottomDockTab::TrendPlotter))
                .padding([4, 10]),
            button(text(self.ui_lang.tr("ISA-18.2 Alarms", "Alarmes ISA-18.2")).size(11))
                .on_press(Message::SelectBottomTab(BottomDockTab::Alarms))
                .style(StudioTheme::tab_button(self.bottom_dock_tab == BottomDockTab::Alarms))
                .padding([4, 10]),
            button(text("Modbus TCP").size(11))
                .on_press(Message::SelectBottomTab(BottomDockTab::Modbus))
                .style(StudioTheme::tab_button(self.bottom_dock_tab == BottomDockTab::Modbus))
                .padding([4, 10]),
            button(text("GPIO / HAL").size(11))
                .on_press(Message::SelectBottomTab(BottomDockTab::Gpio))
                .style(StudioTheme::tab_button(self.bottom_dock_tab == BottomDockTab::Gpio))
                .padding([4, 10]),
            iced::widget::Space::with_width(Length::Fill),
            detach_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    }

    fn view_bottom_dock(&self) -> Element<'_, Message> {
        let tabs = self.view_bottom_dock_tabs(false);
        container(
            column![
                tabs,
                self.view_bottom_dock_content(),
            ]
            .spacing(10)
            .padding([8, 16]),
        )
        .style(StudioTheme::header_container_style)
        .width(Length::Fill)
        .height(180)
        .into()
    }

    fn view_detached_bottom(&self) -> Element<'_, Message> {
        let tabs = self.view_bottom_dock_tabs(true);
        let content = container(self.view_bottom_dock_content())
            .padding([8, 16])
            .width(Length::Fill)
            .height(Length::Fill);

        container(
            column![
                tabs,
                content,
            ]
            .spacing(10)
            .padding([8, 16]),
        )
        .style(move |_| container::Style {
            background: Some(Background::Color(StudioTheme::BG_BASE)),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn view_bottom_dock_content(&self) -> Element<'_, Message> {
        match self.bottom_dock_tab {
            BottomDockTab::Diagnostics => {
                let mut diag_list = column![].spacing(6);
                if self.diagnostics.is_empty() && self.scenario_logs.is_empty() {
                    diag_list = diag_list.push(
                        text(self.ui_lang.tr("No syntax or type errors. Zero combinational loops.", "Aucune erreur de syntaxe ou de type. Zéro boucle combinatoire."))
                            .size(12)
                            .color(StudioTheme::ACCENT_EMERALD),
                    );
                } else {
                    for d in &self.diagnostics {
                        let row_item = row![
                            container(text(format!("{:?}", d.severity)).size(10).color(StudioTheme::ACCENT_ROSE))
                                .padding([2, 6])
                                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.957, 0.247, 0.369, 0.15))),
                            text(&d.message).size(12),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center);
                        diag_list = diag_list.push(row_item);
                    }
                    for log in &self.scenario_logs {
                        diag_list = diag_list.push(
                            text(log).size(12).color(if log.contains("[PASS]") || log.contains("[SUCCÈS]") {
                                StudioTheme::ACCENT_EMERALD
                            } else {
                                StudioTheme::ACCENT_ROSE
                            }),
                        );
                    }
                }
                scrollable(diag_list).height(120).into()
            }
            BottomDockTab::TrendPlotter => {
                let toolbar = row![
                    button(text(if self.trend_plotter.paused {
                        self.ui_lang.tr("Resume", "Reprendre")
                    } else {
                        self.ui_lang.tr("Pause", "Pause")
                    }).size(11))
                    .on_press(Message::TrendTogglePause)
                    .style(StudioTheme::secondary_button)
                    .padding([4, 8]),
                    button(text(self.ui_lang.tr("Clear", "Effacer")).size(11))
                        .on_press(Message::TrendClear)
                        .style(StudioTheme::secondary_button)
                        .padding([4, 8]),
                ]
                .spacing(8);

                let canvas = iced::widget::canvas::Canvas::new(TrendCanvas {
                    channels: &self.trend_plotter.channels,
                    time_window_s: self.trend_plotter.time_window_s,
                    start_time: self.trend_plotter.start_time,
                })
                .width(Length::Fill)
                .height(130);

                column![toolbar, canvas].spacing(8).into()
            }
            BottomDockTab::Alarms => {
                render_alarms_panel(self.runtime.as_ref(), self.ui_lang)
            }
            BottomDockTab::Modbus => {
                let status_str = if self.modbus_server.is_some() {
                    format!("[RUNNING] on port {}", self.modbus_port)
                } else {
                    "[STOPPED]".to_string()
                };

                let res_log = self.modbus_last_test_result.as_deref().unwrap_or("No tests executed yet.");

                row![
                    column![
                        text(format!("Modbus TCP Server ({status_str})")).size(12).color(StudioTheme::ACCENT_CYAN),
                        row![
                            text("Port:").size(12),
                            text_input("5020", &self.modbus_port.to_string())
                                .on_input(Message::ModbusPortChanged)
                                .width(70)
                                .padding([4, 6])
                                .style(StudioTheme::styled_input),
                            button(text(if self.modbus_server.is_some() { "Stop Server" } else { "Start Server" }).size(11))
                                .on_press(Message::ToggleModbusServer)
                                .style(if self.modbus_server.is_some() { StudioTheme::danger_button } else { StudioTheme::success_button })
                                .padding([4, 10]),
                            button(text("Test Ping").size(11))
                                .on_press(Message::TestModbusPing)
                                .style(StudioTheme::secondary_button)
                                .padding([4, 10]),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    ]
                    .spacing(8)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Modbus Log:").size(11).color(StudioTheme::TEXT_MUTED),
                        text(res_log).size(12).color(StudioTheme::TEXT_SECONDARY),
                    ]
                    .spacing(6)
                    .width(Length::FillPortion(1)),
                ]
                .spacing(16)
                .into()
            }
            BottomDockTab::Gpio => {
                let hal_status = if self.hal_enabled { "ENABLED" } else { "DISABLED" };
                row![
                    button(text(format!("HAL Driver ({hal_status})")).size(11))
                        .on_press(Message::ToggleHalEnabled)
                        .style(if self.hal_enabled { StudioTheme::success_button } else { StudioTheme::secondary_button })
                        .padding([4, 10]),
                    button(text("Toggle Mock BCM 17 (IX0.0)").size(11))
                        .on_press(Message::MockGpioPinToggled(17))
                        .style(StudioTheme::secondary_button)
                        .padding([4, 10]),
                ]
                .spacing(12)
                .into()
            }
        }
    }
}
