use std::collections::HashSet;

use egui::{Color32, FontId, Id, Order, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use ferrite_lang_sfc::{
    ActionQualifier, ConvergenceKind, DivergenceKind, SfcAction, SfcProgram, SfcStep,
    SfcTransition, StepKind,
};
use ferrite_runtime::PlcRuntime;

use crate::i18n::UiLanguage;

const COLOR_ACTIVE_STEP: Color32 = Color32::from_rgb(46, 213, 115);
const COLOR_FORCED_STEP: Color32 = Color32::from_rgb(255, 107, 107);
const COLOR_INACTIVE_STEP: Color32 = Color32::from_rgb(120, 135, 160);
const COLOR_STEP_DROP: Color32 = Color32::from_rgb(72, 202, 228);
const COLOR_SELECTED: Color32 = Color32::from_rgb(0, 229, 255);
const COLOR_TRANSITION: Color32 = Color32::from_rgb(255, 184, 77);
const COLOR_OR_BRANCH: Color32 = Color32::from_rgb(198, 120, 221);
const COLOR_ACTION_BG: Color32 = Color32::from_rgb(30, 38, 52);

/// Persistent selection & confirmation modal state for the SFC editor.
#[derive(Debug, Clone, Default)]
struct SfcSelectionState {
    selected_steps: HashSet<usize>,
    selected_transitions: HashSet<usize>,
    marquee_start: Option<Pos2>,
    confirm_delete_multi: bool,
    confirm_clear_pou: bool,
    active_step_edit: Option<usize>,
    active_trans_edit: Option<usize>,
}

impl SfcSelectionState {
    fn total_selected(&self) -> usize {
        self.selected_steps.len() + self.selected_transitions.len()
    }

    fn clear_selection(&mut self) {
        self.selected_steps.clear();
        self.selected_transitions.clear();
        self.active_step_edit = None;
        self.active_trans_edit = None;
    }
}

fn delete_selected_sfc_items(
    program: &mut SfcProgram,
    sel: &mut SfcSelectionState,
    modified: &mut bool,
) {
    let mut tr_indices: Vec<usize> = sel.selected_transitions.iter().copied().collect();
    tr_indices.sort_unstable_by(|a, b| b.cmp(a));
    for t_idx in tr_indices {
        if t_idx < program.transitions.len() {
            program.transitions.remove(t_idx);
            *modified = true;
        }
    }

    let mut step_indices: Vec<usize> = sel.selected_steps.iter().copied().collect();
    step_indices.sort_unstable_by(|a, b| b.cmp(a));
    for s_idx in step_indices {
        if s_idx < program.steps.len() {
            program.steps.remove(s_idx);
            *modified = true;
        }
    }

    sel.clear_selection();
}

/// Descriptor for an SFC step across any SFC POU in the project (used for cross-SFC transition & action pickers).
#[derive(Debug, Clone)]
pub struct SfcStepRef {
    pub pou_name: String,
    pub step_id: String,
    pub step_name: String,
    pub state_sym: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SfcDragItem {
    Step(usize),
    Transition(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SfcDropAction {
    MoveStep { from_idx: usize, to_idx: usize },
    MoveTransition { from_idx: usize, to_slot_idx: usize },
}

fn step_state_symbols(pou_name: &str, step_id: &str) -> (String, String, String, String) {
    let prefix = if step_id.starts_with('X') || step_id.starts_with('x') {
        step_id.to_string()
    } else {
        format!("X{step_id}")
    };
    let short_state = format!("{prefix}.state");
    let short_time = format!("{prefix}.t");
    let qual_state = if pou_name.is_empty() {
        short_state.clone()
    } else {
        format!("{pou_name}.{short_state}")
    };
    let qual_time = if pou_name.is_empty() {
        short_time.clone()
    } else {
        format!("{pou_name}.{short_time}")
    };
    (qual_state, short_state, qual_time, short_time)
}

/// Renders the visual Grafcet (IEC 60848 SFC) canvas with:
/// - AND (`===`) & OR (`---`) Divergence and Convergence builder & 2D parallel branch rendering
/// - Cross-SFC active state transition picker (`🔗 + SFC State ▾`)
/// - Cross-SFC & live Set / Force step state controls (`S`, `R`, `F`, `FZ` + live Set/Force buttons)
/// - Drag-and-drop reordering of Steps and Transitions
pub fn render_sfc_editor(
    ui: &mut Ui,
    program: &mut SfcProgram,
    runtime: Option<&mut PlcRuntime>,
    _all_sfc_steps: &[SfcStepRef],
    modified: &mut bool,
    lang: UiLanguage,
) {
    let sel_id = Id::new(("sfc_selection_state", program.name.clone()));
    let mut sel_state: SfcSelectionState = ui
        .ctx()
        .data_mut(|d| d.get_temp(sel_id))
        .unwrap_or_default();

    // Prune stale indices
    sel_state
        .selected_steps
        .retain(|&s_idx| s_idx < program.steps.len());
    sel_state
        .selected_transitions
        .retain(|&t_idx| t_idx < program.transitions.len());

    let total_selected = sel_state.total_selected();

    // Handle Delete / Suppr key when no text input has keyboard focus
    let delete_pressed = !ui.ctx().wants_keyboard_input()
        && ui.input(|i| i.key_pressed(egui::Key::Delete));
    if delete_pressed && total_selected > 0 {
        if total_selected == 1 {
            delete_selected_sfc_items(program, &mut sel_state, modified);
        } else {
            sel_state.confirm_delete_multi = true;
        }
    }

    ui.horizontal_wrapped(|ui| {
        ui.heading(format!(
            "{} — {}",
            lang.tr("Grafcet / SFC", "Grafcet / SFC (CEI 60848)"),
            program.name
        ));
        ui.separator();
        if ui
            .button(lang.tr("+ Add Step", "+ Ajouter Étape"))
            .clicked()
        {
            let next_id = program.steps.len().to_string();
            program.steps.push(SfcStep {
                id: next_id.clone(),
                name: format!("Step_{next_id}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            *modified = true;
        }
        if ui
            .button(lang.tr("+ Add Transition", "+ Ajouter Transition"))
            .clicked()
        {
            let next_t = program.transitions.len() + 1;
            let from = program
                .steps
                .first()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "0".to_string());
            let to = program
                .steps
                .last()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "0".to_string());
            program.transitions.push(SfcTransition {
                id: format!("T{next_t}"),
                from_steps: vec![from],
                to_steps: vec![to],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
            });
            *modified = true;
        }

        if ui
            .button(
                RichText::new(lang.tr("Clear POU", "Vider le POU"))
                    .color(Color32::from_rgb(255, 107, 107)),
            )
            .on_hover_text(lang.tr(
                "Clear all steps and transitions in this SFC POU (asks confirmation)",
                "Supprimer toutes les étapes et transitions de ce POU SFC (avec confirmation)",
            ))
            .clicked()
        {
            sel_state.confirm_clear_pou = true;
        }

        if sel_state.total_selected() > 0 {
            let count = sel_state.total_selected();
            let del_lbl = match lang {
                UiLanguage::En => format!("Delete Selected ({count}) [Del]"),
                UiLanguage::Fr => format!("Supprimer sélection ({count}) [Suppr]"),
            };
            if ui
                .button(RichText::new(del_lbl).color(Color32::from_rgb(255, 107, 107)))
                .clicked()
            {
                if count == 1 {
                    delete_selected_sfc_items(program, &mut sel_state, modified);
                } else {
                    sel_state.confirm_delete_multi = true;
                }
            }
            if ui
                .small_button(lang.tr("Clear Selection", "Désélectionner"))
                .clicked()
            {
                sel_state.clear_selection();
            }
        }

        ui.separator();

        // Quick Topology Builders for AND / OR Divergence & Convergence
        if ui
            .button(lang.tr("+ AND Divergence (==)", "+ Divergence ET (==)"))
            .on_hover_text(lang.tr(
                "Split the last step into 2 simultaneous parallel branches (IEC 60848 double line ===)",
                "Diviser la dernière étape en 2 branches parallèles simultanées (double barre CEI 60848 ===)",
            ))
            .clicked()
        {
            let src_id = program
                .steps
                .last()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "0".to_string());
            let id_a = program.steps.len().to_string();
            let id_b = (program.steps.len() + 1).to_string();
            program.steps.push(SfcStep {
                id: id_a.clone(),
                name: format!("Branch_{id_a}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            program.steps.push(SfcStep {
                id: id_b.clone(),
                name: format!("Branch_{id_b}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            let next_t = program.transitions.len() + 1;
            program.transitions.push(SfcTransition {
                id: format!("T{next_t}_AndDiv"),
                from_steps: vec![src_id],
                to_steps: vec![id_a, id_b],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
            });
            *modified = true;
        }

        if ui
            .button(lang.tr("+ AND Convergence (==)", "+ Convergence ET (==)"))
            .on_hover_text(lang.tr(
                "Synchronize the last 2 parallel steps into a new step when BOTH are active (===)",
                "Synchroniser les 2 dernières étapes parallèles vers une nouvelle étape quand TOUTES sont actives (===)",
            ))
            .clicked()
            && program.steps.len() >= 2
        {
            let len = program.steps.len();
            let from_a = program.steps[len - 2].id.clone();
            let from_b = program.steps[len - 1].id.clone();
            let join_id = len.to_string();
            program.steps.push(SfcStep {
                id: join_id.clone(),
                name: format!("Step_Join_{join_id}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            let next_t = program.transitions.len() + 1;
            program.transitions.push(SfcTransition {
                id: format!("T{next_t}_AndConv"),
                from_steps: vec![from_a, from_b],
                to_steps: vec![join_id],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
            });
            *modified = true;
        }

        if ui
            .button(lang.tr("+ OR Divergence (--)", "+ Divergence OU (--)"))
            .on_hover_text(lang.tr(
                "Create an exclusive selection split from the last step into 2 alternative branches (---)",
                "Créer un aiguillage exclusif (OU) depuis la dernière étape vers 2 branches alternatives (---)",
            ))
            .clicked()
        {
            let src_id = program
                .steps
                .last()
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "0".to_string());
            let id_a = program.steps.len().to_string();
            let id_b = (program.steps.len() + 1).to_string();
            program.steps.push(SfcStep {
                id: id_a.clone(),
                name: format!("Alt_{id_a}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            program.steps.push(SfcStep {
                id: id_b.clone(),
                name: format!("Alt_{id_b}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            let t1 = program.transitions.len() + 1;
            let t2 = t1 + 1;
            program.transitions.push(SfcTransition {
                id: format!("T{t1}_OrA"),
                from_steps: vec![src_id.clone()],
                to_steps: vec![id_a],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::Or,
            });
            program.transitions.push(SfcTransition {
                id: format!("T{t2}_OrB"),
                from_steps: vec![src_id],
                to_steps: vec![id_b],
                receptivity: "FALSE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::Or,
            });
            *modified = true;
        }

        if ui
            .button(lang.tr("+ OR Convergence (--)", "+ Convergence OU (--)"))
            .on_hover_text(lang.tr(
                "Merge the last 2 alternative steps into a new step when EITHER step is active (---)",
                "Regrouper les 2 dernières étapes alternatives vers une nouvelle étape dès que L'UNE est active (---)",
            ))
            .clicked()
            && program.steps.len() >= 2
        {
            let len = program.steps.len();
            let from_a = program.steps[len - 2].id.clone();
            let from_b = program.steps[len - 1].id.clone();
            let join_id = len.to_string();
            program.steps.push(SfcStep {
                id: join_id.clone(),
                name: format!("Step_Merge_{join_id}"),
                kind: StepKind::Normal,
                actions: vec![],
            });
            let next_t = program.transitions.len() + 1;
            program.transitions.push(SfcTransition {
                id: format!("T{next_t}_OrConv"),
                from_steps: vec![from_a, from_b],
                to_steps: vec![join_id],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::Or,
                divergence: DivergenceKind::And,
            });
            *modified = true;
        }
    });
    ui.label(
        RichText::new(lang.tr(
            "Tip: Drag on empty canvas to box-select multiple steps/transitions | Click to select | Del/Suppr to delete | Drag a step/transition to reorder",
            "Astuce : Glisser sur le fond pour sélectionner plusieurs étapes/transitions | Cliquer pour sélectionner | Suppr pour supprimer | Glisser une étape/transition pour déplacer",
        ))
        .small()
        .color(Color32::from_rgb(145, 160, 185)),
    );
    ui.add_space(4.0);

    // Confirmation popup when deleting multiple selected Steps / Transitions
    if sel_state.confirm_delete_multi {
        let count = sel_state.total_selected();
        let step_cnt = sel_state.selected_steps.len();
        let tr_cnt = sel_state.selected_transitions.len();
        egui::Window::new(lang.tr(
            "Confirm Multiple Deletion",
            "Confirmer la suppression multiple",
        ))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            ui.add_space(4.0);
            let msg = match lang {
                UiLanguage::En => format!(
                    "Are you sure you want to delete the {count} selected item(s) ({step_cnt} step(s), {tr_cnt} transition(s))?"
                ),
                UiLanguage::Fr => format!(
                    "Voulez-vous vraiment supprimer les {count} élément(s) sélectionné(s) ({step_cnt} étape(s), {tr_cnt} transition(s)) ?"
                ),
            };
            ui.label(msg);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new(lang.tr(
                            "Delete All Selected",
                            "Supprimer la sélection",
                        ))
                        .color(Color32::from_rgb(255, 107, 107))
                        .strong(),
                    )
                    .clicked()
                {
                    delete_selected_sfc_items(program, &mut sel_state, modified);
                    sel_state.confirm_delete_multi = false;
                }
                if ui.button(lang.tr("Cancel", "Annuler")).clicked() {
                    sel_state.confirm_delete_multi = false;
                }
            });
        });
    }

    // Confirmation popup to Clear the entire SFC POU
    if sel_state.confirm_clear_pou {
        egui::Window::new(lang.tr(
            "Confirm Clear POU",
            "Confirmer le vidage du POU",
        ))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            ui.add_space(4.0);
            let msg = match lang {
                UiLanguage::En => format!(
                    "Are you sure you want to clear the entire POU '{}' ({} step(s), {} transition(s))?",
                    program.name,
                    program.steps.len(),
                    program.transitions.len()
                ),
                UiLanguage::Fr => format!(
                    "Voulez-vous vraiment vider tout le POU '{}' ({} étape(s), {} transition(s)) ?",
                    program.name,
                    program.steps.len(),
                    program.transitions.len()
                ),
            };
            ui.label(msg);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new(lang.tr("Yes, Clear POU", "Oui, vider le POU"))
                            .color(Color32::from_rgb(255, 107, 107))
                            .strong(),
                    )
                    .clicked()
                {
                    program.steps.clear();
                    program.transitions.clear();
                    sel_state.clear_selection();
                    sel_state.confirm_clear_pou = false;
                    *modified = true;
                }
                if ui.button(lang.tr("Cancel", "Annuler")).clicked() {
                    sel_state.confirm_clear_pou = false;
                }
            });
        });
    }

    egui::ScrollArea::vertical()
        .id_salt("sfc_scroll_area")
        .show(ui, |ui| {
            // Visual Grafcet Sequence Canvas with direct inline editing on Steps, Actions & Transitions
            draw_sfc_canvas(
                ui,
                program,
                runtime.as_deref(),
                modified,
                lang,
                &mut sel_state,
            );
        });

    ui.ctx().data_mut(|d| d.insert_temp(sel_id, sel_state));
}

/// Computes visual rows of step indices so that parallel branches created by AND Divergence
/// (`to_steps.len() > 1`) or OR Divergence (multiple transitions sharing `from_steps`) render side-by-side.
fn compute_sfc_step_rows(program: &SfcProgram) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut placed: HashSet<usize> = HashSet::new();

    let mut idx = 0;
    while idx < program.steps.len() {
        if placed.contains(&idx) {
            idx += 1;
            continue;
        }
        // Check if any transition diverges into `program.steps[idx]` along with sibling steps
        let step_id = &program.steps[idx].id;
        let mut sibling_row: Vec<usize> = Vec::new();

        // Case A: Single transition with multiple `to_steps` (AND divergence) containing `step_id`
        if let Some(and_tr) = program
            .transitions
            .iter()
            .find(|t| t.to_steps.len() > 1 && t.to_steps.contains(step_id))
        {
            for tid in &and_tr.to_steps {
                if let Some(s_pos) = program.steps.iter().position(|s| &s.id == tid)
                    && !placed.contains(&s_pos)
                    && !sibling_row.contains(&s_pos)
                {
                    sibling_row.push(s_pos);
                }
            }
        }

        // Case B: Multiple transitions sharing the same `from_steps` (OR divergence) targeting `step_id`
        if sibling_row.len() <= 1
            && let Some(parent_tr) = program
                .transitions
                .iter()
                .find(|t| t.to_steps.contains(step_id) && !t.from_steps.is_empty())
        {
            let shared_from = &parent_tr.from_steps;
            let or_siblings: Vec<&SfcTransition> = program
                .transitions
                .iter()
                .filter(|t| &t.from_steps == shared_from)
                .collect();
            if or_siblings.len() > 1 {
                sibling_row.clear();
                for otr in or_siblings {
                    for tid in &otr.to_steps {
                        if let Some(s_pos) = program.steps.iter().position(|s| &s.id == tid)
                            && !placed.contains(&s_pos)
                            && !sibling_row.contains(&s_pos)
                        {
                            sibling_row.push(s_pos);
                        }
                    }
                }
            }
        }

        if sibling_row.is_empty() {
            sibling_row.push(idx);
        }
        for &s_pos in &sibling_row {
            placed.insert(s_pos);
        }
        rows.push(sibling_row);
        idx += 1;
    }

    rows
}

fn step_box_width(step: &SfcStep) -> f32 {
    let name_len = step.name.chars().count() as f32;
    let id_len = (step.id.chars().count() + 12) as f32;
    (name_len.max(id_len) * 7.6 + 44.0).max(188.0)
}

fn step_action_width(step: &SfcStep) -> f32 {
    step.actions
        .iter()
        .map(|act| {
            let has_dur = matches!(act.qualifier, ActionQualifier::L | ActionQualifier::D);
            let target_len = act.target.chars().count() as f32;
            let base = if has_dur { 165.0 } else { 105.0 };
            (base + target_len * 7.4).max(245.0)
        })
        .fold(0.0_f32, f32::max)
}

fn row_column_stride(program: &SfcProgram, row: &[usize], outgoing_trs: &[usize]) -> f32 {
    let mut max_col_w: f32 = 360.0;
    for &idx in row {
        if let Some(step) = program.steps.get(idx) {
            let sw = step_box_width(step);
            let aw = if step.actions.is_empty() {
                0.0
            } else {
                24.0 + step_action_width(step)
            };
            max_col_w = max_col_w.max(sw + aw + 76.0);
        }
    }
    for &t_idx in outgoing_trs {
        if let Some(tr) = program.transitions.get(t_idx) {
            let line1_len =
                tr.id.chars().count() + tr.from_steps.join(",").len() + tr.to_steps.join(",").len() + 12;
            let line2_len = tr.receptivity.chars().count() + 6;
            let pill_w = (line1_len.max(line2_len) as f32 * 7.4 + 28.0).max(200.0);
            max_col_w = max_col_w.max(pill_w + 96.0);
        }
    }
    max_col_w
}

fn row_vertical_span(program: &SfcProgram, row: &[usize], outgoing_trs: &[usize]) -> f32 {
    let step_h = 68.0_f32;
    let max_acts = row
        .iter()
        .filter_map(|&i| program.steps.get(i))
        .map(|s| s.actions.len())
        .max()
        .unwrap_or(0);
    let actions_h = (max_acts as f32) * 30.0;
    let step_zone_h = step_h.max(actions_h);

    let has_and_div = outgoing_trs
        .first()
        .and_then(|&ti| program.transitions.get(ti))
        .is_some_and(|tr| tr.to_steps.len() > 1);
    if has_and_div {
        step_zone_h + 148.0
    } else {
        step_zone_h + 124.0
    }
}

fn draw_sfc_canvas(
    ui: &mut Ui,
    program: &mut SfcProgram,
    runtime: Option<&PlcRuntime>,
    modified: &mut bool,
    lang: UiLanguage,
    sel_state: &mut SfcSelectionState,
) {
    let rows = compute_sfc_step_rows(program);

    // Pre-compute unified column stride & required canvas dimensions so all rows align and never overlap
    let mut temp_drawn: HashSet<usize> = HashSet::new();
    let mut total_height = 60.0_f32;
    let mut global_col_stride = 360.0_f32;
    let mut max_branch_cols = 1usize;
    let mut row_outgoing_trs: Vec<Vec<usize>> = Vec::with_capacity(rows.len());

    for row in &rows {
        let row_step_ids: Vec<String> = row
            .iter()
            .map(|&s_idx| program.steps[s_idx].id.clone())
            .collect();
        let outgoing_trs: Vec<usize> = program
            .transitions
            .iter()
            .enumerate()
            .filter(|(t_idx, tr)| {
                !temp_drawn.contains(t_idx)
                    && tr.from_steps.iter().any(|fs| row_step_ids.contains(fs))
            })
            .map(|(t_idx, _)| t_idx)
            .collect();
        for &ti in &outgoing_trs {
            temp_drawn.insert(ti);
        }
        let stride = row_column_stride(program, row, &outgoing_trs);
        global_col_stride = global_col_stride.max(stride);
        let branch_cols = row.len().max(outgoing_trs.len()).max(1);
        max_branch_cols = max_branch_cols.max(branch_cols);
        total_height += row_vertical_span(program, row, &outgoing_trs);
        row_outgoing_trs.push(outgoing_trs);
    }

    let min_canvas_width =
        (240.0 + (max_branch_cols as f32) * global_col_stride + 140.0).max(860.0);

    egui::ScrollArea::horizontal()
        .id_salt(("sfc_canvas_hscroll", program.name.clone()))
        .show(ui, |ui| {
            let desired_width = ui.available_width().max(min_canvas_width);
            let canvas_height = total_height.max(180.0);
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(desired_width, canvas_height),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);

            painter.rect_filled(rect, 6.0, Color32::from_rgb(16, 20, 26));

            let dnd_id = Id::new("sfc_canvas_dnd_state");
            let mut active_drag: Option<SfcDragItem> = ui.ctx().data_mut(|d| d.get_temp(dnd_id));
            let mut completed_drop: Option<SfcDropAction> = None;

            let pointer_pos = ui.ctx().pointer_interact_pos();
            let pointer_released = ui.input(|i| i.pointer.any_released());
            let press_origin = ui.input(|i| i.pointer.press_origin());
            let modifiers = ui.input(|i| i.modifiers);

            let half_max_span =
                ((max_branch_cols.saturating_sub(1)) as f32) * global_col_stride * 0.5;
            let base_spine_x = rect.left() + 210.0 + half_max_span;

            // Pre-compute exact horizontal center `cx` and row index for every step ID
            // so transitions and AND/OR divergences connect directly to their target steps!
            let mut step_layout_map: std::collections::HashMap<String, (usize, f32)> =
                std::collections::HashMap::new();
            for (r_idx, row) in rows.iter().enumerate() {
                let cols = row.len();
                let row_start_x =
                    base_spine_x - ((cols.saturating_sub(1)) as f32) * global_col_stride * 0.5;
                for (c_idx, &s_idx) in row.iter().enumerate() {
                    let cx = row_start_x + (c_idx as f32) * global_col_stride;
                    step_layout_map.insert(program.steps[s_idx].id.clone(), (r_idx, cx));
                }
            }

            let mut cur_y = rect.top() + 26.0;
            let mut step_hit_rects: Vec<(usize, Rect)> = Vec::new();
            let mut tr_hit_rects: Vec<(usize, Rect)> = Vec::new();
            let mut inline_widget_rects: Vec<Rect> = Vec::new();

            for (row_idx, row) in rows.iter().enumerate() {
                let cols = row.len();
                let outgoing_trs = &row_outgoing_trs[row_idx];
                let row_start_x =
                    base_spine_x - ((cols.saturating_sub(1)) as f32) * global_col_stride * 0.5;

                let mut step_centers_x: Vec<f32> = Vec::with_capacity(cols);
                let step_h = 68.0_f32;
                let mut max_actions_in_row = 0usize;

                for (col_idx, &idx) in row.iter().enumerate() {
                    let step_w = step_box_width(&program.steps[idx]);
                    let act_w = step_action_width(&program.steps[idx]);
                    max_actions_in_row = max_actions_in_row.max(program.steps[idx].actions.len());
                    let cx = row_start_x + (col_idx as f32) * global_col_stride;
                    step_centers_x.push(cx);

                    let step_id_str = program.steps[idx].id.clone();
                    let step_kind = program.steps[idx].kind;
                    let (qual_state, short_state, qual_time, short_time) =
                        step_state_symbols(&program.name, &step_id_str);

                    let (is_active, is_forced, elapsed_str) = if let Some(rt) = runtime {
                        let st_sym = if rt.tags.get_tag(&qual_state).is_some() {
                            &qual_state
                        } else {
                            &short_state
                        };
                        let tm_sym = if rt.tags.get_tag(&qual_time).is_some() {
                            &qual_time
                        } else {
                            &short_time
                        };
                        let act = rt
                            .read_value(st_sym)
                            .ok()
                            .and_then(|v| v.as_bool().ok())
                            .unwrap_or(false);
                        let forced = rt
                            .tags
                            .get_tag(st_sym)
                            .and_then(|t| t.forced.as_ref())
                            .is_some();
                        let el = rt
                            .read_value(tm_sym)
                            .ok()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "T#0ms".to_string());
                        (act, forced, el)
                    } else {
                        (false, false, "T#0ms".to_string())
                    };

                    let step_rect = Rect::from_min_size(
                        Pos2::new(cx - step_w * 0.5, cur_y),
                        Vec2::new(step_w, step_h),
                    );
                    let step_header_rect = Rect::from_min_size(
                        step_rect.min,
                        Vec2::new(step_w, 32.0),
                    );
                    let add_act_btn_rect = Rect::from_min_size(
                        Pos2::new(step_rect.right() - 48.0, step_rect.top() + 5.0),
                        Vec2::new(42.0, 22.0),
                    );
                    let step_name_edit_rect = Rect::from_min_max(
                        Pos2::new(step_rect.left() + 6.0, step_rect.top() + 35.0),
                        Pos2::new(step_rect.right() - 6.0, step_rect.bottom() - 6.0),
                    );
                    step_hit_rects.push((idx, step_header_rect));
                    inline_widget_rects.push(step_name_edit_rect);
                    inline_widget_rects.push(add_act_btn_rect);

                    // Start dragging this Step if drag started inside `step_header_rect` (and not on +Act button)
                    if response.drag_started()
                        && let Some(origin) = press_origin
                        && step_header_rect.contains(origin)
                        && !add_act_btn_rect.contains(origin)
                    {
                        active_drag = Some(SfcDragItem::Step(idx));
                    }

                    let step_slot_hovered = matches!(active_drag, Some(SfcDragItem::Step(_)))
                        && pointer_pos.is_some_and(|p| step_rect.expand(6.0).contains(p));

                    if let Some(SfcDragItem::Step(from_idx)) = active_drag {
                        let slot_rect = step_rect.expand(5.0);
                        let fill = if step_slot_hovered {
                            Color32::from_rgba_unmultiplied(72, 202, 228, 55)
                        } else {
                            Color32::from_rgba_unmultiplied(72, 202, 228, 18)
                        };
                        painter.rect(
                            slot_rect,
                            6.0,
                            fill,
                            Stroke::new(
                                if step_slot_hovered { 2.8_f32 } else { 1.4_f32 },
                                COLOR_STEP_DROP,
                            ),
                            StrokeKind::Inside,
                        );
                        if step_slot_hovered && pointer_released && from_idx != idx {
                            completed_drop = Some(SfcDropAction::MoveStep {
                                from_idx,
                                to_idx: idx,
                            });
                        }
                    }

                    let is_step_selected = sel_state.selected_steps.contains(&idx);
                    if is_step_selected {
                        painter.rect(
                            step_rect.expand(4.0),
                            6.0,
                            Color32::from_rgba_unmultiplied(0, 229, 255, 35),
                            Stroke::new(2.2_f32, COLOR_SELECTED),
                            StrokeKind::Inside,
                        );
                    }

                    let border_col = if step_slot_hovered {
                        COLOR_STEP_DROP
                    } else if is_step_selected {
                        COLOR_SELECTED
                    } else if is_forced {
                        COLOR_FORCED_STEP
                    } else if is_active {
                        COLOR_ACTIVE_STEP
                    } else {
                        COLOR_INACTIVE_STEP
                    };

                    painter.rect(
                        step_rect,
                        4.0,
                        Color32::from_rgb(26, 34, 46),
                        Stroke::new(2.2_f32, border_col),
                        StrokeKind::Inside,
                    );
                    if step_kind == StepKind::Initial {
                        painter.rect_stroke(
                            step_rect.shrink(4.0),
                            2.0,
                            Stroke::new(1.2_f32, border_col),
                            StrokeKind::Inside,
                        );
                    }

                    if is_active {
                        painter.circle_filled(
                            Pos2::new(step_rect.left() + 13.0, step_rect.top() + 16.0),
                            4.5,
                            COLOR_ACTIVE_STEP,
                        );
                        painter.text(
                            Pos2::new(step_rect.left() - 8.0, step_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            elapsed_str,
                            FontId::monospace(10.5),
                            COLOR_ACTIVE_STEP,
                        );
                    }
                    if is_forced {
                        painter.text(
                            Pos2::new(step_rect.left() + 25.0, step_rect.top() + 16.0),
                            egui::Align2::LEFT_CENTER,
                            "F!",
                            FontId::monospace(10.0),
                            COLOR_FORCED_STEP,
                        );
                    }

                    painter.text(
                        Pos2::new(step_rect.center().x - 16.0, step_rect.top() + 16.0),
                        egui::Align2::CENTER_CENTER,
                        format!("X{step_id_str}"),
                        FontId::monospace(13.0),
                        Color32::WHITE,
                    );

                    // Direct "+Act" button right on the step header with plenty of vertical clearance
                    ui.allocate_new_ui(
                        egui::UiBuilder::new().max_rect(add_act_btn_rect),
                        |ui| {
                            ui.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
                            if ui
                                .add_sized(
                                    add_act_btn_rect.size(),
                                    egui::Button::new(RichText::new("+Act").size(11.0)),
                                )
                                .on_hover_text(lang.tr(
                                    "Add action directly to this step",
                                    "Ajouter une action directement à cette étape",
                                ))
                                .clicked()
                            {
                                program.steps[idx].actions.push(SfcAction {
                                    qualifier: ActionQualifier::N,
                                    target: "Out_Coil".to_string(),
                                    duration: None,
                                });
                                *modified = true;
                                sel_state.active_step_edit = Some(idx);
                            }
                        },
                    );

                    // Direct inline Step Name input inside the step box
                    ui.allocate_new_ui(
                        egui::UiBuilder::new().max_rect(step_name_edit_rect),
                        |ui| {
                            let resp = ui.add_sized(
                                step_name_edit_rect.size(),
                                egui::TextEdit::singleline(&mut program.steps[idx].name)
                                    .font(FontId::monospace(11.0))
                                    .hint_text(lang.tr("Step Name", "Nom d'étape")),
                            );
                            if resp.changed() {
                                *modified = true;
                            }
                            if resp.gained_focus() || resp.clicked() {
                                sel_state.active_step_edit = Some(idx);
                                sel_state.active_trans_edit = None;
                            }
                        },
                    );

                    // Draw & edit associated actions directly to the right of every step
                    if !program.steps[idx].actions.is_empty() {
                        let act_x = step_rect.right() + 20.0;
                        painter.line_segment(
                            [
                                Pos2::new(step_rect.right(), cur_y + 16.0),
                                Pos2::new(act_x, cur_y + 16.0),
                            ],
                            Stroke::new(2.0_f32, border_col),
                        );

                        let mut remove_act_idx: Option<usize> = None;
                        let act_len = program.steps[idx].actions.len();
                        for a_idx in 0..act_len {
                            let ay = cur_y + (a_idx as f32) * 30.0;
                            let act_rect =
                                Rect::from_min_size(Pos2::new(act_x, ay), Vec2::new(act_w, 26.0));
                            inline_widget_rects.push(act_rect);

                            painter.rect(
                                act_rect,
                                3.0,
                                COLOR_ACTION_BG,
                                Stroke::new(1.2_f32, border_col),
                                StrokeKind::Inside,
                            );

                            let inner_act_rect = act_rect.shrink2(Vec2::new(4.0, 2.0));
                            ui.allocate_new_ui(
                                egui::UiBuilder::new().max_rect(inner_act_rect),
                                |ui| {
                                    ui.horizontal_centered(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        let act = &mut program.steps[idx].actions[a_idx];
                                        egui::ComboBox::from_id_salt((
                                            "canvas_inline_act_qual",
                                            idx,
                                            a_idx,
                                        ))
                                        .width(44.0)
                                        .selected_text(format!("{:?}", act.qualifier))
                                        .show_ui(ui, |ui| {
                                            for q in [
                                                ActionQualifier::N,
                                                ActionQualifier::S,
                                                ActionQualifier::R,
                                                ActionQualifier::F,
                                                ActionQualifier::FZ,
                                                ActionQualifier::L,
                                                ActionQualifier::D,
                                                ActionQualifier::P,
                                            ] {
                                                if ui
                                                    .selectable_value(
                                                        &mut act.qualifier,
                                                        q,
                                                        format!("{q:?}"),
                                                    )
                                                    .changed()
                                                {
                                                    if matches!(
                                                        q,
                                                        ActionQualifier::L | ActionQualifier::D
                                                    ) && act.duration.is_none()
                                                    {
                                                        act.duration = Some("T#2s".to_string());
                                                    }
                                                    *modified = true;
                                                }
                                            }
                                        });

                                        if matches!(
                                            act.qualifier,
                                            ActionQualifier::L | ActionQualifier::D
                                        ) {
                                            let mut dur = act
                                                .duration
                                                .clone()
                                                .unwrap_or_else(|| "T#2s".to_string());
                                            if ui
                                                .add(
                                                    egui::TextEdit::singleline(&mut dur)
                                                        .desired_width(48.0)
                                                        .font(FontId::monospace(10.5)),
                                                )
                                                .changed()
                                            {
                                                act.duration = Some(dur);
                                                *modified = true;
                                            }
                                        }

                                        let target_w = (inner_act_rect.width()
                                            - if matches!(
                                                act.qualifier,
                                                ActionQualifier::L | ActionQualifier::D
                                            ) {
                                                132.0
                                            } else {
                                                78.0
                                            })
                                        .max(70.0);
                                        let t_resp = ui.add(
                                            egui::TextEdit::singleline(&mut act.target)
                                                .desired_width(target_w)
                                                .font(FontId::monospace(11.0))
                                                .hint_text(lang.tr("Action target", "Action cible")),
                                        );
                                        if t_resp.changed() {
                                            *modified = true;
                                        }
                                        if t_resp.gained_focus() || t_resp.clicked() {
                                            sel_state.active_step_edit = Some(idx);
                                            sel_state.active_trans_edit = None;
                                        }

                                        if ui
                                            .small_button("x")
                                            .on_hover_text(lang.tr(
                                                "Delete action",
                                                "Supprimer l'action",
                                            ))
                                            .clicked()
                                        {
                                            remove_act_idx = Some(a_idx);
                                            *modified = true;
                                        }
                                    });
                                },
                            );
                        }
                        if let Some(rm_i) = remove_act_idx {
                            program.steps[idx].actions.remove(rm_i);
                        }
                    }
                }

                let step_bottom_y = cur_y + step_h;
                let step_zone_bottom_y =
                    step_bottom_y.max(cur_y + (max_actions_in_row as f32) * 30.0);
                let bar_y = step_zone_bottom_y + 56.0;
                let row_span = row_vertical_span(program, row, outgoing_trs);
                let next_y = cur_y + row_span;

                if outgoing_trs.len() > 1 {
                    // OR Divergence: Multiple outgoing transitions from this row
                    let split_y = step_zone_bottom_y + 18.0;

                    // Draw vertical links from each step in the row down to the OR split bar
                    for &cx in &step_centers_x {
                        painter.line_segment(
                            [Pos2::new(cx, step_bottom_y), Pos2::new(cx, split_y)],
                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                        );
                    }

                    // Determine horizontal branch X for each outgoing transition (aligned with its target step if in a later row)
                    let tr_count = outgoing_trs.len();
                    let default_or_start_x = base_spine_x
                        - ((tr_count.saturating_sub(1)) as f32) * global_col_stride * 0.5;
                    let mut branch_xs: Vec<f32> = Vec::with_capacity(tr_count);
                    for (b_i, &t_idx) in outgoing_trs.iter().enumerate() {
                        let target_cx = program.transitions[t_idx]
                            .to_steps
                            .first()
                            .and_then(|tid| step_layout_map.get(tid))
                            .filter(|&&(r_i, _)| r_i > row_idx)
                            .map(|&(_, cx)| cx)
                            .unwrap_or(default_or_start_x + (b_i as f32) * global_col_stride);
                        branch_xs.push(target_cx);
                    }

                    let mut left_x = step_centers_x
                        .iter()
                        .chain(branch_xs.iter())
                        .copied()
                        .fold(base_spine_x, f32::min);
                    let mut right_x = step_centers_x
                        .iter()
                        .chain(branch_xs.iter())
                        .copied()
                        .fold(base_spine_x, f32::max);
                    if (right_x - left_x).abs() < 4.0 {
                        left_x -= 20.0;
                        right_x += 20.0;
                    }

                    // Single horizontal line (IEC 60848 OR Divergence `---`)
                    painter.line_segment(
                        [Pos2::new(left_x, split_y), Pos2::new(right_x, split_y)],
                        Stroke::new(2.2_f32, COLOR_OR_BRANCH),
                    );
                    painter.text(
                        Pos2::new(left_x - 12.0, split_y),
                        egui::Align2::RIGHT_CENTER,
                        "[OR Div --]",
                        FontId::monospace(10.5),
                        COLOR_OR_BRANCH,
                    );

                    for (b_i, &t_idx) in outgoing_trs.iter().enumerate() {
                        let bx = branch_xs[b_i];
                        // Vertical link from OR split bar down to transition crossbar
                        painter.line_segment(
                            [Pos2::new(bx, split_y), Pos2::new(bx, bar_y)],
                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                        );

                        // Link from transition crossbar down to target step(s)
                        for tid in &program.transitions[t_idx].to_steps {
                            if let Some(&(target_row, target_cx)) = step_layout_map.get(tid) {
                                if target_row > row_idx {
                                    if (target_cx - bx).abs() < 1.5 {
                                        painter.line_segment(
                                            [Pos2::new(bx, bar_y), Pos2::new(target_cx, next_y)],
                                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                        );
                                    } else {
                                        let route_y = next_y - 14.0;
                                        painter.line_segment(
                                            [Pos2::new(bx, bar_y), Pos2::new(bx, route_y)],
                                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                        );
                                        painter.line_segment(
                                            [
                                                Pos2::new(bx, route_y),
                                                Pos2::new(target_cx, route_y),
                                            ],
                                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                        );
                                        painter.line_segment(
                                            [
                                                Pos2::new(target_cx, route_y),
                                                Pos2::new(target_cx, next_y),
                                            ],
                                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                        );
                                    }
                                } else {
                                    painter.line_segment(
                                        [Pos2::new(bx, bar_y), Pos2::new(bx, bar_y + 28.0)],
                                        Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                    );
                                    painter.text(
                                        Pos2::new(bx, bar_y + 38.0),
                                        egui::Align2::CENTER_CENTER,
                                        format!("↩ X{tid}"),
                                        FontId::monospace(10.5),
                                        COLOR_STEP_DROP,
                                    );
                                }
                            }
                        }

                        let header_line = {
                            let tr = &program.transitions[t_idx];
                            format!(
                                "{} (X{} -> X{})",
                                tr.id,
                                tr.from_steps.join(","),
                                tr.to_steps.join(",")
                            )
                        };
                        let cond_len = program.transitions[t_idx].receptivity.chars().count();
                        let box_w = (header_line.chars().count().max(cond_len + 4) as f32 * 7.4
                            + 24.0)
                            .max(185.0);
                        let pill_rect = Rect::from_min_size(
                            Pos2::new(bx + 22.0, bar_y - 23.0),
                            Vec2::new(box_w, 46.0),
                        );
                        let pill_header_rect = Rect::from_min_size(
                            pill_rect.min,
                            Vec2::new(box_w, 19.0),
                        );
                        let pill_edit_rect = Rect::from_min_max(
                            Pos2::new(pill_rect.left() + 5.0, pill_rect.top() + 20.0),
                            Pos2::new(pill_rect.right() - 5.0, pill_rect.bottom() - 4.0),
                        );
                        let tr_hit_rect = Rect::from_min_max(
                            Pos2::new(bx - 24.0, bar_y - 23.0),
                            pill_header_rect.max,
                        );
                        tr_hit_rects.push((t_idx, tr_hit_rect));
                        inline_widget_rects.push(pill_edit_rect);

                        if response.drag_started()
                            && let Some(origin) = press_origin
                            && tr_hit_rect.contains(origin)
                        {
                            active_drag = Some(SfcDragItem::Transition(t_idx));
                        }

                        let is_tr_selected = sel_state.selected_transitions.contains(&t_idx);
                        let tr_col = if is_tr_selected {
                            COLOR_SELECTED
                        } else {
                            COLOR_TRANSITION
                        };

                        painter.line_segment(
                            [Pos2::new(bx - 18.0, bar_y), Pos2::new(bx + 18.0, bar_y)],
                            Stroke::new(4.0_f32, tr_col),
                        );
                        painter.rect(
                            pill_rect,
                            4.0,
                            if is_tr_selected {
                                Color32::from_rgba_unmultiplied(0, 229, 255, 35)
                            } else {
                                Color32::from_rgb(22, 28, 38)
                            },
                            Stroke::new(if is_tr_selected { 2.0_f32 } else { 1.0_f32 }, tr_col),
                            StrokeKind::Inside,
                        );
                        painter.text(
                            Pos2::new(pill_rect.left() + 8.0, pill_rect.top() + 10.0),
                            egui::Align2::LEFT_CENTER,
                            header_line,
                            FontId::monospace(10.5),
                            tr_col,
                        );

                        // Direct inline receptivity input inside transition pill
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(pill_edit_rect),
                            |ui| {
                                let resp = ui.add_sized(
                                    pill_edit_rect.size(),
                                    egui::TextEdit::singleline(
                                        &mut program.transitions[t_idx].receptivity,
                                    )
                                    .font(FontId::monospace(10.5))
                                    .hint_text(lang.tr("Receptivity", "Réceptivité")),
                                );
                                if resp.changed() {
                                    *modified = true;
                                }
                                if resp.gained_focus() || resp.clicked() {
                                    sel_state.active_trans_edit = Some(t_idx);
                                    sel_state.active_step_edit = None;
                                }
                            },
                        );
                    }
                } else if let Some(&t_idx) = outgoing_trs.first() {
                    let from_len = program.transitions[t_idx].from_steps.len();
                    let to_len = program.transitions[t_idx].to_steps.len();
                    let conv_kind = program.transitions[t_idx].convergence;
                    let header_line = {
                        let tr = &program.transitions[t_idx];
                        let from_label = tr.from_steps.join(",");
                        let to_label = tr.to_steps.join(",");
                        format!("{}  (X{from_label} -> X{to_label})", tr.id)
                    };
                    let cond_len = program.transitions[t_idx].receptivity.chars().count();

                    let min_x = step_centers_x.first().copied().unwrap_or(base_spine_x);
                    let max_x = step_centers_x.last().copied().unwrap_or(base_spine_x);
                    let mid_x = (min_x + max_x) * 0.5;

                    // 1. Links from current row's steps down to the transition crossbar at `(mid_x, bar_y)`
                    if from_len > 1 {
                        let conv_y = step_zone_bottom_y + 20.0;
                        for &cx in &step_centers_x {
                            painter.line_segment(
                                [Pos2::new(cx, step_bottom_y), Pos2::new(cx, conv_y)],
                                Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                            );
                        }
                        let left_bar = min_x - 24.0;
                        let right_bar = max_x + 24.0;
                        match conv_kind {
                            ConvergenceKind::And => {
                                painter.line_segment(
                                    [
                                        Pos2::new(left_bar, conv_y - 2.5),
                                        Pos2::new(right_bar, conv_y - 2.5),
                                    ],
                                    Stroke::new(2.2_f32, COLOR_STEP_DROP),
                                );
                                painter.line_segment(
                                    [
                                        Pos2::new(left_bar, conv_y + 2.5),
                                        Pos2::new(right_bar, conv_y + 2.5),
                                    ],
                                    Stroke::new(2.2_f32, COLOR_STEP_DROP),
                                );
                                painter.text(
                                    Pos2::new(left_bar - 10.0, conv_y),
                                    egui::Align2::RIGHT_CENTER,
                                    "[AND Conv ==]",
                                    FontId::monospace(10.5),
                                    COLOR_STEP_DROP,
                                );
                            }
                            ConvergenceKind::Or => {
                                painter.line_segment(
                                    [Pos2::new(left_bar, conv_y), Pos2::new(right_bar, conv_y)],
                                    Stroke::new(2.2_f32, COLOR_OR_BRANCH),
                                );
                                painter.text(
                                    Pos2::new(left_bar - 10.0, conv_y),
                                    egui::Align2::RIGHT_CENTER,
                                    "[OR Conv --]",
                                    FontId::monospace(10.5),
                                    COLOR_OR_BRANCH,
                                );
                            }
                        }
                        painter.line_segment(
                            [Pos2::new(mid_x, conv_y + 2.5), Pos2::new(mid_x, bar_y)],
                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                        );
                    } else {
                        for &cx in &step_centers_x {
                            painter.line_segment(
                                [Pos2::new(cx, step_bottom_y), Pos2::new(cx, bar_y)],
                                Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                            );
                        }
                    }

                    // 2. Links from transition crossbar `(mid_x, bar_y)` down to target step(s)
                    if to_len > 1 {
                        // AND Divergence (`to_steps.len() > 1`): draw IEC 60848 double line `===` below transition
                        // and connect each target step's exact column X straight down to `next_y`!
                        let div_y = bar_y + 36.0;
                        painter.line_segment(
                            [Pos2::new(mid_x, bar_y), Pos2::new(mid_x, div_y - 2.5)],
                            Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                        );

                        let mut target_xs: Vec<f32> = program.transitions[t_idx]
                            .to_steps
                            .iter()
                            .filter_map(|tid| step_layout_map.get(tid).map(|&(_, cx)| cx))
                            .collect();
                        if target_xs.is_empty() {
                            let start = base_spine_x
                                - ((to_len.saturating_sub(1)) as f32) * global_col_stride * 0.5;
                            for i in 0..to_len {
                                target_xs.push(start + (i as f32) * global_col_stride);
                            }
                        }
                        let min_tx = target_xs.iter().copied().fold(mid_x, f32::min);
                        let max_tx = target_xs.iter().copied().fold(mid_x, f32::max);
                        let left_bar = min_tx - 24.0;
                        let right_bar = max_tx + 24.0;

                        painter.line_segment(
                            [Pos2::new(left_bar, div_y - 2.5), Pos2::new(right_bar, div_y - 2.5)],
                            Stroke::new(2.2_f32, COLOR_STEP_DROP),
                        );
                        painter.line_segment(
                            [Pos2::new(left_bar, div_y + 2.5), Pos2::new(right_bar, div_y + 2.5)],
                            Stroke::new(2.2_f32, COLOR_STEP_DROP),
                        );
                        painter.text(
                            Pos2::new(left_bar - 10.0, div_y),
                            egui::Align2::RIGHT_CENTER,
                            "[AND Div ==]",
                            FontId::monospace(10.5),
                            COLOR_STEP_DROP,
                        );

                        for tx in target_xs {
                            painter.line_segment(
                                [Pos2::new(tx, div_y + 2.5), Pos2::new(tx, next_y)],
                                Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                            );
                        }
                    } else if let Some(tid) = program.transitions[t_idx].to_steps.first() {
                        if let Some(&(target_row, target_cx)) = step_layout_map.get(tid) {
                            if target_row > row_idx {
                                if (target_cx - mid_x).abs() < 1.5 {
                                    painter.line_segment(
                                        [Pos2::new(mid_x, bar_y), Pos2::new(target_cx, next_y)],
                                        Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                    );
                                } else {
                                    let route_y = next_y - 16.0;
                                    painter.line_segment(
                                        [Pos2::new(mid_x, bar_y), Pos2::new(mid_x, route_y)],
                                        Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                    );
                                    painter.line_segment(
                                        [
                                            Pos2::new(mid_x, route_y),
                                            Pos2::new(target_cx, route_y),
                                        ],
                                        Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                    );
                                    painter.line_segment(
                                        [
                                            Pos2::new(target_cx, route_y),
                                            Pos2::new(target_cx, next_y),
                                        ],
                                        Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                    );
                                }
                            } else {
                                // Loop-back transition (e.g. X6 -> X0)
                                painter.line_segment(
                                    [Pos2::new(mid_x, bar_y), Pos2::new(mid_x, bar_y + 28.0)],
                                    Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                                );
                                painter.text(
                                    Pos2::new(mid_x, bar_y + 40.0),
                                    egui::Align2::CENTER_CENTER,
                                    format!("↩ X{tid}"),
                                    FontId::monospace(11.0),
                                    COLOR_STEP_DROP,
                                );
                            }
                        } else {
                            painter.line_segment(
                                [Pos2::new(mid_x, bar_y), Pos2::new(mid_x, next_y)],
                                Stroke::new(2.0_f32, COLOR_INACTIVE_STEP),
                            );
                        }
                    }

                    // Transition crossbar + 2-line pill badge with direct inline receptivity input
                    let pill_w = (header_line.chars().count().max(cond_len + 4) as f32 * 7.4
                        + 26.0)
                        .max(200.0);
                    let pill_rect = Rect::from_min_size(
                        Pos2::new(mid_x + 26.0, bar_y - 23.0),
                        Vec2::new(pill_w, 46.0),
                    );
                    let pill_header_rect = Rect::from_min_size(
                        pill_rect.min,
                        Vec2::new(pill_w, 19.0),
                    );
                    let pill_edit_rect = Rect::from_min_max(
                        Pos2::new(pill_rect.left() + 5.0, pill_rect.top() + 20.0),
                        Pos2::new(pill_rect.right() - 5.0, pill_rect.bottom() - 4.0),
                    );
                    let tr_hit_rect = Rect::from_min_max(
                        Pos2::new(mid_x - 26.0, bar_y - 23.0),
                        pill_header_rect.max,
                    );
                    tr_hit_rects.push((t_idx, tr_hit_rect));
                    inline_widget_rects.push(pill_edit_rect);

                    if response.drag_started()
                        && let Some(origin) = press_origin
                        && tr_hit_rect.contains(origin)
                    {
                        active_drag = Some(SfcDragItem::Transition(t_idx));
                    }

                    let is_tr_selected = sel_state.selected_transitions.contains(&t_idx);
                    let tr_col = if is_tr_selected {
                        COLOR_SELECTED
                    } else {
                        COLOR_TRANSITION
                    };

                    painter.line_segment(
                        [Pos2::new(mid_x - 20.0, bar_y), Pos2::new(mid_x + 20.0, bar_y)],
                        Stroke::new(4.0_f32, tr_col),
                    );
                    painter.rect(
                        pill_rect,
                        4.0,
                        if is_tr_selected {
                            Color32::from_rgba_unmultiplied(0, 229, 255, 35)
                        } else {
                            Color32::from_rgb(22, 28, 38)
                        },
                        Stroke::new(if is_tr_selected { 2.0_f32 } else { 1.0_f32 }, tr_col),
                        StrokeKind::Inside,
                    );
                    painter.text(
                        Pos2::new(pill_rect.left() + 8.0, pill_rect.top() + 10.0),
                        egui::Align2::LEFT_CENTER,
                        header_line,
                        FontId::monospace(11.0),
                        tr_col,
                    );

                    // Direct inline receptivity input inside transition pill
                    ui.allocate_new_ui(
                        egui::UiBuilder::new().max_rect(pill_edit_rect),
                        |ui| {
                            let resp = ui.add_sized(
                                pill_edit_rect.size(),
                                egui::TextEdit::singleline(
                                    &mut program.transitions[t_idx].receptivity,
                                )
                                .font(FontId::monospace(11.0))
                                .hint_text(lang.tr("Receptivity", "Réceptivité")),
                            );
                            if resp.changed() {
                                *modified = true;
                            }
                            if resp.gained_focus() || resp.clicked() {
                                sel_state.active_trans_edit = Some(t_idx);
                                sel_state.active_step_edit = None;
                            }
                        },
                    );

                    // Highlight compatible Transition drop slots when dragging a Transition
                    if let Some(SfcDragItem::Transition(from_t_idx)) = active_drag {
                        let tr_hovered = pointer_pos.is_some_and(|p| tr_hit_rect.contains(p));
                        let fill = if tr_hovered {
                            Color32::from_rgba_unmultiplied(255, 184, 77, 60)
                        } else {
                            Color32::from_rgba_unmultiplied(255, 184, 77, 20)
                        };
                        painter.rect(
                            tr_hit_rect,
                            4.0,
                            fill,
                            Stroke::new(
                                if tr_hovered { 2.6_f32 } else { 1.3_f32 },
                                COLOR_TRANSITION,
                            ),
                            StrokeKind::Inside,
                        );
                        if tr_hovered && pointer_released && let Some(&first_s) = row.first() {
                            completed_drop = Some(SfcDropAction::MoveTransition {
                                from_idx: from_t_idx,
                                to_slot_idx: first_s,
                            });
                        }
                    }
                }

                cur_y = next_y;
            }

            // Handle click selection on steps, transitions, or empty canvas space (ignore clicks inside inline widgets)
            if response.clicked()
                && let Some(click_pos) = pointer_pos
                && !inline_widget_rects.iter().any(|r| r.contains(click_pos))
            {
                if let Some((s_idx, _)) = step_hit_rects.iter().find(|(_, r)| r.contains(click_pos))
                {
                    if modifiers.ctrl || modifiers.command || modifiers.shift {
                        if !sel_state.selected_steps.remove(s_idx) {
                            sel_state.selected_steps.insert(*s_idx);
                        }
                    } else {
                        sel_state.clear_selection();
                        sel_state.selected_steps.insert(*s_idx);
                    }
                    sel_state.active_step_edit = Some(*s_idx);
                    sel_state.active_trans_edit = None;
                } else if let Some((t_idx, _)) =
                    tr_hit_rects.iter().find(|(_, r)| r.contains(click_pos))
                {
                    if modifiers.ctrl || modifiers.command || modifiers.shift {
                        if !sel_state.selected_transitions.remove(t_idx) {
                            sel_state.selected_transitions.insert(*t_idx);
                        }
                    } else {
                        sel_state.clear_selection();
                        sel_state.selected_transitions.insert(*t_idx);
                    }
                    sel_state.active_trans_edit = Some(*t_idx);
                    sel_state.active_step_edit = None;
                } else if !modifiers.ctrl && !modifiers.command && !modifiers.shift {
                    sel_state.clear_selection();
                }
            }

            // Start marquee drag-box selection when dragging starts on empty canvas space
            if response.drag_started()
                && active_drag.is_none()
                && let Some(origin) = press_origin
                && !inline_widget_rects.iter().any(|r| r.contains(origin))
            {
                let on_step = step_hit_rects.iter().any(|(_, r)| r.contains(origin));
                let on_tr = tr_hit_rects.iter().any(|(_, r)| r.contains(origin));
                if !on_step && !on_tr {
                    sel_state.marquee_start = Some(origin);
                    if !modifiers.ctrl && !modifiers.command && !modifiers.shift {
                        sel_state.clear_selection();
                    }
                }
            }

            // Update & render marquee drag-selection box
            if let Some(start_pos) = sel_state.marquee_start {
                if let Some(curr_pos) = pointer_pos {
                    let marquee_rect = Rect::from_two_pos(start_pos, curr_pos);
                    painter.rect(
                        marquee_rect,
                        3.0,
                        Color32::from_rgba_unmultiplied(0, 229, 255, 28),
                        Stroke::new(1.5_f32, COLOR_SELECTED),
                        StrokeKind::Inside,
                    );
                    if !modifiers.ctrl && !modifiers.command && !modifiers.shift {
                        sel_state.clear_selection();
                    }
                    for (s_idx, s_rect) in &step_hit_rects {
                        if s_rect.intersects(marquee_rect) {
                            sel_state.selected_steps.insert(*s_idx);
                        }
                    }
                    for (t_idx, t_rect) in &tr_hit_rects {
                        if t_rect.intersects(marquee_rect) {
                            sel_state.selected_transitions.insert(*t_idx);
                        }
                    }
                }
                if pointer_released {
                    sel_state.marquee_start = None;
                }
            }

            // Apply any completed Step or Transition drop onto a compatible slot
            if let Some(action) = completed_drop {
                match action {
                    SfcDropAction::MoveStep { from_idx, to_idx } => {
                        if from_idx < program.steps.len() && to_idx < program.steps.len() {
                            let step = program.steps.remove(from_idx);
                            program.steps.insert(to_idx, step);
                            *modified = true;
                        }
                    }
                    SfcDropAction::MoveTransition {
                        from_idx,
                        to_slot_idx,
                    } => {
                        if from_idx < program.transitions.len() && !program.steps.is_empty() {
                            let from_step_id =
                                program.steps[to_slot_idx % program.steps.len()].id.clone();
                            let to_step_id = program.steps[(to_slot_idx + 1) % program.steps.len()]
                                .id
                                .clone();
                            let mut tr = program.transitions.remove(from_idx);
                            tr.from_steps = vec![from_step_id];
                            tr.to_steps = vec![to_step_id];
                            let target_t_idx = to_slot_idx.min(program.transitions.len());
                            program.transitions.insert(target_t_idx, tr);
                            *modified = true;
                        }
                    }
                }
                active_drag = None;
            }

            // Draw floating ghost badge while dragging a Step or Transition
            if let Some(drag) = active_drag {
                if pointer_released {
                    active_drag = None;
                } else if let Some(ptr) = pointer_pos {
                    let (badge_txt, badge_col) = match drag {
                        SfcDragItem::Step(s_idx) => {
                            let name = program
                                .steps
                                .get(s_idx)
                                .map(|s| {
                                    format!("{} X{} ({})", lang.tr("Step", "Étape"), s.id, s.name)
                                })
                                .unwrap_or_default();
                            (name, COLOR_STEP_DROP)
                        }
                        SfcDragItem::Transition(t_idx) => {
                            let label = program
                                .transitions
                                .get(t_idx)
                                .map(|t| format!("Transition {} : {}", t.id, t.receptivity))
                                .unwrap_or_default();
                            (label, COLOR_TRANSITION)
                        }
                    };
                    let layer_id = egui::LayerId::new(Order::Tooltip, Id::new("sfc_drag_ghost"));
                    let l_painter = ui.ctx().layer_painter(layer_id);
                    let ghost_rect =
                        Rect::from_center_size(ptr + Vec2::new(0.0, -24.0), Vec2::new(220.0, 30.0));
                    l_painter.rect(
                        ghost_rect,
                        5.0,
                        Color32::from_rgba_unmultiplied(24, 32, 46, 235),
                        Stroke::new(2.0_f32, badge_col),
                        StrokeKind::Inside,
                    );
                    l_painter.text(
                        ghost_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        badge_txt,
                        FontId::monospace(11.5),
                        Color32::WHITE,
                    );
                }
            }

            ui.ctx().data_mut(|d| {
                if let Some(drag) = active_drag {
                    d.insert_temp(dnd_id, drag);
                } else {
                    d.remove::<SfcDragItem>(dnd_id);
                }
            });
        });
}
