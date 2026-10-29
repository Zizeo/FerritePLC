use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Alignment, Element, Length};

use ferrite_lang_st::parse_st_statements;
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub fn render_st_editor<'a>(
    name: &'a str,
    content: &'a text_editor::Content,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let source_str = content.text();
    let parse_status = parse_st_statements(&source_str, name);

    let status_badge = match &parse_status {
        Ok(stmts) => {
            let msg = match lang {
                UiLanguage::En => format!("Syntax OK ({} statement(s))", stmts.len()),
                UiLanguage::Fr => format!("Syntaxe OK ({} instruction(s))", stmts.len()),
            };
            container(text(msg).size(12).color(StudioTheme::ACCENT_EMERALD))
                .padding([4, 10])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                    0.063, 0.725, 0.506, 0.15,
                )))
        }
        Err(err) => container(
            text(err.to_string())
                .size(12)
                .color(StudioTheme::ACCENT_ROSE),
        )
        .padding([4, 10])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            0.957, 0.247, 0.369, 0.15,
        ))),
    };

    let pou_name = name.to_string();
    let header = row![
        text(format!(
            "{} — {}.st",
            lang.tr("Structured Text", "Texte Structuré (ST)"),
            name
        ))
        .size(16),
        status_badge,
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("Clear POU", "Vider le POU")).size(12))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([6, 12]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let editor_widget = text_editor(content)
        .on_action(move |action| Message::StEditorAction(pou_name.clone(), action))
        .padding(12);

    let editor_container = container(editor_widget)
        .style(StudioTheme::canvas_container_style)
        .width(Length::Fill)
        .height(Length::FillPortion(3));

    // Live Referenced Process Values in PLC
    let mut live_table = column![
        text(lang.tr(
            "Live Process Values Referenced in PLC:",
            "Valeurs Temps Réel Référencées dans l'API :",
        ))
        .size(13)
        .color(StudioTheme::TEXT_SECONDARY),
    ]
    .spacing(6);

    if let Some(rt) = runtime {
        let mut found_any = false;
        for tag in rt.tags.tags() {
            if source_str.contains(&tag.name) {
                found_any = true;
                let val_str = rt
                    .read_value(&tag.name)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|_| "-".to_string());

                let tag_row = row![
                    text(&tag.name).size(12).width(160),
                    text(tag.data_type.to_string())
                        .size(12)
                        .color(StudioTheme::TEXT_MUTED)
                        .width(90),
                    text(val_str)
                        .size(12)
                        .color(StudioTheme::ACCENT_CYAN)
                        .width(120),
                ]
                .spacing(12)
                .align_y(Alignment::Center);

                live_table = live_table.push(tag_row);
            }
        }
        if !found_any {
            live_table = live_table.push(
                text(lang.tr(
                    "No matching symbolic tags referenced in active ST code.",
                    "Aucune variable symbolique référencée dans le code ST actif.",
                ))
                .size(12)
                .color(StudioTheme::TEXT_MUTED),
            );
        }
    }

    let live_card = container(scrollable(live_table).height(120))
        .padding(14)
        .style(StudioTheme::card_style)
        .width(Length::Fill)
        .height(Length::FillPortion(1));

    column![header, editor_container, live_card,]
        .spacing(12)
        .padding(16)
        .into()
}
