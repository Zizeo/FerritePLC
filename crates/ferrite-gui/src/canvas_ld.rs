use std::collections::HashSet;

use egui::{Color32, FontId, Id, Order, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use ferrite_lang_ld::{CoilKind, ContactKind, LdCoil, LdElement, LdProgram, LdRung};
use ferrite_runtime::PlcRuntime;

use crate::i18n::UiLanguage;

const COLOR_ENERGIZED: Color32 = Color32::from_rgb(46, 213, 115);
const COLOR_DEENERGIZED: Color32 = Color32::from_rgb(110, 125, 145);
const COLOR_DROP_TARGET: Color32 = Color32::from_rgb(72, 202, 228);
const COLOR_SELECTED: Color32 = Color32::from_rgb(255, 215, 0);
const COLOR_BG_CANVAS: Color32 = Color32::from_rgb(18, 22, 28);
const COLOR_CARD_BG: Color32 = Color32::from_rgb(28, 34, 44);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DraggedLdContact {
    rung_idx: usize,
    elem_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LdContactDropMove {
    from_rung: usize,
    from_elem: usize,
    to_rung: usize,
    to_insert_idx: usize,
}

#[derive(Debug, Clone, Default)]
struct LdSelectionState {
    selected: HashSet<(usize, usize)>,
    marquee_start: Option<Pos2>,
    confirm_delete_multi: bool,
    confirm_clear_pou: bool,
}

fn delete_selected_contacts(program: &mut LdProgram, selected: &mut HashSet<(usize, usize)>) {
    let mut by_rung: Vec<(usize, usize)> = selected.iter().copied().collect();
    // Sort descending by (rung_idx, elem_idx) so removing elements never invalidates earlier indices
    by_rung.sort_by(|a, b| b.cmp(a));
    for (r_idx, e_idx) in by_rung {
        if let Some(rung) = program.rungs.get_mut(r_idx)
            && e_idx < rung.elements.len()
        {
            rung.elements.remove(e_idx);
        }
    }
    selected.clear();
}

/// Renders the visual 2D Ladder Diagram canvas with:
/// - Direct inline editing of contact symbols/types and coil symbols/types on the 2D rung canvas
/// - Live online power-flow highlighting (selecting a contact never toggles its runtime value)
/// - Drag-and-drop contact positioning to compatible slots
/// - Click & Drag-box (marquee) multi-contact selection
/// - `Delete` (`Suppr`) key deletion (instant for 1 contact, confirmation modal with count for multiple)
/// - `Clear POU` button with confirmation modal
pub fn render_ld_editor(
    ui: &mut Ui,
    program: &mut LdProgram,
    runtime: Option<&mut PlcRuntime>,
    modified: &mut bool,
    lang: UiLanguage,
) {
    let runtime_ref: Option<&PlcRuntime> = runtime.as_deref();
    let sel_id = Id::new(("ld_selection_state", program.name.clone()));
    let mut sel_state: LdSelectionState =
        ui.ctx().data_mut(|d| d.get_temp(sel_id)).unwrap_or_default();

    // Prune any out-of-bounds selected indices
    sel_state
        .selected
        .retain(|&(r, e)| r < program.rungs.len() && e < program.rungs[r].elements.len());

    let selected_count = sel_state.selected.len();

    // Check `Delete` (`Suppr`) key press when no text input has focus
    let suppr_pressed =
        !ui.ctx().wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Delete));
    let mut trigger_delete_selected = suppr_pressed && selected_count > 0;

    ui.horizontal_wrapped(|ui| {
        ui.heading(format!(
            "{} — {}",
            lang.tr("Ladder Diagram", "Diagramme à Contacts (Ladder)"),
            program.name
        ));
        ui.separator();
        if ui
            .button(lang.tr("+ Add Rung", "+ Ajouter Réseau"))
            .clicked()
        {
            let next_idx = program.rungs.len() + 1;
            program.rungs.push(LdRung {
                id: format!("Rung_{next_idx}"),
                comment: lang
                    .tr("New ladder rung", "Nouveau réseau Ladder")
                    .to_string(),
                elements: vec![LdElement::Contact {
                    symbol: "EStop_OK".to_string(),
                    kind: ContactKind::NormallyOpen,
                }],
                coils: vec![LdCoil {
                    symbol: "Permissive_OK".to_string(),
                    kind: CoilKind::Normal,
                }],
            });
            *modified = true;
        }

        if selected_count > 0 {
            let del_btn_label = match lang {
                UiLanguage::En => format!("Delete Selected ({selected_count}) [Del]"),
                UiLanguage::Fr => format!("Supprimer Sélection ({selected_count}) [Suppr]"),
            };
            if ui
                .add(
                    egui::Button::new(RichText::new(del_btn_label).color(Color32::WHITE))
                        .fill(Color32::from_rgb(175, 55, 55)),
                )
                .clicked()
            {
                trigger_delete_selected = true;
            }
            if ui
                .button(lang.tr("Clear Selection", "Désélectionner"))
                .clicked()
            {
                sel_state.selected.clear();
            }
        }

        if ui
            .add(
                egui::Button::new(
                    RichText::new(lang.tr("Clear POU", "Vider le POU"))
                        .color(Color32::from_rgb(255, 184, 77)),
                ),
            )
            .on_hover_text(lang.tr(
                "Clear all rungs and contacts in this Ladder POU (asks confirmation)",
                "Vider tous les réseaux et contacts de ce POU Ladder (avec confirmation)",
            ))
            .clicked()
        {
            sel_state.confirm_clear_pou = true;
        }

        ui.label(
            RichText::new(lang.tr(
                "Tip: Edit symbols & types directly inside contacts/coils | Drag contact bottom half to reorder | Drag empty space to box-select",
                "Astuce : Éditez symboles & types directement dans les contacts/bobines | Glissez le bas d'un contact pour déplacer | Glissez sur le fond pour sélectionner",
            ))
            .italics()
            .color(Color32::from_rgb(160, 175, 195)),
        );
    });
    ui.add_space(4.0);

    // Handle Delete (Suppr) trigger: immediate if 1 contact selected, confirmation popup if > 1
    if trigger_delete_selected {
        if selected_count == 1 {
            delete_selected_contacts(program, &mut sel_state.selected);
            *modified = true;
        } else if selected_count > 1 {
            sel_state.confirm_delete_multi = true;
        }
    }

    // Modal 1: Confirmation when deleting multiple selected contacts
    if sel_state.confirm_delete_multi {
        egui::Window::new(lang.tr(
            "Confirm Multiple Contact Deletion",
            "Confirmer la Suppression Multiple de Contacts",
        ))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.set_min_width(340.0);
            let msg = match lang {
                UiLanguage::En => format!(
                    "Are you sure you want to delete the {selected_count} selected contacts?"
                ),
                UiLanguage::Fr => format!(
                    "Voulez-vous vraiment supprimer les {selected_count} contacts sélectionnés ?"
                ),
            };
            ui.colored_label(Color32::from_rgb(255, 184, 77), msg);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                let confirm_lbl = match lang {
                    UiLanguage::En => format!("Delete {selected_count} Contacts"),
                    UiLanguage::Fr => format!("Supprimer {selected_count} Contacts"),
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(confirm_lbl).color(Color32::WHITE).strong())
                            .fill(Color32::from_rgb(175, 55, 55)),
                    )
                    .clicked()
                {
                    delete_selected_contacts(program, &mut sel_state.selected);
                    sel_state.confirm_delete_multi = false;
                    *modified = true;
                }
                if ui.button(lang.tr("Cancel", "Annuler")).clicked() {
                    sel_state.confirm_delete_multi = false;
                }
            });
        });
    }

    // Modal 2: Confirmation when clearing the entire Ladder POU
    if sel_state.confirm_clear_pou {
        egui::Window::new(lang.tr(
            "Confirm Clear Ladder POU",
            "Confirmer Vider le POU Ladder",
        ))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ui.ctx(), |ui| {
            ui.set_min_width(340.0);
            let msg = match lang {
                UiLanguage::En => format!(
                    "Are you sure you want to clear the entire POU '{}' ({} rungs)?",
                    program.name,
                    program.rungs.len()
                ),
                UiLanguage::Fr => format!(
                    "Voulez-vous vraiment vider entièrement le POU '{}' ({} réseaux) ?",
                    program.name,
                    program.rungs.len()
                ),
            };
            ui.colored_label(Color32::from_rgb(255, 107, 107), msg);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(lang.tr("Confirm Clear POU", "Confirmer Vider le POU"))
                                .color(Color32::WHITE)
                                .strong(),
                        )
                        .fill(Color32::from_rgb(175, 55, 55)),
                    )
                    .clicked()
                {
                    program.rungs.clear();
                    sel_state.selected.clear();
                    sel_state.confirm_clear_pou = false;
                    *modified = true;
                }
                if ui.button(lang.tr("Cancel", "Annuler")).clicked() {
                    sel_state.confirm_clear_pou = false;
                }
            });
        });
    }

    let dnd_id = Id::new("ld_contact_dnd_state");
    let active_drag: Option<DraggedLdContact> = ui.ctx().data_mut(|d| d.get_temp(dnd_id));
    let mut new_drag: Option<DraggedLdContact> = active_drag;
    let mut completed_drop: Option<LdContactDropMove> = None;

    egui::ScrollArea::vertical()
        .id_salt("ld_scroll_area")
        .show(ui, |ui| {
            let mut remove_rung_idx: Option<usize> = None;

            for (r_idx, rung) in program.rungs.iter_mut().enumerate() {
                egui::Frame::NONE
                    .fill(COLOR_CARD_BG)
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(55, 65, 85)))
                    .inner_margin(8.0)
                    .corner_radius(6.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(format!("#{} [{}]", r_idx + 1, rung.id));
                            if ui.text_edit_singleline(&mut rung.comment).changed() {
                                *modified = true;
                            }
                            if ui
                                .small_button(lang.tr("+ NO --| |--", "+ NO --| |--"))
                                .on_hover_text(lang.tr(
                                    "Add Normally Open Contact",
                                    "Ajouter Contact Normalement Ouvert",
                                ))
                                .clicked()
                            {
                                rung.elements.push(LdElement::Contact {
                                    symbol: "EStop_OK".to_string(),
                                    kind: ContactKind::NormallyOpen,
                                });
                                *modified = true;
                            }
                            if ui
                                .small_button(lang.tr("+ NC --|/|--", "+ NF --|/|--"))
                                .on_hover_text(lang.tr(
                                    "Add Normally Closed Contact",
                                    "Ajouter Contact Normalement Fermé",
                                ))
                                .clicked()
                            {
                                rung.elements.push(LdElement::Contact {
                                    symbol: "High_Temp_Alarm".to_string(),
                                    kind: ContactKind::NormallyClosed,
                                });
                                *modified = true;
                            }
                            if ui
                                .small_button(
                                    lang.tr("+ Rising --|P|--", "+ Front Montant --|P|--"),
                                )
                                .on_hover_text(lang.tr(
                                    "Add Rising Edge (Positive Transition) Contact",
                                    "Ajouter Contact à Front Montant (Transition Positive)",
                                ))
                                .clicked()
                            {
                                rung.elements.push(LdElement::Contact {
                                    symbol: "Start_Button".to_string(),
                                    kind: ContactKind::RisingEdge,
                                });
                                *modified = true;
                            }
                            if ui
                                .small_button(
                                    lang.tr("+ Falling --|N|--", "+ Front Descendant --|N|--"),
                                )
                                .on_hover_text(lang.tr(
                                    "Add Falling Edge (Negative Transition) Contact",
                                    "Ajouter Contact à Front Descendant (Transition Négative)",
                                ))
                                .clicked()
                            {
                                rung.elements.push(LdElement::Contact {
                                    symbol: "Start_Button".to_string(),
                                    kind: ContactKind::FallingEdge,
                                });
                                *modified = true;
                            }
                            if ui
                                .small_button(lang.tr("+ Coil --( )--", "+ Bobine --( )--"))
                                .clicked()
                            {
                                rung.coils.push(LdCoil {
                                    symbol: "Permissive_OK".to_string(),
                                    kind: CoilKind::Normal,
                                });
                                *modified = true;
                            }
                            if ui
                                .small_button(lang.tr("Delete Rung", "Supprimer Réseau"))
                                .clicked()
                            {
                                remove_rung_idx = Some(r_idx);
                            }
                        });

                        // Visual 2D Power Rail & Rung Diagram Canvas with direct inline fields
                        draw_rung_canvas(
                            ui,
                            r_idx,
                            rung,
                            runtime_ref,
                            modified,
                            lang,
                            &mut new_drag,
                            &mut completed_drop,
                            &mut sel_state,
                        );
                    });
                ui.add_space(6.0);
            }

            if let Some(idx) = remove_rung_idx {
                program.rungs.remove(idx);
                sel_state.selected.clear();
                *modified = true;
            }
        });

    // Apply any completed contact drag-and-drop move to a compatible contact slot
    if let Some(mv) = completed_drop {
        if mv.from_rung < program.rungs.len()
            && mv.to_rung < program.rungs.len()
            && mv.from_elem < program.rungs[mv.from_rung].elements.len()
        {
            let elem = program.rungs[mv.from_rung].elements.remove(mv.from_elem);
            let target_elements = &mut program.rungs[mv.to_rung].elements;
            let mut insert_at = mv.to_insert_idx;
            if mv.from_rung == mv.to_rung && mv.from_elem < insert_at {
                insert_at = insert_at.saturating_sub(1);
            }
            let clamped = insert_at.min(target_elements.len());
            target_elements.insert(clamped, elem);
            sel_state.selected.clear();
            sel_state.selected.insert((mv.to_rung, clamped));
            *modified = true;
        }
        new_drag = None;
    }

    // Draw floating ghost badge while dragging a contact
    if let Some(drag) = new_drag {
        sel_state.marquee_start = None;
        if ui.input(|i| i.pointer.any_released()) {
            new_drag = None;
        } else if let Some(ptr) = ui.ctx().pointer_interact_pos()
            && let Some(r) = program.rungs.get(drag.rung_idx)
            && let Some(elem) = r.elements.get(drag.elem_idx)
        {
            let label = match elem {
                LdElement::Contact { symbol, kind } => {
                    let g = match kind {
                        ContactKind::NormallyOpen => "--| |--",
                        ContactKind::NormallyClosed => "--|/|--",
                        ContactKind::RisingEdge => "--|P|--",
                        ContactKind::FallingEdge => "--|N|--",
                    };
                    format!("{symbol} {g}")
                }
                LdElement::Branch { .. } => "OR Branch".to_string(),
                LdElement::InlineFb { instance, .. } => format!("FB {instance}"),
            };
            let layer_id = egui::LayerId::new(Order::Tooltip, Id::new("ld_drag_ghost"));
            let painter = ui.ctx().layer_painter(layer_id);
            let ghost_rect =
                Rect::from_center_size(ptr + Vec2::new(0.0, -24.0), Vec2::new(135.0, 32.0));
            painter.rect(
                ghost_rect,
                5.0,
                Color32::from_rgba_unmultiplied(26, 38, 56, 230),
                Stroke::new(2.0_f32, COLOR_DROP_TARGET),
                StrokeKind::Inside,
            );
            painter.text(
                ghost_rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::monospace(12.0),
                Color32::WHITE,
            );
        }
    }

    // Draw marquee selection box if dragging across empty space
    if let Some(start) = sel_state.marquee_start {
        if ui.input(|i| i.pointer.any_released()) {
            sel_state.marquee_start = None;
        } else if let Some(ptr) = ui.ctx().pointer_interact_pos() {
            let marquee_rect = Rect::from_two_pos(start, ptr);
            let layer_id = egui::LayerId::new(Order::Tooltip, Id::new("ld_marquee_box"));
            let painter = ui.ctx().layer_painter(layer_id);
            painter.rect(
                marquee_rect,
                2.0,
                Color32::from_rgba_unmultiplied(255, 215, 0, 35),
                Stroke::new(1.6_f32, COLOR_SELECTED),
                StrokeKind::Inside,
            );
        }
    }

    ui.ctx().data_mut(|d| {
        if let Some(drag) = new_drag {
            d.insert_temp(dnd_id, drag);
        } else {
            d.remove::<DraggedLdContact>(dnd_id);
        }
        d.insert_temp(sel_id, sel_state);
    });
}

fn ld_element_width(elem: &LdElement) -> f32 {
    let char_len = match elem {
        LdElement::Contact { symbol, .. } => symbol.chars().count(),
        LdElement::Branch { .. } => 12,
        LdElement::InlineFb { instance, .. } => instance.chars().count() + 4,
    };
    (char_len as f32 * 7.6 + 54.0).max(162.0)
}

fn ld_coil_width(coil: &LdCoil) -> f32 {
    (coil.symbol.chars().count() as f32 * 7.6 + 54.0).max(162.0)
}

#[allow(clippy::too_many_arguments)]
fn draw_rung_canvas(
    ui: &mut Ui,
    rung_idx: usize,
    rung: &mut LdRung,
    runtime: Option<&PlcRuntime>,
    modified: &mut bool,
    lang: UiLanguage,
    active_drag: &mut Option<DraggedLdContact>,
    completed_drop: &mut Option<LdContactDropMove>,
    sel_state: &mut LdSelectionState,
) {
    let contacts_total_w: f32 = rung
        .elements
        .iter()
        .map(|e| ld_element_width(e) + 28.0)
        .sum();
    let coils_total_w: f32 = rung
        .coils
        .iter()
        .map(|c| ld_coil_width(c) + 20.0)
        .sum::<f32>()
        .max(175.0);
    let min_rung_width = (36.0 + contacts_total_w + 48.0 + coils_total_w + 24.0).max(680.0);

    egui::ScrollArea::horizontal()
        .id_salt(("ld_rung_hscroll", rung_idx))
        .show(ui, |ui| {
            let desired_width = ui.available_width().max(min_rung_width);
            let canvas_height = 96.0;
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(desired_width, canvas_height),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);

            painter.rect_filled(rect, 4.0, COLOR_BG_CANVAS);

            let left_rail_x = rect.left() + 18.0;
            let right_rail_x = rect.right() - 18.0;
            let mid_y = rect.center().y + 4.0;

            // Left power rail (always energized)
            painter.line_segment(
                [
                    Pos2::new(left_rail_x, rect.top() + 8.0),
                    Pos2::new(left_rail_x, rect.bottom() - 8.0),
                ],
                Stroke::new(4.0_f32, COLOR_ENERGIZED),
            );
            // Right return rail
            painter.line_segment(
                [
                    Pos2::new(right_rail_x, rect.top() + 8.0),
                    Pos2::new(right_rail_x, rect.bottom() - 8.0),
                ],
                Stroke::new(4.0_f32, COLOR_DEENERGIZED),
            );

            let click_pos = if response.clicked() {
                response.interact_pointer_pos()
            } else {
                None
            };
            let pointer_pos = ui.ctx().pointer_interact_pos();
            let pointer_released = ui.input(|i| i.pointer.any_released());
            let press_origin = ui.input(|i| i.pointer.press_origin());
            let multi_mod = ui.input(|i| i.modifiers.ctrl || i.modifiers.shift);

            // Pre-calculate contact rectangles using dynamic width per symbol
            let box_h = 64.0;
            let mut contact_rects: Vec<Rect> = Vec::with_capacity(rung.elements.len());
            let mut temp_x = left_rail_x;
            for elem in &rung.elements {
                let box_w = ld_element_width(elem);
                let nx = temp_x + 28.0;
                let br = Rect::from_min_size(
                    Pos2::new(nx, mid_y - box_h * 0.5),
                    Vec2::new(box_w, box_h),
                );
                contact_rects.push(br);
                temp_x = br.right();
            }

            // Pre-calculate coil rectangles using dynamic width per symbol
            let coil_start_x = (right_rail_x - coils_total_w).max(temp_x + 36.0);
            let mut coil_rects: Vec<Rect> = Vec::with_capacity(rung.coils.len());
            let mut temp_cx = coil_start_x;
            for coil in &rung.coils {
                let box_w = ld_coil_width(coil);
                let cr = Rect::from_min_size(
                    Pos2::new(temp_cx, mid_y - box_h * 0.5),
                    Vec2::new(box_w, box_h),
                );
                coil_rects.push(cr);
                temp_cx = cr.right() + 16.0;
            }

            let current_marquee: Option<Rect> = sel_state
                .marquee_start
                .zip(pointer_pos)
                .map(|(a, b)| Rect::from_two_pos(a, b));

            // Compatible contact drop slots along the contact portion of this rung
            let mut slot_positions: Vec<(usize, Pos2)> = Vec::with_capacity(rung.elements.len() + 1);
            let mut inline_widget_rects: Vec<Rect> = Vec::new();
            let mut remove_elem_idx: Option<usize> = None;
            let mut remove_coil_idx: Option<usize> = None;

            let mut cur_x = left_rail_x;
            let mut power = true;

            for (e_idx, &box_rect) in contact_rects.iter().enumerate() {
                let next_x = cur_x + 28.0;
                slot_positions.push((e_idx, Pos2::new((cur_x + next_x) * 0.5, mid_y)));

                let in_color = if power {
                    COLOR_ENERGIZED
                } else {
                    COLOR_DEENERGIZED
                };
                painter.line_segment(
                    [Pos2::new(cur_x, mid_y), Pos2::new(next_x, mid_y)],
                    Stroke::new(2.5_f32, in_color),
                );

                // Top area holds inline symbol TextEdit + delete button; bottom-left holds draggable contact glyph; bottom-right holds kind ComboBox
                let top_edit_rect = Rect::from_min_max(
                    Pos2::new(box_rect.left() + 5.0, box_rect.top() + 5.0),
                    Pos2::new(box_rect.right() - 28.0, box_rect.top() + 29.0),
                );
                let del_btn_rect = Rect::from_min_size(
                    Pos2::new(box_rect.right() - 25.0, box_rect.top() + 5.0),
                    Vec2::new(20.0, 22.0),
                );
                let kind_combo_rect = Rect::from_min_size(
                    Pos2::new(box_rect.right() - 62.0, box_rect.bottom() - 28.0),
                    Vec2::new(56.0, 23.0),
                );
                inline_widget_rects.push(top_edit_rect);
                inline_widget_rects.push(del_btn_rect);
                inline_widget_rects.push(kind_combo_rect);

                let on_inline_widget = |pt: Pos2| {
                    top_edit_rect.contains(pt)
                        || del_btn_rect.contains(pt)
                        || kind_combo_rect.contains(pt)
                };

                // Start dragging this contact if drag started inside `box_rect` (outside inline widgets)
                if response.drag_started()
                    && sel_state.marquee_start.is_none()
                    && let Some(origin) = press_origin
                    && box_rect.contains(origin)
                    && !on_inline_widget(origin)
                {
                    *active_drag = Some(DraggedLdContact {
                        rung_idx,
                        elem_idx: e_idx,
                    });
                    if !sel_state.selected.contains(&(rung_idx, e_idx)) {
                        if !multi_mod {
                            sel_state.selected.clear();
                        }
                        sel_state.selected.insert((rung_idx, e_idx));
                    }
                }

                // Update selection if marquee box intersects this contact
                if let Some(m_rect) = current_marquee
                    && m_rect.intersects(box_rect)
                {
                    sel_state.selected.insert((rung_idx, e_idx));
                }

                // Handle click selection on this contact (never toggles boolean state!)
                if let Some(pos) = click_pos
                    && box_rect.contains(pos)
                    && !on_inline_widget(pos)
                {
                    if multi_mod {
                        if !sel_state.selected.remove(&(rung_idx, e_idx)) {
                            sel_state.selected.insert((rung_idx, e_idx));
                        }
                    } else {
                        sel_state.selected.clear();
                        sel_state.selected.insert((rung_idx, e_idx));
                    }
                }

                let is_selected = sel_state.selected.contains(&(rung_idx, e_idx));
                let is_being_dragged = *active_drag
                    == Some(DraggedLdContact {
                        rung_idx,
                        elem_idx: e_idx,
                    });

                let elem_passes = eval_visual_element(&rung.elements[e_idx], runtime);
                power = power && elem_passes;
                let elem_color = if is_being_dragged {
                    COLOR_DROP_TARGET
                } else if is_selected {
                    COLOR_SELECTED
                } else if elem_passes {
                    COLOR_ENERGIZED
                } else {
                    COLOR_DEENERGIZED
                };

                if is_selected {
                    painter.rect_filled(
                        box_rect.expand(3.0),
                        6.0,
                        Color32::from_rgba_unmultiplied(255, 215, 0, 38),
                    );
                }

                painter.rect(
                    box_rect,
                    4.0,
                    Color32::from_rgb(26, 33, 46),
                    Stroke::new(if is_selected { 2.6_f32 } else { 2.0_f32 }, elem_color),
                    StrokeKind::Inside,
                );

                match &mut rung.elements[e_idx] {
                    LdElement::Contact { symbol, kind } => {
                        let glyph = match kind {
                            ContactKind::NormallyOpen => "--| |--",
                            ContactKind::NormallyClosed => "--|/|--",
                            ContactKind::RisingEdge => "--|P|--",
                            ContactKind::FallingEdge => "--|N|--",
                        };
                        painter.text(
                            Pos2::new(box_rect.left() + 44.0, box_rect.bottom() - 16.0),
                            egui::Align2::CENTER_CENTER,
                            glyph,
                            FontId::monospace(13.0),
                            elem_color,
                        );

                        // Direct inline Symbol TextEdit
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(top_edit_rect),
                            |ui| {
                                let resp = ui.add_sized(
                                    top_edit_rect.size(),
                                    egui::TextEdit::singleline(symbol)
                                        .font(FontId::monospace(11.0))
                                        .hint_text(lang.tr("Tag", "Symbole")),
                                );
                                if resp.changed() {
                                    *modified = true;
                                }
                            },
                        );

                        // Direct inline Delete Contact button
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(del_btn_rect),
                            |ui| {
                                ui.spacing_mut().button_padding = Vec2::new(2.0, 1.0);
                                if ui
                                    .add_sized(
                                        del_btn_rect.size(),
                                        egui::Button::new(RichText::new("x").size(10.5)),
                                    )
                                    .on_hover_text(lang.tr(
                                        "Delete contact",
                                        "Supprimer le contact",
                                    ))
                                    .clicked()
                                {
                                    remove_elem_idx = Some(e_idx);
                                    *modified = true;
                                }
                            },
                        );

                        // Direct inline ContactKind ComboBox
                        let short_kind = match kind {
                            ContactKind::NormallyOpen => "NO",
                            ContactKind::NormallyClosed => "NC",
                            ContactKind::RisingEdge => "P",
                            ContactKind::FallingEdge => "N",
                        };
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(kind_combo_rect),
                            |ui| {
                                egui::ComboBox::from_id_salt((
                                    "ld_inline_contact_kind",
                                    rung_idx,
                                    e_idx,
                                ))
                                .width(50.0)
                                .selected_text(short_kind)
                                .show_ui(ui, |ui| {
                                    for (k, lbl) in [
                                        (ContactKind::NormallyOpen, "NO --| |--"),
                                        (ContactKind::NormallyClosed, "NC --|/|--"),
                                        (ContactKind::RisingEdge, "P  --|P|--"),
                                        (ContactKind::FallingEdge, "N  --|N|--"),
                                    ] {
                                        if ui.selectable_value(kind, k, lbl).changed() {
                                            *modified = true;
                                        }
                                    }
                                });
                            },
                        );
                    }
                    LdElement::Branch { paths } => {
                        let glyph = if paths.is_empty() { "[ ]" } else { "[OR]" };
                        painter.text(
                            Pos2::new(box_rect.center().x, box_rect.top() + 18.0),
                            egui::Align2::CENTER_CENTER,
                            "OR Branch",
                            FontId::monospace(11.5),
                            Color32::WHITE,
                        );
                        painter.text(
                            Pos2::new(box_rect.center().x, box_rect.bottom() - 16.0),
                            egui::Align2::CENTER_CENTER,
                            glyph,
                            FontId::monospace(13.0),
                            elem_color,
                        );
                    }
                    LdElement::InlineFb { instance, .. } => {
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(top_edit_rect),
                            |ui| {
                                if ui
                                    .add_sized(
                                        top_edit_rect.size(),
                                        egui::TextEdit::singleline(instance)
                                            .font(FontId::monospace(11.0)),
                                    )
                                    .changed()
                                {
                                    *modified = true;
                                }
                            },
                        );
                        painter.text(
                            Pos2::new(box_rect.center().x, box_rect.bottom() - 16.0),
                            egui::Align2::CENTER_CENTER,
                            "[FB]",
                            FontId::monospace(13.0),
                            elem_color,
                        );
                    }
                }

                cur_x = box_rect.right();
            }

            // Trailing compatible contact slot (after the last contact, before the coil zone)
            slot_positions.push((rung.elements.len(), Pos2::new(cur_x + 16.0, mid_y)));

            // Render compatible contact drop slots when a contact is currently being dragged
            if let Some(drag) = *active_drag {
                for (slot_idx, slot_pos) in slot_positions {
                    let slot_rect = Rect::from_center_size(slot_pos, Vec2::new(26.0, 52.0));
                    let hovered = pointer_pos.is_some_and(|p| slot_rect.contains(p));
                    let stroke_w = if hovered { 3.0_f32 } else { 1.5_f32 };
                    let fill_col = if hovered {
                        Color32::from_rgba_unmultiplied(72, 202, 228, 70)
                    } else {
                        Color32::from_rgba_unmultiplied(72, 202, 228, 25)
                    };
                    painter.rect(
                        slot_rect,
                        4.0,
                        fill_col,
                        Stroke::new(stroke_w, COLOR_DROP_TARGET),
                        StrokeKind::Inside,
                    );
                    if hovered && pointer_released {
                        *completed_drop = Some(LdContactDropMove {
                            from_rung: drag.rung_idx,
                            from_elem: drag.elem_idx,
                            to_rung: rung_idx,
                            to_insert_idx: slot_idx,
                        });
                    }
                }
            }

            // Wire from last contact to coils on the right side
            let coil_start_x = (right_rail_x - coils_total_w).max(cur_x + 36.0);
            let rung_color = if power {
                COLOR_ENERGIZED
            } else {
                COLOR_DEENERGIZED
            };

            painter.line_segment(
                [Pos2::new(cur_x, mid_y), Pos2::new(coil_start_x, mid_y)],
                Stroke::new(2.5_f32, rung_color),
            );

            let mut cx = coil_start_x;
            for (c_idx, &coil_rect) in coil_rects.iter().enumerate() {
                let top_edit_rect = Rect::from_min_max(
                    Pos2::new(coil_rect.left() + 8.0, coil_rect.top() + 5.0),
                    Pos2::new(coil_rect.right() - 28.0, coil_rect.top() + 29.0),
                );
                let del_btn_rect = Rect::from_min_size(
                    Pos2::new(coil_rect.right() - 25.0, coil_rect.top() + 5.0),
                    Vec2::new(20.0, 22.0),
                );
                let kind_combo_rect = Rect::from_min_size(
                    Pos2::new(coil_rect.right() - 62.0, coil_rect.bottom() - 28.0),
                    Vec2::new(54.0, 23.0),
                );
                inline_widget_rects.push(top_edit_rect);
                inline_widget_rects.push(del_btn_rect);
                inline_widget_rects.push(kind_combo_rect);

                let coil_active = runtime
                    .and_then(|rt| rt.read_value(&rung.coils[c_idx].symbol).ok())
                    .and_then(|v| v.as_bool().ok())
                    .unwrap_or(power);
                let c_color = if coil_active {
                    COLOR_ENERGIZED
                } else {
                    COLOR_DEENERGIZED
                };

                painter.rect(
                    coil_rect,
                    14.0,
                    Color32::from_rgb(26, 33, 46),
                    Stroke::new(2.0_f32, c_color),
                    StrokeKind::Inside,
                );
                let glyph = match rung.coils[c_idx].kind {
                    CoilKind::Normal => "--( )--",
                    CoilKind::Set => "--(S)--",
                    CoilKind::Reset => "--(R)--",
                };
                painter.text(
                    Pos2::new(coil_rect.left() + 44.0, coil_rect.bottom() - 16.0),
                    egui::Align2::CENTER_CENTER,
                    glyph,
                    FontId::monospace(13.0),
                    c_color,
                );

                // Direct inline Coil Symbol TextEdit
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(top_edit_rect),
                    |ui| {
                        let resp = ui.add_sized(
                            top_edit_rect.size(),
                            egui::TextEdit::singleline(&mut rung.coils[c_idx].symbol)
                                .font(FontId::monospace(11.0))
                                .hint_text(lang.tr("Coil Tag", "Bobine")),
                        );
                        if resp.changed() {
                            *modified = true;
                        }
                    },
                );

                // Direct inline Delete Coil button
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(del_btn_rect),
                    |ui| {
                        ui.spacing_mut().button_padding = Vec2::new(2.0, 1.0);
                        if ui
                            .add_sized(
                                del_btn_rect.size(),
                                egui::Button::new(RichText::new("x").size(10.5)),
                            )
                            .on_hover_text(lang.tr("Delete coil", "Supprimer la bobine"))
                            .clicked()
                        {
                            remove_coil_idx = Some(c_idx);
                            *modified = true;
                        }
                    },
                );

                // Direct inline CoilKind ComboBox
                let short_coil_kind = match rung.coils[c_idx].kind {
                    CoilKind::Normal => "( )",
                    CoilKind::Set => "(S)",
                    CoilKind::Reset => "(R)",
                };
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(kind_combo_rect),
                    |ui| {
                        egui::ComboBox::from_id_salt((
                            "ld_inline_coil_kind",
                            rung_idx,
                            c_idx,
                        ))
                        .width(48.0)
                        .selected_text(short_coil_kind)
                        .show_ui(ui, |ui| {
                            for (k, lbl) in [
                                (CoilKind::Normal, "( ) Normal"),
                                (CoilKind::Set, "(S) Set"),
                                (CoilKind::Reset, "(R) Reset"),
                            ] {
                                if ui
                                    .selectable_value(&mut rung.coils[c_idx].kind, k, lbl)
                                    .changed()
                                {
                                    *modified = true;
                                }
                            }
                        });
                    },
                );

                painter.line_segment(
                    [
                        Pos2::new(coil_rect.right(), mid_y),
                        Pos2::new((coil_rect.right() + 20.0).min(right_rail_x), mid_y),
                    ],
                    Stroke::new(2.5_f32, c_color),
                );
                cx = coil_rect.right() + 16.0;
            }
            let _ = cx;

            if let Some(rm_e) = remove_elem_idx {
                rung.elements.remove(rm_e);
                sel_state.selected.clear();
            }
            if let Some(rm_c) = remove_coil_idx {
                rung.coils.remove(rm_c);
            }

            // If drag started inside this rung canvas but NOT on any contact, coil, or inline widget -> start marquee selection
            if response.drag_started()
                && active_drag.is_none()
                && let Some(origin) = press_origin
                && rect.contains(origin)
                && !contact_rects.iter().any(|r| r.contains(origin))
                && !coil_rects.iter().any(|r| r.contains(origin))
                && !inline_widget_rects.iter().any(|r| r.contains(origin))
            {
                if !multi_mod {
                    sel_state.selected.clear();
                }
                sel_state.marquee_start = Some(origin);
            }

            // If clicked on empty canvas space (not on any contact, coil, or inline widget) -> clear selection
            if let Some(cpos) = click_pos
                && !contact_rects.iter().any(|r| r.contains(cpos))
                && !coil_rects.iter().any(|r| r.contains(cpos))
                && !inline_widget_rects.iter().any(|r| r.contains(cpos))
                && !multi_mod
            {
                sel_state.selected.clear();
            }
        });
}

fn eval_visual_element(elem: &LdElement, runtime: Option<&PlcRuntime>) -> bool {
    let Some(rt) = runtime else {
        return false;
    };
    match elem {
        LdElement::Contact { symbol, kind } => {
            let sig = rt
                .read_value(symbol)
                .ok()
                .and_then(|v| v.as_bool().ok())
                .unwrap_or(false);
            match kind {
                ContactKind::NormallyOpen | ContactKind::RisingEdge => sig,
                ContactKind::NormallyClosed | ContactKind::FallingEdge => !sig,
            }
        }
        LdElement::Branch { paths } => paths
            .iter()
            .any(|p| p.iter().all(|e| eval_visual_element(e, Some(rt)))),
        LdElement::InlineFb { instance, .. } => {
            rt.tags.get_fb(instance).map(|fb| fb.q).unwrap_or(false)
        }
    }
}
