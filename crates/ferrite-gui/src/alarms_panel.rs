use egui::{Color32, RichText, Ui};
use ferrite_core::alarm::{AlarmSeverity, AlarmState};
use ferrite_runtime::PlcRuntime;

/// Renders the ISA-18.2 Alarm Management Panel and Sequence of Events (SOE) Journal.
pub fn render_alarms_panel(ui: &mut Ui, runtime_opt: Option<&mut PlcRuntime>) {
    let Some(runtime) = runtime_opt else {
        ui.centered_and_justified(|ui| {
            ui.label(
                RichText::new("PLC Runtime inactive. Start or step scan to monitor alarms.")
                    .italics()
                    .color(Color32::GRAY),
            );
        });
        return;
    };

    let active_count = runtime.alarms.iter().filter(|a| a.state.is_active()).count();
    let unack_count = runtime
        .alarms
        .iter()
        .filter(|a| a.state == AlarmState::ActiveUnacknowledged || a.state == AlarmState::ClearedUnacknowledged)
        .count();

    // Toolbar header
    ui.horizontal(|ui| {
        crate::theme::StudioTheme::badge(ui, "ALARM", crate::theme::StudioTheme::BG_SURFACE, crate::theme::StudioTheme::ACCENT_ROSE);
        ui.heading(RichText::new("ISA-18.2 Alarm Management").strong().color(crate::theme::StudioTheme::TEXT_PRIMARY));
        ui.add_space(16.0);

        if unack_count > 0 {
            ui.label(
                RichText::new(format!("! {unack_count} Unacknowledged"))
                    .color(crate::theme::StudioTheme::ACCENT_ROSE)
                    .strong(),
            );
        } else {
            ui.label(
                RichText::new("✓ All Alarms Acknowledged")
                    .color(crate::theme::StudioTheme::ACCENT_EMERALD),
            );
        }

        ui.add_space(8.0);
        ui.label(format!("({active_count} Active)"));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(RichText::new("✓ Acknowledge All").color(Color32::WHITE).strong())
                .clicked()
            {
                runtime.acknowledge_all_alarms();
            }
        });
    });

    ui.separator();

    // Split into Active Alarms (top) and SOE Journal (bottom)
    ui.columns(2, |cols| {
        // Left Column: Active Alarms
        crate::theme::StudioTheme::card_frame().show(&mut cols[0], |ui| {
            ui.label(RichText::new("● Active Alarms").strong().color(crate::theme::StudioTheme::TEXT_PRIMARY));
            ui.add_space(4.0);

            let mut ack_alarm_id: Option<String> = None;

            egui::ScrollArea::vertical()
                .id_salt("active_alarms_scroll")
                .max_height(220.0)
                .show(ui, |ui| {
                    let active_alarms: Vec<_> = runtime
                        .alarms
                        .iter()
                        .filter(|a| a.state.is_active())
                        .cloned()
                        .collect();

                    if active_alarms.is_empty() {
                        ui.label(
                            RichText::new("No active alarms. System operating normally.")
                                .italics()
                                .color(Color32::DARK_GRAY),
                        );
                    } else {
                        egui::Grid::new("active_alarms_grid")
                            .striped(true)
                            .min_col_width(70.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Severity").strong());
                                ui.label(RichText::new("ID").strong());
                                ui.label(RichText::new("Tag").strong());
                                ui.label(RichText::new("State").strong());
                                ui.label(RichText::new("Message").strong());
                                ui.label(RichText::new("Action").strong());
                                ui.end_row();

                                for alarm in &active_alarms {
                                    let (sev_txt, sev_color) = match alarm.definition.severity {
                                        AlarmSeverity::Critical => ("CRIT", Color32::from_rgb(230, 40, 40)),
                                        AlarmSeverity::Major => ("MAJ", Color32::from_rgb(230, 130, 30)),
                                        AlarmSeverity::Minor => ("MIN", Color32::from_rgb(220, 190, 40)),
                                        AlarmSeverity::Warning => ("WARN", Color32::from_rgb(50, 140, 220)),
                                    };

                                    ui.label(RichText::new(sev_txt).color(sev_color).strong());
                                    ui.monospace(&alarm.definition.id);
                                    ui.monospace(&alarm.definition.tag);

                                    let state_txt = match alarm.state {
                                        AlarmState::ActiveUnacknowledged => "UNACK",
                                        AlarmState::ActiveAcknowledged => "ACKED",
                                        _ => "OTHER",
                                    };
                                    ui.label(state_txt);
                                    ui.label(&alarm.definition.message);

                                    if alarm.state == AlarmState::ActiveUnacknowledged {
                                        if ui.button("ACK").clicked() {
                                            ack_alarm_id = Some(alarm.definition.id.clone());
                                        }
                                    } else {
                                        ui.label("—");
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                });

            if let Some(id) = ack_alarm_id {
                runtime.acknowledge_alarm(&id);
            }
        });

        // Right Column: Sequence of Events (SOE) Alarm Journal
        crate::theme::StudioTheme::card_frame().show(&mut cols[1], |ui| {
            ui.label(RichText::new("Sequence of Events (SOE) Journal").strong().color(crate::theme::StudioTheme::TEXT_PRIMARY));
            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .id_salt("soe_journal_scroll")
                .max_height(220.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if runtime.alarm_journal.is_empty() {
                        ui.label(
                            RichText::new("Journal empty.").italics().color(Color32::DARK_GRAY),
                        );
                    } else {
                        egui::Grid::new("soe_journal_grid")
                            .striped(true)
                            .min_col_width(60.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Time").strong());
                                ui.label(RichText::new("Severity").strong());
                                ui.label(RichText::new("Alarm ID").strong());
                                ui.label(RichText::new("State").strong());
                                ui.label(RichText::new("Event Description").strong());
                                ui.end_row();

                                for record in runtime.alarm_journal.iter().rev().take(50) {
                                    ui.monospace(format!("T+{}ms", record.timestamp_ms));

                                    let (sev_txt, sev_color) = match record.severity {
                                        AlarmSeverity::Critical => ("CRIT", Color32::from_rgb(230, 40, 40)),
                                        AlarmSeverity::Major => ("MAJ", Color32::from_rgb(230, 130, 30)),
                                        AlarmSeverity::Minor => ("MIN", Color32::from_rgb(220, 190, 40)),
                                        AlarmSeverity::Warning => ("WARN", Color32::from_rgb(50, 140, 220)),
                                    };
                                    ui.label(RichText::new(sev_txt).color(sev_color));
                                    ui.monospace(&record.alarm_id);
                                    ui.label(format!("{:?}", record.state));
                                    ui.label(&record.message);
                                    ui.end_row();
                                }
                            });
                    }
                });
        });
    });
}
