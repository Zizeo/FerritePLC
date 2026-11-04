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
    // 1. Toolbar Header - Responsive Layout
    let title_block = column![
        text(crate::tr!("symbols.title", lang)).size(16),
        text(crate::tr!("symbols.subtitle", lang))
            .size(11)
            .color(StudioTheme::TEXT_MUTED),
    ]
    .spacing(2);

    let buttons_row = row![
        button(
            row![
                text("+").size(12).color(StudioTheme::ACCENT_CYAN),
                text(crate::tr!("symbols.add_tag", lang)).size(11),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .on_press(Message::AddGlobalTag)
        .style(StudioTheme::primary_button)
        .padding([8, 16]),
        button(
            row![
                text("+").size(12),
                text(crate::tr!("symbols.add_timer", lang)).size(11),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .on_press(Message::AddFbInstance(StandardFbKind::Ton))
        .style(StudioTheme::secondary_button)
        .padding([8, 16]),
        button(
            row![
                text("+").size(12),
                text(crate::tr!("symbols.add_pid", lang)).size(11),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .on_press(Message::AddFbInstance(StandardFbKind::PidMixed))
        .style(StudioTheme::secondary_button)
        .padding([8, 16]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let header = container(
        row![
            title_block,
            iced::widget::Space::with_width(Length::Fill),
            buttons_row,
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill);

    // 2. PLC Scan Configuration Card - Form with labels stacked above inputs
    let scan_config_card = container(
        row![
            text(crate::tr!("symbols.scan_config", lang)).size(12),
            iced::widget::Space::with_width(Length::Fill),
            column![
                text(crate::tr!("symbols.scan_cycle", lang))
                    .size(11)
                    .color(StudioTheme::TEXT_SECONDARY),
                text_input("50", &project.manifest.scan_cycle_ms.to_string())
                    .on_input(|s| {
                        if let Ok(ms) = s.parse::<u64>() {
                            Message::ScanCycleMsChanged(ms)
                        } else {
                            Message::None
                        }
                    })
                    .padding([4, 8])
                    .style(StudioTheme::styled_input),
            ]
            .spacing(4)
            .width(Length::FillPortion(1)),
            column![
                text(crate::tr!("symbols.watchdog", lang))
                    .size(11)
                    .color(StudioTheme::TEXT_SECONDARY),
                text_input("100000", &project.manifest.watchdog_us.to_string())
                    .on_input(|s| {
                        if let Ok(us) = s.parse::<u64>() {
                            Message::WatchdogUsChanged(us)
                        } else {
                            Message::None
                        }
                    })
                    .padding([4, 8])
                    .style(StudioTheme::styled_input),
            ]
            .spacing(4)
            .width(Length::FillPortion(1)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .padding(12)
    .style(StudioTheme::card_style)
    .width(Length::Fill);

    // 3. Global Symbolic Tags Table - Proportionate FillPortion columns
    let mut tags_table = column![
        row![
            text(crate::tr!("symbols.col_name", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(3)),
            text(crate::tr!("symbols.col_type", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(2)),
            text(crate::tr!("symbols.col_addr", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(2)),
            text(crate::tr!("symbols.col_init", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(2)),
            text(crate::tr!("symbols.col_comment", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(4)),
            text(crate::tr!("symbols.col_action", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::Shrink),
        ]
        .spacing(12)
    ]
    .spacing(6);

    for (idx, tag) in project.global_tags.tags.iter().enumerate() {
        let addr_str = tag.address.map(|a| a.to_string()).unwrap_or_default();
        let init_val_str = tag.initial_value.to_string();

        let row_item = row![
            text_input("Tag_Name", &tag.name)
                .on_input(move |s| Message::TagNameChanged(idx, s))
                .width(Length::FillPortion(3))
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
            .width(Length::FillPortion(2))
            .style(StudioTheme::styled_pick_list),
            text_input("%IX0.0", &addr_str)
                .on_input(move |s| Message::TagAddressChanged(idx, s))
                .width(Length::FillPortion(2))
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            text_input("0", &init_val_str)
                .on_input(move |s| Message::TagInitialValueChanged(idx, s))
                .width(Length::FillPortion(2))
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            text_input("Comment", &tag.comment)
                .on_input(move |s| Message::TagCommentChanged(idx, s))
                .width(Length::FillPortion(4))
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            button(text(crate::tr!("symbols.delete", lang)).size(11))
                .on_press(Message::RemoveGlobalTag(idx))
                .style(StudioTheme::danger_button)
                .padding([8, 16]),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        tags_table = tags_table.push(row_item);
    }

    let global_tags_card = container(
        column![
            text(format!(
                "{} — symbols/global_tags.toml",
                crate::tr!("symbols.tags_title", lang, count = project.global_tags.tags.len())
            ))
            .size(13),
            tags_table,
        ]
        .spacing(12),
    )
    .padding(14)
    .style(StudioTheme::card_style)
    .width(Length::Fill);

    // 4. Shared Function Block Instances Table
    let mut fb_table = column![
        row![
            text(crate::tr!("symbols.col_name", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(3)),
            text(crate::tr!("symbols.col_type", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(2)),
            text(crate::tr!("symbols.col_params", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::FillPortion(6)),
            text(crate::tr!("symbols.col_action", lang))
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::Shrink),
        ]
        .spacing(12)
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
                    .width(Length::FillPortion(1))
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
                    .width(Length::FillPortion(1))
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
                    .width(Length::FillPortion(1))
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
                container(
                    text(fb.kind.formula_hint())
                        .size(10.0)
                        .color(StudioTheme::TEXT_MUTED),
                )
                .padding([2, 5])
                .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
            ]
            .spacing(6)
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
                    .width(Length::FillPortion(1))
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
                    .width(Length::FillPortion(1))
                    .padding([3, 6])
                    .style(StudioTheme::styled_input),
            ]
            .spacing(6)
            .align_y(Alignment::Center)
            .into()
        };

        let row_item = row![
            text_input("Timer_1", &fb.name)
                .on_input(move |s| Message::FbNameChanged(idx, s))
                .width(Length::FillPortion(3))
                .padding([4, 8])
                .style(StudioTheme::styled_input),
            pick_list(&ALL_FB_KINDS[..], Some(FbKindWrapper(fb.kind)), move |k| {
                Message::FbKindChanged(idx, k.0)
            })
            .width(Length::FillPortion(2))
            .style(StudioTheme::styled_pick_list),
            container(param_view).width(Length::FillPortion(6)),
            button(text(crate::tr!("symbols.delete", lang)).size(11))
                .on_press(Message::RemoveFbInstance(idx))
                .style(StudioTheme::danger_button)
                .padding([8, 16]),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        fb_table = fb_table.push(row_item);
    }

    let fb_instances_card = container(
        column![
            text(format!(
                "{} — symbols/fb_instances.toml",
                crate::tr!("symbols.fb_title", lang, count = project.fb_instances.instances.len())
            ))
            .size(13),
            fb_table,
        ]
        .spacing(12),
    )
    .padding(14)
    .style(StudioTheme::card_style)
    .width(Length::Fill);

    scrollable(
        column![
            header,
            scan_config_card,
            global_tags_card,
            fb_instances_card,
        ]
        .spacing(14)
        .padding(16),
    )
    .into()
}
