use egui::{Color32, Stroke, Ui};
use ferrite_core::{DirectAddress, FbInstance, IecType, IecValue, StandardFbKind, TagEntry};
use ferrite_project::Project;

use crate::i18n::UiLanguage;

/// Renders the Global Tags (`symbols/global_tags.toml`) and Shared Function Block Instances
/// (`symbols/fb_instances.toml`) editor, plus Task Schedule settings.
pub fn render_symbols_editor(
    ui: &mut Ui,
    project: &mut Project,
    modified: &mut bool,
    lang: UiLanguage,
) {
    ui.horizontal(|ui| {
        ui.heading(lang.tr(
            "Unified Symbol Table & Function Block Registry",
            "Table des Symboles Unifiée & Registre des Blocs Fonctionnels",
        ));
        ui.separator();
        if ui
            .button(lang.tr("+ Add Global Tag", "+ Ajouter Variable Globale"))
            .clicked()
        {
            let next_n = project.global_tags.tags.len() + 1;
            project.global_tags.tags.push(TagEntry::new(
                format!("New_Tag_{next_n}"),
                IecType::Bool,
                None,
                IecValue::Bool(false),
                lang.tr("User variable", "Variable utilisateur"),
            ));
            *modified = true;
        }
        if ui
            .button(lang.tr(
                "+ Add Function Block Instance",
                "+ Ajouter Instance de Bloc FB",
            ))
            .clicked()
        {
            let next_n = project.fb_instances.instances.len() + 1;
            project
                .fb_instances
                .instances
                .push(FbInstance::new(format!("Timer_{next_n}"), StandardFbKind::Ton));
            *modified = true;
        }
        if ui
            .button(lang.tr(
                "+ Add PID Corrector",
                "+ Ajouter Correcteur PID",
            ))
            .clicked()
        {
            let next_n = project.fb_instances.instances.len() + 1;
            project
                .fb_instances
                .instances
                .push(FbInstance::new(format!("PID_{next_n}"), StandardFbKind::PidMixed));
            *modified = true;
        }
    });
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt("symbols_editor_scroll")
        .show(ui, |ui| {
            // 1. Scan Cycle & Watchdog Configuration
            egui::Frame::NONE
                .fill(Color32::from_rgb(26, 32, 42))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 65, 85)))
                .inner_margin(8.0)
                .corner_radius(5.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(lang.tr(
                            "PLC Scan Configuration:",
                            "Configuration du Cycle API :",
                        ));
                        ui.label(lang.tr("Scan Cycle (ms):", "Cycle de scrutation (ms) :"));
                        if ui
                            .add(
                                egui::DragValue::new(&mut project.manifest.scan_cycle_ms)
                                    .range(5..=2000),
                            )
                            .changed()
                        {
                            *modified = true;
                        }
                        ui.separator();
                        ui.label(lang.tr("Watchdog Timeout (us):", "Chien de garde (us) :"));
                        if ui
                            .add(
                                egui::DragValue::new(&mut project.manifest.watchdog_us)
                                    .range(100..=1_000_000),
                            )
                            .changed()
                        {
                            *modified = true;
                        }
                    });
                });
            ui.add_space(8.0);

            // 2. Global Tags Table (`symbols/global_tags.toml`)
            ui.strong(format!(
                "{} ({}) — symbols/global_tags.toml",
                lang.tr("Global Symbolic Tags", "Variables Symboliques Globales"),
                project.global_tags.tags.len()
            ));
            let mut remove_tag: Option<usize> = None;
            egui::Grid::new("global_tags_grid")
                .striped(true)
                .min_col_width(80.0)
                .show(ui, |ui| {
                    ui.strong(lang.tr("Name", "Nom"));
                    ui.strong(lang.tr("IEC Type", "Type CEI"));
                    ui.strong(lang.tr(
                        "Direct Address (%I/%Q/%M)",
                        "Adresse Directe (%I/%Q/%M)",
                    ));
                    ui.strong(lang.tr("Initial Value", "Valeur Initiale"));
                    ui.strong(lang.tr("Comment", "Commentaire"));
                    ui.strong("Action");
                    ui.end_row();

                    for (idx, tag) in project.global_tags.tags.iter_mut().enumerate() {
                        if ui.text_edit_singleline(&mut tag.name).changed() {
                            *modified = true;
                        }
                        egui::ComboBox::from_id_salt(format!("tag_type_cb_{idx}"))
                            .selected_text(tag.data_type.to_string())
                            .show_ui(ui, |ui| {
                                for ty in [
                                    IecType::Bool,
                                    IecType::Int,
                                    IecType::Dint,
                                    IecType::Real,
                                    IecType::Time,
                                    IecType::String,
                                ] {
                                    if ui
                                        .selectable_value(
                                            &mut tag.data_type,
                                            ty,
                                            ty.to_string(),
                                        )
                                        .changed()
                                    {
                                        tag.initial_value = ty.default_value();
                                        *modified = true;
                                    }
                                }
                            });

                        let mut addr_str =
                            tag.address.map(|a| a.to_string()).unwrap_or_default();
                        if ui
                            .add(egui::TextEdit::singleline(&mut addr_str).desired_width(95.0))
                            .changed()
                        {
                            if addr_str.trim().is_empty() {
                                tag.address = None;
                                *modified = true;
                            } else if let Ok(parsed) = addr_str.trim().parse::<DirectAddress>() {
                                tag.address = Some(parsed);
                                *modified = true;
                            }
                        }

                        let mut init_str = tag.initial_value.to_string();
                        if ui
                            .add(egui::TextEdit::singleline(&mut init_str).desired_width(85.0))
                            .changed()
                            && let Ok(val) =
                                IecValue::parse_literal(&init_str, Some(tag.data_type))
                        {
                            tag.initial_value = val;
                            *modified = true;
                        }

                        if ui.text_edit_singleline(&mut tag.comment).changed() {
                            *modified = true;
                        }
                        if ui
                            .small_button(lang.tr("Delete", "Supprimer"))
                            .clicked()
                        {
                            remove_tag = Some(idx);
                        }
                        ui.end_row();
                    }
                });
            if let Some(i) = remove_tag {
                project.global_tags.tags.remove(i);
                *modified = true;
            }

            ui.add_space(12.0);
            ui.separator();

            // 3. Shared Function Block Instances (`symbols/fb_instances.toml`)
            ui.strong(format!(
                "{} ({}) — symbols/fb_instances.toml",
                lang.tr(
                    "Shared Function Block Instances",
                    "Instances de Blocs Fonctionnels Partagés"
                ),
                project.fb_instances.instances.len()
            ));
            let mut remove_fb: Option<usize> = None;
            egui::Grid::new("fb_instances_grid")
                .striped(true)
                .min_col_width(110.0)
                .show(ui, |ui| {
                    ui.strong(lang.tr("Instance Name", "Nom d'Instance"));
                    ui.strong(lang.tr("FB Type", "Type FB"));
                    ui.strong(lang.tr(
                        "Parameters (Gain Kp / Ti / Td or PT / PV)",
                        "Paramètres (Gain Kp / Ti / Td ou PT / PV)",
                    ));
                    ui.strong(lang.tr("Formula / Range", "Formule / Bornes"));
                    ui.strong("Action");
                    ui.end_row();

                    for (idx, fb) in project.fb_instances.instances.iter_mut().enumerate() {
                        if ui.text_edit_singleline(&mut fb.name).changed() {
                            *modified = true;
                        }
                        egui::ComboBox::from_id_salt(format!("fb_kind_cb_{idx}"))
                            .selected_text(fb.kind.label())
                            .show_ui(ui, |ui| {
                                for k in StandardFbKind::ALL {
                                    if ui
                                        .selectable_value(&mut fb.kind, k, k.label())
                                        .changed()
                                    {
                                        if k.is_corrector() {
                                            fb.in_bool = true;
                                        }
                                        *modified = true;
                                    }
                                }
                            });
                        if fb.kind.is_corrector() {
                            ui.horizontal(|ui| {
                                ui.label("Kp:");
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut fb.kp)
                                            .speed(0.1)
                                            .range(-10000.0..=10000.0),
                                    )
                                    .changed()
                                {
                                    *modified = true;
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
                                    {
                                        *modified = true;
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
                                    {
                                        *modified = true;
                                    }
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.monospace(fb.kind.formula_hint());
                                ui.label("Min:");
                                if ui
                                    .add(egui::DragValue::new(&mut fb.out_min).speed(1.0))
                                    .changed()
                                {
                                    *modified = true;
                                }
                                ui.label("Max:");
                                if ui
                                    .add(egui::DragValue::new(&mut fb.out_max).speed(1.0))
                                    .changed()
                                {
                                    *modified = true;
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.label("PT(ms):");
                                if ui.add(egui::DragValue::new(&mut fb.pt_ms)).changed() {
                                    *modified = true;
                                }
                                ui.label("PV:");
                                if ui.add(egui::DragValue::new(&mut fb.pv)).changed() {
                                    *modified = true;
                                }
                            });
                            ui.label("-");
                        }
                        if ui
                            .small_button(lang.tr("Delete", "Supprimer"))
                            .clicked()
                        {
                            remove_fb = Some(idx);
                        }
                        ui.end_row();
                    }
                });
            if let Some(i) = remove_fb {
                project.fb_instances.instances.remove(i);
                *modified = true;
            }
        });
}
