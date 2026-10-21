use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Id, RichText, Ui};
use ferrite_lang_st::{StProgram, parse_st_statements};
use ferrite_runtime::PlcRuntime;

use crate::i18n::UiLanguage;

/// Renders the Structured Text (`StProgram`) code editor with IEC 61131-3 syntax highlighting,
/// live parser verification, and inline symbol monitoring.
pub fn render_st_editor(
    ui: &mut Ui,
    program: &mut StProgram,
    runtime: Option<&PlcRuntime>,
    modified: &mut bool,
    lang: UiLanguage,
) {
    let clear_id = Id::new(("st_confirm_clear_pou", program.name.clone()));
    let mut confirm_clear: bool = ui.ctx().data_mut(|d| d.get_temp(clear_id)).unwrap_or(false);

    let parse_status = parse_st_statements(&program.source, &program.name);

    ui.horizontal(|ui| {
        ui.heading(format!(
            "{} — {}.st",
            lang.tr("Structured Text", "Texte Structuré (ST)"),
            program.name
        ));
        ui.separator();
        if ui
            .button(
                RichText::new(lang.tr("Clear POU", "Vider le POU"))
                    .color(Color32::from_rgb(255, 107, 107)),
            )
            .on_hover_text(lang.tr(
                "Clear all Structured Text source code in this POU (asks confirmation)",
                "Effacer tout le code ST de ce POU (avec confirmation)",
            ))
            .clicked()
        {
            confirm_clear = true;
        }
        ui.separator();
        match &parse_status {
            Ok(stmts) => {
                let msg = match lang {
                    UiLanguage::En => format!("Syntax OK ({} top-level statement(s))", stmts.len()),
                    UiLanguage::Fr => {
                        format!("Syntaxe OK ({} instruction(s) principale(s))", stmts.len())
                    }
                };
                ui.colored_label(Color32::from_rgb(46, 213, 115), msg);
            }
            Err(err) => {
                ui.colored_label(Color32::from_rgb(255, 107, 107), err.to_string());
            }
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
                UiLanguage::En => {
                    format!("Are you sure you want to clear the entire POU '{}.st'?", program.name)
                }
                UiLanguage::Fr => {
                    format!("Voulez-vous vraiment vider tout le POU '{}.st' ?", program.name)
                }
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
                    program.source.clear();
                    program.statements.clear();
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

    let mut layouter = |ui: &Ui, string: &str, wrap_width: f32| {
        let mut job = highlight_st_source(string);
        job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(job))
    };

    egui::ScrollArea::vertical()
        .id_salt("st_editor_scroll")
        .show(ui, |ui| {
            let response = ui.add(
                egui::TextEdit::multiline(&mut program.source)
                    .font(FontId::monospace(13.5))
                    .code_editor()
                    .desired_rows(18)
                    .desired_width(f32::INFINITY)
                    .layouter(&mut layouter),
            );
            if response.changed() {
                *modified = true;
                if let Ok(stmts) = parse_st_statements(&program.source, &program.name) {
                    program.statements = stmts;
                }
            }

            if let Some(rt) = runtime {
                ui.add_space(8.0);
                ui.separator();
                ui.strong(lang.tr(
                    "Live Process Values Referenced in PLC:",
                    "Valeurs Temps Réel Référencées dans l'API :",
                ));
                egui::Grid::new("st_live_watch_grid")
                    .striped(true)
                    .min_col_width(110.0)
                    .show(ui, |ui| {
                        for tag in rt.tags.tags() {
                            if program.source.contains(&tag.name) {
                                ui.monospace(&tag.name);
                                ui.label(tag.data_type.to_string());
                                let val_str = rt
                                    .read_value(&tag.name)
                                    .map(|v| v.to_string())
                                    .unwrap_or_else(|_| "-".to_string());
                                ui.colored_label(Color32::from_rgb(72, 202, 228), val_str);
                                ui.end_row();
                            }
                        }
                    });
            }
        });
}

/// Builds an `egui::text::LayoutJob` with IEC 61131-3 Structured Text syntax highlighting.
pub fn highlight_st_source(src: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font = FontId::monospace(13.5);

    let fmt_default = TextFormat {
        font_id: font.clone(),
        color: Color32::from_rgb(220, 228, 240),
        ..Default::default()
    };
    let fmt_keyword = TextFormat {
        font_id: font.clone(),
        color: Color32::from_rgb(199, 146, 234),
        ..Default::default()
    };
    let fmt_comment = TextFormat {
        font_id: font.clone(),
        color: Color32::from_rgb(110, 130, 150),
        italics: true,
        ..Default::default()
    };
    let fmt_literal = TextFormat {
        font_id: font.clone(),
        color: Color32::from_rgb(247, 140, 108),
        ..Default::default()
    };
    let fmt_addr = TextFormat {
        font_id: font,
        color: Color32::from_rgb(130, 170, 255),
        ..Default::default()
    };

    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        // Check `(* ... *)` comment
        if i + 1 < len && bytes[i] == b'(' && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b')') {
                i += 1;
            }
            i = (i + 2).min(len);
            job.append(&src[start..i], 0.0, fmt_comment.clone());
            continue;
        }
        // Check `// ...` comment
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            let start = i;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
            job.append(&src[start..i], 0.0, fmt_comment.clone());
            continue;
        }
        // Direct address `%IX...`
        if bytes[i] == b'%' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.') {
                i += 1;
            }
            job.append(&src[start..i], 0.0, fmt_addr.clone());
            continue;
        }
        // Word / Keyword / Time literal
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'#'
                    || bytes[i] == b'.')
            {
                i += 1;
            }
            let word = &src[start..i];
            let upper = word.to_ascii_uppercase();
            if upper.starts_with("T#")
                || upper.starts_with("TIME#")
                || upper == "TRUE"
                || upper == "FALSE"
            {
                job.append(word, 0.0, fmt_literal.clone());
            } else if is_st_keyword(&upper) {
                job.append(word, 0.0, fmt_keyword.clone());
            } else if word.starts_with('X') && (word.ends_with(".state") || word.ends_with(".t")) {
                job.append(word, 0.0, fmt_addr.clone());
            } else {
                job.append(word, 0.0, fmt_default.clone());
            }
            continue;
        }
        // Numeric literal
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            job.append(&src[start..i], 0.0, fmt_literal.clone());
            continue;
        }

        // Single UTF-8 character fallback
        let ch_len = src[i..].chars().next().map_or(1, char::len_utf8);
        job.append(&src[i..i + ch_len], 0.0, fmt_default.clone());
        i += ch_len;
    }

    job
}

fn is_st_keyword(upper: &str) -> bool {
    matches!(
        upper,
        "PROGRAM"
            | "END_PROGRAM"
            | "IF"
            | "THEN"
            | "ELSIF"
            | "ELSE"
            | "END_IF"
            | "CASE"
            | "OF"
            | "END_CASE"
            | "FOR"
            | "TO"
            | "BY"
            | "DO"
            | "END_FOR"
            | "WHILE"
            | "END_WHILE"
            | "AND"
            | "OR"
            | "XOR"
            | "NOT"
            | "MOD"
    )
}
