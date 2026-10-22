use std::collections::HashMap;

use egui::{Color32, FontId, Id, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use ferrite_core::{FbInstance, StandardFbKind};
use ferrite_lang_fbd::{FbdBlock, FbdBlockKind, FbdInputSource, FbdProgram};
use ferrite_runtime::PlcRuntime;

use crate::i18n::UiLanguage;

const COLOR_WIRE_TRUE: Color32 = Color32::from_rgb(46, 213, 115);
const COLOR_WIRE_NUM: Color32 = Color32::from_rgb(72, 202, 228);
const COLOR_WIRE_IDLE: Color32 = Color32::from_rgb(120, 135, 160);

#[derive(Debug, Clone, Default)]
struct FbdSelectionState {
    active_block: Option<usize>,
}

/// Removes block `b_idx` from `program` and converts any downstream wire links
/// referencing the deleted block into safe literal `FALSE` values.
fn delete_fbd_block(program: &mut FbdProgram, b_idx: usize) {
    if b_idx >= program.blocks.len() {
        return;
    }
    let deleted_id = program.blocks.remove(b_idx).id;
    for blk in &mut program.blocks {
        for src in blk.inputs.values_mut() {
            if let FbdInputSource::Link { from_block, .. } = src
                && *from_block == deleted_id
            {
                *src = FbdInputSource::Value {
                    expr: "FALSE".to_string(),
                };
            }
        }
    }
}

/// Synchronizes a corrector parameter (`KP`, `TI`, `TD`) across the FBD block's input pin,
/// the project's `FbInstance` registry, and the live `PlcRuntime`.
fn sync_corrector_param(
    blk: &mut FbdBlock,
    fb_instances: &mut [FbInstance],
    mut runtime: Option<&mut PlcRuntime>,
    pin: &str,
    val: f32,
) {
    let formatted = format!("{val:.2}");
    if let Some(FbdInputSource::Value { expr }) = blk.inputs.get_mut(pin) {
        *expr = formatted;
    } else if !blk.inputs.contains_key(pin) {
        blk.inputs
            .insert(pin.to_string(), FbdInputSource::Value { expr: formatted });
    }
    if let FbdBlockKind::StandardFb { instance, .. } = &blk.kind {
        if let Some(inst) = fb_instances.iter_mut().find(|f| &f.name == instance) {
            match pin {
                "KP" => inst.kp = val,
                "TI" => inst.ti_s = val,
                "TD" => inst.td_s = val,
                _ => {}
            }
        }
        if let Some(ref mut rt) = runtime
            && let Some(rt_fb) = rt.tags.get_fb_mut(instance)
        {
            match pin {
                "KP" => rt_fb.kp = val,
                "TI" => rt_fb.ti_s = val,
                "TD" => rt_fb.td_s = val,
                _ => {}
            }
        }
    }
}

/// Reads the current numeric parameter (`KP`, `TI`, `TD`) from the FBD block pin or `FbInstance`.
fn read_corrector_param(blk: &FbdBlock, fb_instances: &[FbInstance], pin: &str, default: f32) -> f32 {
    if let Some(FbdInputSource::Value { expr }) = blk.inputs.get(pin) {
        if let Ok(v) = expr.trim().parse::<f32>() {
            return v;
        }
        if let Some(ms) = ferrite_core::parse_iec_time_literal(expr) {
            return (ms as f32) / 1000.0;
        }
    }
    if let FbdBlockKind::StandardFb { instance, .. } = &blk.kind
        && let Some(inst) = fb_instances.iter().find(|f| &f.name == instance)
    {
        return match pin {
            "KP" => inst.kp,
            "TI" => inst.ti_s,
            "TD" => inst.td_s,
            _ => default,
        };
    }
    default
}

/// Updates the `StandardFbKind` of a corrector block and syncs its required pins (`TI`, `TD`) and `FbInstance`.
fn apply_corrector_kind(
    blk: &mut FbdBlock,
    fb_instances: &mut Vec<FbInstance>,
    mut runtime: Option<&mut PlcRuntime>,
    new_kind: StandardFbKind,
) {
    let kp = read_corrector_param(blk, fb_instances, "KP", 2.0);
    let ti = read_corrector_param(blk, fb_instances, "TI", 5.0);
    let td = read_corrector_param(blk, fb_instances, "TD", 0.5);
    if let FbdBlockKind::StandardFb { instance, fb_type } = &mut blk.kind {
        *fb_type = Some(new_kind);
        if let Some(inst) = fb_instances.iter_mut().find(|f| &f.name == instance) {
            inst.kind = new_kind;
            inst.in_bool = true;
        } else {
            let mut inst = FbInstance::new(instance.clone(), new_kind);
            inst.kp = kp;
            inst.ti_s = ti;
            inst.td_s = td;
            fb_instances.push(inst);
        }
        if let Some(ref mut rt) = runtime
            && let Some(rt_fb) = rt.tags.get_fb_mut(instance)
        {
            rt_fb.kind = new_kind;
            rt_fb.in_bool = true;
        }
    }
    blk.inputs
        .entry("KP".to_string())
        .or_insert_with(|| FbdInputSource::Value {
            expr: format!("{kp:.2}"),
        });
    if new_kind.has_integral() {
        blk.inputs
            .entry("TI".to_string())
            .or_insert_with(|| FbdInputSource::Value {
                expr: format!("{ti:.2}"),
            });
    } else {
        blk.inputs.remove("TI");
    }
    if new_kind.has_derivative() {
        blk.inputs
            .entry("TD".to_string())
            .or_insert_with(|| FbdInputSource::Value {
                expr: format!("{td:.2}"),
            });
    } else {
        blk.inputs.remove("TD");
    }
}

fn draw_in_canvas_fbd_inspector(
    ui: &mut Ui,
    program: &mut FbdProgram,
    fb_instances: &mut Vec<FbInstance>,
    mut runtime: Option<&mut PlcRuntime>,
    sel_state: &mut FbdSelectionState,
    modified: &mut bool,
    lang: UiLanguage,
) {
    if let Some(b_idx) = sel_state.active_block
        && b_idx < program.blocks.len()
    {
        let mut remove_this_blk = false;
        egui::Frame::NONE
            .fill(Color32::from_rgb(22, 28, 38))
            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(255, 215, 0)))
            .inner_margin(8.0)
            .corner_radius(6.0)
            .show(ui, |ui| {
                let blk = &mut program.blocks[b_idx];
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(
                        Color32::from_rgb(255, 215, 0),
                        RichText::new(format!(
                            "✏ {} [{}] ({})",
                            lang.tr("Direct Canvas Edit Block", "Édition Directe Bloc"),
                            blk.id,
                            block_kind_label(&blk.kind)
                        ))
                        .strong(),
                    );
                    ui.label("ID:");
                    if ui
                        .add(egui::TextEdit::singleline(&mut blk.id).desired_width(110.0))
                        .changed()
                    {
                        *modified = true;
                    }
                    if let FbdBlockKind::StandardFb { instance, .. } = &mut blk.kind {
                        ui.label(lang.tr("Instance:", "Instance :"));
                        if ui
                            .add(egui::TextEdit::singleline(instance).desired_width(110.0))
                            .changed()
                        {
                            *modified = true;
                        }
                    }
                    if ui
                        .small_button(
                            RichText::new(lang.tr("Delete Block", "Supprimer Bloc"))
                                .color(Color32::from_rgb(255, 107, 107)),
                        )
                        .clicked()
                    {
                        remove_this_blk = true;
                    }
                    if ui.small_button("✕").clicked() {
                        sel_state.active_block = None;
                    }
                });

                // Dedicated Quick Tuning Bar when this block is a P / PI / PID Corrector
                let corr_kind_opt = match &blk.kind {
                    FbdBlockKind::StandardFb { instance, fb_type } => fb_type
                        .or_else(|| {
                            fb_instances
                                .iter()
                                .find(|f| &f.name == instance)
                                .map(|f| f.kind)
                        })
                        .filter(|k| k.is_corrector()),
                    _ => None,
                };
                if let Some(mut corr_kind) = corr_kind_opt {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(
                            Color32::from_rgb(72, 202, 228),
                            RichText::new(lang.tr(
                                "⚙ Corrector Tuning (Gain / Ti / Td):",
                                "⚙ Réglage Correcteur (Gain / Ti / Td) :",
                            ))
                            .strong(),
                        );
                        egui::ComboBox::from_id_salt(("insp_corr_mode", b_idx))
                            .selected_text(corr_kind.label())
                            .show_ui(ui, |ui| {
                                for k in StandardFbKind::CORRECTORS {
                                    if ui
                                        .selectable_value(&mut corr_kind, k, k.label())
                                        .changed()
                                    {
                                        apply_corrector_kind(
                                            blk,
                                            fb_instances,
                                            runtime.as_deref_mut(),
                                            corr_kind,
                                        );
                                        *modified = true;
                                    }
                                }
                            });

                        let mut kp = read_corrector_param(blk, fb_instances, "KP", 2.0);
                        ui.label(lang.tr("Gain (Kp):", "Gain (Kp) :"));
                        if ui
                            .add(
                                egui::DragValue::new(&mut kp)
                                    .speed(0.1)
                                    .range(-10000.0..=10000.0),
                            )
                            .changed()
                        {
                            sync_corrector_param(
                                blk,
                                fb_instances,
                                runtime.as_deref_mut(),
                                "KP",
                                kp,
                            );
                            *modified = true;
                        }

                        if corr_kind.has_integral() {
                            let mut ti = read_corrector_param(blk, fb_instances, "TI", 5.0);
                            ui.label("Ti (s):");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut ti)
                                        .speed(0.1)
                                        .range(0.001..=10000.0),
                                )
                                .changed()
                            {
                                sync_corrector_param(
                                    blk,
                                    fb_instances,
                                    runtime.as_deref_mut(),
                                    "TI",
                                    ti,
                                );
                                *modified = true;
                            }
                        }

                        if corr_kind.has_derivative() {
                            let mut td = read_corrector_param(blk, fb_instances, "TD", 0.5);
                            ui.label("Td (s):");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut td)
                                        .speed(0.05)
                                        .range(0.0..=10000.0),
                                )
                                .changed()
                            {
                                sync_corrector_param(
                                    blk,
                                    fb_instances,
                                    runtime,
                                    "TD",
                                    td,
                                );
                                *modified = true;
                            }
                        }

                        ui.monospace(corr_kind.formula_hint());
                    });
                    ui.add_space(4.0);
                }

                    // Inputs inline editor
                    let mut remove_in_pin: Option<String> = None;
                    let mut input_keys: Vec<String> = blk.inputs.keys().cloned().collect();
                    input_keys.sort();
                    for pin in input_keys {
                        if let Some(src) = blk.inputs.get_mut(&pin) {
                            ui.horizontal_wrapped(|ui| {
                                ui.add_space(12.0);
                                if ui
                                    .small_button(
                                        RichText::new("✕").color(Color32::from_rgb(255, 107, 107)),
                                    )
                                    .on_hover_text(lang.tr("Delete input pin", "Supprimer l'entrée"))
                                    .clicked()
                                {
                                    remove_in_pin = Some(pin.clone());
                                }
                                ui.monospace(format!("In [{pin}] <-"));
                                let is_link = matches!(src, FbdInputSource::Link { .. });
                                if ui
                                    .selectable_label(
                                        !is_link,
                                        lang.tr("Symbol/Literal", "Symbole/Littéral"),
                                    )
                                    .clicked()
                                    && is_link
                                {
                                    *src = FbdInputSource::Value {
                                        expr: "TRUE".to_string(),
                                    };
                                    *modified = true;
                                }
                                if ui
                                    .selectable_label(is_link, lang.tr("Wire", "Liaison"))
                                    .clicked()
                                    && !is_link
                                {
                                    *src = FbdInputSource::Link {
                                        from_block: "Blk_1".to_string(),
                                        from_pin: "OUT".to_string(),
                                    };
                                    *modified = true;
                                }
                                match src {
                                    FbdInputSource::Value { expr } => {
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(expr)
                                                    .desired_width(140.0),
                                            )
                                            .changed()
                                        {
                                            *modified = true;
                                        }
                                    }
                                    FbdInputSource::Link {
                                        from_block,
                                        from_pin,
                                    } => {
                                        ui.label(lang.tr("From:", "De :"));
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(from_block)
                                                    .desired_width(90.0),
                                            )
                                            .changed()
                                        {
                                            *modified = true;
                                        }
                                        ui.label("Pin:");
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(from_pin)
                                                    .desired_width(60.0),
                                            )
                                            .changed()
                                        {
                                            *modified = true;
                                        }
                                    }
                                }
                            });
                        }
                    }
                    if let Some(p) = remove_in_pin {
                        blk.inputs.remove(&p);
                        *modified = true;
                    }

                    // Outputs inline editor
                    let mut remove_out_pin: Option<String> = None;
                    let mut output_keys: Vec<String> = blk.outputs.keys().cloned().collect();
                    output_keys.sort();
                    for pin in output_keys {
                        if let Some(target) = blk.outputs.get_mut(&pin) {
                            ui.horizontal_wrapped(|ui| {
                                ui.add_space(12.0);
                                if ui
                                    .small_button(
                                        RichText::new("✕").color(Color32::from_rgb(255, 107, 107)),
                                    )
                                    .on_hover_text(lang.tr("Delete output pin", "Supprimer la sortie"))
                                    .clicked()
                                {
                                    remove_out_pin = Some(pin.clone());
                                }
                                ui.monospace(format!("Out [{pin}] =>"));
                                if ui
                                    .add(
                                        egui::TextEdit::singleline(target)
                                            .desired_width(160.0),
                                    )
                                    .changed()
                                {
                                    *modified = true;
                                }
                            });
                        }
                    }
                    if let Some(p) = remove_out_pin {
                        blk.outputs.remove(&p);
                        *modified = true;
                    }
                });

            if remove_this_blk {
                delete_fbd_block(program, b_idx);
                sel_state.active_block = None;
                *modified = true;
            }
            ui.add_space(4.0);
        }
}

/// Renders the visual Function Block Diagram (`FbdProgram`) 2D block network canvas
/// and the interactive Block / Pin / Wire editor.
pub fn render_fbd_editor(
    ui: &mut Ui,
    program: &mut FbdProgram,
    fb_instances: &mut Vec<FbInstance>,
    mut runtime: Option<&mut PlcRuntime>,
    modified: &mut bool,
    lang: UiLanguage,
) {
    let clear_id = Id::new(("fbd_confirm_clear_pou", program.name.clone()));
    let mut confirm_clear: bool = ui.ctx().data_mut(|d| d.get_temp(clear_id)).unwrap_or(false);

    let sel_id = Id::new(("fbd_selection_state", program.name.clone()));
    let mut sel_state: FbdSelectionState =
        ui.ctx().data_mut(|d| d.get_temp(sel_id)).unwrap_or_default();

    if let Some(b_idx) = sel_state.active_block
        && b_idx >= program.blocks.len()
    {
        sel_state.active_block = None;
    }

    ui.horizontal_wrapped(|ui| {
        ui.heading(format!(
            "{} — {}",
            lang.tr(
                "Function Block Diagram",
                "Schéma Bloc Fonctionnel (FBD)"
            ),
            program.name
        ));
        ui.separator();
        if ui
            .button(lang.tr("+ P", "+ Correcteur P"))
            .on_hover_text(StandardFbKind::P.formula_hint())
            .clicked()
        {
            add_starter_corrector(program, fb_instances, StandardFbKind::P);
            *modified = true;
        }
        if ui
            .button(lang.tr("+ PI", "+ Correcteur PI"))
            .on_hover_text(StandardFbKind::Pi.formula_hint())
            .clicked()
        {
            add_starter_corrector(program, fb_instances, StandardFbKind::Pi);
            *modified = true;
        }
        if ui
            .button(lang.tr("+ PID Parallel", "+ PID Parallèle"))
            .on_hover_text(StandardFbKind::PidParallel.formula_hint())
            .clicked()
        {
            add_starter_corrector(program, fb_instances, StandardFbKind::PidParallel);
            *modified = true;
        }
        if ui
            .button(lang.tr("+ PID Series", "+ PID Série"))
            .on_hover_text(StandardFbKind::PidSeries.formula_hint())
            .clicked()
        {
            add_starter_corrector(program, fb_instances, StandardFbKind::PidSeries);
            *modified = true;
        }
        if ui
            .button(lang.tr("+ PID Mixed", "+ PID Mixte"))
            .on_hover_text(StandardFbKind::PidMixed.formula_hint())
            .clicked()
        {
            add_starter_corrector(program, fb_instances, StandardFbKind::PidMixed);
            *modified = true;
        }
        ui.separator();
        if ui
            .button(lang.tr("+ AND", "+ ET"))
            .clicked()
        {
            add_starter_block(program, FbdBlockKind::And);
            *modified = true;
        }
        if ui.button(lang.tr("+ OR", "+ OU")).clicked() {
            add_starter_block(program, FbdBlockKind::Or);
            *modified = true;
        }
        if ui
            .button(lang.tr("+ GT (>)", "+ Comp >"))
            .clicked()
        {
            add_starter_block(program, FbdBlockKind::Gt);
            *modified = true;
        }
        if ui
            .button(lang.tr("+ TON", "+ Tempo TON"))
            .clicked()
        {
            add_starter_block(
                program,
                FbdBlockKind::StandardFb {
                    instance: "Fill_Timer".to_string(),
                    fb_type: Some(StandardFbKind::Ton),
                },
            );
            *modified = true;
        }
        if ui
            .button(
                RichText::new(lang.tr("Clear POU", "Vider le POU"))
                    .color(Color32::from_rgb(255, 107, 107)),
            )
            .on_hover_text(lang.tr(
                "Clear all blocks in this FBD POU (asks confirmation)",
                "Supprimer tous les blocs de ce POU FBD (avec confirmation)",
            ))
            .clicked()
        {
            confirm_clear = true;
        }
    });
    ui.add_space(4.0);

    if confirm_clear {
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
                    "Are you sure you want to clear the entire POU '{}' ({} block(s))?",
                    program.name,
                    program.blocks.len()
                ),
                UiLanguage::Fr => format!(
                    "Voulez-vous vraiment vider tout le POU '{}' ({} bloc(s)) ?",
                    program.name,
                    program.blocks.len()
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
                    program.blocks.clear();
                    confirm_clear = false;
                    *modified = true;
                }
                if ui.button(lang.tr("Cancel", "Annuler")).clicked() {
                    confirm_clear = false;
                }
            });
        });
    }

    ui.ctx().data_mut(|d| d.insert_temp(clear_id, confirm_clear));

    egui::ScrollArea::vertical()
        .id_salt("fbd_scroll_area")
        .show(ui, |ui| {
            // In-Canvas Direct Editor Card when a Block is clicked on the canvas
            draw_in_canvas_fbd_inspector(
                ui,
                program,
                fb_instances,
                runtime.as_deref_mut(),
                &mut sel_state,
                modified,
                lang,
            );

            // 1. Visual 2D Block Network Canvas (with direct on-block Gain / Ti / Td tuning for P/PI/PID)
            draw_fbd_canvas(
                ui,
                program,
                fb_instances,
                runtime,
                &mut sel_state,
                modified,
            );

            ui.add_space(10.0);
            ui.separator();

            // 2. Interactive Block & Pin Editor
            let fbd_header = format!(
                "{} ({})",
                lang.tr(
                    "Edit Function Blocks & Signal Wires",
                    "Éditer Blocs Fonctionnels & Liaisons"
                ),
                program.blocks.len()
            );
            egui::CollapsingHeader::new(fbd_header)
                .default_open(true)
                .show(ui, |ui| {
                    let mut remove_blk: Option<usize> = None;
                    for (b_idx, blk) in program.blocks.iter_mut().enumerate() {
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(26, 32, 42))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 65, 85)))
                            .inner_margin(6.0)
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    ui.strong(format!(
                                        "{} [{}] ({})",
                                        lang.tr("Block", "Bloc"),
                                        blk.id,
                                        block_kind_label(&blk.kind)
                                    ));
                                    ui.label("ID:");
                                    if ui
                                        .add(
                                            egui::TextEdit::singleline(&mut blk.id)
                                                .desired_width(130.0),
                                        )
                                        .changed()
                                    {
                                        *modified = true;
                                    }
                                    if let FbdBlockKind::StandardFb { instance, .. } =
                                        &mut blk.kind
                                    {
                                        ui.label(lang.tr("FB Instance:", "Instance FB :"));
                                        if ui
                                            .add(
                                                egui::TextEdit::singleline(instance)
                                                    .desired_width(120.0),
                                            )
                                            .changed()
                                        {
                                            *modified = true;
                                        }
                                    }
                                    if ui
                                        .small_button(
                                            lang.tr("Delete Block", "Supprimer Bloc"),
                                        )
                                        .clicked()
                                    {
                                        remove_blk = Some(b_idx);
                                    }
                                });

                                let mut input_keys: Vec<String> =
                                    blk.inputs.keys().cloned().collect();
                                input_keys.sort();
                                for pin in input_keys {
                                    if let Some(src) = blk.inputs.get_mut(&pin) {
                                        ui.horizontal(|ui| {
                                            ui.add_space(14.0);
                                            ui.monospace(format!("In [{pin}] <-"));
                                            let is_link =
                                                matches!(src, FbdInputSource::Link { .. });
                                            if ui
                                                .selectable_label(
                                                    !is_link,
                                                    lang.tr("Symbol/Literal", "Symbole/Littéral"),
                                                )
                                                .clicked()
                                                && is_link
                                            {
                                                *src = FbdInputSource::Value {
                                                    expr: "TRUE".to_string(),
                                                };
                                                *modified = true;
                                            }
                                            if ui
                                                .selectable_label(
                                                    is_link,
                                                    lang.tr("Block Wire", "Liaison Bloc"),
                                                )
                                                .clicked()
                                                && !is_link
                                            {
                                                *src = FbdInputSource::Link {
                                                    from_block: "Blk_Pump_Gate".to_string(),
                                                    from_pin: "OUT".to_string(),
                                                };
                                                *modified = true;
                                            }
                                            match src {
                                                FbdInputSource::Value { expr } => {
                                                    if ui.text_edit_singleline(expr).changed() {
                                                        *modified = true;
                                                    }
                                                }
                                                FbdInputSource::Link {
                                                    from_block,
                                                    from_pin,
                                                } => {
                                                    ui.label(lang.tr("Block:", "Bloc :"));
                                                    if ui
                                                        .add(
                                                            egui::TextEdit::singleline(from_block)
                                                                .desired_width(110.0),
                                                        )
                                                        .changed()
                                                    {
                                                        *modified = true;
                                                    }
                                                    ui.label(lang.tr("Pin:", "Broche :"));
                                                    if ui
                                                        .add(
                                                            egui::TextEdit::singleline(from_pin)
                                                                .desired_width(55.0),
                                                        )
                                                        .changed()
                                                    {
                                                        *modified = true;
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }

                                let mut output_keys: Vec<String> =
                                    blk.outputs.keys().cloned().collect();
                                output_keys.sort();
                                for pin in output_keys {
                                    if let Some(target) = blk.outputs.get_mut(&pin) {
                                        ui.horizontal(|ui| {
                                            ui.add_space(14.0);
                                            ui.monospace(format!("Out [{pin}] =>"));
                                            if ui.text_edit_singleline(target).changed() {
                                                *modified = true;
                                            }
                                        });
                                    }
                                }
                            });
                        ui.add_space(4.0);
                    }
                    if let Some(bi) = remove_blk {
                        delete_fbd_block(program, bi);
                        sel_state.active_block = None;
                        *modified = true;
                    }
                });
        });

    ui.ctx().data_mut(|d| d.insert_temp(sel_id, sel_state));
}

fn add_starter_corrector(
    program: &mut FbdProgram,
    fb_instances: &mut Vec<FbInstance>,
    corr_kind: StandardFbKind,
) {
    let idx = program.blocks.len() + 1;
    let inst_name = format!("{}_{idx}", corr_kind.label());
    if !fb_instances.iter().any(|f| f.name == inst_name) {
        fb_instances.push(FbInstance::new(inst_name.clone(), corr_kind));
    }

    let mut inputs = HashMap::new();
    inputs.insert(
        "SP".to_string(),
        FbdInputSource::Value {
            expr: "60.0".to_string(),
        },
    );
    inputs.insert(
        "PV".to_string(),
        FbdInputSource::Value {
            expr: "Tank_Temp".to_string(),
        },
    );
    inputs.insert(
        "KP".to_string(),
        FbdInputSource::Value {
            expr: "2.00".to_string(),
        },
    );
    if corr_kind.has_integral() {
        inputs.insert(
            "TI".to_string(),
            FbdInputSource::Value {
                expr: "5.00".to_string(),
            },
        );
    }
    if corr_kind.has_derivative() {
        inputs.insert(
            "TD".to_string(),
            FbdInputSource::Value {
                expr: "0.50".to_string(),
            },
        );
    }
    let mut outputs = HashMap::new();
    outputs.insert("OUT".to_string(), "Tank_Temp".to_string());

    program.blocks.push(FbdBlock {
        id: format!("Blk_{inst_name}"),
        kind: FbdBlockKind::StandardFb {
            instance: inst_name,
            fb_type: Some(corr_kind),
        },
        inputs,
        outputs,
    });
}

fn add_starter_block(program: &mut FbdProgram, kind: FbdBlockKind) {
    let idx = program.blocks.len() + 1;
    let mut inputs = HashMap::new();
    let mut outputs = HashMap::new();
    match &kind {
        FbdBlockKind::StandardFb { .. } => {
            inputs.insert(
                "IN".to_string(),
                FbdInputSource::Value {
                    expr: "Permissive_OK".to_string(),
                },
            );
            inputs.insert(
                "PT".to_string(),
                FbdInputSource::Value {
                    expr: "T#200ms".to_string(),
                },
            );
            outputs.insert("Q".to_string(), "Pump_Active".to_string());
        }
        _ => {
            inputs.insert(
                "IN1".to_string(),
                FbdInputSource::Value {
                    expr: "Permissive_OK".to_string(),
                },
            );
            inputs.insert(
                "IN2".to_string(),
                FbdInputSource::Value {
                    expr: "Fill_Valve".to_string(),
                },
            );
            outputs.insert("OUT".to_string(), "Pump_Active".to_string());
        }
    }
    program.blocks.push(FbdBlock {
        id: format!("Blk_{idx}"),
        kind,
        inputs,
        outputs,
    });
}

pub fn block_kind_label(kind: &FbdBlockKind) -> String {
    match kind {
        FbdBlockKind::And => "AND".to_string(),
        FbdBlockKind::Or => "OR".to_string(),
        FbdBlockKind::Xor => "XOR".to_string(),
        FbdBlockKind::Not => "NOT".to_string(),
        FbdBlockKind::Add => "ADD".to_string(),
        FbdBlockKind::Sub => "SUB".to_string(),
        FbdBlockKind::Mul => "MUL".to_string(),
        FbdBlockKind::Div => "DIV".to_string(),
        FbdBlockKind::Gt => "GT (>)".to_string(),
        FbdBlockKind::Ge => "GE (>=)".to_string(),
        FbdBlockKind::Lt => "LT (<)".to_string(),
        FbdBlockKind::Le => "LE (<=)".to_string(),
        FbdBlockKind::Eq => "EQ (=)".to_string(),
        FbdBlockKind::Ne => "NE (<>)".to_string(),
        FbdBlockKind::StandardFb { instance, fb_type } => match fb_type {
            Some(t) => format!("{} ({instance})", t.label()),
            None => format!("FB ({instance})"),
        },
        FbdBlockKind::Composite { type_name } => format!("COMP ({type_name})"),
    }
}

fn draw_fbd_canvas(
    ui: &mut Ui,
    program: &mut FbdProgram,
    fb_instances: &mut Vec<FbInstance>,
    mut runtime: Option<&mut PlcRuntime>,
    sel_state: &mut FbdSelectionState,
    modified: &mut bool,
) {
    let rows = (program.blocks.len().div_ceil(2)).max(1);
    let canvas_height = (rows as f32) * 235.0 + 44.0;
    let min_fbd_width = 1160.0_f32;

    egui::ScrollArea::horizontal()
        .id_salt(("fbd_canvas_hscroll", program.name.clone()))
        .show(ui, |ui| {
            let desired_width = ui.available_width().max(min_fbd_width);
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(desired_width, canvas_height), Sense::click());
            let painter = ui.painter_at(rect);

            painter.rect_filled(rect, 6.0, Color32::from_rgb(16, 20, 26));

            let mut out_pin_positions: HashMap<String, Pos2> = HashMap::new();
            let mut in_links_to_draw: Vec<(String, Pos2)> = Vec::new();
            let mut block_rects: Vec<(usize, Rect)> = Vec::with_capacity(program.blocks.len());
            let mut inline_widget_rects: Vec<Rect> = Vec::new();
            let mut delete_req: Option<usize> = None;

            let col_stride = 560.0_f32;
            let row_stride = 225.0_f32;

            for idx in 0..program.blocks.len() {
                let col = idx % 2;
                let row = idx / 2;

                let corr_kind_opt = match &program.blocks[idx].kind {
                    FbdBlockKind::StandardFb { instance, fb_type } => fb_type
                        .or_else(|| {
                            fb_instances
                                .iter()
                                .find(|f| &f.name == instance)
                                .map(|f| f.kind)
                        })
                        .filter(|k| k.is_corrector()),
                    _ => None,
                };

                let header_title = format!(
                    "{} [{}]",
                    program.blocks[idx].id,
                    block_kind_label(&program.blocks[idx].kind)
                );
                let bw = (header_title.chars().count() as f32 * 7.2 + 48.0).max(if corr_kind_opt.is_some() {
                    300.0
                } else {
                    200.0
                });
                let pin_rows = program.blocks[idx]
                    .inputs
                    .len()
                    .max(program.blocks[idx].outputs.len())
                    .max(2);
                let extra_tuning_h = if corr_kind_opt.is_some() { 34.0 } else { 0.0 };
                let bh = 36.0 + (pin_rows as f32) * 26.0 + extra_tuning_h;

                let bx = rect.left() + 195.0 + (col as f32) * col_stride;
                let by = rect.top() + 26.0 + (row as f32) * row_stride;
                let blk_rect = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(bw, bh));
                block_rects.push((idx, blk_rect));

                let is_blk_selected = sel_state.active_block == Some(idx);
                if is_blk_selected {
                    painter.rect_filled(
                        blk_rect.expand(4.0),
                        7.0,
                        Color32::from_rgba_unmultiplied(255, 215, 0, 35),
                    );
                }

                // Block body & header
                painter.rect(
                    blk_rect,
                    5.0,
                    Color32::from_rgb(28, 36, 48),
                    Stroke::new(
                        if is_blk_selected { 2.4_f32 } else { 1.8_f32 },
                        if is_blk_selected {
                            Color32::from_rgb(255, 215, 0)
                        } else if corr_kind_opt.is_some() {
                            Color32::from_rgb(72, 202, 228)
                        } else {
                            Color32::from_rgb(80, 145, 220)
                        },
                    ),
                    StrokeKind::Inside,
                );
                let header_rect = Rect::from_min_size(blk_rect.min, Vec2::new(bw, 26.0));
                painter.rect_filled(
                    header_rect,
                    4.0,
                    if corr_kind_opt.is_some() {
                        Color32::from_rgb(32, 74, 105)
                    } else {
                        Color32::from_rgb(38, 60, 92)
                    },
                );
                painter.text(
                    header_rect.center() - Vec2::new(10.0, 0.0),
                    egui::Align2::CENTER_CENTER,
                    header_title,
                    FontId::monospace(11.0),
                    Color32::WHITE,
                );

                // On-canvas delete button (top-right of block header)
                let del_rect = Rect::from_min_size(
                    Pos2::new(blk_rect.right() - 24.0, blk_rect.top() + 3.0),
                    Vec2::new(20.0, 20.0),
                );
                inline_widget_rects.push(del_rect);
                if ui
                    .put(
                        del_rect,
                        egui::Button::new(
                            RichText::new("✕").color(Color32::from_rgb(255, 107, 107)),
                        )
                        .small(),
                    )
                    .on_hover_text("Delete block (Del)")
                    .clicked()
                {
                    delete_req = Some(idx);
                }

                {
                    let blk = &program.blocks[idx];
                    let rt_ref = runtime.as_deref();
                    let mut in_keys: Vec<&String> = blk.inputs.keys().collect();
                    in_keys.sort();
                    for (p_idx, pin_name) in in_keys.iter().enumerate() {
                        let py = blk_rect.top() + 44.0 + (p_idx as f32) * 26.0;
                        let pin_pos = Pos2::new(blk_rect.left(), py);
                        painter.circle_filled(pin_pos, 3.5, COLOR_WIRE_NUM);
                        painter.text(
                            Pos2::new(blk_rect.left() + 8.0, py),
                            egui::Align2::LEFT_CENTER,
                            *pin_name,
                            FontId::monospace(11.0),
                            Color32::from_rgb(200, 210, 225),
                        );

                        if let Some(src) = blk.inputs.get(*pin_name) {
                            match src {
                                FbdInputSource::Value { expr } => {
                                    let live_val = rt_ref.and_then(|rt| rt.read_value(expr).ok());
                                    let wire_col = match live_val {
                                        Some(ref v) => match v.as_bool() {
                                            Ok(true) => COLOR_WIRE_TRUE,
                                            Ok(false) => COLOR_WIRE_IDLE,
                                            Err(_) => COLOR_WIRE_NUM,
                                        },
                                        None => COLOR_WIRE_IDLE,
                                    };
                                    let label = match live_val {
                                        Some(v) => format!("{expr} ({v})"),
                                        None => expr.clone(),
                                    };
                                    painter.line_segment(
                                        [Pos2::new(pin_pos.x - 18.0, py), pin_pos],
                                        Stroke::new(2.0_f32, wire_col),
                                    );
                                    let lbl_w =
                                        (label.chars().count() as f32 * 6.8 + 12.0).max(60.0);
                                    let lbl_rect = Rect::from_min_size(
                                        Pos2::new(pin_pos.x - 22.0 - lbl_w, py - 10.0),
                                        Vec2::new(lbl_w, 20.0),
                                    );
                                    painter.rect_filled(lbl_rect, 3.0, Color32::from_rgb(20, 26, 34));
                                    painter.text(
                                        Pos2::new(pin_pos.x - 26.0, py),
                                        egui::Align2::RIGHT_CENTER,
                                        label,
                                        FontId::monospace(10.5),
                                        wire_col,
                                    );
                                }
                                FbdInputSource::Link { from_block, .. } => {
                                    in_links_to_draw.push((from_block.clone(), pin_pos));
                                }
                            }
                        }
                    }

                    let mut out_keys: Vec<(&String, &String)> = blk.outputs.iter().collect();
                    out_keys.sort_by_key(|(k, _)| *k);
                    for (o_idx, (out_pin, target_sym)) in out_keys.iter().enumerate() {
                        let out_y = blk_rect.top() + 54.0 + (o_idx as f32) * 26.0;
                        let out_pos = Pos2::new(blk_rect.right(), out_y);
                        out_pin_positions.insert(blk.id.clone(), out_pos);
                        painter.circle_filled(out_pos, 4.0, COLOR_WIRE_TRUE);

                        let live_out = rt_ref.and_then(|rt| rt.read_value(target_sym).ok());
                        let out_col = match live_out {
                            Some(ref v) => match v.as_bool() {
                                Ok(true) => COLOR_WIRE_TRUE,
                                Ok(false) => COLOR_WIRE_IDLE,
                                Err(_) => COLOR_WIRE_NUM,
                            },
                            None => COLOR_WIRE_IDLE,
                        };
                        let out_label = match live_out {
                            Some(v) => format!("{target_sym} = {v}"),
                            None => (*target_sym).clone(),
                        };
                        painter.text(
                            Pos2::new(blk_rect.right() - 8.0, out_y),
                            egui::Align2::RIGHT_CENTER,
                            *out_pin,
                            FontId::monospace(11.0),
                            Color32::WHITE,
                        );
                        painter.line_segment(
                            [out_pos, Pos2::new(out_pos.x + 18.0, out_y)],
                            Stroke::new(2.2_f32, out_col),
                        );
                        let lbl_w = (out_label.chars().count() as f32 * 6.8 + 12.0).max(70.0);
                        let lbl_rect = Rect::from_min_size(
                            Pos2::new(out_pos.x + 20.0, out_y - 10.0),
                            Vec2::new(lbl_w, 20.0),
                        );
                        painter.rect_filled(lbl_rect, 3.0, Color32::from_rgb(20, 26, 34));
                        painter.text(
                            Pos2::new(out_pos.x + 24.0, out_y),
                            egui::Align2::LEFT_CENTER,
                            out_label,
                            FontId::monospace(10.5),
                            out_col,
                        );
                    }
                }

                // Inline On-Canvas Parameter Controls for P / PI / PID (Parallel, Series, Mixed) blocks
                if let Some(mut corr_kind) = corr_kind_opt {
                    let tune_rect = Rect::from_min_max(
                        Pos2::new(blk_rect.left() + 6.0, blk_rect.bottom() - 30.0),
                        Pos2::new(blk_rect.right() - 6.0, blk_rect.bottom() - 4.0),
                    );
                    inline_widget_rects.push(tune_rect);
                    painter.rect_filled(tune_rect, 3.0, Color32::from_rgb(20, 28, 38));

                    let blk = &mut program.blocks[idx];
                    let mut child_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(tune_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    child_ui.spacing_mut().item_spacing.x = 4.0;
                    egui::ComboBox::from_id_salt(("fbd_canvas_corr_kind", &program.name, idx))
                        .width(96.0)
                        .selected_text(corr_kind.label())
                        .show_ui(&mut child_ui, |ui| {
                            for k in StandardFbKind::CORRECTORS {
                                if ui
                                    .selectable_value(&mut corr_kind, k, k.label())
                                    .changed()
                                {
                                    apply_corrector_kind(
                                        blk,
                                        fb_instances,
                                        runtime.as_deref_mut(),
                                        corr_kind,
                                    );
                                    *modified = true;
                                }
                            }
                        });

                    let mut kp = read_corrector_param(blk, fb_instances, "KP", 2.0);
                    child_ui.label(RichText::new("Kp").size(10.5));
                    if child_ui
                        .add(
                            egui::DragValue::new(&mut kp)
                                .speed(0.1)
                                .range(-10000.0..=10000.0),
                        )
                        .changed()
                    {
                        sync_corrector_param(
                            blk,
                            fb_instances,
                            runtime.as_deref_mut(),
                            "KP",
                            kp,
                        );
                        *modified = true;
                    }

                    if corr_kind.has_integral() {
                        let mut ti = read_corrector_param(blk, fb_instances, "TI", 5.0);
                        child_ui.label(RichText::new("Ti").size(10.5));
                        if child_ui
                            .add(
                                egui::DragValue::new(&mut ti)
                                    .speed(0.1)
                                    .range(0.001..=10000.0),
                            )
                            .changed()
                        {
                            sync_corrector_param(
                                blk,
                                fb_instances,
                                runtime.as_deref_mut(),
                                "TI",
                                ti,
                            );
                            *modified = true;
                        }
                    }

                    if corr_kind.has_derivative() {
                        let mut td = read_corrector_param(blk, fb_instances, "TD", 0.5);
                        child_ui.label(RichText::new("Td").size(10.5));
                        if child_ui
                            .add(
                                egui::DragValue::new(&mut td)
                                    .speed(0.05)
                                    .range(0.0..=10000.0),
                            )
                            .changed()
                        {
                            sync_corrector_param(
                                blk,
                                fb_instances,
                                runtime.as_deref_mut(),
                                "TD",
                                td,
                            );
                            *modified = true;
                        }
                    }
                }
            }

            let click_pos = ui.ctx().pointer_interact_pos();
            if response.clicked()
                && let Some(pos) = click_pos
                && !inline_widget_rects.iter().any(|r| r.contains(pos))
            {
                if let Some((b_idx, _)) = block_rects.iter().find(|(_, r)| r.contains(pos)) {
                    sel_state.active_block = Some(*b_idx);
                } else {
                    sel_state.active_block = None;
                }
            }

            // Draw inter-block link wires routed cleanly above/around labels
            for (from_blk, dst_pos) in in_links_to_draw {
                if let Some(&src_pos) = out_pin_positions.get(&from_blk) {
                    let route_y = (src_pos.y.min(dst_pos.y) - 36.0).max(rect.top() + 8.0);
                    painter.line_segment(
                        [src_pos, Pos2::new(src_pos.x + 12.0, src_pos.y)],
                        Stroke::new(2.0_f32, COLOR_WIRE_TRUE),
                    );
                    painter.line_segment(
                        [
                            Pos2::new(src_pos.x + 12.0, src_pos.y),
                            Pos2::new(src_pos.x + 12.0, route_y),
                        ],
                        Stroke::new(2.0_f32, COLOR_WIRE_TRUE),
                    );
                    painter.line_segment(
                        [
                            Pos2::new(src_pos.x + 12.0, route_y),
                            Pos2::new(dst_pos.x - 14.0, route_y),
                        ],
                        Stroke::new(2.0_f32, COLOR_WIRE_TRUE),
                    );
                    painter.line_segment(
                        [
                            Pos2::new(dst_pos.x - 14.0, route_y),
                            Pos2::new(dst_pos.x - 14.0, dst_pos.y),
                        ],
                        Stroke::new(2.0_f32, COLOR_WIRE_TRUE),
                    );
                    painter.line_segment(
                        [Pos2::new(dst_pos.x - 14.0, dst_pos.y), dst_pos],
                        Stroke::new(2.0_f32, COLOR_WIRE_TRUE),
                    );
                }
            }

            // Keyboard Delete / Backspace removes the selected block
            if delete_req.is_none()
                && let Some(sel) = sel_state.active_block
                && !ui.ctx().wants_keyboard_input()
                && ui.input(|i| {
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                })
            {
                delete_req = Some(sel);
            }
            if let Some(d_idx) = delete_req {
                delete_fbd_block(program, d_idx);
                sel_state.active_block = None;
                *modified = true;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blk(id: &str, inputs: HashMap<String, FbdInputSource>) -> FbdBlock {
        FbdBlock {
            id: id.to_string(),
            kind: FbdBlockKind::And,
            inputs,
            outputs: HashMap::new(),
        }
    }

    #[test]
    fn delete_block_rewires_dangling_links_to_false() {
        let mut down_inputs = HashMap::new();
        down_inputs.insert(
            "IN1".to_string(),
            FbdInputSource::Link {
                from_block: "A".to_string(),
                from_pin: "OUT".to_string(),
            },
        );
        let mut program = FbdProgram {
            name: "t".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![blk("A", HashMap::new()), blk("B", down_inputs)],
        };
        delete_fbd_block(&mut program, 0);
        assert_eq!(program.blocks.len(), 1);
        assert_eq!(program.blocks[0].id, "B");
        assert!(matches!(
            program.blocks[0].inputs.get("IN1"),
            Some(FbdInputSource::Value { expr }) if expr == "FALSE"
        ));
        // Out-of-range index is a no-op
        delete_fbd_block(&mut program, 5);
        assert_eq!(program.blocks.len(), 1);
    }
}
