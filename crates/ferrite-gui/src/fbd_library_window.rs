//! Dedicated Function Block Library Palette Window.
//!
//! Provides a categorized, searchable IEC 61131-3 catalog of logic primitives,
//! timers, counters, triggers, bistables, and industrial PID regulation blocks
//! with one-click insertion into the active Function Block Diagram (FBD) POU.

use iced::border::Radius;
use iced::widget::{button, column, container, row, scrollable, text, text_editor, text_input};
use iced::{Alignment, Background, Border, Element, Length};

use ferrite_core::StandardFbKind;
use ferrite_lang_fbd::FbdBlockKind;
use ferrite_project::PouSource;

use crate::app::{FerriteStudioApp, Message, WorkspaceTab};
use crate::theme::StudioTheme;

pub struct FbdCatalogItem {
    pub name: &'static str,
    pub category: &'static str,
    pub description_en: &'static str,
    pub description_fr: &'static str,
    pub pin_signature: &'static str,
    pub make_kind: fn(usize) -> FbdBlockKind,
}

pub static FBD_CATALOG: &[FbdCatalogItem] = &[
    // --- Logic & Comparison ---
    FbdCatalogItem {
        name: "AND",
        category: "Logic & Math",
        description_en: "Boolean AND gate (all inputs true)",
        description_fr: "Porte logique ET (toutes entrées vraies)",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::And,
    },
    FbdCatalogItem {
        name: "OR",
        category: "Logic & Math",
        description_en: "Boolean OR gate (at least one input true)",
        description_fr: "Porte logique OU (au moins une entrée vraie)",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Or,
    },
    FbdCatalogItem {
        name: "XOR",
        category: "Logic & Math",
        description_en: "Boolean Exclusive OR gate",
        description_fr: "Porte logique OU Exclusif",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Xor,
    },
    FbdCatalogItem {
        name: "NOT",
        category: "Logic & Math",
        description_en: "Boolean logic inverter",
        description_fr: "Inverseur logique NON",
        pin_signature: "IN -> OUT",
        make_kind: |_| FbdBlockKind::Not,
    },
    FbdCatalogItem {
        name: "ADD",
        category: "Logic & Math",
        description_en: "Arithmetic addition",
        description_fr: "Addition arithmétique",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Add,
    },
    FbdCatalogItem {
        name: "SUB",
        category: "Logic & Math",
        description_en: "Arithmetic subtraction",
        description_fr: "Soustraction arithmétique",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Sub,
    },
    FbdCatalogItem {
        name: "MUL",
        category: "Logic & Math",
        description_en: "Arithmetic multiplication",
        description_fr: "Multiplication arithmétique",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Mul,
    },
    FbdCatalogItem {
        name: "DIV",
        category: "Logic & Math",
        description_en: "Arithmetic division",
        description_fr: "Division arithmétique",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Div,
    },
    FbdCatalogItem {
        name: "GT (>)",
        category: "Logic & Math",
        description_en: "Greater-than comparator",
        description_fr: "Comparateur strictement supérieur",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Gt,
    },
    FbdCatalogItem {
        name: "GE (>=)",
        category: "Logic & Math",
        description_en: "Greater-than-or-equal comparator",
        description_fr: "Comparateur supérieur ou égal",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Ge,
    },
    FbdCatalogItem {
        name: "LT (<)",
        category: "Logic & Math",
        description_en: "Less-than comparator",
        description_fr: "Comparateur strictement inférieur",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Lt,
    },
    FbdCatalogItem {
        name: "LE (<=)",
        category: "Logic & Math",
        description_en: "Less-than-or-equal comparator",
        description_fr: "Comparateur inférieur ou égal",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Le,
    },
    FbdCatalogItem {
        name: "EQ (=)",
        category: "Logic & Math",
        description_en: "Equality comparator",
        description_fr: "Comparateur d'égalité",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Eq,
    },
    FbdCatalogItem {
        name: "NE (<>)",
        category: "Logic & Math",
        description_en: "Inequality comparator",
        description_fr: "Comparateur de non-égalité",
        pin_signature: "IN1, IN2 -> OUT",
        make_kind: |_| FbdBlockKind::Ne,
    },

    // --- Timers & Counters ---
    FbdCatalogItem {
        name: "TON",
        category: "Timers & Counters",
        description_en: "On-Delay Timer (IEC 61131-3)",
        description_fr: "Temporisateur retard au travail (IEC 61131-3)",
        pin_signature: "IN, PT -> Q, ET",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("TON_{}", idx + 1),
            fb_type: Some(StandardFbKind::Ton),
        },
    },
    FbdCatalogItem {
        name: "TOF",
        category: "Timers & Counters",
        description_en: "Off-Delay Timer (IEC 61131-3)",
        description_fr: "Temporisateur retard au repos (IEC 61131-3)",
        pin_signature: "IN, PT -> Q, ET",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("TOF_{}", idx + 1),
            fb_type: Some(StandardFbKind::Tof),
        },
    },
    FbdCatalogItem {
        name: "TP",
        category: "Timers & Counters",
        description_en: "Pulse Timer (IEC 61131-3)",
        description_fr: "Temporisateur impulsionnel (IEC 61131-3)",
        pin_signature: "IN, PT -> Q, ET",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("TP_{}", idx + 1),
            fb_type: Some(StandardFbKind::Tp),
        },
    },
    FbdCatalogItem {
        name: "CTU",
        category: "Timers & Counters",
        description_en: "Up Counter (IEC 61131-3)",
        description_fr: "Compteur croissant (IEC 61131-3)",
        pin_signature: "CU, R, PV -> Q, CV",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("CTU_{}", idx + 1),
            fb_type: Some(StandardFbKind::Ctu),
        },
    },
    FbdCatalogItem {
        name: "CTD",
        category: "Timers & Counters",
        description_en: "Down Counter (IEC 61131-3)",
        description_fr: "Compteur décroissant (IEC 61131-3)",
        pin_signature: "CD, LD, PV -> Q, CV",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("CTD_{}", idx + 1),
            fb_type: Some(StandardFbKind::Ctd),
        },
    },
    FbdCatalogItem {
        name: "CTUD",
        category: "Timers & Counters",
        description_en: "Up/Down Counter (IEC 61131-3)",
        description_fr: "Compteur/Décompteur bidirectionnel (IEC 61131-3)",
        pin_signature: "CU, CD, R, LD, PV -> QU, QD, CV",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("CTUD_{}", idx + 1),
            fb_type: Some(StandardFbKind::Ctud),
        },
    },
    FbdCatalogItem {
        name: "BLINK",
        category: "Timers & Counters",
        description_en: "Asymmetric square-wave clock generator (OSCAT)",
        description_fr: "Générateur d'horloge rectangulaire asymétrique",
        pin_signature: "ENABLE, TIMELOW, TIMEHIGH -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("BLINK_{}", idx + 1),
            fb_type: Some(StandardFbKind::Blink),
        },
    },

    // --- Regulation & Controllers ---
    FbdCatalogItem {
        name: "HYSTERESIS",
        category: "Regulation & PID",
        description_en: "Schmitt-trigger deadband comparator (LOW..HIGH)",
        description_fr: "Comparateur à hystérésis / bande morte (LOW..HIGH)",
        pin_signature: "IN, LOW, HIGH -> Q",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("HYST_{}", idx + 1),
            fb_type: Some(StandardFbKind::Hysteresis),
        },
    },
    FbdCatalogItem {
        name: "RAMP_REAL",
        category: "Regulation & PID",
        description_en: "Slew-rate limiter with rising/falling slopes (units/s)",
        description_fr: "Limiteur de rampe avec pentes montée/descente (unités/s)",
        pin_signature: "IN, ASC, DSC, RESET -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("RAMP_{}", idx + 1),
            fb_type: Some(StandardFbKind::RampReal),
        },
    },
    FbdCatalogItem {
        name: "SCALE_LIN",
        category: "Regulation & PID",
        description_en: "Linear analog sensor/actuator scaling (4-20mA / 0-27648)",
        description_fr: "Mise à l'échelle linéaire analogique (4-20mA / 0-27648)",
        pin_signature: "IN, IN_MIN, IN_MAX, OUT_MIN, OUT_MAX -> OUT, ERR",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("SCALE_{}", idx + 1),
            fb_type: Some(StandardFbKind::ScaleLin),
        },
    },
    FbdCatalogItem {
        name: "PID (Parallel)",
        category: "Regulation & PID",
        description_en: "Parallel ISA PID corrector with anti-windup & tracking",
        description_fr: "Correcteur PID parallèle ISA avec anti-emballement & poursuite",
        pin_signature: "SP, PV, AUTO, TR, TR_VAL -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("PID_Par_{}", idx + 1),
            fb_type: Some(StandardFbKind::PidParallel),
        },
    },
    FbdCatalogItem {
        name: "PID (Series)",
        category: "Regulation & PID",
        description_en: "Interacting Series PID corrector",
        description_fr: "Correcteur PID série (interagissant)",
        pin_signature: "SP, PV, AUTO, TR, TR_VAL -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("PID_Ser_{}", idx + 1),
            fb_type: Some(StandardFbKind::PidSeries),
        },
    },
    FbdCatalogItem {
        name: "PID (Mixed)",
        category: "Regulation & PID",
        description_en: "Standard Mixed / Ideal ISA PID corrector",
        description_fr: "Correcteur PID mixte / idéal standard",
        pin_signature: "SP, PV, AUTO, TR, TR_VAL -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("PID_Mix_{}", idx + 1),
            fb_type: Some(StandardFbKind::PidMixed),
        },
    },
    FbdCatalogItem {
        name: "PI",
        category: "Regulation & PID",
        description_en: "Proportional-Integral corrector",
        description_fr: "Correcteur Proportionnel-Intégral",
        pin_signature: "SP, PV, AUTO, TR, TR_VAL -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("PI_{}", idx + 1),
            fb_type: Some(StandardFbKind::Pi),
        },
    },
    FbdCatalogItem {
        name: "P",
        category: "Regulation & PID",
        description_en: "Proportional-only corrector",
        description_fr: "Correcteur Proportionnel pur",
        pin_signature: "SP, PV -> OUT",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("P_{}", idx + 1),
            fb_type: Some(StandardFbKind::P),
        },
    },

    // --- Bistables & Edge Triggers ---
    FbdCatalogItem {
        name: "SR",
        category: "Bistables & Triggers",
        description_en: "Set-dominant bistable latch",
        description_fr: "Bascule bistable priorité à l'enclenchement",
        pin_signature: "S1, R -> Q1",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("SR_{}", idx + 1),
            fb_type: Some(StandardFbKind::Sr),
        },
    },
    FbdCatalogItem {
        name: "RS",
        category: "Bistables & Triggers",
        description_en: "Reset-dominant bistable latch",
        description_fr: "Bascule bistable priorité au déclenchement",
        pin_signature: "S, R1 -> Q1",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("RS_{}", idx + 1),
            fb_type: Some(StandardFbKind::Rs),
        },
    },
    FbdCatalogItem {
        name: "R_TRIG",
        category: "Bistables & Triggers",
        description_en: "Rising edge detection trigger",
        description_fr: "Détecteur de front montant",
        pin_signature: "CLK -> Q",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("R_TRIG_{}", idx + 1),
            fb_type: Some(StandardFbKind::RTrig),
        },
    },
    FbdCatalogItem {
        name: "F_TRIG",
        category: "Bistables & Triggers",
        description_en: "Falling edge detection trigger",
        description_fr: "Détecteur de front descendant",
        pin_signature: "CLK -> Q",
        make_kind: |idx| FbdBlockKind::StandardFb {
            instance: format!("F_TRIG_{}", idx + 1),
            fb_type: Some(StandardFbKind::FTrig),
        },
    },
];

/// Renders the dedicated Function Block Library Palette window.
pub fn render_fbd_library_window(app: &FerriteStudioApp) -> Element<'_, Message> {
    let lang = app.ui_lang;
    let query = app.fbd_library_search_query.trim().to_lowercase();

    // Determine the active or candidate FBD POU target
    let active_fbd_pou = if let WorkspaceTab::Pou(ref pou_name) = app.active_tab
        && matches!(app.project.pous.get(pou_name), Some(PouSource::Fbd(_)))
    {
        Some(pou_name.clone())
    } else {
        app.project
            .manifest
            .pous
            .iter()
            .find(|p| matches!(app.project.pous.get(&p.name), Some(PouSource::Fbd(_))))
            .map(|p| p.name.clone())
    };

    let target_caption = if let Some(ref target) = active_fbd_pou {
        format!(
            "{}: {}",
            lang.tr("Target FBD POU", "POU FBD cible"),
            target
        )
    } else {
        lang.tr(
            "No active FBD POU (open an FBD POU first)",
            "Aucun POU FBD actif (ouvrez un POU FBD)",
        )
        .to_string()
    };

    let header = row![
        text(lang.tr("Function Block Library", "Bibliothèque de Blocs Fonctionnels"))
            .size(16),
        iced::widget::Space::with_width(Length::Fill),
        button(
            row![
                text("+").size(13),
                text(lang.tr("New Custom Block", "Nouveau Bloc Personnalisé")).size(11),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .on_press(Message::ToggleCustomFbdBuilder)
        .style(if app.custom_fbd_builder_open {
            StudioTheme::secondary_button
        } else {
            StudioTheme::primary_button
        })
        .padding([8, 16]),
        container(
            text(target_caption)
                .size(11)
                .color(if active_fbd_pou.is_some() {
                    StudioTheme::ACCENT_CYAN
                } else {
                    StudioTheme::TEXT_MUTED
                }),
        )
        .padding([3, 8])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.05
        ))),
        button(
            text(crate::icons::icon_close())
                .size(11)
                .shaping(iced::widget::text::Shaping::Advanced),
        )
        .on_press(Message::CloseFbdLibraryWindow)
        .style(StudioTheme::ghost_button)
        .padding([4, 8]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let search_input = text_input(
        lang.tr("Filter blocks (e.g. PID, TON, AND)...", "Filtrer les blocs (ex: PID, TON, ET)..."),
        &app.fbd_library_search_query,
    )
    .on_input(Message::FbdLibrarySearchChanged)
    .padding([6, 10])
    .style(StudioTheme::styled_input);

    let categories = [
        ("Logic & Math", lang.tr("Logic & Math Primitives", "Primitives Logiques & Arithmétiques"), StudioTheme::ACCENT_CYAN),
        ("Timers & Counters", lang.tr("Timers & Counters (IEC 61131-3)", "Temporisateurs & Compteurs (IEC 61131-3)"), StudioTheme::ACCENT_AMBER),
        ("Regulation & PID", lang.tr("Regulation & Correctors", "Régulation & Correcteurs"), StudioTheme::ACCENT_PURPLE),
        ("Bistables & Triggers", lang.tr("Bistables & Edge Triggers", "Bascules & Détecteurs de Fronts"), StudioTheme::ACCENT_EMERALD),
    ];

    let mut content_col = column![].spacing(14);

    // --- Custom Blocks Section ---
    let matching_custom: Vec<&ferrite_lang_fbd::CompositeFbDef> = app
        .global_custom_fbs
        .iter()
        .filter(|item| {
            if query.is_empty() {
                return true;
            }
            item.name.to_lowercase().contains(&query)
                || item.description.to_lowercase().contains(&query)
                || item.input_pins.iter().any(|p| p.to_lowercase().contains(&query))
                || item.output_pins.iter().any(|p| p.to_lowercase().contains(&query))
        })
        .collect();

    if !matching_custom.is_empty() {
        let section_header = row![
            container(text("Custom").size(10).color(StudioTheme::ACCENT_AMBER))
                .padding([2, 6])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.05
                ))),
            text(lang.tr("Custom Blocks (Structured Text)", "Blocs Personnalisés (Texte Structuré)"))
                .size(13)
                .color(StudioTheme::TEXT_PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut cards_col = column![].spacing(6);

        for item in matching_custom {
            let add_btn = if active_fbd_pou.is_some() {
                button(text(lang.tr("+ Add", "+ Ajouter")).size(11))
                    .on_press(Message::AddCustomFbdToCanvas((*item).clone()))
                    .style(StudioTheme::primary_button)
                    .padding([8, 16])
            } else {
                button(text(lang.tr("+ Add", "+ Ajouter")).size(11))
                    .style(StudioTheme::ghost_button)
                    .padding([8, 16])
            };

            let pin_sig = format!("{} -> {}", item.input_pins.join(", "), item.output_pins.join(", "));
            let desc = if item.description.is_empty() {
                lang.tr("Custom ST Function Block", "Bloc fonctionnel ST personnalisé")
            } else {
                &item.description
            };

            let card = container(
                row![
                    column![
                        row![
                            text(&item.name).size(12),
                            text("|").size(11).color(StudioTheme::BORDER_CARD),
                            text(pin_sig)
                                .size(11)
                                .color(StudioTheme::ACCENT_AMBER),
                            container(text("ST").size(9).color(StudioTheme::ACCENT_CYAN))
                                .padding([1, 4])
                                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.0, 0.8, 1.0, 0.1))),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                        text(desc)
                            .size(11)
                            .color(StudioTheme::TEXT_SECONDARY)
                            .wrapping(iced::widget::text::Wrapping::Word),
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    row![
                        add_btn,
                        button(text(lang.tr("Edit", "Modifier")).size(11))
                            .on_press(Message::EditCustomFbdBlock(item.name.clone()))
                            .style(StudioTheme::secondary_button)
                            .padding([8, 16]),
                        button(
                            text(crate::icons::icon_close())
                                .size(10)
                                .shaping(iced::widget::text::Shaping::Advanced),
                        )
                        .on_press(Message::DeleteCustomFbdBlock(item.name.clone()))
                        .style(StudioTheme::ghost_button)
                        .padding([4, 6]),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_SURFACE)),
                border: Border {
                    color: StudioTheme::BORDER_CARD,
                    width: 1.0,
                    radius: Radius::from(6.0),
                },
                ..Default::default()
            });

            cards_col = cards_col.push(card);
        }

        content_col = content_col.push(column![section_header, cards_col].spacing(6));
    }

    for (cat_id, cat_title, badge_color) in categories {
        let matching_items: Vec<&FbdCatalogItem> = FBD_CATALOG
            .iter()
            .filter(|item| item.category == cat_id)
            .filter(|item| {
                if query.is_empty() {
                    return true;
                }
                item.name.to_lowercase().contains(&query)
                    || item.description_en.to_lowercase().contains(&query)
                    || item.description_fr.to_lowercase().contains(&query)
                    || item.pin_signature.to_lowercase().contains(&query)
            })
            .collect();

        if matching_items.is_empty() {
            continue;
        }

        let section_header = row![
            container(text(cat_id).size(10).color(badge_color))
                .padding([2, 6])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.05
                ))),
            text(cat_title).size(13).color(StudioTheme::TEXT_PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let mut cards_col = column![].spacing(6);

        for item in matching_items {
            let desc = match lang {

                crate::i18n::UiLanguage::Fr => item.description_fr,
                _ => item.description_en,
            };

            let add_btn = if let Some(ref target_pou) = active_fbd_pou {
                let current_block_count = if let Some(PouSource::Fbd(fbd)) = app.project.pous.get(target_pou) {
                    fbd.blocks.len()
                } else {
                    0
                };
                let kind = (item.make_kind)(current_block_count);
                button(text(lang.tr("+ Add", "+ Ajouter")).size(11))
                    .on_press(Message::FbdAddBlock(target_pou.clone(), kind))
                    .style(StudioTheme::primary_button)
                    .padding([8, 16])
            } else {
                button(text(lang.tr("+ Add", "+ Ajouter")).size(11))
                    .style(StudioTheme::ghost_button)
                    .padding([8, 16])
            };

            let card = container(
                row![
                    column![
                        row![
                            text(item.name).size(12),
                            text("|").size(11).color(StudioTheme::BORDER_CARD),
                            text(item.pin_signature)
                                .size(11)
                                .color(StudioTheme::ACCENT_CYAN),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                        text(desc)
                            .size(11)
                            .color(StudioTheme::TEXT_SECONDARY)
                            .wrapping(iced::widget::text::Wrapping::Word),
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    add_btn,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_SURFACE)),
                border: Border {
                    color: StudioTheme::BORDER_CARD,
                    width: 1.0,
                    radius: Radius::from(6.0),
                },
                ..Default::default()
            });

            cards_col = cards_col.push(card);
        }

        content_col = content_col.push(column![section_header, cards_col].spacing(6));
    }

    let builder_panel: Option<Element<'_, Message>> = if app.custom_fbd_builder_open {
        let builder_header = row![
            text(if let Some(ref editing) = app.custom_fbd_editing_name {
                format!("{}: {}", lang.tr("Edit Custom Block", "Modifier le bloc"), editing)
            } else {
                lang.tr("Create New Custom FBD Block (ST)", "Créer un nouveau bloc FBD (ST)").to_string()
            })
            .size(13)
            .color(StudioTheme::ACCENT_CYAN),
            iced::widget::Space::with_width(Length::Fill),
            button(
                text(crate::icons::icon_close())
                    .size(10)
                    .shaping(iced::widget::text::Shaping::Advanced),
            )
            .on_press(Message::ToggleCustomFbdBuilder)
            .style(StudioTheme::ghost_button)
            .padding([2, 6]),
        ]
        .align_y(Alignment::Center);

        let name_row = row![
            column![
                text(lang.tr("Block Name:", "Nom du bloc :")).size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("Name (e.g. DualCheck)", &app.custom_fbd_name_input)
                    .on_input(Message::CustomFbdNameChanged)
                    .padding([5, 8])
                    .style(StudioTheme::styled_input),
            ]
            .width(Length::FillPortion(1))
            .spacing(3),
            column![
                text(lang.tr("Description:", "Description :")).size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("Description...", &app.custom_fbd_desc_input)
                    .on_input(Message::CustomFbdDescChanged)
                    .padding([5, 8])
                    .style(StudioTheme::styled_input),
            ]
            .width(Length::FillPortion(2))
            .spacing(3),
        ]
        .spacing(8);

        let pins_row = row![
            column![
                text(lang.tr("Input Pins (comma separated):", "Entrées (séparées par virgule) :")).size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("IN1, IN2", &app.custom_fbd_inputs_input)
                    .on_input(Message::CustomFbdInputsChanged)
                    .padding([5, 8])
                    .style(StudioTheme::styled_input),
            ]
            .width(Length::FillPortion(1))
            .spacing(3),
            column![
                text(lang.tr("Output Pins (comma separated):", "Sorties (séparées par virgule) :")).size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("OUT, ALARM", &app.custom_fbd_outputs_input)
                    .on_input(Message::CustomFbdOutputsChanged)
                    .padding([5, 8])
                    .style(StudioTheme::styled_input),
            ]
            .width(Length::FillPortion(1))
            .spacing(3),
        ]
        .spacing(8);

        let st_editor_widget = text_editor(&app.custom_fbd_st_content)
            .on_action(Message::CustomFbdStAction)
            .padding(8);

        let st_editor_container = container(st_editor_widget)
            .height(Length::Fixed(120.0))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_SURFACE)),
                border: Border {
                    color: StudioTheme::BORDER_HAIRLINE,
                    width: 1.0,
                    radius: Radius::from(4.0),
                },
                ..Default::default()
            });

        let st_code_section = column![
            text(lang.tr(
                "Structured Text Logic (press Enter for new lines):",
                "Logique Texte Structuré (Entrée pour nouvelle ligne) :"
            ))
            .size(11)
            .color(StudioTheme::TEXT_SECONDARY),
            st_editor_container,
        ]
        .spacing(4);

        let status_badge = if let Some(ref err) = app.custom_fbd_validation_error {
            container(text(format!("⚠ {err}")).size(11).color(StudioTheme::ACCENT_ROSE))
                .padding([3, 8])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.957, 0.247, 0.369, 0.15)))
        } else {
            container(text(lang.tr("✔ Valid ST logic", "✔ Logique ST valide")).size(11).color(StudioTheme::ACCENT_EMERALD))
                .padding([3, 8])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(0.063, 0.725, 0.506, 0.15)))
        };

        let actions_row = row![
            status_badge,
            iced::widget::Space::with_width(Length::Fill),
            button(text(lang.tr("Cancel", "Annuler")).size(11))
                .on_press(Message::ToggleCustomFbdBuilder)
                .style(StudioTheme::ghost_button)
                .padding([8, 16]),
            button(text(lang.tr("Save Block", "Enregistrer le bloc")).size(11))
                .on_press(Message::SaveCustomFbdBlock)
                .style(StudioTheme::primary_button)
                .padding([8, 16]),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        Some(
            container(
                column![
                    builder_header,
                    name_row,
                    pins_row,
                    st_code_section,
                    actions_row,
                ]
                .spacing(8),
            )
            .padding(12)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(StudioTheme::BG_ELEVATED)),
                border: Border {
                    color: StudioTheme::ACCENT_CYAN,
                    width: 1.0,
                    radius: Radius::from(8.0),
                },
                ..Default::default()
            })
            .into(),
        )
    } else {
        None
    };

    let mut main_col = column![header, search_input].spacing(12);
    if let Some(panel) = builder_panel {
        main_col = main_col.push(panel);
    }
    main_col = main_col.push(scrollable(content_col).height(Length::Fill));

    container(main_col.padding(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(StudioTheme::BG_BASE)),
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_project::Project;

    #[test]
    fn test_render_fbd_library_window_with_custom_blocks() -> Result<(), Box<dyn std::error::Error>> {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("ferrite_palette_test_{unique_id}"));
        let p = Project::scaffold(&temp_dir, "PaletteTest")?;
        let mut app = FerriteStudioApp::new(p);

        app.global_custom_fbs.push(ferrite_lang_fbd::CompositeFbDef {
            name: "MathClamp".to_string(),
            description: "Clamps value between min and max".to_string(),
            input_pins: vec!["IN".to_string(), "MIN".to_string(), "MAX".to_string()],
            output_pins: vec!["OUT".to_string()],
            st_code: Some("OUT := LIMIT(MIN, IN, MAX);".to_string()),
            blocks: vec![],
        });

        // 1. Render when builder is closed
        let _ = render_fbd_library_window(&app);

        // 2. Render when builder is open
        app.custom_fbd_builder_open = true;
        app.custom_fbd_name_input = "MathClamp".to_string();
        app.custom_fbd_st_code_input = "OUT := LIMIT(MIN, IN, MAX);".to_string();
        app.custom_fbd_st_content = iced::widget::text_editor::Content::with_text(&app.custom_fbd_st_code_input);
        let _ = render_fbd_library_window(&app);

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
