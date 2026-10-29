use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

use ferrite_core::alarm::{AlarmSeverity, AlarmState};
use ferrite_runtime::PlcRuntime;

use crate::app::Message;
use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub fn render_alarms_panel<'a>(
    runtime_opt: Option<&'a PlcRuntime>,
    lang: UiLanguage,
) -> Element<'a, Message> {
    let Some(runtime) = runtime_opt else {
        return container(
            text(lang.tr(
                "PLC Runtime inactive. Start or step scan to monitor alarms.",
                "API inactive. Démarrez ou avancez le cycle pour surveiller les alarmes.",
            ))
            .color(StudioTheme::TEXT_MUTED),
        )
        .padding(24)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    };

    let active_count = runtime
        .alarms
        .iter()
        .filter(|a| a.state.is_active())
        .count();
    let unack_count = runtime
        .alarms
        .iter()
        .filter(|a| {
            a.state == AlarmState::ActiveUnacknowledged
                || a.state == AlarmState::ClearedUnacknowledged
        })
        .count();

    // Toolbar Header
    let unack_badge = if unack_count > 0 {
        container(
            text(format!("! {unack_count} Unack"))
                .size(12)
                .color(StudioTheme::ACCENT_ROSE),
        )
        .padding([3, 8])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            0.957, 0.247, 0.369, 0.15,
        )))
    } else {
        container(
            text(lang.tr("[OK] All Acknowledged", "[OK] Tout Acquitté"))
                .size(12)
                .color(StudioTheme::ACCENT_EMERALD),
        )
        .padding([3, 8])
        .style(StudioTheme::subtle_badge_style(iced::Color::from_rgba(
            0.063, 0.725, 0.506, 0.15,
        )))
    };

    let toolbar = row![
        text("ISA-18.2 Alarm Management").size(15),
        unack_badge,
        text(format!("({active_count} Active)"))
            .size(12)
            .color(StudioTheme::TEXT_SECONDARY),
        iced::widget::Space::with_width(Length::Fill),
        button(text(lang.tr("Acknowledge All", "Tout Acquitter")).size(12))
            .on_press(Message::AcknowledgeAllAlarms)
            .style(StudioTheme::primary_button)
            .padding([6, 12]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    // Left card: Active Alarms
    let mut active_list = column![
        row![
            text("Severity")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(50),
            text("ID").size(11).color(StudioTheme::TEXT_MUTED).width(90),
            text("Tag")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(110),
            text("State")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(70),
            text("Message")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::Fill),
            text("Action")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(60),
        ]
        .spacing(8)
    ]
    .spacing(6);

    let mut found_active = false;
    for alarm in runtime.alarms.iter().filter(|a| a.state.is_active()) {
        found_active = true;
        let (sev_txt, sev_color) = match alarm.definition.severity {
            AlarmSeverity::Critical => ("CRIT", StudioTheme::ACCENT_ROSE),
            AlarmSeverity::Major => ("MAJ", StudioTheme::ACCENT_AMBER),
            AlarmSeverity::Minor => ("MIN", iced::Color::from_rgb(0.9, 0.8, 0.2)),
            AlarmSeverity::Warning => ("WARN", StudioTheme::ACCENT_CYAN),
        };

        let state_txt = match alarm.state {
            AlarmState::ActiveUnacknowledged => "UNACK",
            AlarmState::ActiveAcknowledged => "ACKED",
            _ => "OTHER",
        };

        let ack_btn: Element<'_, Message> = if alarm.state == AlarmState::ActiveUnacknowledged {
            button(text("ACK").size(10))
                .on_press(Message::AcknowledgeAlarm(alarm.definition.id.clone()))
                .style(StudioTheme::secondary_button)
                .padding([2, 8])
                .into()
        } else {
            text("-").size(12).color(StudioTheme::TEXT_MUTED).into()
        };

        let item_row = row![
            text(sev_txt).size(11).color(sev_color).width(50),
            text(alarm.definition.id.clone()).size(11).width(90),
            text(alarm.definition.tag.clone()).size(11).width(110),
            text(state_txt).size(11).width(70),
            text(alarm.definition.message.clone())
                .size(11)
                .width(Length::Fill),
            ack_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        active_list = active_list.push(item_row);
    }

    if !found_active {
        active_list = active_list.push(
            text(lang.tr(
                "No active alarms. System operating normally.",
                "Aucune alarme active. Fonctionnement normal.",
            ))
            .color(StudioTheme::TEXT_MUTED)
            .size(12),
        );
    }

    let active_card = container(
        column![
            text(lang.tr("Active Alarms", "Alarmes Actives")).size(13),
            scrollable(active_list).height(160),
        ]
        .spacing(10),
    )
    .padding(14)
    .style(StudioTheme::card_style)
    .width(Length::FillPortion(1));

    // Right card: Sequence of Events (SOE) Journal
    let mut journal_list = column![
        row![
            text("Time")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(70),
            text("Severity")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(50),
            text("ID").size(11).color(StudioTheme::TEXT_MUTED).width(80),
            text("State")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(80),
            text("Event")
                .size(11)
                .color(StudioTheme::TEXT_MUTED)
                .width(Length::Fill),
        ]
        .spacing(8)
    ]
    .spacing(6);

    if runtime.alarm_journal.is_empty() {
        journal_list = journal_list.push(
            text(lang.tr("Journal empty.", "Journal vide."))
                .color(StudioTheme::TEXT_MUTED)
                .size(12),
        );
    } else {
        for record in runtime.alarm_journal.iter().rev().take(30) {
            let (sev_txt, sev_color) = match record.severity {
                AlarmSeverity::Critical => ("CRIT", StudioTheme::ACCENT_ROSE),
                AlarmSeverity::Major => ("MAJ", StudioTheme::ACCENT_AMBER),
                AlarmSeverity::Minor => ("MIN", iced::Color::from_rgb(0.9, 0.8, 0.2)),
                AlarmSeverity::Warning => ("WARN", StudioTheme::ACCENT_CYAN),
            };

            let row_item = row![
                text(format!("T+{}ms", record.timestamp_ms))
                    .size(11)
                    .width(70),
                text(sev_txt).size(11).color(sev_color).width(50),
                text(record.alarm_id.clone()).size(11).width(80),
                text(format!("{:?}", record.state)).size(11).width(80),
                text(record.message.clone()).size(11).width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            journal_list = journal_list.push(row_item);
        }
    }

    let journal_card = container(
        column![
            text(lang.tr(
                "Sequence of Events (SOE) Journal",
                "Journal Chronologique d'Événements (SOE)"
            ))
            .size(13),
            scrollable(journal_list).height(160),
        ]
        .spacing(10),
    )
    .padding(14)
    .style(StudioTheme::card_style)
    .width(Length::FillPortion(1));

    column![
        toolbar,
        row![active_card, journal_card]
            .spacing(16)
            .width(Length::Fill),
    ]
    .spacing(12)
    .into()
}
