use iced::border::Radius;
use iced::widget::{button, column, container, mouse_area, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};

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
            "SP" => lang.tr("SP (Set)", "SP (Cons)").to_string(),
            "PV" => lang.tr("PV (Proc)", "PV (Mes)").to_string(),
            "KP" => "Kp".to_string(),
            "TI" => "Ti (s)".to_string(),
            "TD" => "Td (s)".to_string(),
            "OUT" => lang.tr("OUT (Cmd)", "OUT (Cmd)").to_string(),
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
    selected_block: Option<usize>,
    drag_hover_block: Option<usize>,
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

    let is_selected = selected_block == Some(b_idx);
    let is_hovered = drag_hover_block == Some(b_idx);

    let drag_handle = mouse_area(
        container(
            text(crate::icons::icon_grip())
                .size(13)
                .shaping(iced::widget::text::Shaping::Advanced)
                .color(if is_selected {
                    StudioTheme::ACCENT_CYAN
                } else {
                    StudioTheme::TEXT_MUTED
                }),
        )
        .padding([4, 6])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.05,
        ))),
    )
    .on_press(Message::FbdBlockDragStarted(p_blk.clone(), b_idx))
    .on_enter(Message::FbdBlockDragHovered(p_blk.clone(), b_idx))
    .on_release(Message::FbdBlockDragDropped);

    // Block Header with Drag Handle, Editable ID, and Remove Button
    let blk_header = row![
        drag_handle,
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
                1.0, 1.0, 1.0, 0.05,
            ))),
        iced::widget::Space::with_width(Length::Fill),
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
        column![
            row![
                text(lang.tr("Architecture:", "Architecture :"))
                    .size(10.5)
                    .color(StudioTheme::TEXT_MUTED),
                pick_list(corrector_kinds, Some(cur_k), {
                    let p = p_blk.clone();
                    move |k| Message::FbdChangeCorrectorKind(p.clone(), b_idx, k)
                })
                .text_size(10.5)
                .padding([2, 5]),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            container(
                text(cur_k.formula_hint())
                    .size(9.5)
                    .color(StudioTheme::ACCENT_CYAN),
            )
            .padding([2, 5])
            .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                0.220, 0.741, 0.973, 0.10,
            ))),
        ]
        .spacing(3)
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
                    .width(if is_corrector { 58 } else { 36 }),
                text_input("Source", &expr_str)
                    .on_input({
                        let p = p_blk.clone();
                        let pn = pin_n.clone();
                        move |s| Message::FbdInputSourceChanged(p.clone(), b_idx, pn.clone(), s)
                    })
                    .width(if is_corrector { 72 } else { 100 })
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
    .width(Length::Fixed(58.0))
    .height(Length::Fixed(52.0))
    .center_x(Length::Fixed(58.0))
    .center_y(Length::Fixed(52.0))
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
                    .width(if is_corrector { 58 } else { 30 }),
                text_input("Target", target)
                    .on_input({
                        let p = p_blk.clone();
                        let pn = pin_n.clone();
                        move |s| Message::FbdOutputTargetChanged(p.clone(), b_idx, pn.clone(), s)
                    })
                    .width(if is_corrector { 72 } else { 100 })
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

    let card_style_fn = move |_theme: &Theme| {
        let border = if is_hovered {
            Border {
                color: iced::Color::from_rgb(0.25, 0.65, 1.0),
                width: 2.0,
                radius: Radius::from(8.0),
            }
        } else if is_selected {
            Border {
                color: StudioTheme::ACCENT_CYAN,
                width: 2.0,
                radius: Radius::from(8.0),
            }
        } else {
            Border {
                color: StudioTheme::BORDER_CARD,
                width: 1.0,
                radius: Radius::from(8.0),
            }
        };

        container::Style {
            background: Some(Background::Color(StudioTheme::BG_ELEVATED)),
            border,
            ..Default::default()
        }
    };

    let card_container = container(card_col.spacing(8))
        .padding(10)
        .style(card_style_fn)
        .width(Length::FillPortion(1));

    mouse_area(card_container)
        .on_enter(Message::FbdBlockDragHovered(p_blk, b_idx))
        .on_release(Message::FbdBlockDragDropped)
        .into()
}

pub fn render_fbd_editor<'a>(
    program: &'a FbdProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
    selected_block: Option<usize>,
    drag_hover_block: Option<usize>,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Header Toolbar with Quick Standard Blocks & Corrector Suite
    let header_row = row![
        text(format!(
            "{} — {}",
            lang.tr(
                "Function Block Diagram",
                "Diagramme Blocs Fonctionnels (FBD)"
            ),
            program.name
        ))
        .size(18),
        iced::widget::Space::with_width(Length::Fixed(16.0)),
        button(
            row![
                text(crate::icons::icon_book())
                    .size(11)
                    .shaping(iced::widget::text::Shaping::Advanced),
                text(lang.tr("Library...", "Bibliothèque...")).size(12),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(Message::OpenFbdLibraryWindow)
        .style(StudioTheme::secondary_button)
        .padding([5, 12]),
        iced::widget::Space::with_width(Length::Fixed(16.0)),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([4, 8]),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let scrollable_header = scrollable(
        container(header_row).padding(Padding {
            top: 2.0,
            right: 4.0,
            bottom: 8.0,
            left: 2.0,
        }),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new()
            .width(4)
            .scroller_width(4)
            .margin(1),
    ))
    .width(Length::Fill);

    let header_container = container(scrollable_header)
        .width(Length::Fill)
        .padding([4, 6])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            0.2, 0.4, 0.6, 0.12,
        )));

    let tip = text(lang.tr(
        "Tip: FBD blocks render with 2-column industrial schematic signal-flow ports. Green indicator dots glow when signals are active in real-time.",
        "Astuce : Les blocs FBD s'affichent en schéma industriel sur 2 colonnes avec ports de signal. Les points verts s'allument en direct lorsque le signal est actif.",
    ))
    .size(12)
    .color(StudioTheme::TEXT_MUTED);

    let top_bar = column![header_container, tip].spacing(4);

    let mut blocks_column = column![].spacing(14);

    // Render blocks in 2-column grid layout
    let chunk_size = 2;
    for (row_idx, chunk) in program.blocks.chunks(chunk_size).enumerate() {
        let b0_idx = row_idx * chunk_size;
        let card0 = render_single_block_card(&pou_name, b0_idx, &chunk[0], runtime, lang, selected_block, drag_hover_block);

        let grid_row = if chunk.len() > 1 {
            let b1_idx = b0_idx + 1;
            let card1 = render_single_block_card(&pou_name, b1_idx, &chunk[1], runtime, lang, selected_block, drag_hover_block);
            row![card0, card1].spacing(12).width(Length::Fill)
        } else {
            let empty_placeholder = iced::widget::Space::with_width(Length::FillPortion(1));
            row![card0, empty_placeholder]
                .spacing(12)
                .width(Length::Fill)
        };

        blocks_column = blocks_column.push(grid_row);
    }

    let scrollable_blocks = scrollable(blocks_column.padding(Padding {
        top: 8.0,
        right: 12.0,
        bottom: 20.0,
        left: 12.0,
    }))
    .width(Length::Fill)
    .height(Length::Fill);

    column![top_bar, scrollable_blocks]
        .spacing(8)
        .padding(8)
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

    #[test]
    fn test_fbd_editor_toolbar_renders_with_library_button() {
        let program = FbdProgram {
            name: "test_prog".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: Vec::new(),
        };
        let _view = render_fbd_editor(&program, None, UiLanguage::En, None, None);
    }

    #[test]
    fn test_fbd_block_drag_and_drop_reorder() {
        let block_a = FbdBlock {
            id: "BlockA".to_string(),
            kind: FbdBlockKind::And,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        };
        let block_b = FbdBlock {
            id: "BlockB".to_string(),
            kind: FbdBlockKind::Or,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
        };
        let program = FbdProgram {
            name: "test_drag_fbd".to_string(),
            comment: String::new(),
            composites: Vec::new(),
            blocks: vec![block_a, block_b],
        };

        // Renders with selection and drag-hover correctly
        let _view = render_fbd_editor(&program, None, UiLanguage::En, Some(0), Some(1));
    }
}