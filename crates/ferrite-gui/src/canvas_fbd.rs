use iced::widget::{button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use ferrite_lang_fbd::{FbdBlockKind, FbdInputSource, FbdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

/// Deletes block at `idx` and safely rewires any downstream link inputs pointing to it to `"FALSE"`.
pub fn delete_fbd_block(program: &mut FbdProgram, idx: usize) {
    if idx >= program.blocks.len() {
        return;
    }
    let removed = program.blocks.remove(idx);
    let removed_id = removed.id;

    for blk in &mut program.blocks {
        for src in blk.inputs.values_mut() {
            if let FbdInputSource::Link { from_block, .. } = src
                && *from_block == removed_id {
                    *src = FbdInputSource::Value {
                        expr: "FALSE".to_string(),
                    };
                }
        }
    }
}

pub fn render_fbd_editor<'a>(
    program: &'a FbdProgram,
    _runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Toolbar Header
    let header = row![
        text(format!(
            "{} — {}",
            lang.tr("Function Block Diagram (FBD)", "Diagramme Blocs Fonctionnels (FBD)"),
            program.name
        ))
        .size(18),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add AND Block", "+ Bloc ET")).size(12))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::And))
            .style(StudioTheme::primary_button)
            .padding([6, 12]),
        button(text(lang.tr("+ Add OR Block", "+ Bloc OU")).size(12))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::Or))
            .style(StudioTheme::secondary_button)
            .padding([6, 12]),
        button(text(lang.tr("+ Add NOT Block", "+ Bloc NON")).size(12))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::Not))
            .style(StudioTheme::secondary_button)
            .padding([6, 12]),
        button(text(lang.tr("+ Add GT Block (>)", "+ Bloc SUP (> )")).size(12))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::Gt))
            .style(StudioTheme::secondary_button)
            .padding([6, 12]),
        button(text(lang.tr("Clear POU", "Vider le POU")).size(12))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([6, 12]),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Tip: FBD blocks evaluate in topological signal-flow order. Inputs can bind to variables (%IW0, Tank_Level) or block outputs (Block_1.OUT).",
        "Astuce : Les blocs FBD sont évalués par ordre topologique. Les entrées peuvent se lier à des variables (%IW0) ou sorties de blocs.",
    ))
    .size(12)
    .color(StudioTheme::TEXT_MUTED);

    let mut blocks_column = column![header, tip].spacing(16);

    for (b_idx, blk) in program.blocks.iter().enumerate() {
        let p_blk = pou_name.clone();

        let kind_str = match &blk.kind {
            FbdBlockKind::And => "AND",
            FbdBlockKind::Or => "OR",
            FbdBlockKind::Xor => "XOR",
            FbdBlockKind::Not => "NOT",
            FbdBlockKind::Add => "ADD (+)",
            FbdBlockKind::Sub => "SUB (-)",
            FbdBlockKind::Mul => "MUL (*)",
            FbdBlockKind::Div => "DIV (/)",
            FbdBlockKind::Gt => "GT (>)",
            FbdBlockKind::Ge => "GE (>=)",
            FbdBlockKind::Lt => "LT (<)",
            FbdBlockKind::Le => "LE (<=)",
            FbdBlockKind::Eq => "EQ (==)",
            FbdBlockKind::Ne => "NE (!=)",
            FbdBlockKind::StandardFb { instance, .. } => instance.as_str(),
            FbdBlockKind::Composite { type_name } => type_name.as_str(),
        };

        let blk_header = row![
            container(
                text(&blk.id).size(13).color(StudioTheme::ACCENT_CYAN)
            )
            .padding([3, 8])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.220, 0.741, 0.973, 0.12))),
            container(
                text(kind_str).size(12).color(StudioTheme::TEXT_PRIMARY)
            )
            .padding([3, 8])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
            iced::widget::Space::with_width(Length::Fill),
            button(text(lang.tr("Delete Block", "Supprimer Bloc")).size(11))
                .on_press(Message::FbdRemoveBlock(p_blk.clone(), b_idx))
                .style(StudioTheme::danger_button)
                .padding([4, 8]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        // Inputs Column
        let mut inputs_col = column![
            text(lang.tr("Inputs (Sources / Wires)", "Entrées (Sources / Fils)"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
        ]
        .spacing(6);

        let mut sorted_inputs: Vec<_> = blk.inputs.iter().collect();
        sorted_inputs.sort_by_key(|&(k, _)| k);

        if sorted_inputs.is_empty() {
            inputs_col = inputs_col.push(
                text(lang.tr("(No inputs configured)", "(Aucune entrée configurée)"))
                    .size(11)
                    .color(StudioTheme::TEXT_MUTED),
            );
        } else {
            for (pin_name, src) in sorted_inputs {
                let pin_n = pin_name.clone();
                let expr_str = match src {
                    FbdInputSource::Value { expr } => expr.clone(),
                    FbdInputSource::Link { from_block, from_pin } => format!("{from_block}.{from_pin}"),
                };

                let in_row = row![
                    text(pin_name).size(11).color(StudioTheme::TEXT_SECONDARY).width(60),
                    text_input("Source / %IX0.0", &expr_str)
                        .on_input({
                            let p = p_blk.clone();
                            let pn = pin_n.clone();
                            move |s| Message::FbdInputSourceChanged(p.clone(), b_idx, pn.clone(), s)
                        })
                        .width(180)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                inputs_col = inputs_col.push(in_row);
            }
        }

        // Outputs Column
        let mut outputs_col = column![
            text(lang.tr("Outputs (Target Symbols)", "Sorties (Variables Cibles)"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
        ]
        .spacing(6);

        let mut sorted_outputs: Vec<_> = blk.outputs.iter().collect();
        sorted_outputs.sort_by_key(|&(k, _)| k);

        if sorted_outputs.is_empty() {
            outputs_col = outputs_col.push(
                text(lang.tr("(Internal to network)", "(Interne au réseau)"))
                    .size(11)
                    .color(StudioTheme::TEXT_MUTED),
            );
        } else {
            for (pin_name, target) in sorted_outputs {
                let pin_n = pin_name.clone();
                let out_row = row![
                    text(pin_name).size(11).color(StudioTheme::TEXT_SECONDARY).width(60),
                    text_input("Target / %QX0.0", target)
                        .on_input({
                            let p = p_blk.clone();
                            let pn = pin_n.clone();
                            move |s| Message::FbdOutputTargetChanged(p.clone(), b_idx, pn.clone(), s)
                        })
                        .width(180)
                        .padding([3, 6])
                        .style(StudioTheme::styled_input),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                outputs_col = outputs_col.push(out_row);
            }
        }

        let block_flow = row![
            container(inputs_col).width(Length::FillPortion(1)),
            container(
                text("==>").size(14).color(StudioTheme::ACCENT_CYAN)
            )
            .padding([0, 16])
            .center_x(Length::Shrink)
            .center_y(Length::Shrink),
            container(outputs_col).width(Length::FillPortion(1)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        let block_card = container(
            column![
                blk_header,
                container(block_flow)
                    .padding(12)
                    .style(StudioTheme::canvas_container_style)
                    .width(Length::Fill),
            ]
            .spacing(10),
        )
        .padding(14)
        .style(StudioTheme::card_style)
        .width(Length::Fill);

        blocks_column = blocks_column.push(block_card);
    }

    scrollable(blocks_column.padding(20))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use ferrite_lang_fbd::FbdBlock;

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
