use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Theme};

use ferrite_core::{IecValue, StandardFbKind};
use ferrite_lang_fbd::{FbdBlock, FbdBlockKind, FbdInputSource, FbdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub fn delete_fbd_block(program: &mut FbdProgram, idx: usize) {
    if idx >= program.blocks.len() {
        return;
    }
    let removed = program.blocks.remove(idx);
    let removed_id = removed.id;

    for blk in &mut program.blocks {
        for src in blk.inputs.values_mut() {
            if let FbdInputSource::Link { from_block, .. } = src
                && *from_block == removed_id
            {
                *src = FbdInputSource::Value {
                    expr: "FALSE".to_string(),
                };
            }
        }
    }
}

fn is_input_energized(rt: &PlcRuntime, src: &FbdInputSource) -> bool {
    match src {
        FbdInputSource::Value { expr } => {
            if let Ok(IecValue::Bool(b)) = rt.read_value(expr) {
                b
            } else {
                expr.trim().eq_ignore_ascii_case("true")
            }
        }
        FbdInputSource::Link {
            from_block,
            from_pin,
        } => {
            let sym = format!("{from_block}.{from_pin}");
            matches!(rt.read_value(&sym), Ok(IecValue::Bool(true)))
        }
    }
}

fn is_output_energized(rt: &PlcRuntime, target: &str) -> bool {
    matches!(rt.read_value(target), Ok(IecValue::Bool(true)))
}

fn operator_symbol(kind: &FbdBlockKind) -> (&str, &'static str) {
    match kind {
        FbdBlockKind::And => ("&", "AND"),
        FbdBlockKind::Or => ("\u{2265}1", "OR"),
        FbdBlockKind::Xor => ("=1", "XOR"),
        FbdBlockKind::Not => ("1", "NOT"),
        FbdBlockKind::Add => ("+", "ADD"),
        FbdBlockKind::Sub => ("-", "SUB"),
        FbdBlockKind::Mul => ("\u{00D7}", "MUL"),
        FbdBlockKind::Div => ("\u{00F7}", "DIV"),
        FbdBlockKind::Gt => (">", "GT"),
        FbdBlockKind::Ge => ("\u{2265}", "GE"),
        FbdBlockKind::Lt => ("<", "LT"),
        FbdBlockKind::Le => ("\u{2264}", "LE"),
        FbdBlockKind::Eq => ("==", "EQ"),
        FbdBlockKind::Ne => ("\u{2260}", "NE"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ton),
            ..
        } => ("TON", "TIMER ON"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Tof),
            ..
        } => ("TOF", "TIMER OFF"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Tp),
            ..
        } => ("TP", "PULSE"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctu),
            ..
        } => ("CTU", "COUNT UP"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Ctd),
            ..
        } => ("CTD", "COUNT DOWN"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidParallel),
            ..
        } => ("PID_PAR", "PARALLEL"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidSeries),
            ..
        } => ("PID_SER", "SERIES"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::PidMixed),
            ..
        } => ("PID_MIX", "MIXED (ISA)"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::Pi),
            ..
        } => ("PI", "CORRECTOR"),
        FbdBlockKind::StandardFb {
            fb_type: Some(StandardFbKind::P),
            ..
        } => ("P", "CORRECTOR"),
        FbdBlockKind::StandardFb { instance, .. } => (instance.as_str(), "FB"),
        FbdBlockKind::Composite { type_name } => (type_name.as_str(), "COMPOSITE"),
    }
}

fn pin_display_label(pin: &str, is_corrector: bool, lang: UiLanguage) -> String {
    if is_corrector {
        match pin.to_ascii_uppercase().as_str() {
            "SP" => lang.tr("Consigne (SP)", "Consigne (SP)").to_string(),
            "PV" => lang.tr("Mesure (PV)", "Mesure (PV)").to_string(),
            "KP" => lang.tr("Gain (Kp)", "Gain (Kp)").to_string(),
            "TI" => "Ti (s)".to_string(),
            "TD" => "Td (s)".to_string(),
            "OUT" => lang.tr("Commande (OUT)", "Commande (OUT)").to_string(),
            other => other.to_string(),
        }
    } else {
        pin.to_string()
    }
}

fn render_single_block_card<'a>(
    program_name: &str,
    b_idx: usize,
    blk: &'a FbdBlock,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let p_blk = program_name.to_string();
    let (op_symbol, op_desc) = operator_symbol(&blk.kind);

    let is_corrector = match &blk.kind {
        FbdBlockKind::StandardFb {
            fb_type: Some(k), ..
        } => k.is_corrector(),
        _ => false,
    };
    let current_corrector_kind = match &blk.kind {
        FbdBlockKind::StandardFb {
            fb_type: Some(k), ..
        } if k.is_corrector() => Some(*k),
        _ => None,
    };

    // Block Header with Editable ID, Reorder Arrows, and Remove Button
    let blk_header = row![
        text_input("Block ID", &blk.id)
            .on_input({
                let p = p_blk.clone();
                move |s| Message::FbdBlockIdChanged(p.clone(), b_idx, s)
            })
            .width(115)
            .padding([2, 6])
            .style(StudioTheme::styled_input),
        container(text(op_desc).size(11).color(StudioTheme::TEXT_MUTED))
            .padding([3, 7])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.05
            ))),
        iced::widget::Space::with_width(Length::Fill),
        button(text("\u{25B2}").size(10.0))
            .on_press(Message::FbdMoveBlockUp(p_blk.clone(), b_idx))
            .style(StudioTheme::ghost_button)
            .padding([1, 5]),
        button(text("\u{25BC}").size(10.0))
            .on_press(Message::FbdMoveBlockDown(p_blk.clone(), b_idx))
            .style(StudioTheme::ghost_button)
            .padding([1, 5]),
        button(text("x").size(10.5))
            .on_press(Message::FbdRemoveBlock(p_blk.clone(), b_idx))
            .style(StudioTheme::ghost_button)
            .padding([1, 5]),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    // Optional Corrector Architecture Switcher Row
    let arch_row = current_corrector_kind.map(|cur_k| {
        let corrector_kinds = vec![
            StandardFbKind::PidParallel,
            StandardFbKind::PidSeries,
            StandardFbKind::PidMixed,
            StandardFbKind::Pi,
            StandardFbKind::P,
        ];
        row![
            text(lang.tr("Architecture:", "Architecture :"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED),
            pick_list(corrector_kinds, Some(cur_k), {
                let p = p_blk.clone();
                move |k| Message::FbdChangeCorrectorKind(p.clone(), b_idx, k)
            })
            .text_size(11)
            .padding([2, 6]),
            container(
                text(cur_k.formula_hint())
                    .size(10.0)
                    .color(StudioTheme::ACCENT_CYAN),
            )
            .padding([2, 6])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                0.220, 0.741, 0.973, 0.10
            ))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
    });

    // Inputs Column
    let mut sorted_inputs: Vec<_> = blk.inputs.iter().collect();
    sorted_inputs.sort_by_key(|&(k, _)| k);

    let mut inputs_col = column![].spacing(6);
    if sorted_inputs.is_empty() {
        inputs_col = inputs_col.push(
            text(lang.tr("(No inputs)", "(Sans entrée)"))
                .size(10.5)
                .color(StudioTheme::TEXT_MUTED),
        );
    } else {
        for (pin_name, src) in sorted_inputs {
            let pin_n = pin_name.clone();
            let expr_str = match src {
                FbdInputSource::Value { expr } => expr.clone(),
                FbdInputSource::Link {
                    from_block,
                    from_pin,
                } => format!("{from_block}.{from_pin}"),
            };

            let is_energized = runtime.is_some_and(|rt| is_input_energized(rt, src));
            let dot_color = if is_energized {
                StudioTheme::ACCENT_EMERALD
            } else {
                StudioTheme::BORDER_CARD
            };

            let in_dot = container(iced::widget::Space::with_width(Length::Fixed(7.0)))
                .height(Length::Fixed(7.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: Border {
                        radius: Radius::from(3.5),
                        ..Default::default()
                    },
                    ..Default::default()
                });

            let display_pin = pin_display_label(pin_name, is_corrector, lang);
            let in_row = row![
                in_dot,
                text(display_pin)
                    .size(10.0)
                    .color(StudioTheme::TEXT_SECONDARY)
                    .width(if is_corrector { 90 } else { 36 }),
                text_input("Source", &expr_str)
                    .on_input({
                        let p = p_blk.clone();
                        let pn = pin_n.clone();
                        move |s| Message::FbdInputSourceChanged(p.clone(), b_idx, pn.clone(), s)
                    })
                    .width(if is_corrector { 85 } else { 105 })
                    .padding([2, 5])
                    .style(StudioTheme::styled_input),
                container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                    .height(Length::Fixed(2.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(dot_color)),
                        ..Default::default()
                    }),
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            inputs_col = inputs_col.push(in_row);
        }
    }

    // Central Operator Box
    let center_box = container(
        column![
            text(op_symbol).size(18).color(StudioTheme::TEXT_PRIMARY),
            text(op_desc).size(9.5).color(StudioTheme::TEXT_MUTED),
        ]
        .spacing(2)
        .align_x(Alignment::Center),
    )
    .width(Length::Fixed(72.0))
    .height(Length::Fixed(64.0))
    .center_x(Length::Fixed(72.0))
    .center_y(Length::Fixed(64.0))
    .style(|_theme: &Theme| container::Style {
        background: Some(Background::Color(StudioTheme::BG_ELEVATED)),
        border: Border {
            color: StudioTheme::BORDER_CARD,
            width: 1.5,
            radius: Radius::from(4.0),
        },
        ..Default::default()
    });

    // Outputs Column
    let mut sorted_outputs: Vec<_> = blk.outputs.iter().collect();
    sorted_outputs.sort_by_key(|&(k, _)| k);

    let mut outputs_col = column![].spacing(6);
    if sorted_outputs.is_empty() {
        outputs_col = outputs_col.push(
            text(lang.tr("(Internal)", "(Interne)"))
                .size(10.5)
                .color(StudioTheme::TEXT_MUTED),
        );
    } else {
        for (pin_name, target) in sorted_outputs {
            let pin_n = pin_name.clone();
            let is_energized = runtime.is_some_and(|rt| is_output_energized(rt, target));
            let dot_color = if is_energized {
                StudioTheme::ACCENT_EMERALD
            } else {
                StudioTheme::BORDER_CARD
            };

            let out_dot = container(iced::widget::Space::with_width(Length::Fixed(7.0)))
                .height(Length::Fixed(7.0))
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: Border {
                        radius: Radius::from(3.5),
                        ..Default::default()
                    },
                    ..Default::default()
                });

            let display_pin = pin_display_label(pin_name, is_corrector, lang);
            let out_row = row![
                container(iced::widget::Space::with_width(Length::Fixed(8.0)))
                    .height(Length::Fixed(2.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(dot_color)),
                        ..Default::default()
                    }),
                text(display_pin)
                    .size(10.0)
                    .color(StudioTheme::TEXT_SECONDARY)
                    .width(if is_corrector { 95 } else { 30 }),
                text_input("Target", target)
                    .on_input({
                        let p = p_blk.clone();
                        let pn = pin_n.clone();
                        move |s| Message::FbdOutputTargetChanged(p.clone(), b_idx, pn.clone(), s)
                    })
                    .width(if is_corrector { 85 } else { 105 })
                    .padding([2, 5])
                    .style(StudioTheme::styled_input),
                out_dot,
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            outputs_col = outputs_col.push(out_row);
        }
    }

    let schematic_body = row![
        container(inputs_col).width(Length::FillPortion(1)),
        center_box,
        container(outputs_col).width(Length::FillPortion(1)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let mut card_col = column![blk_header];
    if let Some(arch) = arch_row {
        card_col = card_col.push(arch);
    }
    card_col = card_col.push(
        container(schematic_body)
            .padding([8, 10])
            .style(StudioTheme::canvas_container_style)
            .width(Length::Fill),
    );

    container(card_col.spacing(8))
        .padding(10)
        .style(StudioTheme::card_style)
        .width(Length::FillPortion(1))
        .into()
}

pub fn render_fbd_editor<'a>(
    program: &'a FbdProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Header Toolbar with Quick Standard Blocks & Corrector Suite
    let header = row![
        text(format!(
            "{} — {}",
            lang.tr(
                "Function Block Diagram",
                "Diagramme Blocs Fonctionnels (FBD)"
            ),
            program.name
        ))
        .size(18),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ AND", "+ ET")).size(11))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::And))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ OR", "+ OU")).size(11))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::Or))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ NOT", "+ NON")).size(11))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::Not))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ GT (>)", "+ SUP (>)")).size(11))
            .on_press(Message::FbdAddBlock(pou_name.clone(), FbdBlockKind::Gt))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ TON", "+ TON")).size(11))
            .on_press(Message::FbdAddBlock(
                pou_name.clone(),
                FbdBlockKind::StandardFb {
                    instance: format!("TON_{}", program.blocks.len() + 1),
                    fb_type: Some(StandardFbKind::Ton),
                },
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ PID (Par)", "+ PID (Par)")).size(11))
            .on_press(Message::FbdAddBlock(
                pou_name.clone(),
                FbdBlockKind::StandardFb {
                    instance: format!("PID_Par_{}", program.blocks.len() + 1),
                    fb_type: Some(StandardFbKind::PidParallel),
                },
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ PID (Ser)", "+ PID (Sér)")).size(11))
            .on_press(Message::FbdAddBlock(
                pou_name.clone(),
                FbdBlockKind::StandardFb {
                    instance: format!("PID_Ser_{}", program.blocks.len() + 1),
                    fb_type: Some(StandardFbKind::PidSeries),
                },
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ PID (Mix)", "+ PID (Mix)")).size(11))
            .on_press(Message::FbdAddBlock(
                pou_name.clone(),
                FbdBlockKind::StandardFb {
                    instance: format!("PID_Mix_{}", program.blocks.len() + 1),
                    fb_type: Some(StandardFbKind::PidMixed),
                },
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ PI", "+ PI")).size(11))
            .on_press(Message::FbdAddBlock(
                pou_name.clone(),
                FbdBlockKind::StandardFb {
                    instance: format!("PI_{}", program.blocks.len() + 1),
                    fb_type: Some(StandardFbKind::Pi),
                },
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("+ P", "+ P")).size(11))
            .on_press(Message::FbdAddBlock(
                pou_name.clone(),
                FbdBlockKind::StandardFb {
                    instance: format!("P_{}", program.blocks.len() + 1),
                    fb_type: Some(StandardFbKind::P),
                },
            ))
            .style(StudioTheme::secondary_button)
            .padding([4, 8]),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([4, 8]),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Tip: FBD blocks render with 2-column industrial schematic signal-flow ports. Green indicator dots glow when signals are active in real-time.",
        "Astuce : Les blocs FBD s'affichent en schéma industriel sur 2 colonnes avec ports de signal. Les points verts s'allument en direct lorsque le signal est actif.",
    ))
    .size(12)
    .color(StudioTheme::TEXT_MUTED);

    let mut blocks_column = column![header, tip].spacing(14);

    // Render blocks in 2-column grid layout
    let chunk_size = 2;
    for (row_idx, chunk) in program.blocks.chunks(chunk_size).enumerate() {
        let b0_idx = row_idx * chunk_size;
        let card0 = render_single_block_card(&pou_name, b0_idx, &chunk[0], runtime, lang);

        let grid_row = if chunk.len() > 1 {
            let b1_idx = b0_idx + 1;
            let card1 = render_single_block_card(&pou_name, b1_idx, &chunk[1], runtime, lang);
            row![card0, card1].spacing(12).width(Length::Fill)
        } else {
            let empty_placeholder = iced::widget::Space::with_width(Length::FillPortion(1));
            row![card0, empty_placeholder]
                .spacing(12)
                .width(Length::Fill)
        };

        blocks_column = blocks_column.push(grid_row);
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

    #[test]
    fn test_delete_fbd_block_rewires_links() {
        let mut program = FbdProgram {
            name: "test_fbd".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![
                FbdBlock {
                    id: "A".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: HashMap::new(),
                    outputs: HashMap::new(),
                },
                FbdBlock {
                    id: "B".to_string(),
                    kind: FbdBlockKind::Or,
                    inputs: [(
                        "IN1".to_string(),
                        FbdInputSource::Link {
                            from_block: "A".to_string(),
                            from_pin: "OUT".to_string(),
                        },
                    )]
                    .into_iter()
                    .collect(),
                    outputs: HashMap::new(),
                },
            ],
        };

        delete_fbd_block(&mut program, 0);
        assert_eq!(program.blocks.len(), 1);
        assert_eq!(program.blocks[0].id, "B");
        assert!(matches!(
            program.blocks[0].inputs.get("IN1"),
            Some(FbdInputSource::Value { expr }) if expr == "FALSE"
        ));
        // Out-of-range index is a safe no-op
        delete_fbd_block(&mut program, 5);
        assert_eq!(program.blocks.len(), 1);
    }
}
