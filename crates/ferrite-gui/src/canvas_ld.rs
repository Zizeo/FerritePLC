use iced::border::Radius;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Background, Border, Element, Length, Point, Rectangle, Theme, Vector};
use serde::{Deserialize, Serialize};

use ferrite_core::IecValue;
use ferrite_lang_ld::{CoilKind, ContactKind, LdElement, LdProgram};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub const CONTACT_CARD_WIDTH: f32 = 108.0;
pub const COIL_CARD_WIDTH: f32 = 108.0;
pub const WIRE_GAP: f32 = 10.0;
pub const RAIL_HEIGHT: f32 = 48.0;

pub const RAIL_LEFT_X: f32 = 80.0;
pub const SLOT_WIDTH: f32 = 104.0;
pub const SLOT_HEIGHT: f32 = 48.0;
pub const BRANCH_VERTICAL_STEP: f32 = 64.0;
pub const RUNG_GAP_Y: f32 = 44.0;
pub const RUNG_HEADER_HEIGHT: f32 = 28.0;
pub const WIRE_PAD: f32 = 20.0;

/// Identifies a specific parallel branch step-down within an element list:
/// `(branch_element_index, path_index)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LdBranchPathSegment {
    pub branch_idx: usize,
    pub path_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LdElementKey {
    Rung(usize),
    Contact {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    Coil {
        rung: usize,
        index: usize,
    },
    Branch {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    InlineFb {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LdDropTarget {
    BeforeElement {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    AfterElement {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
        index: usize,
    },
    NewBranch {
        rung: usize,
        branch_path: Vec<LdBranchPathSegment>,
    },
    CoilSlot {
        rung: usize,
        index: usize,
    },
    NewRung {
        index: usize,
    },
}

#[derive(Debug, Clone)]
pub struct LdPlacedElement {
    pub key: LdElementKey,
    pub bounds: Rectangle,
    pub energized: bool,
}

#[derive(Debug, Clone)]
pub struct LdPlacedBranch {
    pub key: LdElementKey,
    pub split_x: f32,
    pub merge_x: f32,
    pub start_y: f32,
    pub paths_y: Vec<f32>,
    pub energized: bool,
}

#[derive(Debug, Clone)]
pub struct LdPlacedRung {
    pub rung_idx: usize,
    pub bounds: Rectangle,
    pub wire_y: f32,
    pub elements: Vec<LdPlacedElement>,
    pub branches: Vec<LdPlacedBranch>,
    pub coils: Vec<LdPlacedElement>,
    pub power_flowing: bool,
}

#[derive(Debug, Clone)]
pub struct LdLayoutResult {
    pub rail_left_x: f32,
    pub rail_right_x: f32,
    pub rungs: Vec<LdPlacedRung>,
    pub total_bounds: Rectangle,
}

#[derive(Debug, Clone)]
pub struct LdCanvasState {
    pub pan: Vector,
    pub zoom: f32,
    pub selected: Option<LdElementKey>,
    pub selected_elements: Vec<LdElementKey>,
    pub drag_marquee: Option<(Point, Point)>,
    pub dragging: Option<(Vec<LdElementKey>, Point)>,
    pub drop_hover: Option<LdDropTarget>,
    pub is_panning: bool,
    pub pan_start: Option<Point>,
}

impl Default for LdCanvasState {
    fn default() -> Self {
        Self {
            pan: Vector::default(),
            zoom: 1.0,
            selected: None,
            selected_elements: Vec::new(),
            drag_marquee: None,
            dragging: None,
            drop_hover: None,
            is_panning: false,
            pan_start: None,
        }
    }
}

pub struct LdLayoutEngine;

impl LdLayoutEngine {
    pub fn calculate_branch_depth(elements: &[LdElement]) -> usize {
        elements
            .iter()
            .map(|e| match e {
                LdElement::Branch { paths } => {
                    let sub_depth: usize = paths.iter().map(|p| Self::calculate_branch_depth(p)).sum();
                    sub_depth.max(paths.len()).max(2)
                }
                _ => 1,
            })
            .max()
            .unwrap_or(1)
    }

    pub fn calculate_horizontal_slots(elements: &[LdElement]) -> usize {
        elements
            .iter()
            .map(|e| match e {
                LdElement::Branch { paths } => {
                    paths
                        .iter()
                        .map(|p| Self::calculate_horizontal_slots(p))
                        .max()
                        .unwrap_or(1)
                }
                _ => 1,
            })
            .sum()
    }

    pub fn compute_layout(program: &LdProgram, runtime: Option<&PlcRuntime>) -> LdLayoutResult {
        // Pass 1: compute max contact horizontal slots across all rungs
        let max_slots = program
            .rungs
            .iter()
            .map(|r| Self::calculate_horizontal_slots(&r.elements))
            .max()
            .unwrap_or(1)
            .max(2);

        let rail_left_x = RAIL_LEFT_X;
        let rail_right_x = (rail_left_x + WIRE_PAD + max_slots as f32 * SLOT_WIDTH + WIRE_PAD + COIL_CARD_WIDTH + WIRE_PAD).max(680.0);

        let mut rungs = Vec::new();
        let mut curr_y = 40.0_f32;

        for (r_idx, rung) in program.rungs.iter().enumerate() {
            let branch_depth = Self::calculate_branch_depth(&rung.elements);
            let coil_stack = rung.coils.len().max(1);
            let effective_depth = branch_depth.max(coil_stack);

            let rung_height = RUNG_HEADER_HEIGHT
                + SLOT_HEIGHT
                + (effective_depth.saturating_sub(1)) as f32 * BRANCH_VERTICAL_STEP
                + RUNG_GAP_Y;

            let rung_top_y = curr_y;
            let wire_y = rung_top_y + RUNG_HEADER_HEIGHT + SLOT_HEIGHT / 2.0;

            let mut placed_elements = Vec::new();
            let mut placed_branches = Vec::new();

            // Place contacts recursively
            let mut power_flowing = true;
            let start_x = rail_left_x + WIRE_PAD;

            Self::layout_elements_recursive(
                &rung.elements,
                r_idx,
                &[],
                start_x,
                wire_y,
                runtime,
                &mut power_flowing,
                &mut placed_elements,
                &mut placed_branches,
            );

            // Place coils stacked before right rail
            let coil_x = rail_right_x - WIRE_PAD - COIL_CARD_WIDTH;
            let mut placed_coils = Vec::new();
            for (c_idx, coil) in rung.coils.iter().enumerate() {
                let coil_y = if rung.coils.len() == 1 {
                    wire_y - SLOT_HEIGHT / 2.0
                } else {
                    wire_y + c_idx as f32 * BRANCH_VERTICAL_STEP - SLOT_HEIGHT / 2.0
                };
                let is_coil_on = power_flowing || runtime.is_some_and(|rt| is_symbol_energized(rt, &coil.symbol));
                placed_coils.push(LdPlacedElement {
                    key: LdElementKey::Coil { rung: r_idx, index: c_idx },
                    bounds: Rectangle {
                        x: coil_x,
                        y: coil_y,
                        width: COIL_CARD_WIDTH,
                        height: SLOT_HEIGHT,
                    },
                    energized: is_coil_on,
                });
            }

            let rung_bounds = Rectangle {
                x: rail_left_x - 20.0,
                y: rung_top_y,
                width: (rail_right_x - rail_left_x) + 40.0,
                height: rung_height,
            };

            rungs.push(LdPlacedRung {
                rung_idx: r_idx,
                bounds: rung_bounds,
                wire_y,
                elements: placed_elements,
                branches: placed_branches,
                coils: placed_coils,
                power_flowing,
            });

            curr_y += rung_height;
        }

        let total_bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: rail_right_x + 80.0,
            height: curr_y + 60.0,
        };

        LdLayoutResult {
            rail_left_x,
            rail_right_x,
            rungs,
            total_bounds,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layout_elements_recursive(
        elements: &[LdElement],
        rung_idx: usize,
        current_path: &[LdBranchPathSegment],
        start_x: f32,
        base_y: f32,
        runtime: Option<&PlcRuntime>,
        power_flowing: &mut bool,
        placed_elements: &mut Vec<LdPlacedElement>,
        placed_branches: &mut Vec<LdPlacedBranch>,
    ) -> f32 {
        let mut curr_x = start_x;

        for (e_idx, elem) in elements.iter().enumerate() {
            match elem {
                LdElement::Contact { symbol, kind } => {
                    let contact_closed = runtime.is_some_and(|rt| {
                        let energized = is_symbol_energized(rt, symbol);
                        match kind {
                            ContactKind::NormallyOpen => energized,
                            ContactKind::NormallyClosed => !energized,
                            ContactKind::RisingEdge | ContactKind::FallingEdge => energized,
                        }
                    });
                    let is_active = *power_flowing && contact_closed;
                    *power_flowing = is_active;

                    let bounds = Rectangle {
                        x: curr_x,
                        y: base_y - SLOT_HEIGHT / 2.0,
                        width: SLOT_WIDTH - 8.0,
                        height: SLOT_HEIGHT,
                    };
                    placed_elements.push(LdPlacedElement {
                        key: LdElementKey::Contact {
                            rung: rung_idx,
                            branch_path: current_path.to_vec(),
                            index: e_idx,
                        },
                        bounds,
                        energized: is_active,
                    });
                    curr_x += SLOT_WIDTH;
                }
                LdElement::InlineFb { instance, .. } => {
                    let fb_q = runtime.is_some_and(|rt| rt.tags.get_fb(instance).map(|fb| fb.q).unwrap_or(false));
                    let is_active = *power_flowing && fb_q;
                    *power_flowing = is_active;

                    let bounds = Rectangle {
                        x: curr_x,
                        y: base_y - SLOT_HEIGHT / 2.0,
                        width: SLOT_WIDTH - 8.0,
                        height: SLOT_HEIGHT,
                    };
                    placed_elements.push(LdPlacedElement {
                        key: LdElementKey::InlineFb {
                            rung: rung_idx,
                            branch_path: current_path.to_vec(),
                            index: e_idx,
                        },
                        bounds,
                        energized: is_active,
                    });
                    curr_x += SLOT_WIDTH;
                }
                LdElement::Branch { paths } => {
                    let split_x = curr_x;
                    let mut max_branch_x = curr_x;
                    let mut paths_y = Vec::new();
                    let initial_power = *power_flowing;
                    let mut any_path_energized = false;

                    for (p_idx, path) in paths.iter().enumerate() {
                        let path_y = base_y + p_idx as f32 * BRANCH_VERTICAL_STEP;
                        paths_y.push(path_y);

                        let mut next_path = current_path.to_vec();
                        next_path.push(LdBranchPathSegment {
                            branch_idx: e_idx,
                            path_idx: p_idx,
                        });

                        let mut path_power = initial_power;
                        let end_x = Self::layout_elements_recursive(
                            path,
                            rung_idx,
                            &next_path,
                            split_x + WIRE_PAD,
                            path_y,
                            runtime,
                            &mut path_power,
                            placed_elements,
                            placed_branches,
                        );
                        if path_power {
                            any_path_energized = true;
                        }
                        max_branch_x = max_branch_x.max(end_x);
                    }

                    let merge_x = max_branch_x + WIRE_PAD;
                    *power_flowing = initial_power && any_path_energized;

                    placed_branches.push(LdPlacedBranch {
                        key: LdElementKey::Branch {
                            rung: rung_idx,
                            branch_path: current_path.to_vec(),
                            index: e_idx,
                        },
                        split_x,
                        merge_x,
                        start_y: base_y,
                        paths_y,
                        energized: *power_flowing,
                    });

                    curr_x = merge_x;
                }
            }
        }

        curr_x
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LdContactKindWrapper(pub ContactKind);

impl std::fmt::Display for LdContactKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            ContactKind::NormallyOpen => write!(f, "NO"),
            ContactKind::NormallyClosed => write!(f, "NC"),
            ContactKind::RisingEdge => write!(f, "P"),
            ContactKind::FallingEdge => write!(f, "N"),
        }
    }
}

pub const ALL_CONTACT_KINDS: [LdContactKindWrapper; 4] = [
    LdContactKindWrapper(ContactKind::NormallyOpen),
    LdContactKindWrapper(ContactKind::NormallyClosed),
    LdContactKindWrapper(ContactKind::RisingEdge),
    LdContactKindWrapper(ContactKind::FallingEdge),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LdCoilKindWrapper(pub CoilKind);

impl std::fmt::Display for LdCoilKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            CoilKind::Normal => write!(f, "( )"),
            CoilKind::Set => write!(f, "[S]"),
            CoilKind::Reset => write!(f, "[R]"),
        }
    }
}

pub const ALL_COIL_KINDS: [LdCoilKindWrapper; 3] = [
    LdCoilKindWrapper(CoilKind::Normal),
    LdCoilKindWrapper(CoilKind::Set),
    LdCoilKindWrapper(CoilKind::Reset),
];

fn is_symbol_energized(rt: &PlcRuntime, symbol: &str) -> bool {
    matches!(rt.read_value(symbol), Ok(IecValue::Bool(true)))
}

fn wire_segment<'a>(active: bool, width: f32) -> Element<'a, Message> {
    let color = if active {
        StudioTheme::ACCENT_EMERALD
    } else {
        StudioTheme::BORDER_CARD
    };
    container(iced::widget::Space::with_width(Length::Fixed(width)))
        .height(Length::Fixed(2.5))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

pub fn render_ld_editor<'a>(
    program: &'a LdProgram,
    runtime: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let pou_name = program.name.clone();

    // 1. Toolbar Header
    let header = row![
        text(format!(
            "{} — {}",
            lang.tr("Ladder Diagram", "Diagramme à Contacts (Ladder)"),
            program.name
        ))
        .size(17),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Rung", "+ Réseau")).size(11))
            .on_press(Message::LdAddRung(pou_name.clone()))
            .style(StudioTheme::primary_button)
            .padding([5, 10]),
        button(text(lang.tr("Clear POU", "Vider")).size(11))
            .on_press(Message::ConfirmClearPou(pou_name.clone()))
            .style(StudioTheme::danger_button)
            .padding([5, 10]),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let tip = text(lang.tr(
        "Tip: Left power rail is energized. Compact contact and coil cards fit on screen with power flow indicator.",
        "Astuce : Rail d'alimentation actif. Cartes compactes pour s'adapter à l'écran avec flux de puissance direct.",
    ))
    .size(11.5)
    .color(StudioTheme::TEXT_MUTED);

    // 2. Rungs List
    let mut rungs_column = column![header, tip].spacing(14);

    for (r_idx, rung) in program.rungs.iter().enumerate() {
        let p_name = pou_name.clone();

        // Rung Header: Rung ID + Comment + Quick buttons
        let rung_header = row![
            container(text(&rung.id).size(11.5).color(StudioTheme::ACCENT_CYAN))
                .padding([2, 6])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                    0.220, 0.741, 0.973, 0.12
                ))),
            text_input("Comment...", &rung.comment,)
                .on_input({
                    let p = p_name.clone();
                    move |s| Message::LdRungCommentChanged(p.clone(), r_idx, s)
                })
                .width(Length::Fill)
                .padding([3, 7])
                .style(StudioTheme::styled_input),
            button(text("+ NO").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::NormallyOpen
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ NC").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::NormallyClosed
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ |P|").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::RisingEdge
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ |N|").size(10.5))
                .on_press(Message::LdAddContactWithKind(
                    p_name.clone(),
                    r_idx,
                    ContactKind::FallingEdge
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text("+ ( )").size(10.5))
                .on_press(Message::LdAddCoilWithKind(
                    p_name.clone(),
                    r_idx,
                    CoilKind::Normal
                ))
                .style(StudioTheme::secondary_button)
                .padding([3, 6]),
            button(text(lang.tr("Del", "Suppr")).size(10.5))
                .on_press(Message::LdRemoveRung(p_name.clone(), r_idx))
                .style(StudioTheme::danger_button)
                .padding([3, 6]),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        // Power Rails & Circuit
        let left_rail = container(iced::widget::Space::with_width(Length::Fixed(4.0)))
            .height(Length::Fixed(RAIL_HEIGHT))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::ACCENT_EMERALD)),
                border: Border {
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        let right_rail = container(iced::widget::Space::with_width(Length::Fixed(4.0)))
            .height(Length::Fixed(RAIL_HEIGHT))
            .style(|_theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::BORDER_CARD)),
                border: Border {
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        let mut power_flowing = true;
        let mut circuit_row = row![left_rail, wire_segment(true, WIRE_GAP),]
            .spacing(0)
            .align_y(Alignment::Center);

        // Contacts and parallel branches in this rung
        for (e_idx, elem) in rung.elements.iter().enumerate() {
            let p_elem = p_name.clone();
            match elem {
                LdElement::Contact { symbol, kind } => {
                    let contact_closed = runtime.is_some_and(|rt| {
                        let energized = is_symbol_energized(rt, symbol);
                        match kind {
                            ContactKind::NormallyOpen => energized,
                            ContactKind::NormallyClosed => !energized,
                            ContactKind::RisingEdge | ContactKind::FallingEdge => energized,
                        }
                    });

                    let is_active = power_flowing && contact_closed;
                    power_flowing = is_active;

                    let border_color = if is_active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::BORDER_CARD
                    };
                    let bg_color = if is_active {
                        iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)
                    } else {
                        StudioTheme::BG_BASE
                    };

                    let glyph_str = match kind {
                        ContactKind::NormallyOpen => "--| |--",
                        ContactKind::NormallyClosed => "--|/|--",
                        ContactKind::RisingEdge => "--|P|--",
                        ContactKind::FallingEdge => "--|N|--",
                    };

                    // Compact 2-level stacked contact card
                    let top_row = row![
                        text_input("Tag", symbol)
                            .on_input({
                                let p = p_elem.clone();
                                move |s| Message::LdContactSymbolChanged(p.clone(), r_idx, e_idx, s)
                            })
                            .width(72)
                            .padding([1, 4])
                            .size(10.5)
                            .style(StudioTheme::styled_input),
                        iced::widget::Space::with_width(Length::Fill),
                        button(text("x").size(9.5))
                            .on_press(Message::LdRemoveContact(p_elem.clone(), r_idx, e_idx))
                            .style(StudioTheme::ghost_button)
                            .padding([0, 3]),
                    ]
                    .align_y(Alignment::Center);

                    let bottom_row = row![
                        text(glyph_str)
                            .size(11.5)
                            .font(iced::Font::MONOSPACE)
                            .color(if is_active {
                                StudioTheme::ACCENT_EMERALD
                            } else {
                                StudioTheme::TEXT_MUTED
                            }),
                        iced::widget::Space::with_width(Length::Fill),
                        pick_list(&ALL_CONTACT_KINDS[..], Some(LdContactKindWrapper(*kind)), {
                            let p = p_elem.clone();
                            move |w| Message::LdContactKindChanged(p.clone(), r_idx, e_idx, w.0)
                        },)
                        .width(44)
                        .style(StudioTheme::styled_pick_list),
                    ]
                    .align_y(Alignment::Center);

                    let contact_card = container(column![top_row, bottom_row].spacing(2))
                        .width(CONTACT_CARD_WIDTH)
                        .padding(4)
                        .style(move |_theme: &Theme| container::Style {
                            background: Some(Background::Color(bg_color)),
                            border: Border {
                                color: border_color,
                                width: if is_active { 1.5 } else { 1.0 },
                                radius: Radius::from(4.0),
                            },
                            ..Default::default()
                        });

                    circuit_row = circuit_row.push(contact_card);
                    circuit_row = circuit_row.push(wire_segment(power_flowing, WIRE_GAP));
                }
                LdElement::Branch { paths } => {
                    let branch_closed = runtime.is_some_and(|rt| {
                        paths.iter().any(|path| {
                            path.iter().all(|sub_elem| match sub_elem {
                                LdElement::Contact { symbol, kind } => {
                                    let energized = is_symbol_energized(rt, symbol);
                                    match kind {
                                        ContactKind::NormallyOpen | ContactKind::RisingEdge => {
                                            energized
                                        }
                                        ContactKind::NormallyClosed | ContactKind::FallingEdge => {
                                            !energized
                                        }
                                    }
                                }
                                _ => true,
                            })
                        })
                    });
                    let is_active = power_flowing && branch_closed;
                    power_flowing = is_active;
                    let branch_box = container(text("[OR Branch]").size(11).color(if is_active {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::ACCENT_PURPLE
                    }))
                    .padding([4, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                        0.75, 0.5, 0.9, 0.12,
                    )));

                    circuit_row = circuit_row.push(branch_box);
                    circuit_row = circuit_row.push(wire_segment(power_flowing, WIRE_GAP));
                }
                LdElement::InlineFb { instance, .. } => {
                    let fb_q = runtime
                        .is_some_and(|rt| rt.tags.get_fb(instance).map(|fb| fb.q).unwrap_or(false));
                    let is_active = power_flowing && fb_q;
                    power_flowing = is_active;
                    let fb_box = container(text(format!("[FB: {instance}]")).size(11).color(
                        if is_active {
                            StudioTheme::ACCENT_EMERALD
                        } else {
                            StudioTheme::ACCENT_CYAN
                        },
                    ))
                    .padding([4, 8])
                    .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                        0.22, 0.74, 0.97, 0.12,
                    )));

                    circuit_row = circuit_row.push(fb_box);
                    circuit_row = circuit_row.push(wire_segment(power_flowing, WIRE_GAP));
                }
            }
        }

        // Spanning expanding wire bus across to coils so coils snap to the right power rail
        let span_color = if power_flowing {
            StudioTheme::ACCENT_EMERALD
        } else {
            StudioTheme::BORDER_CARD
        };
        let span_wire = container(iced::widget::Space::with_width(Length::Fill))
            .height(Length::Fixed(2.5))
            .style(move |_theme: &Theme| container::Style {
                background: Some(Background::Color(span_color)),
                ..Default::default()
            });
        circuit_row = circuit_row.push(span_wire);

        // Coils in this rung
        for (c_idx, coil) in rung.coils.iter().enumerate() {
            let p_coil = p_name.clone();
            let is_coil_on =
                power_flowing || runtime.is_some_and(|rt| is_symbol_energized(rt, &coil.symbol));

            let coil_border = if is_coil_on {
                StudioTheme::ACCENT_EMERALD
            } else {
                StudioTheme::BORDER_CARD
            };
            let coil_bg = if is_coil_on {
                iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)
            } else {
                StudioTheme::BG_BASE
            };

            let glyph_str = match coil.kind {
                CoilKind::Normal => "--( )--",
                CoilKind::Set => "--[S]--",
                CoilKind::Reset => "--[R]--",
            };

            // Compact 2-level stacked coil card
            let top_row = row![
                text_input("Coil", &coil.symbol)
                    .on_input({
                        let p = p_coil.clone();
                        move |s| Message::LdCoilSymbolChanged(p.clone(), r_idx, c_idx, s)
                    })
                    .width(72)
                    .padding([1, 4])
                    .size(10.5)
                    .style(StudioTheme::styled_input),
                iced::widget::Space::with_width(Length::Fill),
                button(text("x").size(9.5))
                    .on_press(Message::LdRemoveCoil(p_coil.clone(), r_idx, c_idx))
                    .style(StudioTheme::ghost_button)
                    .padding([0, 3]),
            ]
            .align_y(Alignment::Center);

            let bottom_row = row![
                text(glyph_str)
                    .size(11.5)
                    .font(iced::Font::MONOSPACE)
                    .color(if is_coil_on {
                        StudioTheme::ACCENT_EMERALD
                    } else {
                        StudioTheme::TEXT_MUTED
                    }),
                iced::widget::Space::with_width(Length::Fill),
                pick_list(&ALL_COIL_KINDS[..], Some(LdCoilKindWrapper(coil.kind)), {
                    let p = p_coil.clone();
                    move |w| Message::LdCoilKindChanged(p.clone(), r_idx, c_idx, w.0)
                },)
                .width(44)
                .style(StudioTheme::styled_pick_list),
            ]
            .align_y(Alignment::Center);

            let coil_card = container(column![top_row, bottom_row].spacing(2))
                .width(COIL_CARD_WIDTH)
                .padding(4)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(coil_bg)),
                    border: Border {
                        color: coil_border,
                        width: if is_coil_on { 1.5 } else { 1.0 },
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                });

            circuit_row = circuit_row.push(wire_segment(is_coil_on, WIRE_GAP));
            circuit_row = circuit_row.push(coil_card);
        }

        // Right Rail Return
        circuit_row = circuit_row.push(wire_segment(true, WIRE_GAP));
        circuit_row = circuit_row.push(right_rail);

        let rung_card = container(
            column![
                rung_header,
                container(circuit_row)
                    .padding([8, 12])
                    .style(StudioTheme::canvas_container_style)
                    .width(Length::Fill),
            ]
            .spacing(8),
        )
        .padding(10)
        .style(StudioTheme::card_style)
        .width(Length::Fill);

        rungs_column = rungs_column.push(rung_card);
    }

    scrollable(rungs_column.padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use ferrite_lang_ld::{LdCoil, LdRung};

    #[test]
    fn test_ld_layout_straight_rung_geometry() {
        let program = LdProgram {
            name: "test_straight".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Rung 0".to_string(),
                elements: vec![
                    LdElement::Contact {
                        symbol: "Start_Btn".to_string(),
                        kind: ContactKind::NormallyOpen,
                    },
                    LdElement::Contact {
                        symbol: "Safety_OK".to_string(),
                        kind: ContactKind::NormallyClosed,
                    },
                ],
                coils: vec![LdCoil {
                    symbol: "Motor_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let layout = LdLayoutEngine::compute_layout(&program, None);
        assert_eq!(layout.rungs.len(), 1);
        let r0 = &layout.rungs[0];
        assert_eq!(r0.elements.len(), 2);
        assert_eq!(r0.coils.len(), 1);
        assert!(layout.rail_left_x < layout.rail_right_x);
        assert!(r0.elements[0].bounds.x < r0.elements[1].bounds.x);
        assert!(r0.elements[1].bounds.x < r0.coils[0].bounds.x);
        assert!(r0.coils[0].bounds.x < layout.rail_right_x);
    }

    #[test]
    fn test_ld_layout_parallel_branch_geometry() {
        let program = LdProgram {
            name: "test_branch".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "Parallel branch".to_string(),
                elements: vec![LdElement::Branch {
                    paths: vec![
                        vec![LdElement::Contact {
                            symbol: "Start_PB".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                        vec![LdElement::Contact {
                            symbol: "Motor_Aux".to_string(),
                            kind: ContactKind::NormallyOpen,
                        }],
                    ],
                }],
                coils: vec![LdCoil {
                    symbol: "Motor_Run".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let layout = LdLayoutEngine::compute_layout(&program, None);
        let r0 = &layout.rungs[0];
        assert_eq!(r0.branches.len(), 1);
        let b0 = &r0.branches[0];
        assert_eq!(b0.paths_y.len(), 2);
        assert!(b0.paths_y[0] < b0.paths_y[1]); // Step down for second leg
        assert!(b0.split_x < b0.merge_x);
    }

    #[test]
    fn test_ld_layout_multiple_coils_stack_height() {
        let program_single = LdProgram {
            name: "test_single_coil".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![],
                coils: vec![LdCoil {
                    symbol: "C1".to_string(),
                    kind: CoilKind::Normal,
                }],
            }],
        };

        let program_multi = LdProgram {
            name: "test_multi_coil".to_string(),
            comment: "".to_string(),
            rungs: vec![LdRung {
                id: "R0".to_string(),
                comment: "".to_string(),
                elements: vec![],
                coils: vec![
                    LdCoil {
                        symbol: "C1".to_string(),
                        kind: CoilKind::Normal,
                    },
                    LdCoil {
                        symbol: "C2".to_string(),
                        kind: CoilKind::Normal,
                    },
                    LdCoil {
                        symbol: "C3".to_string(),
                        kind: CoilKind::Normal,
                    },
                ],
            }],
        };

        let l_single = LdLayoutEngine::compute_layout(&program_single, None);
        let l_multi = LdLayoutEngine::compute_layout(&program_multi, None);
        assert!(l_multi.rungs[0].bounds.height > l_single.rungs[0].bounds.height);
        assert_eq!(l_multi.rungs[0].coils.len(), 3);
        assert!(l_multi.rungs[0].coils[0].bounds.y < l_multi.rungs[0].coils[1].bounds.y);
        assert!(l_multi.rungs[0].coils[1].bounds.y < l_multi.rungs[0].coils[2].bounds.y);
    }
}
