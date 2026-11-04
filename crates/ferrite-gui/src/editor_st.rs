use std::ops::Range;

use iced::advanced::text::highlighter::{self, Format};
use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Alignment, Color, Element, Font, Length};

use ferrite_lang_st::{Token, parse_st_statements, tokenize_with_spans};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StHighlightKind {
    Keyword,
    Literal,
    Operator,
    Identifier,
    Comment,
}

impl StHighlightKind {
    pub fn to_format(self) -> Format<Font> {
        let color = match self {
            StHighlightKind::Keyword => Some(StudioTheme::ACCENT_PURPLE),
            StHighlightKind::Literal => Some(StudioTheme::ACCENT_AMBER),
            StHighlightKind::Operator => Some(StudioTheme::ACCENT_CYAN),
            StHighlightKind::Identifier => Some(StudioTheme::TEXT_PRIMARY),
            StHighlightKind::Comment => Some(Color::from_rgb(0.45, 0.55, 0.50)),
        };
        Format { color, font: None }
    }
}

pub struct StSyntaxHighlighter {
    current_line: usize,
}

impl highlighter::Highlighter for StSyntaxHighlighter {
    type Settings = ();
    type Highlight = StHighlightKind;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Self::Highlight)>;

    fn new(_settings: &Self::Settings) -> Self {
        Self { current_line: 0 }
    }

    fn update(&mut self, _new_settings: &Self::Settings) {}

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let mut spans = Vec::new();
        if let Some(comment_idx) = line.find("//").or_else(|| line.find("(*")) {
            let code_part = &line[..comment_idx];
            if let Ok((toks, ranges)) = tokenize_with_spans(code_part, "line") {
                for (tok, range) in toks.into_iter().zip(ranges) {
                    spans.push((range, classify_st_token(&tok)));
                }
            }
            spans.push((comment_idx..line.len(), StHighlightKind::Comment));
        } else if let Ok((toks, ranges)) = tokenize_with_spans(line, "line") {
            for (tok, range) in toks.into_iter().zip(ranges) {
                spans.push((range, classify_st_token(&tok)));
            }
        }
        self.current_line += 1;
        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

fn classify_st_token(tok: &Token) -> StHighlightKind {
    match tok {
        Token::Program
        | Token::EndProgram
        | Token::If
        | Token::Then
        | Token::Elsif
        | Token::Else
        | Token::EndIf
        | Token::Case
        | Token::Of
        | Token::EndCase
        | Token::For
        | Token::To
        | Token::By
        | Token::Do
        | Token::EndFor
        | Token::While
        | Token::EndWhile
        | Token::Repeat
        | Token::Until
        | Token::EndRepeat
        | Token::Exit
        | Token::Continue
        | Token::Return
        | Token::And
        | Token::Or
        | Token::Xor
        | Token::Not
        | Token::Mod => StHighlightKind::Keyword,
        Token::True
        | Token::False
        | Token::TimeLiteral(_)
        | Token::DirectAddress(_)
        | Token::RealLiteral(_)
        | Token::IntLiteral(_)
        | Token::StringLiteral(_) => StHighlightKind::Literal,
        Token::Ident(_) => StHighlightKind::Identifier,
        _ => StHighlightKind::Operator,
    }
}

pub fn render_st_editor<'a>(
    name: &'a str,
    content: &'a text_editor::Content,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let source_str = content.text();
    let parse_status = parse_st_statements(&source_str, name);

    let (cursor_line, cursor_col) = content.cursor_position();

    let status_badge = match &parse_status {
        Ok(stmts) => {
            let msg = match lang {
                UiLanguage::En => format!("Syntax OK ({} statement(s))", stmts.len()),
                UiLanguage::Fr => format!("Syntaxe OK ({} instruction(s))", stmts.len()),
                UiLanguage::De => format!("Syntax OK ({} Anweisung(en))", stmts.len()),
                UiLanguage::Es => format!("Sintaxis OK ({} instrucción(es))", stmts.len()),
                UiLanguage::It => format!("Sintassi OK ({} istruzione/i)", stmts.len()),
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

    let cursor_badge = container(
        text(format!("Ln {}, Col {}", cursor_line + 1, cursor_col + 1))
            .size(11)
            .font(Font::MONOSPACE)
            .color(StudioTheme::TEXT_SECONDARY),
    )
    .padding([4, 8])
    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
        1.0, 1.0, 1.0, 0.05,
    )));

    let pou_name = name.to_string();
    let header = row![
        text(format!(
            "{} — {}.st",
            lang.tr("Structured Text", "Texte Structuré (ST)"),
            name
        ))
        .size(16),
        status_badge,
        cursor_badge,
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("Clear POU", "Vider le POU")).size(12))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([8, 16]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let line_count = content.line_count().max(1);
    let gutter_str: String = (1..=line_count)
        .map(|n| format!("{n:>3}"))
        .collect::<Vec<_>>()
        .join("\n");

    let gutter_col = container(
        text(gutter_str)
            .size(13)
            .font(Font::MONOSPACE)
            .color(StudioTheme::TEXT_MUTED),
    )
    .padding([12, 8])
    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
        1.0, 1.0, 1.0, 0.03,
    )));

    let editor_widget = text_editor(content)
        .on_action(move |action| Message::StEditorAction(pou_name.clone(), action))
        .font(Font::MONOSPACE)
        .size(13)
        .highlight_with::<StSyntaxHighlighter>((), |highlight, _theme| highlight.to_format())
        .padding(12);

    let editor_row = row![gutter_col, editor_widget]
        .spacing(0)
        .height(Length::Fill);

    let editor_container = container(editor_row)
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
                    text(&tag.name).size(12).width(Length::FillPortion(2)),
                    text(tag.data_type.to_string())
                        .size(12)
                        .color(StudioTheme::TEXT_MUTED)
                        .width(Length::FillPortion(1)),
                    text(val_str)
                        .size(12)
                        .color(StudioTheme::ACCENT_CYAN)
                        .width(Length::FillPortion(2)),
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
