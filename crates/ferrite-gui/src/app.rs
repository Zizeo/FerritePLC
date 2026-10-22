use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Stroke, Visuals};
use ferrite_core::{IecType, IecValue, LanguageKind};
use ferrite_ir::ValidationDiagnostic;
use ferrite_project::{PouSource, Project};
use ferrite_runtime::PlcRuntime;

use crate::canvas_fbd::render_fbd_editor;
use crate::canvas_ld::render_ld_editor;
use crate::canvas_sfc::{SfcStepRef, render_sfc_editor};
use crate::editor_st::render_st_editor;
use crate::editor_symbols::render_symbols_editor;
use crate::i18n::UiLanguage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceTab {
    Pou(String),
    SymbolsAndConfig,
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
            UiTheme::Dark => Visuals::dark(),
            UiTheme::Light => Visuals::light(),
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
    pub show_close_confirm_modal: bool,
    pub allowed_to_close: bool,
    pub undo_stack: Vec<Project>,
    pub redo_stack: Vec<Project>,
    last_committed_project: Project,
    last_edit_time: Option<Instant>,
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
            show_close_confirm_modal: false,
            allowed_to_close: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit_time: None,
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

    fn trigger_single_step(&mut self) {
        let lang = self.ui_lang;
        if self.modified {
            self.recompile_and_reset();
            self.modified = false;
        }
        if let Some(ref mut rt) = self.runtime {
            match rt.step_scan() {
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
                    self.status_banner =
                        format!("{}: {e}", lang.tr("Scan error", "Erreur de cycle"));
                }
            }
        }
    }

    pub fn render_left_explorer_body(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        ui.horizontal(|ui| {
            if ui
                .small_button(lang.tr("📂 Open...", "📂 Ouvrir..."))
                .on_hover_text(lang.tr(
                    "Open project folder via OS file explorer",
                    "Ouvrir un dossier projet via l'explorateur de fichiers OS",
                ))
                .clicked()
            {
                self.open_project_folder_dialog();
            }
            if ui
                .small_button(lang.tr("💾 Save", "💾 Enregistrer"))
                .on_hover_text(lang.tr(
                    "Save project to current folder",
                    "Enregistrer le projet dans le dossier actuel",
                ))
                .clicked()
            {
                self.save_project();
            }
            if ui
                .small_button(lang.tr("💾 Save As...", "💾 Enregistrer sous..."))
                .on_hover_text(lang.tr(
                    "Save project to another folder via OS file explorer",
                    "Enregistrer le projet dans un autre dossier via l'explorateur OS",
                ))
                .clicked()
            {
                self.save_project_as_dialog();
            }
        });
        ui.separator();

        let is_sym_tab = self.active_tab == WorkspaceTab::SymbolsAndConfig;
        if ui
            .selectable_label(
                is_sym_tab,
                RichText::new(lang.tr(
                    "Global Tags & FB Registry",
                    "Variables Globales & Blocs FB",
                ))
                .strong(),
            )
            .clicked()
        {
            self.active_tab = WorkspaceTab::SymbolsAndConfig;
        }

        ui.separator();
        ui.strong(lang.tr(
            "Program Organization Units (POUs):",
            "Unités d'Organisation de Programme (POU) :",
        ));
        let entries = self.project.manifest.pous.clone();
        for entry in entries {
            let selected = self.active_tab == WorkspaceTab::Pou(entry.name.clone());
            let badge_col = match entry.language {
                LanguageKind::Ld => Color32::from_rgb(86, 182, 194),
                LanguageKind::Sfc => Color32::from_rgb(229, 192, 123),
                LanguageKind::St => Color32::from_rgb(198, 120, 221),
                LanguageKind::Fbd => Color32::from_rgb(97, 175, 239),
            };
            ui.horizontal(|ui| {
                ui.colored_label(
                    badge_col,
                    RichText::new(format!("[{}]", entry.language)).monospace(),
                );
                if ui.selectable_label(selected, &entry.name).clicked() {
                    self.active_tab = WorkspaceTab::Pou(entry.name.clone());
                }
            });
        }

        ui.separator();
        ui.strong(lang.tr("Create New POU:", "Créer un Nouveau POU :"));
        ui.add(
            egui::TextEdit::singleline(&mut self.new_pou_name)
                .hint_text(lang.tr("POU name...", "Nom du POU...")),
        );
        ui.horizontal(|ui| {
            for l in [
                LanguageKind::Ld,
                LanguageKind::Sfc,
                LanguageKind::St,
                LanguageKind::Fbd,
            ] {
                ui.selectable_value(&mut self.new_pou_lang, l, l.to_string());
            }
        });
        if ui
            .button(lang.tr("+ Create POU", "+ Créer POU"))
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
    }

    pub fn render_right_io_body(&mut self, ui: &mut egui::Ui) {
        let lang = self.ui_lang;
        if let Some(ref mut rt) = self.runtime {
            egui::ScrollArea::vertical()
                .id_salt("right_watch_scroll")
                .show(ui, |ui| {
                    ui.strong(lang.tr(
                        "Live Process Image & Symbolic Tags:",
                        "Mémoire Image Temps Réel & Variables :",
                    ));
                    let tag_snapshots: Vec<_> = rt.tags.tags().to_vec();
                    let mut pending_stimulus: Option<(String, IecValue)> = None;
                    let mut pending_force: Option<(String, IecValue)> = None;
                    let mut pending_unforce: Option<String> = None;

                    egui::Grid::new("live_io_watch_grid")
                        .striped(true)
                        .min_col_width(55.0)
                        .show(ui, |ui| {
                            ui.strong(lang.tr("Symbol", "Symbole"));
                            ui.strong(lang.tr("Addr", "Adr"));
                            ui.strong(lang.tr(
                                "Live Value / Control",
                                "Valeur / Commande",
                            ));
                            ui.strong(lang.tr("Force", "Forçage"));
                            ui.end_row();

                            for tag in &tag_snapshots {
                                ui.monospace(&tag.name);
                                let addr_str = tag
                                    .address
                                    .map(|a| a.to_string())
                                    .unwrap_or_else(|| "-".to_string());
                                ui.monospace(addr_str);

                                let current_val = rt
                                    .read_value(&tag.name)
                                    .unwrap_or_else(|_| tag.initial_value.clone());

                                let is_sfc_state = tag.name.ends_with(".state");
                                if tag.read_only_outside_sfc && !is_sfc_state {
                                    let col = match current_val {
                                        IecValue::Bool(true) => {
                                            Color32::from_rgb(46, 213, 115)
                                        }
                                        _ => Color32::from_rgb(175, 190, 210),
                                    };
                                    ui.colored_label(col, current_val.to_string());
                                } else {
                                    match tag.data_type {
                                        IecType::Bool => {
                                            let b =
                                                current_val.as_bool().unwrap_or(false);
                                            let btn_col = if b {
                                                Color32::from_rgb(46, 213, 115)
                                            } else {
                                                Color32::from_rgb(70, 80, 95)
                                            };
                                            let btn_txt = if b {
                                                lang.tr("TRUE (ON)", "VRAI (ON)")
                                            } else {
                                                lang.tr("FALSE", "FAUX")
                                            };
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(btn_txt)
                                                            .color(Color32::WHITE),
                                                    )
                                                    .fill(btn_col),
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
                                            let mut v =
                                                current_val.as_int().unwrap_or(0);
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
                                            let mut v =
                                                current_val.as_dint().unwrap_or(0);
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
                                            let mut v =
                                                current_val.as_real().unwrap_or(0.0);
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
                                            .color(Color32::from_rgb(255, 184, 77)),
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
                    ui.strong(lang.tr(
                        "Shared Function Block Instances:",
                        "Instances de Blocs Fonctionnels Partagés :",
                    ));
                    for fb in rt.tags.fbs_mut() {
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(26, 32, 42))
                            .stroke(Stroke::new(
                                1.0_f32,
                                Color32::from_rgb(55, 65, 85),
                            ))
                            .inner_margin(6.0)
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(&fb.name);
                                    ui.monospace(format!("({})", fb.kind.label()));
                                    let q_col = if fb.q {
                                        Color32::from_rgb(46, 213, 115)
                                    } else {
                                        Color32::GRAY
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
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label("Kp:");
                                        if ui
                                            .add(
                                                egui::DragValue::new(&mut fb.kp)
                                                    .speed(0.1)
                                                    .range(-10000.0..=10000.0),
                                            )
                                            .changed()
                                            && let Some(p_fb) = self
                                                .project
                                                .fb_instances
                                                .instances
                                                .iter_mut()
                                                .find(|f| f.name == fb.name)
                                        {
                                            p_fb.kp = fb.kp;
                                        }
                                        if fb.kind.has_integral() {
                                            ui.label("Ti(s):");
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut fb.ti_s)
                                                        .speed(0.1)
                                                        .range(0.001..=10000.0),
                                                )
                                                .changed()
                                                && let Some(p_fb) = self
                                                    .project
                                                    .fb_instances
                                                    .instances
                                                    .iter_mut()
                                                    .find(|f| f.name == fb.name)
                                            {
                                                p_fb.ti_s = fb.ti_s;
                                            }
                                        }
                                        if fb.kind.has_derivative() {
                                            ui.label("Td(s):");
                                            if ui
                                                .add(
                                                    egui::DragValue::new(&mut fb.td_s)
                                                        .speed(0.05)
                                                        .range(0.0..=10000.0),
                                                )
                                                .changed()
                                                && let Some(p_fb) = self
                                                    .project
                                                    .fb_instances
                                                    .instances
                                                    .iter_mut()
                                                    .find(|f| f.name == fb.name)
                                            {
                                                p_fb.td_s = fb.td_s;
                                            }
                                        }
                                        if ui
                                            .small_button(lang.tr("Reset ∫", "RAZ ∫"))
                                            .clicked()
                                        {
                                            fb.reset_corrector();
                                        }
                                    });
                                } else {
                                    let ratio = if fb.pt_ms > 0 {
                                        (fb.et_ms as f32 / fb.pt_ms as f32).clamp(0.0, 1.0)
                                    } else {
                                        0.0
                                    };
                                    ui.add(
                                        egui::ProgressBar::new(ratio).text(format!(
                                            "ET: {} ms / PT: {} ms | CV: {}",
                                            fb.et_ms, fb.pt_ms, fb.cv
                                        )),
                                    );
                                }
                            });
                        ui.add_space(4.0);
                    }
                });
        } else {
            ui.colored_label(
                Color32::from_rgb(255, 107, 107),
                lang.tr(
                    "PLC Runtime offline due to validation error. Fix diagnostics and click 'Check & Compile'.",
                    "Exécuteur API hors ligne suite à une erreur de validation. Corrigez les erreurs puis cliquez sur 'Vérifier & Compiler'.",
                ),
            );
        }
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
                if let Some(ref mut rt) = self.runtime
                    && let Err(err) = rt.step_scan()
                {
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
            .exact_height(34.0)
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    // Quick toggle button for Left Project Explorer
                    let left_toggle_txt = if self.show_left_explorer {
                        lang.tr("< Explorer", "< Explorateur")
                    } else {
                        lang.tr("> Explorer", "> Explorateur")
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

                    // Collapsible FILE / PROJECT Menu
                    ui.menu_button(lang.tr("File & Project v", "Fichier & Projet v"), |ui| {
                        ui.set_min_width(260.0);
                        ui.strong(lang.tr("Project Management", "Gestion du Projet"));
                        if ui
                            .button(lang.tr("💾 Save Project", "💾 Enregistrer le Projet"))
                            .clicked()
                        {
                            self.save_project();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "💾 Save Project As... (OS Explorer)",
                                "💾 Enregistrer sous... (Explorateur OS)",
                            ))
                            .clicked()
                        {
                            self.save_project_as_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "📂 Open Project Folder... (OS Explorer)",
                                "📂 Ouvrir Dossier Projet... (Explorateur OS)",
                            ))
                            .clicked()
                        {
                            self.open_project_folder_dialog();
                            ui.close_menu();
                        }
                        if ui
                            .button(lang.tr(
                                "📄 Open FerriteProject.fplc... (OS Explorer)",
                                "📄 Ouvrir FerriteProject.fplc... (Explorateur OS)",
                            ))
                            .clicked()
                        {
                            self.open_project_manifest_dialog();
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
                        ui.separator();
                        ui.label(lang.tr("Project Folder Path:", "Chemin du Dossier Projet :"));
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.open_path_input)
                                    .hint_text(lang.tr("Project folder path", "Chemin du projet"))
                                    .desired_width(185.0),
                            );
                            if ui
                                .small_button("📂")
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
                    ui.menu_button(lang.tr("PLC & Simulation v", "API & Simulation v"), |ui| {
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
                    ui.menu_button(lang.tr("Panels v", "Panneaux v"), |ui| {
                        ui.set_min_width(220.0);
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

                    ui.separator();

                    // Collapsible OPTIONS Menu (UI Language + Theme)
                    ui.menu_button(lang.tr("Options v", "Options v"), |ui| {
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
                            lang.tr("🌙 Dark", "🌙 Sombre"),
                        );
                        ui.selectable_value(
                            &mut self.ui_theme,
                            UiTheme::Light,
                            lang.tr("☀ Light", "☀ Clair"),
                        );
                    });

                    ui.separator();

                    // Project Title + Live Status Badge + Telemetry
                    ui.label(
                        RichText::new(format!("FerritePLC — {}", self.project.manifest.name))
                            .strong()
                            .color(Color32::from_rgb(72, 202, 228)),
                    );
                    if self.modified {
                        ui.colored_label(
                            Color32::from_rgb(255, 184, 77),
                            lang.tr("[Modified]", "[Modifié]"),
                        );
                    }

                    let status_color = if self.sim_running {
                        Color32::from_rgb(46, 213, 115)
                    } else {
                        Color32::from_rgb(255, 184, 77)
                    };
                    let status_text = if self.sim_running {
                        lang.tr("[RUNNING]", "[EN MARCHE]")
                    } else {
                        lang.tr("[PAUSED]", "[EN PAUSE]")
                    };
                    if ui
                        .add(egui::Button::new(
                            RichText::new(status_text).color(status_color).strong(),
                        ))
                        .on_hover_text(lang.tr(
                            "Click to toggle PLC Run / Pause",
                            "Cliquer pour Démarrer / Mettre en Pause l'API",
                        ))
                        .clicked()
                    {
                        if !self.sim_running && self.modified {
                            self.recompile_and_reset();
                            self.modified = false;
                        }
                        self.sim_running = !self.sim_running;
                        self.last_tick = Instant::now();
                    }

                    if let Some(ref rt) = self.runtime {
                        ui.monospace(format!(
                            "#{} | {}ms | {}us",
                            rt.scan_count, rt.virtual_time_ms, rt.last_scan_duration_us,
                        ));
                    }

                    // Right-aligned quick toggle button for Virtual I/O Panel
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let right_toggle_txt = if self.show_right_io {
                            lang.tr("Virtual I/O >", "Baie E/S >")
                        } else {
                            lang.tr("< Virtual I/O", "< Baie E/S")
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

        // 2. Collapsible Bottom Diagnostics & Scenario Dock
        if self.show_bottom_diagnostics {
            egui::TopBottomPanel::bottom("bottom_diagnostics_dock")
                .resizable(true)
                .default_height(125.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(lang.tr("Status:", "État :"));
                        ui.label(&self.status_banner);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .small_button(lang.tr("Hide", "Masquer"))
                                    .clicked()
                                {
                                    self.show_bottom_diagnostics = false;
                                }
                            },
                        );
                    });
                    ui.separator();

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
                                            self.active_tab = WorkspaceTab::Pou(diag.pou.clone());
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
                });
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

        // 5. Center Main Editor / Visual Canvas Area
        egui::CentralPanel::default().show(ctx, |ui| match &self.active_tab.clone() {
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
        });

        // Snapshot any project edit made this frame into the undo history
        self.track_history();
    }
}
