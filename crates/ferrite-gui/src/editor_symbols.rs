use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length};

use ferrite_core::{IecType, StandardFbKind};
use ferrite_project::Project;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FbKindWrapper(pub StandardFbKind);

impl std::fmt::Display for FbKindWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.label())
    }
}

pub const ALL_FB_KINDS: [FbKindWrapper; 14] = [
    FbKindWrapper(StandardFbKind::Ton),
    FbKindWrapper(StandardFbKind::Tof),
    FbKindWrapper(StandardFbKind::Tp),
    FbKindWrapper(StandardFbKind::Ctu),
    FbKindWrapper(StandardFbKind::Ctd),
    FbKindWrapper(StandardFbKind::RTrig),
    FbKindWrapper(StandardFbKind::FTrig),
    FbKindWrapper(StandardFbKind::Sr),
    FbKindWrapper(StandardFbKind::Rs),
    FbKindWrapper(StandardFbKind::P),
    FbKindWrapper(StandardFbKind::Pi),
    FbKindWrapper(StandardFbKind::PidParallel),
    FbKindWrapper(StandardFbKind::PidSeries),
    FbKindWrapper(StandardFbKind::PidMixed),
];

pub fn render_symbols_editor<'a>(project: &'a Project, lang: UiLanguage) -> Element<'a, Message> {
    // 1. Toolbar Header
    let header = row![
        text(lang.tr(
            "Unified Symbol Table & Function Block Registry",
            "Table des Symboles Unifiée & Registre des Blocs Fonctionnels",
        ))
        .size(18),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("+ Add Global Tag", "+ Ajouter Variable Globale")).size(12))
            .on_press(Message::AddGlobalTag)
            .style(StudioTheme::primary_button)
            .padding([6, 12]),
        button(text(lang.tr("+ Add Timer (TON)", "+ Ajouter Temporisateur (TON)")).size(12))
            .on_press(Message::AddFbInstance(StandardFbKind::Ton))
            .style(StudioTheme::secondary_button)
            .padding([6, 12]),
        button(text(lang.tr("+ Add PID Corrector", "+ Ajouter Correcteur PID")).size(12))
            .on_press(Message::AddFbInstance(StandardFbKind::PidMixed))
            .style(StudioTheme::secondary_button)
            .padding([6, 12]),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    // 2. PLC Scan Configuration Card
    let scan_config_card = container(
        row![
            text(lang.tr("PLC Scan Configuration:", "Configuration du Cycle API :")).size(13),
            text(lang.tr("Scan Cycle (ms):", "Cycle (ms) :"))
                .size(12)
                .color(StudioTheme::TEXT_SECONDARY),
            text_input("50", &project.manifest.scan_cycle_ms.to_string(),)
                .on_input(|s| {
                    if let Ok(ms) = s.parse::<u64>() {
                        Message::ScanCycleMsChanged(ms)
                    } else {
                        Message::None
                    }
                })
                .width(70)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            text(lang.tr("Watchdog (µs):", "Chien de garde (µs) :"))
                .size(12)
                .color(StudioTheme::TEXT_SECONDARY),
            text_input("100000", &project.manifest.watchdog_us.to_string(),)
                .on_input(|s| {
                    if let Ok(us) = s.parse::<u64>() {
                        Message::WatchdogUsChanged(us)
                    } else {
                        Message::None
                    }
                })
                .width(90)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(12)
    .style(StudioTheme::card_style)
    .width(Length::Fill);

    // 3. Global Symbolic Tags Table
    let mut tags_table = column![
        row![
            text(lang.tr("Name", "Nom"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(160),
            text(lang.tr("IEC Type", "Type CEI"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(100),
            text(lang.tr("Direct Address (%I/%Q/%M)", "Adresse Directe (%I/%Q/%M)"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(130),
            text(lang.tr("Initial Value", "Valeur Initiale"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(110),
            text(lang.tr("Comment", "Commentaire"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::Fill),
            text("Action")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(60),
        ]
        .spacing(10)
    ]
    .spacing(6);

    for (idx, tag) in project.global_tags.tags.iter().enumerate() {
        let addr_str = tag.address.map(|a| a.to_string()).unwrap_or_default();
        let init_val_str = tag.initial_value.to_string();

        let row_item = row![
            text_input("Tag_Name", &tag.name)
                .on_input(move |s| Message::TagNameChanged(idx, s))
                .width(160)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            pick_list(
                &[
                    IecType::Bool,
                    IecType::Int,
                    IecType::Dint,
                    IecType::Real,
                    IecType::Time,
                    IecType::String,
                ][..],
                Some(tag.data_type),
                move |ty| Message::TagTypeChanged(idx, ty),
            )
            .width(100)
            .style(StudioTheme::styled_pick_list),
            text_input("%IX0.0", &addr_str)
                .on_input(move |s| Message::TagAddressChanged(idx, s))
                .width(130)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            text_input("0", &init_val_str)
                .on_input(move |s| Message::TagInitialValueChanged(idx, s))
                .width(110)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            text_input("Comment", &tag.comment)
                .on_input(move |s| Message::TagCommentChanged(idx, s))
                .width(Length::Fill)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            button(text(lang.tr("Delete", "Suppr")).size(11))
                .on_press(Message::RemoveGlobalTag(idx))
                .style(StudioTheme::danger_button)
                .padding([4, 8]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        tags_table = tags_table.push(row_item);
    }

    let global_tags_card = container(
        column![
            row![
                text(format!(
                    "{} ({}) — symbols/global_tags.toml",
                    lang.tr("Global Symbolic Tags", "Variables Symboliques Globales"),
                    project.global_tags.tags.len()
                ))
                .size(14),
            ],
            tags_table,
        ]
        .spacing(12),
    )
    .padding(16)
    .style(StudioTheme::card_style)
    .width(Length::Fill);

    // 4. Shared Function Block Instances Table
    let mut fb_table = column![
        row![
            text(lang.tr("Instance Name", "Nom d'Instance"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(160),
            text(lang.tr("FB Type", "Type FB"))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(120),
            text(lang.tr(
                "Parameters (Kp / Ti / Td / PT)",
                "Paramètres (Kp / Ti / Td / PT)"
            ))
            .size(11)
            .color(StudioTheme::TEXT_MUTED)
            .width(Length::Fill),
            text("Action")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(60),
        ]
        .spacing(10)
    ]
    .spacing(6);

    for (idx, fb) in project.fb_instances.instances.iter().enumerate() {
        let param_view: Element<'_, Message> = if fb.kind.is_corrector() {
            let kp_str = format!("{:.2}", fb.kp);
            let ti_str = format!("{:.2}", fb.ti_s);
            let td_str = format!("{:.2}", fb.td_s);

            row![
                text("Kp:").size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("1.0", &kp_str)
                    .on_input(move |s| {
                        if let Ok(v) = s.parse::<f32>() {
                            Message::FbGainChanged(idx, v)
                        } else {
                            Message::None
                        }
                    })
                    .width(65)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                text("Ti(s):").size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("2.0", &ti_str)
                    .on_input(move |s| {
                        if let Ok(v) = s.parse::<f32>() {
                            Message::FbTiChanged(idx, v)
                        } else {
                            Message::None
                        }
                    })
                    .width(65)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                text("Td(s):").size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("0.5", &td_str)
                    .on_input(move |s| {
                        if let Ok(v) = s.parse::<f32>() {
                            Message::FbTdChanged(idx, v)
                        } else {
                            Message::None
                        }
                    })
                    .width(65)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                text(fb.kind.formula_hint())
                    .size(11)
                    .color(StudioTheme::TEXT_MUTED),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        } else {
            let pt_str = format!("{}", fb.pt_ms);
            let pv_str = format!("{}", fb.pv);

            row![
                text("PT(ms):").size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("5000", &pt_str)
                    .on_input(move |s| {
                        if let Ok(v) = s.parse::<u64>() {
                            Message::FbPtChanged(idx, v)
                        } else {
                            Message::None
                        }
                    })
                    .width(80)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                text("PV:").size(11).color(StudioTheme::TEXT_SECONDARY),
                text_input("10", &pv_str)
                    .on_input(move |s| {
                        if let Ok(v) = s.parse::<i16>() {
                            Message::FbPvChanged(idx, v)
                        } else {
                            Message::None
                        }
                    })
                    .width(60)
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        };

        let row_item = row![
            text_input("Timer_1", &fb.name)
                .on_input(move |s| Message::FbNameChanged(idx, s))
                .width(160)
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            pick_list(&ALL_FB_KINDS[..], Some(FbKindWrapper(fb.kind)), move |k| {
                Message::FbKindChanged(idx, k.0)
            },)
            .width(120)
            .style(StudioTheme::styled_pick_list),
            param_view,
            button(text(lang.tr("Delete", "Suppr")).size(11))
                .on_press(Message::RemoveFbInstance(idx))
                .style(StudioTheme::danger_button)
                .padding([4, 8]),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        fb_table = fb_table.push(row_item);
    }

    let fb_instances_card = container(
        column![
            row![
                text(format!(
                    "{} ({}) — symbols/fb_instances.toml",
                    lang.tr(
                        "Shared Function Block Instances",
                        "Instances de Blocs Fonctionnels Partagés"
                    ),
                    project.fb_instances.instances.len()
                ))
                .size(14),
            ],
            fb_table,
        ]
        .spacing(12),
    )
    .padding(16)
    .style(StudioTheme::card_style)
    .width(Length::Fill);

    scrollable(
        column![
            header,
            scan_config_card,
            global_tags_card,
            fb_instances_card,
        ]
        .spacing(16)
        .padding(20),
    )
    .into()
}
