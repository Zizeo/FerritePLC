use std::ops::Range;

use iced::advanced::text::highlighter::{self, Format};
use iced::widget::{
    button, column, container, rich_text, row, scrollable, span, text, text_editor,
};
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

pub fn build_st_gutter_spans<'a>(
    line_count: usize,
    cursor_line: usize,
    err_line: Option<usize>,
    pou_name: &'a str,
) -> Vec<iced::widget::text::Span<'a, Message, Font>> {
    let digits = line_count.to_string().len().max(2);
    let mut spans = Vec::with_capacity(line_count);
    for idx in 0..line_count {
        let n = idx + 1;
        let is_current = idx == cursor_line;
        let is_error = err_line == Some(n);

        let (indicator, color) = if is_error {
            ("!", StudioTheme::ACCENT_ROSE)
        } else if is_current {
            (">", StudioTheme::ACCENT_CYAN)
        } else {
            (" ", StudioTheme::TEXT_MUTED)
        };

        let nl = if idx + 1 < line_count { "\n" } else { "" };
        let formatted = format!("{indicator} {:>digits$}{nl}", n, digits = digits);

        let s = span(formatted)
            .font(Font::MONOSPACE)
            .size(13)
            .color(color)
            .link(Message::StJumpToLine(pou_name.to_string(), idx));

        spans.push(s);
    }
    spans
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

    let err_line = match &parse_status {
        Err(ferrite_lang_st::StError::SyntaxError { message, .. }) => message
            .strip_prefix("line ")
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<usize>().ok()),
        Err(ferrite_lang_st::StError::LexError { span_start, .. }) => {
            Some(source_str[..(*span_start).min(source_str.len())].lines().count())
        }
        _ => None,
    };

    let line_count = content.line_count().max(1);
    let spans = build_st_gutter_spans(line_count, cursor_line, err_line, name);
    let gutter_widget = rich_text(spans).font(Font::MONOSPACE).size(13);

    let gutter_col = container(gutter_widget)
        .padding(iced::Padding {
            top: 12.0,
            right: 10.0,
            bottom: 12.0,
            left: 8.0,
        })
        .style(StudioTheme::gutter_style);

    let editor_widget = text_editor(content)
        .on_action(move |action| Message::StEditorAction(pou_name.clone(), action))
        .font(Font::MONOSPACE)
        .size(13)
        .highlight_with::<StSyntaxHighlighter>((), |highlight, _theme| highlight.to_format())
        .padding(12);

    let editor_row = row![gutter_col, editor_widget]
        .spacing(0)
        .width(Length::Fill);

    let editor_scroll = scrollable(editor_row)
        .id(scrollable::Id::new("st_editor_scrollable"))
        .width(Length::Fill)
        .height(Length::Fill);

    let editor_container = container(editor_scroll)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_gutter_active_line_and_scaling() {
        let spans = build_st_gutter_spans(5, 2, None, "test_pou");
        assert_eq!(spans.len(), 5);

        // Line 1 (index 0): inactive
        assert_eq!(spans[0].text.as_ref(), "   1\n");
        assert_eq!(spans[0].color, Some(StudioTheme::TEXT_MUTED));
        assert!(matches!(&spans[0].link, Some(Message::StJumpToLine(name, 0)) if name == "test_pou"));

        // Line 3 (index 2): active cursor line
        assert_eq!(spans[2].text.as_ref(), ">  3\n");
        assert_eq!(spans[2].color, Some(StudioTheme::ACCENT_CYAN));
        assert!(matches!(&spans[2].link, Some(Message::StJumpToLine(name, 2)) if name == "test_pou"));

        // Line 5 (index 4): last line, no trailing newline
        assert_eq!(spans[4].text.as_ref(), "   5");
        assert_eq!(spans[4].color, Some(StudioTheme::TEXT_MUTED));
    }

    #[test]
    fn test_dynamic_gutter_error_line_indicator() {
        let spans = build_st_gutter_spans(4, 0, Some(2), "calc_pid");
        assert_eq!(spans.len(), 4);

        // Line 1 (index 0): active cursor line
        assert_eq!(spans[0].text.as_ref(), ">  1\n");
        assert_eq!(spans[0].color, Some(StudioTheme::ACCENT_CYAN));

        // Line 2 (index 1): error line
        assert_eq!(spans[1].text.as_ref(), "!  2\n");
        assert_eq!(spans[1].color, Some(StudioTheme::ACCENT_ROSE));
        assert!(matches!(&spans[1].link, Some(Message::StJumpToLine(name, 1)) if name == "calc_pid"));
    }

    #[test]
    fn test_dynamic_gutter_triple_digit_scaling() {
        let spans = build_st_gutter_spans(120, 99, None, "large_st");
        assert_eq!(spans.len(), 120);

        // With 120 lines, digits = 3
        // Line 1 should be right-padded to 3 digits
        assert_eq!(spans[0].text.as_ref(), "    1\n");

        // Line 100 (index 99, active cursor line):
        assert_eq!(spans[99].text.as_ref(), "> 100\n");
        assert_eq!(spans[99].color, Some(StudioTheme::ACCENT_CYAN));

        // Line 120 (index 119, last line):
        assert_eq!(spans[119].text.as_ref(), "  120");
    }

    #[test]
    fn test_render_st_editor_runs_cleanly() {
        let content = text_editor::Content::with_text("x := 42;\nIF x > 10 THEN\n  y := 1;\nEND_IF;");
        let _element = render_st_editor("test_st", &content, None, UiLanguage::En);
    }
}
