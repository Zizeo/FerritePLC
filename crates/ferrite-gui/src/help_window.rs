//! In-application help manual and comprehensive symbol reference guide.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpCategory {
    Architecture,
    LadderDiagram,
    SfcGrafcet,
    StructuredText,
    FunctionBlockDiagram,
    MemoryAddressing,
    Simulation,
    FieldbusProtocols,
    HardwareEmbedded,
    DigitalTwin,
    ProjectTools,
}

impl HelpCategory {
    pub fn all() -> &'static [HelpCategory] {
        &[
            HelpCategory::Architecture,
            HelpCategory::LadderDiagram,
            HelpCategory::SfcGrafcet,
            HelpCategory::StructuredText,
            HelpCategory::FunctionBlockDiagram,
            HelpCategory::MemoryAddressing,
            HelpCategory::Simulation,
            HelpCategory::FieldbusProtocols,
            HelpCategory::HardwareEmbedded,
            HelpCategory::DigitalTwin,
            HelpCategory::ProjectTools,
        ]
    }

    pub fn title(self, lang: crate::i18n::UiLanguage) -> &'static str {
        match self {
            HelpCategory::Architecture => lang.tr("1. System Architecture", "1. Architecture Système"),
            HelpCategory::LadderDiagram => lang.tr("2. Ladder Diagram (LD)", "2. Schéma Ladder (LD)"),
            HelpCategory::SfcGrafcet => lang.tr("3. Grafcet / SFC", "3. Grafcet / SFC"),
            HelpCategory::StructuredText => lang.tr("4. Structured Text (ST)", "4. Texte Structuré (ST)"),
            HelpCategory::FunctionBlockDiagram => {
                lang.tr("5. Function Block Diagram (FBD)", "5. Blocs Fonctionnels (FBD)")
            }
            HelpCategory::MemoryAddressing => lang.tr("6. Memory & Tags", "6. Mémoire & Variables"),
            HelpCategory::Simulation => lang.tr("7. Simulation & Diagnostics", "7. Simulation & Diagnostics"),
            HelpCategory::FieldbusProtocols => {
                lang.tr("8. Fieldbus & Communication", "8. Bus de Terrain & Communication")
            }
            HelpCategory::HardwareEmbedded => {
                lang.tr("9. Hardware & Embedded HAL", "9. Matériel & HAL Embarqué")
            }
            HelpCategory::DigitalTwin => lang.tr("10. 2D Digital Twin", "10. Jumeau Numérique 2D"),
            HelpCategory::ProjectTools => lang.tr("11. Tools & Shortcuts", "11. Outils & Raccourcis"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpTopic {
    // 1. Architecture
    ArchScanLoop,
    ArchProjectFiles,
    ArchInterLang,
    // 2. Ladder Diagram
    LdContacts,
    LdCoils,
    LdBranches,
    LdCompareMath,
    LdInlineFb,
    // 3. SFC / Grafcet
    SfcSteps,
    SfcActions,
    SfcDivergenceConvergence,
    SfcIntrospection,
    // 4. Structured Text
    StSyntax,
    StControlFlow,
    StMathFunctions,
    // 5. Function Block Diagram
    FbdStandardBlocks,
    FbdPidController,
    FbdLogicMath,
    FbdCustomComposite,
    // 6. Memory & Tags
    MemDirectAddressing,
    MemTagTable,
    // 7. Simulation & Diagnostics
    SimExecutionAndForces,
    SimAlarmsAndRetain,
    SimTrendPlotter,
    // 8. Fieldbus & Protocols
    FieldbusTcpIot,
    FieldbusModbusRtu,
    // 9. Hardware & Embedded
    HardwareRpiGpio,
    // 10. Digital Twin
    DigitalTwinSimulation,
    // 11. Tools & Shortcuts
    ToolsPlcOpenXml,
    CanvasShortcuts,
}

pub struct HelpTopicMeta {
    pub id: HelpTopic,
    pub category: HelpCategory,
    pub title_en: &'static str,
    pub title_fr: &'static str,
    pub keywords: &'static [&'static str],
}

pub static TOPIC_CATALOG: &[HelpTopicMeta] = &[
    // 1. Architecture
    HelpTopicMeta {
        id: HelpTopic::ArchScanLoop,
        category: HelpCategory::Architecture,
        title_en: "Deterministic Scan Cycle & Clock",
        title_fr: "Cycle de Scan Déterministe & Horloge",
        keywords: &["scan", "cycle", "loop", "clock", "watchdog", "phases", "virtual time", "inputs", "outputs"],
    },
    HelpTopicMeta {
        id: HelpTopic::ArchProjectFiles,
        category: HelpCategory::Architecture,
        title_en: "Project Structure & Manifest (.fplc)",
        title_fr: "Structure du Projet & Manifeste (.fplc)",
        keywords: &["project", "manifest", "fplc", "file", "pou", "symbols", "scaffold", "json", "toml"],
    },
    HelpTopicMeta {
        id: HelpTopic::ArchInterLang,
        category: HelpCategory::Architecture,
        title_en: "Inter-Language Communication & Process Image",
        title_fr: "Communication Inter-Langages & Image Processus",
        keywords: &["inter-language", "process image", "memory map", "pou", "executablepou", "cross-language", "shared", "context"],
    },
    // 2. Ladder Diagram
    HelpTopicMeta {
        id: HelpTopic::LdContacts,
        category: HelpCategory::LadderDiagram,
        title_en: "Ladder Contacts (--[ ]--, --[/]--, --[P]--, --[N]--)",
        title_fr: "Contacts Ladder (--[ ]--, --[/]--, --[P]--, --[N]--)",
        keywords: &["contact", "ld", "ladder", "normally open", "normally closed", "no", "nc", "rising edge", "falling edge", "pulse", "--[ ]--", "--[/]--", "--[P]--", "--[N]--"],
    },
    HelpTopicMeta {
        id: HelpTopic::LdCoils,
        category: HelpCategory::LadderDiagram,
        title_en: "Ladder Coils (--( )--, --[S]--, --[R]--)",
        title_fr: "Bobines Ladder (--( )--, --[S]--, --[R]--)",
        keywords: &["coil", "ld", "ladder", "normal coil", "set", "reset", "latch", "unlatch", "--( )--", "--[S]--", "--[R]--"],
    },
    HelpTopicMeta {
        id: HelpTopic::LdBranches,
        category: HelpCategory::LadderDiagram,
        title_en: "Rung Branches & Power Flow",
        title_fr: "Branches de Rung & Flux d'Énergie",
        keywords: &["branch", "rung", "parallel", "or branch", "power rail", "dissolution", "wire"],
    },
    HelpTopicMeta {
        id: HelpTopic::LdCompareMath,
        category: HelpCategory::LadderDiagram,
        title_en: "Comparison & Math/Move Rung Blocks",
        title_fr: "Blocs de Comparaison & Calcul Math/Move sur Rung",
        keywords: &["compare", "math", "move", "rung", "gt", "lt", "eq", "ge", "le", "ne", "add", "sub", "mul", "div"],
    },
    HelpTopicMeta {
        id: HelpTopic::LdInlineFb,
        category: HelpCategory::LadderDiagram,
        title_en: "Inline Function Blocks in Ladder Rungs",
        title_fr: "Blocs Fonctionnels Intégrés en Rung Ladder",
        keywords: &["inline fb", "function block", "ton", "ctu", "ladder", "rung", "enable", "en", "eno"],
    },
    // 3. SFC / Grafcet
    HelpTopicMeta {
        id: HelpTopic::SfcSteps,
        category: HelpCategory::SfcGrafcet,
        title_en: "Steps & Initial States ([[ 0 ]], [ 1 ])",
        title_fr: "Étapes & États Initiaux ([[ 0 ]], [ 1 ])",
        keywords: &["step", "initial", "normal", "macro", "sfc", "grafcet", "state", "token", "[[ 0 ]]", "[ 1 ]"],
    },
    HelpTopicMeta {
        id: HelpTopic::SfcActions,
        category: HelpCategory::SfcGrafcet,
        title_en: "Action Qualifiers (N, S, R, L, D, P)",
        title_fr: "Qualificateurs d'Action (N, S, R, L, D, P)",
        keywords: &["action", "qualifier", "stored", "reset", "limited", "delayed", "pulse", "N", "S", "R", "L", "D", "P", "duration"],
    },
    HelpTopicMeta {
        id: HelpTopic::SfcDivergenceConvergence,
        category: HelpCategory::SfcGrafcet,
        title_en: "Divergences & Convergences (AND === / OR —)",
        title_fr: "Divergences & Convergences (ET === / OU —)",
        keywords: &["divergence", "convergence", "and", "or", "receptivity", "transition", "iec 60848", "rules", "alternation", "==="],
    },
    HelpTopicMeta {
        id: HelpTopic::SfcIntrospection,
        category: HelpCategory::SfcGrafcet,
        title_en: "Grafcet Introspection Symbols (X<i>.state, X<i>.t)",
        title_fr: "Symboles d'Introspection Grafcet (X<i>.state, X<i>.t)",
        keywords: &["introspection", "x.state", "x.t", "step activity", "elapsed time", "cross-language", "receptivity"],
    },
    // 4. Structured Text
    HelpTopicMeta {
        id: HelpTopic::StSyntax,
        category: HelpCategory::StructuredText,
        title_en: "Structured Text Syntax & Operators",
        title_fr: "Syntaxe & Opérateurs Structured Text",
        keywords: &["st", "structured text", "syntax", "operators", "arithmetic", "bitwise", "relational", ":=", "+", "-", "*", "/", "MOD", "AND", "OR", "NOT"],
    },
    HelpTopicMeta {
        id: HelpTopic::StControlFlow,
        category: HelpCategory::StructuredText,
        title_en: "Control Flow (IF, CASE, FOR, WHILE)",
        title_fr: "Structures de Contrôle (IF, CASE, FOR, WHILE)",
        keywords: &["control flow", "if", "then", "else", "elsif", "case", "of", "for", "to", "by", "while", "do"],
    },
    HelpTopicMeta {
        id: HelpTopic::StMathFunctions,
        category: HelpCategory::StructuredText,
        title_en: "Standard Math Functions (ABS, SQRT, etc.)",
        title_fr: "Fonctions Mathématiques Standards (ABS, SQRT, etc.)",
        keywords: &["math", "functions", "abs", "sqrt", "sin", "cos", "tan", "exp", "ln", "log", "asin", "acos", "atan"],
    },
    // 5. Function Block Diagram
    HelpTopicMeta {
        id: HelpTopic::FbdStandardBlocks,
        category: HelpCategory::FunctionBlockDiagram,
        title_en: "Standard Function Blocks (TON, TOF, TP, CTU, etc.)",
        title_fr: "Blocs Fonctionnels Standards (TON, TOF, TP, CTU, etc.)",
        keywords: &["fbd", "function block", "ton", "tof", "tp", "ctu", "ctd", "r_trig", "f_trig", "sr", "rs", "timer", "counter"],
    },
    HelpTopicMeta {
        id: HelpTopic::FbdPidController,
        category: HelpCategory::FunctionBlockDiagram,
        title_en: "PID Closed-Loop Controller",
        title_fr: "Régulateur Boucle Fermée PID",
        keywords: &["pid", "controller", "closed loop", "parallel", "series", "mixed", "ideal", "kp", "ti", "td", "anti-windup"],
    },
    HelpTopicMeta {
        id: HelpTopic::FbdLogicMath,
        category: HelpCategory::FunctionBlockDiagram,
        title_en: "FBD Logic, Comparison & Math Blocks",
        title_fr: "Blocs Logiques, Comparaisons & Calculs FBD",
        keywords: &["fbd", "logic", "and", "or", "xor", "not", "add", "sub", "mul", "div", "gt", "lt", "eq", "math"],
    },
    HelpTopicMeta {
        id: HelpTopic::FbdCustomComposite,
        category: HelpCategory::FunctionBlockDiagram,
        title_en: "Custom & Composite User Function Blocks",
        title_fr: "Blocs Fonctionnels Utilisateur & Composites",
        keywords: &["composite", "custom", "user fb", "custom fbd library", "encapsulation", "reusable", "sub-diagram"],
    },
    // 6. Memory & Addressing
    HelpTopicMeta {
        id: HelpTopic::MemDirectAddressing,
        category: HelpCategory::MemoryAddressing,
        title_en: "Direct Addressing (%I, %Q, %M)",
        title_fr: "Adressage Direct (%I, %Q, %M)",
        keywords: &[
            "addressing", "direct", "%i", "%q", "%m", "%ix", "%qx", "%mx", "%ib", "%qb", "%mb",
            "%iw", "%qw", "%mw", "%id", "%qd", "%md", "inputs", "outputs", "memory", "bit",
            "byte", "word", "dword",
        ],
    },
    HelpTopicMeta {
        id: HelpTopic::MemTagTable,
        category: HelpCategory::MemoryAddressing,
        title_en: "Symbolic Tag Table & Data Types",
        title_fr: "Table des Variables & Types IEC",
        keywords: &["tags", "variables", "symbols", "bool", "int", "dint", "real", "time", "string", "iec types", "retained"],
    },
    // 7. Simulation & Diagnostics
    HelpTopicMeta {
        id: HelpTopic::SimExecutionAndForces,
        category: HelpCategory::Simulation,
        title_en: "Scan Engine, Virtual Clock & I/O Forcing",
        title_fr: "Moteur de Scan, Horloge Virtuelle & Forçage E/S",
        keywords: &["simulation", "run", "pause", "step", "cycle", "forces", "force table", "unforce", "virtual clock", "[!]"],
    },
    HelpTopicMeta {
        id: HelpTopic::SimAlarmsAndRetain,
        category: HelpCategory::Simulation,
        title_en: "ISA 18.2 Alarm Lifecycle & Retain Snapshots",
        title_fr: "Cycle de Vie Alarmes ISA 18.2 & Mémoire Rémanente",
        keywords: &["alarms", "isa 18.2", "journal", "unacknowledged", "acknowledged", "retain", "snapshots", "persistence"],
    },
    HelpTopicMeta {
        id: HelpTopic::SimTrendPlotter,
        category: HelpCategory::Simulation,
        title_en: "Real-Time Trend Plotter & Oscilloscope",
        title_fr: "Traceur de Tendances Temps Réel & Oscilloscope",
        keywords: &["trend", "plotter", "oscilloscope", "waveform", "sampling", "history", "live", "trigger", "channels"],
    },
    // 8. Fieldbus & Communication
    HelpTopicMeta {
        id: HelpTopic::FieldbusTcpIot,
        category: HelpCategory::FieldbusProtocols,
        title_en: "Modbus TCP, OPC UA & MQTT Telemetry",
        title_fr: "Modbus TCP, OPC UA & Télémesure MQTT",
        keywords: &["fieldbus", "modbus", "modbus tcp", "opc ua", "mqtt", "telemetry", "holding registers", "coils", "address space"],
    },
    HelpTopicMeta {
        id: HelpTopic::FieldbusModbusRtu,
        category: HelpCategory::FieldbusProtocols,
        title_en: "Modbus RTU Serial (RS-485 / RS-232)",
        title_fr: "Modbus RTU Série (RS-485 / RS-232)",
        keywords: &["rtu", "serial", "rs485", "rs232", "baud rate", "crc", "parity", "t1.5", "t3.5", "modbus rtu", "master", "slave"],
    },
    // 9. Hardware & Embedded HAL
    HelpTopicMeta {
        id: HelpTopic::HardwareRpiGpio,
        category: HelpCategory::HardwareEmbedded,
        title_en: "Raspberry Pi 40-Pin GPIO Header Visualizer",
        title_fr: "Visualiseur GPIO 40 Broches Raspberry Pi",
        keywords: &["gpio", "raspberry pi", "rpi", "hardware", "hal", "40-pin", "pinout", "levels"],
    },
    // 10. Digital Twin
    HelpTopicMeta {
        id: HelpTopic::DigitalTwinSimulation,
        category: HelpCategory::DigitalTwin,
        title_en: "Digital Twin 2D Simulation Canvas",
        title_fr: "Canvas de Simulation Digital Twin 2D",
        keywords: &["digital twin", "twin", "simulation", "physics", "tanks", "pumps", "valves", "conveyors", "sensors", "schematic"],
    },
    // 11. Tools & Shortcuts
    HelpTopicMeta {
        id: HelpTopic::ToolsPlcOpenXml,
        category: HelpCategory::ProjectTools,
        title_en: "PLCopen TC6 XML Export & Import",
        title_fr: "Import & Export PLCopen TC6 XML",
        keywords: &["plcopen", "tc6", "xml", "interoperability", "export", "import", "twincat", "codesys", "standard"],
    },
    HelpTopicMeta {
        id: HelpTopic::CanvasShortcuts,
        category: HelpCategory::ProjectTools,
        title_en: "Studio Navigation & Keyboard Shortcuts",
        title_fr: "Navigation Studio & Raccourcis Clavier",
        keywords: &["shortcuts", "hotkeys", "keyboard", "f1", "ctrl+s", "ctrl+o", "ctrl+z", "ctrl+y", "pan", "zoom", "marquee", "wiring"],
    },
];

pub fn all_topics() -> &'static [HelpTopicMeta] {
    TOPIC_CATALOG
}

pub fn get_topic_meta(topic: HelpTopic) -> Option<&'static HelpTopicMeta> {
    TOPIC_CATALOG.iter().find(|t| t.id == topic)
}

pub fn filter_topics(query: &str) -> Vec<HelpTopic> {
    let clean = query.trim().to_lowercase();
    if clean.is_empty() {
        return TOPIC_CATALOG.iter().map(|t| t.id).collect();
    }
    TOPIC_CATALOG
        .iter()
        .filter(|t| {
            t.title_en.to_lowercase().contains(&clean)
                || t.title_fr.to_lowercase().contains(&clean)
                || t.keywords.iter().any(|k| k.to_lowercase().contains(&clean))
        })
        .map(|t| t.id)
        .collect()
}

use iced::border::Radius;
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub fn view_help_window<'a>(
    app: &'a crate::app::FerriteStudioApp,
) -> Element<'a, crate::app::Message> {
    let lang = app.ui_lang;
    let selected = app.help_selected_topic;
    let query = &app.help_search_query;
    let matched_topics = filter_topics(query);

    // Top Header Bar
    let title_badge = row![
        container(
            text(crate::icons::icon_book())
                .size(16)
                .shaping(iced::widget::text::Shaping::Advanced)
        )
        .padding([2, 6]),
        column![
            text(lang.tr(
                "FerritePLC — Help & Industrial Reference Manual",
                "FerritePLC — Manuel d'Aide & Référence Industrielle"
            ))
            .size(14)
            .color(StudioTheme::TEXT_PRIMARY),
            text(lang.tr(
                "IEC 61131-3 & IEC 60848 Comprehensive Documentation",
                "Documentation Complète CEI 61131-3 & CEI 60848"
            ))
            .size(11)
            .color(StudioTheme::TEXT_MUTED),
        ]
        .spacing(1),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let search_input = container(
        text_input(
            lang.tr(
                "Search symbols, topics, rules (e.g. contact, ton, %mw)...",
                "Rechercher symboles, règles, mots-clés...",
            ),
            query,
        )
        .on_input(crate::app::Message::HelpSearchChanged)
        .size(12)
        .padding([6, 10]),
    )
    .width(Length::Fixed(380.0));

    let close_btn = button(
        row![
            text(crate::icons::icon_close())
                .size(12)
                .shaping(iced::widget::text::Shaping::Advanced),
            text(lang.tr("Close", "Fermer")).size(12),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .on_press(crate::app::Message::CloseHelpWindow)
    .style(StudioTheme::ghost_button)
    .padding([5, 12]);

    let top_bar = container(
        row![
            title_badge,
            Space::with_width(Length::Fill),
            search_input,
            Space::with_width(Length::Fixed(12.0)),
            close_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(StudioTheme::header_container_style)
    .width(Length::Fill);

    // Left Topic Sidebar
    let mut sidebar_col = column![].spacing(10);

    for &category in HelpCategory::all() {
        let category_topics: Vec<_> = TOPIC_CATALOG
            .iter()
            .filter(|t| t.category == category && matched_topics.contains(&t.id))
            .collect();

        if category_topics.is_empty() {
            continue;
        }

        let cat_header = container(
            text(category.title(lang))
                .size(11)
                .color(StudioTheme::ACCENT_CYAN),
        )
        .padding([4, 8]);

        let mut topics_col = column![cat_header].spacing(2);

        for topic_meta in category_topics {
            let is_active = topic_meta.id == selected;
            let topic_title = lang.tr(topic_meta.title_en, topic_meta.title_fr);

            let btn_style = if is_active {
                StudioTheme::primary_button
            } else {
                StudioTheme::ghost_button
            };

            let topic_btn = button(
                text(topic_title).size(11.5).color(if is_active {
                    StudioTheme::TEXT_PRIMARY
                } else {
                    StudioTheme::TEXT_SECONDARY
                }),
            )
            .width(Length::Fill)
            .on_press(crate::app::Message::SelectHelpTopic(topic_meta.id))
            .style(btn_style)
            .padding([5, 10]);

            topics_col = topics_col.push(topic_btn);
        }

        sidebar_col = sidebar_col.push(topics_col);
    }

    if matched_topics.is_empty() {
        sidebar_col = sidebar_col.push(
            container(
                column![
                    text(lang.tr(
                        "No matching topics found.",
                        "Aucun sujet correspondant trouvé.",
                    ))
                    .size(12)
                    .color(StudioTheme::TEXT_MUTED),
                    text(lang.tr(
                        "Try keywords like 'contact', 'sfc', 'pid', '%mw'...",
                        "Essayez des mots-clés comme 'contact', 'sfc', 'pid', '%mw'...",
                    ))
                    .size(11)
                    .color(StudioTheme::TEXT_MUTED),
                ]
                .spacing(4),
            )
            .padding([16, 8]),
        );
    }

    let sidebar = container(
        scrollable(sidebar_col)
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .width(Length::Fixed(270.0))
    .height(Length::Fill)
    .padding([12, 8])
    .style(StudioTheme::sidebar_container_style);

    // Right Content Pane
    let topic_content = render_topic_content(selected, lang);
    let content_pane = container(
        scrollable(topic_content)
            .height(Length::Fill)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(20)
    .style(StudioTheme::canvas_container_style);

    // Main 2-Pane View
    let body = row![sidebar, content_pane]
        .spacing(1)
        .width(Length::Fill)
        .height(Length::Fill);

    column![top_bar, body]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub fn render_topic_content<'a, Message: 'a>(
    topic: HelpTopic,
    lang: UiLanguage,
) -> Element<'a, Message> {
    match topic {
        HelpTopic::ArchScanLoop => render_arch_scan_loop(lang),
        HelpTopic::ArchProjectFiles => render_arch_project_files(lang),
        HelpTopic::ArchInterLang => render_arch_inter_lang(lang),
        HelpTopic::LdContacts => render_ld_contacts(lang),
        HelpTopic::LdCoils => render_ld_coils(lang),
        HelpTopic::LdBranches => render_ld_branches(lang),
        HelpTopic::LdCompareMath => render_ld_compare_math(lang),
        HelpTopic::LdInlineFb => render_ld_inline_fb(lang),
        HelpTopic::SfcSteps => render_sfc_steps(lang),
        HelpTopic::SfcActions => render_sfc_actions(lang),
        HelpTopic::SfcDivergenceConvergence => render_sfc_div_conv(lang),
        HelpTopic::SfcIntrospection => render_sfc_introspection(lang),
        HelpTopic::StSyntax => render_st_syntax(lang),
        HelpTopic::StControlFlow => render_st_control_flow(lang),
        HelpTopic::StMathFunctions => render_st_math(lang),
        HelpTopic::FbdStandardBlocks => render_fbd_standard_blocks(lang),
        HelpTopic::FbdPidController => render_fbd_pid(lang),
        HelpTopic::FbdLogicMath => render_fbd_logic_math(lang),
        HelpTopic::FbdCustomComposite => render_fbd_custom_composite(lang),
        HelpTopic::MemDirectAddressing => render_mem_direct(lang),
        HelpTopic::MemTagTable => render_mem_tags(lang),
        HelpTopic::SimExecutionAndForces => render_sim_exec_forces(lang),
        HelpTopic::SimAlarmsAndRetain => render_sim_alarms_retain(lang),
        HelpTopic::SimTrendPlotter => render_sim_trend_plotter(lang),
        HelpTopic::FieldbusTcpIot => render_fieldbus_tcp_iot(lang),
        HelpTopic::FieldbusModbusRtu => render_fieldbus_modbus_rtu(lang),
        HelpTopic::HardwareRpiGpio => render_hardware_rpi(lang),
        HelpTopic::DigitalTwinSimulation => render_digital_twin(lang),
        HelpTopic::ToolsPlcOpenXml => render_tools_plcopen_xml(lang),
        HelpTopic::CanvasShortcuts => render_canvas_shortcuts(lang),
    }
}

// ---------------------------------------------------------------------------
// UI Helper Components
// ---------------------------------------------------------------------------

fn section_title<'a, Message: 'a>(text_str: &str) -> Element<'a, Message> {
    container(
        text(text_str.to_string())
            .size(15)
            .color(StudioTheme::TEXT_PRIMARY),
    )
    .padding([8, 0])
    .into()
}

fn paragraph<'a, Message: 'a>(text_str: &str) -> Element<'a, Message> {
    text(text_str.to_string())
        .size(13)
        .color(StudioTheme::TEXT_SECONDARY)
        .into()
}

fn callout_box<'a, Message: 'a>(
    title: &str,
    content: &str,
    accent: Color,
) -> Element<'a, Message> {
    container(
        column![
            text(title.to_string()).size(13).color(accent),
            text(content.to_string()).size(12).color(StudioTheme::TEXT_SECONDARY),
        ]
        .spacing(4),
    )
    .padding(10)
    .style(move |theme: &Theme| container::Style {
        background: Some(Background::Color(StudioTheme::bg_surface(theme))),
        border: Border {
            color: accent,
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    })
    .into()
}

fn symbol_card<'a, Message: 'a>(
    glyph: &str,
    name: &str,
    kind: &str,
    semantics: &str,
    example: &str,
) -> Element<'a, Message> {
    let glyph_badge = container(
        text(glyph.to_string())
            .size(15)
            .color(StudioTheme::ACCENT_CYAN),
    )
    .padding([5, 10])
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(StudioTheme::bg_base(theme))),
        border: Border {
            color: StudioTheme::border_card(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    });

    let header_row = row![
        glyph_badge,
        column![
            text(name.to_string()).size(13).color(StudioTheme::TEXT_PRIMARY),
            text(kind.to_string()).size(11).color(StudioTheme::TEXT_MUTED),
        ]
        .spacing(2),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let content_col = column![
        header_row,
        text(semantics.to_string()).size(12).color(StudioTheme::TEXT_SECONDARY),
        container(text(example.to_string()).size(11).color(StudioTheme::ACCENT_EMERALD))
            .padding([4, 8])
            .style(|theme: &Theme| container::Style {
                background: Some(Background::Color(StudioTheme::bg_base(theme))),
                border: Border {
                    color: StudioTheme::border_hairline(theme),
                    width: 1.0,
                    radius: Radius::from(4.0),
                },
                ..Default::default()
            }),
    ]
    .spacing(8);

    container(content_col)
        .padding(12)
        .style(StudioTheme::card_style)
        .into()
}

fn code_snippet<'a, Message: 'a>(code: &str) -> Element<'a, Message> {
    container(
        text(code.to_string())
            .size(12)
            .color(StudioTheme::TEXT_PRIMARY),
    )
    .padding(10)
    .style(|theme: &Theme| container::Style {
        background: Some(Background::Color(StudioTheme::bg_base(theme))),
        border: Border {
            color: StudioTheme::border_hairline(theme),
            width: 1.0,
            radius: Radius::from(6.0),
        },
        ..Default::default()
    })
    .into()
}

fn table_card<'a, Message: 'a>(headers: &[&str], rows: &[&[&str]]) -> Element<'a, Message> {
    let header_cells = headers.iter().map(|&h| {
        container(text(h.to_string()).size(12).color(StudioTheme::ACCENT_CYAN))
            .padding([6, 8])
            .width(Length::Fill)
            .into()
    });
    let header_row = row(header_cells.collect::<Vec<_>>()).spacing(4);

    let mut body_rows: Vec<Element<'a, Message>> = Vec::new();
    body_rows.push(header_row.into());

    for row_data in rows {
        let cells = row_data.iter().map(|&cell| {
            container(text(cell.to_string()).size(11).color(StudioTheme::TEXT_SECONDARY))
                .padding([4, 8])
                .width(Length::Fill)
                .into()
        });
        let r = row(cells.collect::<Vec<_>>())
            .spacing(4)
            .padding([2, 0]);
        body_rows.push(r.into());
    }

    container(column(body_rows).spacing(4))
        .padding(10)
        .style(StudioTheme::card_style)
        .into()
}

// ---------------------------------------------------------------------------
// 1. Architecture
// ---------------------------------------------------------------------------

fn render_arch_scan_loop<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Deterministic PLC Scan Cycle & Virtual Clock", "Cycle de Scan Déterministe & Horloge Virtuelle"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC enforces a strictly deterministic, 4-phase execution model conforming to industrial standard IEC 61131-3. Each scan cycle executes atomically without dynamic heap allocations on the hot path.",
            "FerritePLC applique un modèle d'exécution strictement déterministe en 4 phases conforme à la norme industrielle CEI 61131-3. Chaque cycle s'exécute de manière atomique sans allocation dynamique sur le chemin critique."
        )),
        section_title(lang.tr("The Four Atomic Scan Phases", "Les Quatre Phases Atomiques du Cycle")),
        table_card(
            &[lang.tr("Phase", "Phase"), lang.tr("Name", "Nom"), lang.tr("Operation Description", "Description de l'Opération")],
            &[
                &["1", lang.tr("Input Scan", "Lecture Entrées"), lang.tr("Latches physical hardware (GPIO) or simulated stimulus into the %I input memory image.", "Verrouille les entrées physiques (GPIO) ou simulées dans l'image mémoire %I.")],
                &["2", lang.tr("Logic Execution", "Exécution Logique"), lang.tr("Executes scheduled POUs (LD rungs, SFC step evolutions, ST scripts, FBD networks) in priority order.", "Exécute les POUs programmées (rungs LD, évolutions SFC, scripts ST, réseaux FBD) par priorité.")],
                &["3", lang.tr("Output Scan", "Écriture Sorties"), lang.tr("Transfers the %Q output image to physical actuator pins, Modbus registers, and Digital Twin actuators.", "Transfère l'image de sortie %Q vers les actionneurs physiques, registres Modbus et jumeau numérique.")],
                &["4", lang.tr("Diagnostics & Clock", "Diagnostics & Horloge"), lang.tr("Records cycle elapsed duration (µs), checks watchdog timeout limits, and steps virtual time.", "Enregistre la durée du cycle (µs), surveille le chien de garde et incrémente l'horloge virtuelle.")],
            ]
        ),
        code_snippet(
            "+-------------+      +----------------+      +--------------+      +-------------------+\n\
             | 1. Input    | ---> | 2. Logic Exec  | ---> | 3. Output    | ---> | 4. Watchdog &     |\n\
             |    Scan %I  |      |    (LD/SFC/ST) |      |    Scan %Q   |      |    Diagnostics    |\n\
             +-------------+      +----------------+      +--------------+      +-------------------+\n\
                    ^                                                                     |\n\
                    +------------------------ Next Cycle Tick ----------------------------+"
        ),
        callout_box(
            lang.tr("Deterministic Time Safety", "Garantie de Temps Déterministe"),
            lang.tr(
                "The virtual clock ensures that timer blocks (TON, TOF, TP) and SFC step timers (X<i>.t) advance precisely by the configured scan interval (default 10 ms), enabling 100% reproducible headless tests and simulation recordings regardless of host CPU scheduling.",
                "L'horloge virtuelle garantit que les blocs temporisateurs (TON, TOF, TP) et durées d'étapes (X<i>.t) avancent exactement selon l'intervalle configuré (10 ms par défaut), assurant des tests sans échec et des simulations parfaitement reproductibles."
            ),
            StudioTheme::ACCENT_CYAN,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_arch_project_files<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Project Structure & Manifest (.fplc)", "Structure du Projet & Manifeste (.fplc)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC projects are stored as clean, human-readable, Git-friendly directories managed by a central TOML manifest (FerriteProject.fplc).",
            "Les projets FerritePLC sont stockés sous forme de répertoires lisibles et versionnables avec Git, pilotés par un fichier manifeste central TOML (FerriteProject.fplc)."
        )),
        section_title(lang.tr("Project File Layout", "Arborescence du Projet")),
        code_snippet(
            "my_automation_plant/\n\
             ├── FerriteProject.fplc        # Main project manifest (scan rate, task priority, metadata)\n\
             ├── symbols/\n\
             │   ├── global_tags.toml       # Global symbols (%I/%Q/%M mappings, types, initial values)\n\
             │   └── fb_instances.toml      # Instantiated Function Blocks (TON, CTU, PID, etc.)\n\
             ├── pous/\n\
             │   ├── main_safety.ld.json    # Ladder Diagram POU\n\
             │   ├── batch_seq.sfc.json     # Grafcet / SFC sequence POU\n\
             │   ├── math_pid.st            # Plain text Structured Text POU\n\
             │   └── pump_ctrl.fbd.json     # Function Block Diagram network POU\n\
             └── sim/\n\
                 └── scenarios.toml         # Headless stimulus scenarios and automated tests"
        ),
        callout_box(
            lang.tr("Atomic File Operations", "Opérations Fichiers Atomiques"),
            lang.tr(
                "When saving, FerritePLC uses a write-to-temp-then-rename strategy to prevent partial writes or corrupted files if the process is terminated mid-save.",
                "Lors de la sauvegarde, FerritePLC utilise une écriture temporaire suivie d'un renommage atomique afin d'éviter toute corruption en cas d'interruption."
            ),
            StudioTheme::ACCENT_AMBER,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_arch_inter_lang<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Inter-Language Communication & Process Image", "Communication Inter-Langages & Image Processus"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC enforces a unified, cross-language execution model allowing all four IEC 61131-3 languages (LD, SFC, ST, FBD) to interact seamlessly through a Shared Process Image (MemoryMap) and Unified Tag Table.",
            "FerritePLC applique un modèle d'exécution unifié et inter-langages permettant aux 4 langages CEI 61131-3 (LD, SFC, ST, FBD) d'interagir harmonieusement via une Image Mémoire Processus Partagée et une Table des Variables Unifiée."
        )),
        section_title(lang.tr("The Shared Memory Model", "Le Modèle Mémoire Partagé")),
        table_card(
            &[lang.tr("Memory Segment", "Segment Mémoire"), lang.tr("Prefix", "Préfixe"), lang.tr("Inter-Language Role", "Rôle Inter-Langages")],
            &[
                &[lang.tr("Discrete & Analog Inputs", "Entrées Tores & Analogiques"), "%I", lang.tr("Latched at scan cycle start; readable by LD contacts, SFC receptivities, ST scripts, and FBD pins.", "Verrouillées en début de cycle ; lisibles par contacts LD, réceptivités SFC, scripts ST et broches FBD.")],
                &[lang.tr("Actuator Outputs", "Sorties Actionneurs"), "%Q", lang.tr("Written by LD coils, ST assignments, SFC actions, or FBD blocks; committed at cycle end.", "Écrites par bobines LD, affectations ST, actions SFC ou blocs FBD ; émises en fin de cycle.")],
                &[lang.tr("Internal Markers & Flags", "Mémentos & Drapeaux Internes"), "%M", lang.tr("Shared read/write scratchpad memory accessible across all POUs in the scan loop.", "Zone mémoire de travail partagée en lecture/écriture accessible à toutes les POUs.")],
                &[lang.tr("SFC Introspection Symbols", "Symboles d'Introspection SFC"), "X<i>.state / X<i>.t", lang.tr("Automatically published step activity and elapsed timers for cross-POU coordination.", "Activité d'étape et chronomètres automatiquement publiés pour coordonner les POUs.")],
            ]
        ),
        section_title(lang.tr("Cross-Language Execution Examples", "Exemples de Liaisons Inter-Langages")),
        symbol_card(
            "LD -> SFC",
            lang.tr("Ladder Permissive Interlock -> Grafcet Transition", "Interverrouillage Ladder -> Transition Grafcet"),
            lang.tr("Safety Interlock Coordination", "Coordination de Sécurité"),
            lang.tr(
                "A Ladder rung checks safety light curtains and emergency stop buttons, energizing a shared boolean 'Safety_Permissive_OK' coil (%M). An SFC transition receptivity binds directly to 'Safety_Permissive_OK AND Sensor_Ready'.",
                "Un rung Ladder vérifie les barrières immatérielles et arrêts d'urgence, activant une bobine 'Safety_Permissive_OK' (%M). Une réceptivité Grafcet se lie directement à 'Safety_Permissive_OK AND Sensor_Ready'."
            ),
            "LD: --[ Estop_OK ]--[ Door_Closed ]--( Safety_Permissive_OK )--  ===>  SFC Transition: [ Safety_Permissive_OK ]"
        ),
        symbol_card(
            "SFC -> ST / FBD",
            lang.tr("Active Step Action -> ST Script or FBD Block", "Action d'Étape -> Script ST ou Bloc FBD"),
            lang.tr("Sequence-Triggered Computation", "Calcul Déclenché par la Séquence"),
            lang.tr(
                "When Step 5 ('Heat_Tank') becomes active, an 'N' action triggers an ST temperature control routine or activates the 'IN' pin of an FBD timer block.",
                "Lorsque l'Étape 5 ('Heat_Tank') s'active, une action 'N' déclenche un script ST de régulation ou active l'entrée 'IN' d'un temporisateur FBD."
            ),
            "Step 5 [N: Target_Temp := 85.0; Heater_On := TRUE;]"
        ),
        symbol_card(
            "FBD -> LD",
            lang.tr("FBD Calculation -> Ladder Permissive Contact", "Calcul FBD -> Contact Permissif Ladder"),
            lang.tr("Closed-Loop Feedback into Interlocks", "Retour de Boucle dans les Interverrouillages"),
            lang.tr(
                "An FBD PID block computes an error status or threshold alarm 'High_Pressure_Trip'. A Ladder rung uses an NC contact --[/ High_Pressure_Trip ]-- to drop the pump contactor instantly.",
                "Un bloc PID FBD calcule un seuil d'alarme 'High_Pressure_Trip'. Un rung Ladder utilise un contact NF --[/ High_Pressure_Trip ]-- pour couper immédiatement la pompe."
            ),
            "FBD: High_Pressure_Trip (BOOL)  ===>  LD: --[/ High_Pressure_Trip ]--( Pump_Contactor )--"
        ),
        callout_box(
            lang.tr("ExecutablePou Contract", "Contrat d'Exécution ExecutablePou"),
            lang.tr(
                "Every compiled POU—regardless of source language (.ld, .sfc, .st, .fbd)—implements the Rust `ExecutablePou` trait and evaluates within the atomic `ScanContext`.",
                "Chaque POU compilée—quel que soit son langage (.ld, .sfc, .st, .fbd)—implémente le trait Rust `ExecutablePou` et s'évalue dans le `ScanContext` atomique."
            ),
            StudioTheme::ACCENT_CYAN,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 2. Ladder Diagram
// ---------------------------------------------------------------------------

fn render_ld_contacts<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Ladder Diagram (LD) Contacts Reference", "Référence des Contacts Ladder (LD)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Contacts test the boolean state of direct addresses (%I, %Q, %M) or named tags. When their condition is met, power flow passes from left to right along the rung.",
            "Les contacts testent l'état booléen d'adresses directes (%I, %Q, %M) ou de variables nommées. Lorsque la condition est vérifiée, le flux d'énergie traverse le contact de gauche à droite."
        )),
        section_title(lang.tr("Contact Types & Semantics", "Types de Contacts & Sémantique")),
        symbol_card(
            "--[ ]--",
            lang.tr("Normally Open (NO) Contact", "Contact Normalement Ouvert (NO)"),
            lang.tr("Examine If Closed (XIC)", "Vérifier Si Fermé (XIC)"),
            lang.tr("Passes power flow when the associated tag or bit evaluates to TRUE (1). Blocked when FALSE (0).", "Laisse passer le flux lorsque la variable ou le bit vaut VRAI (1). Bloque lorsque FAUX (0)."),
            "Start_Button --[ ]--  (Conducts when button is pressed)"
        ),
        symbol_card(
            "--[/]--",
            lang.tr("Normally Closed (NC) Contact", "Contact Normalement Fermé (NC)"),
            lang.tr("Examine If Open (XIO)", "Vérifier Si Ouvert (XIO)"),
            lang.tr("Passes power flow when the associated tag or bit evaluates to FALSE (0). Blocked when TRUE (1).", "Laisse passer le flux lorsque la variable ou le bit vaut FAUX (0). Bloque lorsque VRAI (1)."),
            "E_Stop_Pushed --[/]--  (Conducts while emergency stop is NOT pressed)"
        ),
        symbol_card(
            "--[P]--",
            lang.tr("Rising Edge Pulse Contact", "Contact Front Montant (Impulsion P)"),
            lang.tr("Positive Transition Detector", "Détecteur de Transition Positive"),
            lang.tr("Passes power flow for exactly ONE scan cycle when the tag transitions from FALSE to TRUE (0 -> 1).", "Laisse passer le flux pendant exactement UN cycle de scan lors de la transition 0 -> 1."),
            "Cycle_Trigger --[P]--  (Generates a 1-cycle pulse on button press)"
        ),
        symbol_card(
            "--[N]--",
            lang.tr("Falling Edge Pulse Contact", "Contact Front Descendant (Impulsion N)"),
            lang.tr("Negative Transition Detector", "Détecteur de Transition Négative"),
            lang.tr("Passes power flow for exactly ONE scan cycle when the tag transitions from TRUE to FALSE (1 -> 0).", "Laisse passer le flux pendant exactement UN cycle de scan lors de la transition 1 -> 0."),
            "Sensor_Release --[N]--  (Generates a pulse when part clears the sensor)"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_ld_coils<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Ladder Diagram (LD) Coils Reference", "Référence des Bobines Ladder (LD)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Coils represent outputs, internal flags, or command actuators. They are located at the right end of a rung and update their target variable based on the incoming power flow.",
            "Les bobines représentent des sorties, des drapeaux internes ou des actionneurs. Situées à l'extrémité droite d'un rung, elles mettent à jour leur variable selon le flux d'énergie entrant."
        )),
        section_title(lang.tr("Coil Types & Action Semantics", "Types de Bobines & Sémantique")),
        symbol_card(
            "--( )--",
            lang.tr("Normal Output Coil", "Bobine de Sortie Normale"),
            lang.tr("Continuous Assign", "Affectation Continue"),
            lang.tr("Energizes (sets TRUE) when power flow reaches the coil; de-energizes (sets FALSE) when power flow is lost.", "S'active (VRAI) tant que le flux d'énergie l'atteint ; retombe à FAUX dès que le flux est interrompu."),
            "Run_Permissive ---> --( Motor_Contactor )--"
        ),
        symbol_card(
            "--[S]--",
            lang.tr("Set (Latch) Coil", "Bobine d'Enclenchement (Set / Latch)"),
            lang.tr("Retentive Latch", "Verrouillage Rémanent"),
            lang.tr("Latches the target variable to TRUE when power flow arrives. The variable remains TRUE indefinitely even after power flow drops.", "Force la variable à VRAI dès que le flux arrive. La variable reste à VRAI indéfiniment même après coupure du flux."),
            "Start_Signal ---> --[S] Run_Mode --"
        ),
        symbol_card(
            "--[R]--",
            lang.tr("Reset (Unlatch) Coil", "Bobine de Déclenchement (Reset / Unlatch)"),
            lang.tr("Retentive Unlatch", "Déverrouillage Rémanent"),
            lang.tr("Forces the target variable to FALSE when power flow arrives. Clears a previously latched Set coil.", "Force la variable à FAUX dès que le flux arrive. Annule l'effet d'une bobine Set préalablement verrouillée."),
            "Stop_Signal ---> --[R] Run_Mode --"
        ),
        symbol_card(
            "--(/)--",
            lang.tr("Inverted Output Coil", "Bobine Inversée"),
            lang.tr("Inverted Assign", "Affectation Inversée"),
            lang.tr("Writes the logical negation of incoming power flow: sets TRUE when no power flow reaches it, FALSE when power flow arrives.", "Écrit l'inverse logique du flux entrant : vaut VRAI quand le flux est absent, et FAUX dès qu'il arrive."),
            "Fault_Detected ---> --(/) Ready_Lamp --"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_ld_branches<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Rung Branches & Power Flow", "Branches de Rung & Flux d'Énergie"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Ladder rungs can branch into parallel paths to evaluate boolean OR logic, seal-in circuits, or drive multiple coils simultaneously.",
            "Les rungs Ladder peuvent se ramifier en branches parallèles pour exprimer des fonctions OU logiques, des circuits d'auto-maintien ou commander plusieurs bobines en parallèle."
        )),
        section_title(lang.tr("Series vs Parallel Power Flow", "Flux d'Énergie Série vs Parallèle")),
        code_snippet(
            "+-- Left Power Rail                                      Right Rail --+\n\
             |                                                                     |\n\
             |   +--[ Start_PB ]--+---[ Safety_OK ]---------------+---( Motor )----+  (OR Branch / Seal-In)\n\
             |   |                |                               |\n\
             |   +--[ Motor_Aux ]-+                               +---( Lamp_Run )--+  (Parallel Coil)\n\
             |                                                                     |\n\
             +---------------------------------------------------------------------+"
        ),
        callout_box(
            lang.tr("Branch Dissolution", "Dissolution Automatique de Branche"),
            lang.tr(
                "When all elements in a parallel branch are deleted, FerritePLC automatically cleans up the empty path and re-merges the rung cleanly without leaving orphaned wires or visual artifacts.",
                "Lorsque tous les éléments d'une branche parallèle sont supprimés, FerritePLC nettoie automatiquement le chemin vide et referme le rung proprement."
            ),
            StudioTheme::ACCENT_INDIGO,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_ld_compare_math<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Comparison & Math/Move Rung Blocks", "Blocs de Comparaison & Calcul Math/Move sur Rung"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC rungs support inline numerical comparison and arithmetic execution. These blocks sit inline within the rung and condition or execute logic during power flow.",
            "Les rungs FerritePLC supportent les blocs de comparaison numérique et d'arithmétique en ligne. Ces blocs s'insèrent dans le flux d'énergie pour tester ou transformer des valeurs."
        )),
        section_title(lang.tr("Comparison Blocks (LdCompareOp)", "Blocs de Comparaison (LdCompareOp)")),
        paragraph(lang.tr(
            "A comparison block acts like a conditional contact: it passes power flow only when the relational condition between Left and Right operands evaluates to TRUE.",
            "Un bloc de comparaison agit comme un contact conditionnel : il transmet le flux d'énergie uniquement lorsque la relation entre l'opérande gauche et droit est VRAIE."
        )),
        table_card(
            &[lang.tr("Operator", "Opérateur"), lang.tr("Symbol", "Symbole"), lang.tr("Condition For Power Flow", "Condition de Continuité")],
            &[
                &["EQ", "==", lang.tr("Passes when Left is equal to Right", "Passe lorsque Gauche est égal à Droite")],
                &["NE", "<>", lang.tr("Passes when Left is not equal to Right", "Passe lorsque Gauche est différent de Droite")],
                &["GT", ">", lang.tr("Passes when Left is strictly greater than Right", "Passe lorsque Gauche est strictement supérieur à Droite")],
                &["GE", ">=", lang.tr("Passes when Left is greater than or equal to Right", "Passe lorsque Gauche est supérieur ou égal à Droite")],
                &["LT", "<", lang.tr("Passes when Left is strictly less than Right", "Passe lorsque Gauche est strictement inférieur à Droite")],
                &["LE", "<=", lang.tr("Passes when Left is less than or equal to Right", "Passe lorsque Gauche est inférieur ou égal à Droite")],
            ]
        ),
        code_snippet(
            "+-- Left Rail                                                      Right Rail --+\n\
             |                                                                                |\n\
             |   +--[ Start_PB ]--+---[ CMP: Tank_Level > 75.0 ]---------------+---( Fill_Stop )--+\n\
             |                                                                                |\n\
             +--------------------------------------------------------------------------------+"
        ),
        section_title(lang.tr("Math & Move Operations (LdMathOp)", "Opérations Arithmétiques & Déplacement (LdMathOp)")),
        paragraph(lang.tr(
            "Math and Move blocks execute numerical calculations when power flow reaches them, storing the result in a destination variable and propagating power to downstream elements.",
            "Les blocs Math et Move effectuent des calculs dès qu'ils sont alimentés, écrivent le résultat dans la variable de destination et propagent l'énergie vers l'aval."
        )),
        symbol_card(
            "[MOVE]",
            lang.tr("Data Move / Transfer", "Transfert de Données (MOVE)"),
            "In1 -> Dest",
            lang.tr("Transfers literal value or variable In1 directly into variable Dest when energized.", "Transfère la valeur ou variable In1 directement dans Dest lors de l'activation."),
            "MOVE: Preset_Value (100) -> Target_Speed"
        ),
        symbol_card(
            "[ADD / SUB]",
            lang.tr("Addition & Subtraction", "Addition & Soustraction"),
            "In1 (+ / -) In2 -> Dest",
            lang.tr("Computes the sum or difference of In1 and In2, storing into Dest.", "Calcule la somme ou la différence entre In1 et In2, et écrit le résultat dans Dest."),
            "ADD: Part_Count + 1 -> Part_Count"
        ),
        symbol_card(
            "[MUL / DIV]",
            lang.tr("Multiplication & Division", "Multiplication & Division"),
            "In1 (* / /) In2 -> Dest",
            lang.tr("Multiplies or divides In1 by In2 (with zero-division protection) into Dest.", "Multiplie ou divise In1 par In2 (avec protection contre la division par zéro) vers Dest."),
            "DIV: Raw_Analog / 10.0 -> Scaled_Pressure"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_ld_inline_fb<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Inline Function Blocks in Ladder Rungs", "Blocs Fonctionnels Intégrés en Rung Ladder"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC allows instantiating IEC 61131-3 Function Blocks (such as TON on-delay timers, CTU counters, or edge triggers) directly within Ladder rungs.",
            "FerritePLC permet d'insérer des blocs fonctionnels CEI 61131-3 (tels que les temporisateurs TON, compteurs CTU ou déclencheurs de front) directement au cœur des rungs Ladder."
        )),
        section_title(lang.tr("Inline Block Architecture (EN / ENO)", "Architecture des Blocs Intégrés (EN / ENO)")),
        code_snippet(
            "+-- Left Rail                                                              Right Rail --+\n\
             |                                                                                         |\n\
             |   +--[ Sensor_Trip ]---+---------[ TON: Timer_1 ]---------+------------( Valve_Open )---+ \n\
             |                        |  EN                    ENO  |    |                             |\n\
             |                        |  IN: Sensor_Trip      Q: --+----+                             |\n\
             |                        |  PT: T#3s            ET: Elapsed                               |\n\
             |                        +-----------------------------+                                  |\n\
             +-----------------------------------------------------------------------------------------+"
        ),
        table_card(
            &[lang.tr("Pin / Concept", "Broche / Concept"), lang.tr("Direction", "Direction"), lang.tr("Role in Ladder Rung", "Rôle dans le Rung")],
            &[
                &["EN (Enable In)", lang.tr("Power Input", "Entrée Énergie"), lang.tr("Controls execution of the block instance. Receives power from upstream contacts.", "Pilote l'exécution du bloc. Reçoit l'énergie des contacts amont.")],
                &["ENO (Enable Out)", lang.tr("Power Output", "Sortie Énergie"), lang.tr("Passes power flow downstream when EN is active and block executed without fault.", "Transmet l'énergie vers l'aval quand EN est actif et que le bloc a fonctionné sans erreur.")],
                &["Inputs Mapping", lang.tr("Parameters", "Paramètres"), lang.tr("Explicitly binds tags or literals to block inputs (e.g., PT := T#5s, PV := 10).", "Lie explicitement des variables ou littéraux aux entrées (ex. PT := T#5s, PV := 10).")],
                &["Outputs Mapping", lang.tr("Feedback", "Retours"), lang.tr("Binds block outputs to process image tags or Ladder coils (e.g., Q, ET, CV).", "Lie les sorties du bloc aux variables de l'image processus (ex. Q, ET, CV).")],
            ]
        ),
        callout_box(
            lang.tr("Shared Function Block Instances", "Instances de Blocs Fonctionnels Partagées"),
            lang.tr(
                "Inline function block instances are registered in `fb_instances.toml`. Their internal states (such as elapsed time `Timer_1.ET` or counter value `Counter_1.CV`) are globally accessible to ST scripts, SFC transitions, and FBD networks.",
                "Les instances de blocs fonctionnels intégrés sont déclarées dans `fb_instances.toml`. Leurs états internes (comme le temps écoulé `Timer_1.ET` ou la valeur `Counter_1.CV`) sont accessibles depuis ST, SFC et FBD."
            ),
            StudioTheme::ACCENT_EMERALD,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 3. Grafcet / SFC
// ---------------------------------------------------------------------------

fn render_sfc_steps<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Grafcet / SFC Steps Reference", "Référence des Étapes Grafcet / SFC"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Sequential Function Chart (SFC / IEC 60848 Grafcet) models sequential control logic as states (Steps), directed transitions, and associated actions.",
            "Le Grafcet (CEI 60848 / SFC CEI 61131-3) modélise la logique séquentielle sous forme d'états (Étapes), de transitions orientées et d'actions associées."
        )),
        section_title(lang.tr("Step Classifications", "Classification des Étapes")),
        symbol_card(
            "[[ 0 ]]",
            lang.tr("Initial Step", "Étape Initiale"),
            lang.tr("Double Frame / Active at Startup", "Double Encadrement / Active au Démarrage"),
            lang.tr("Automatically active with 1 token when the PLC boots or resets. Every valid SFC chart must contain at least one initial step.", "Automatiquement active avec 1 jeton dès la mise sous tension de l'automate. Tout grafcet valide doit comporter au moins une étape initiale."),
            "Step 0: Initial safe wait state"
        ),
        symbol_card(
            "[ 1 ]",
            lang.tr("Normal Step", "Étape Normale"),
            lang.tr("Single Frame / Sequential State", "Simple Encadrement / État Séquentiel"),
            lang.tr("Activated when an immediate upstream transition fires. Deactivated when its downstream transition fires.", "Activée lorsque la transition amont franchissable est validée. Désactivée dès le franchissement de la transition aval."),
            "Step 1: Fill Tank, Step 2: Heat Fluid"
        ),
        symbol_card(
            "[[M1]]",
            lang.tr("Macro Step", "Étape Macro"),
            lang.tr("Encapsulated Sub-Grafcet", "Sous-Grafcet Encapsulé"),
            lang.tr("Represents a complete sub-sequence with its own dedicated expansion chart, entry step (SM_IN) and exit step (SM_OUT).", "Représente une sous-séquence complète dotée de sa propre page d'extension, avec étape d'entrée (SM_IN) et de sortie (SM_OUT)."),
            "Macro Step M1: CIP Clean-in-Place cycle"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_sfc_actions<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("SFC Action Qualifiers (N, S, R, L, D, P)", "Qualificateurs d'Action SFC (N, S, R, L, D, P)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Actions define what happens while a step is active. The action qualifier controls the execution timing, storage, and pulse characteristics.",
            "Les actions définissent les opérations effectuées pendant qu'une étape est active. Le qualificateur détermine le mode temporel, mémorisé ou impulsionnel."
        )),
        section_title(lang.tr("IEC 61131-3 Action Qualifiers", "Qualificateurs d'Action CEI 61131-3")),
        table_card(
            &[lang.tr("Qualifier", "Qualif."), lang.tr("Name", "Nom"), lang.tr("Behavior & Lifecycle", "Comportement & Cycle de Vie")],
            &[
                &["N", lang.tr("Non-Stored", "Non-Mémorisé"), lang.tr("Active continuously while the step is active. Immediately deactivates when step clears.", "Actif en continu tant que l'étape est active. Retombe à zéro dès la désactivation de l'étape.")],
                &["S", lang.tr("Set / Stored", "Mémorisé (Set)"), lang.tr("Sets target variable to TRUE on step activation. Remains TRUE even after step clears until an R action resets it.", "Force la variable à VRAI dès l'activation. Reste à VRAI après désactivation jusqu'à un Reset explicite.")],
                &["R", lang.tr("Reset", "Remise à Zéro"), lang.tr("Forces target variable to FALSE upon step activation, canceling any prior S qualifier.", "Force la variable à FAUX dès l'activation, annulant l'effet d'un qualificateur S antérieur.")],
                &["L", lang.tr("Time Limited", "Limité dans le Temps"), lang.tr("Active while step is active, but shuts off after specified duration (e.g. L T#5s) even if step remains active.", "Actif avec l'étape, mais s'interrompt après la durée spécifiée (ex. L T#5s) même si l'étape reste active.")],
                &["D", lang.tr("Time Delayed", "Retardé"), lang.tr("Waits for duration (e.g. D T#3s) before turning ON. Shuts off immediately when step clears.", "Attend la durée spécifiée avant de passer à VRAI. Retombe à zéro dès que l'étape se désactive.")],
                &["P", lang.tr("Pulse", "Impulsion"), lang.tr("Fires for exactly ONE scan cycle upon step rising edge activation.", "S'exécute pendant exactement UN cycle de scan lors de l'activation montante de l'étape.")],
            ]
        ),
        callout_box(
            lang.tr("Action Targets", "Cibles d'Action"),
            lang.tr(
                "An action can either assign a boolean tag directly (e.g. 'Valve_1') or execute inline Structured Text statements (e.g. 'Speed := Speed + 10;').",
                "Une action peut affecter directement un tag booléen (ex. 'Valve_1') ou exécuter des instructions Structured Text en ligne (ex. 'Speed := Speed + 10;')."
            ),
            StudioTheme::ACCENT_EMERALD,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_sfc_div_conv<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Divergences, Convergences & IEC 60848 Rules", "Divergences, Convergences & Règles CEI 60848"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "SFC topologies support parallel branches (simultaneous sequences) and alternative branches (sequence selection).",
            "Les topologies SFC supportent des branches parallèles (séquences simultanées) et des aiguillages alternatifs (sélection de séquence)."
        )),
        section_title(lang.tr("Parallel vs Alternative Structures", "Structures Parallèles vs Alternatives")),
        symbol_card(
            "===",
            lang.tr("AND Divergence / Convergence (Double Bar)", "Divergence / Convergence ET (Double Barre)"),
            lang.tr("Simultaneous Sequences", "Séquences Simultanées"),
            lang.tr(
                "Divergence: 1 transition activates multiple parallel branches simultaneously.\n\
                 Convergence: Synchronizes parallel branches; fires only when ALL incoming steps are active AND receptivity is TRUE.",
                "Divergence : 1 transition active simultanément plusieurs branches parallèles.\n\
                 Convergence : Synchronise les branches ; ne franchit que si TOUTES les étapes amont sont actives ET la réceptivité est VRAIE."
            ),
            "Step 1 ---> [ t1 ] ===> Step 2 || Step 3"
        ),
        symbol_card(
            "---",
            lang.tr("OR Divergence / Convergence (Single Bar)", "Divergence / Convergence OU (Simple Barre)"),
            lang.tr("Sequence Selection / Alternative Paths", "Sélection de Séquence / Chemins Alternatifs"),
            lang.tr(
                "Divergence: 1 step leads to multiple competing transitions (priority evaluated from left to right).\n\
                 Convergence: Merges distinct alternative paths back into a single common downstream step.",
                "Divergence : 1 étape mène à plusieurs transitions concurrentes (évaluées de gauche à droite).\n\
                 Convergence : Réunit différentes branches alternatives vers une étape aval commune."
            ),
            "Step 10 ---> [ Auto_Mode ] -> Step 11  OR  [ Manual_Mode ] -> Step 12"
        ),
        section_title(lang.tr("The 5 IEC 60848 Evolution Rules", "Les 5 Règles d'Évolution CEI 60848")),
        table_card(
            &[lang.tr("Rule", "Règle"), lang.tr("Principle", "Principe"), lang.tr("Mandatory Behavior", "Comportement Obligatoire")],
            &[
                &["1", lang.tr("Initial State", "Situation Initiale"), lang.tr("Initial steps are activated unconditionally at controller startup.", "Les étapes initiales sont activées inconditionnellement au démarrage de l'automate.")],
                &["2", lang.tr("Clearance Condition", "Franchissement"), lang.tr("A transition is cleared if and only if all upstream steps are active AND its receptivity is TRUE.", "Une transition est franchie si et seulement si toutes ses étapes amont sont actives ET sa réceptivité est VRAIE.")],
                &["3", lang.tr("Simultaneous Evolution", "Évolution Simultanée"), lang.tr("Firing a transition simultaneously deactivates all upstream steps and activates all downstream steps.", "Le franchissement désactive simultanément toutes les étapes amont et active toutes les étapes aval.")],
                &["4", lang.tr("Simultaneous Transitions", "Transitions Simultanées"), lang.tr("All simultaneously clearable transitions are fired together in the same scan tick.", "Toutes les transitions franchissables simultanément sont franchies ensemble au même cycle.")],
                &["5", lang.tr("Simultaneous Activation & Deactivation", "Activation & Désactivation Simultanée"), lang.tr("If a step is simultaneously activated and deactivated, it remains ACTIVE without interruption.", "Si une étape doit être simultanément activée et désactivée, elle reste ACTIVE sans interruption.")],
            ]
        ),
    ]
    .spacing(12)
    .into()
}

fn render_sfc_introspection<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Grafcet Introspection Symbols (X<i>.state, X<i>.t)", "Symboles d'Introspection Grafcet (X<i>.state, X<i>.t)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Every step in an SFC chart automatically registers introspection symbols in the unified tag table. This enables seamless cross-language communication without manual glue code.",
            "Chaque étape d'un grafcet enregistre automatiquement des symboles d'introspection dans la table des variables unifiée. Cela permet une communication inter-langages fluide sans code passerelle manuel."
        )),
        section_title(lang.tr("Auto-Exposed Step Variables", "Variables d'Étape Auto-Exposées")),
        symbol_card(
            "X<i>.state",
            lang.tr("Step Activity Status (BOOL)", "État d'Activité de l'Étape (BOOL)"),
            lang.tr("Read-Only Boolean Flag", "Indicateur Booléen en Lecture Seule"),
            lang.tr(
                "Evaluates to TRUE while step <i> is active, and FALSE when inactive. Ladder contacts, FBD pins, or ST expressions can read this directly.",
                "Vaut VRAI tant que l'étape <i> est active, et FAUX sinon. Utilisable directement dans un contact Ladder, une broche FBD ou une condition ST."
            ),
            "Ladder Contact: --[ X2.state ]--  (Closes when Step 2 is active)"
        ),
        symbol_card(
            "X<i>.t",
            lang.tr("Step Elapsed Active Duration (TIME)", "Temps Écoulé dans l'Étape (TIME)"),
            lang.tr("Step Activity Timer", "Chronomètre d'Activité d'Étape"),
            lang.tr(
                "Accumulates the elapsed time since step <i> was last activated. Automatically resets to T#0s when the step deactivates.",
                "Mesure le temps écoulé depuis la dernière activation de l'étape <i>. Se remet automatiquement à T#0s à la désactivation."
            ),
            "Transition Condition: X3.t >= T#5s AND Sensor_Level_OK"
        ),
        callout_box(
            lang.tr("Deterministic Write Protection", "Protection en Écriture"),
            lang.tr(
                "Step state flags (X<i>.state) are strictly read-only outside the SFC engine. To activate or clear steps externally, use macro commands or SFC control primitives.",
                "Les indicateurs d'étape (X<i>.state) sont strictement protégés en lecture seule hors du moteur SFC afin de préserver l'intégrité déterministe."
            ),
            StudioTheme::ACCENT_ROSE,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 4. Structured Text
// ---------------------------------------------------------------------------

fn render_st_syntax<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Structured Text (ST) Syntax & Operators", "Syntaxe & Opérateurs Structured Text (ST)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Structured Text is an imperative, block-structured high-level programming language defined in IEC 61131-3, ideal for complex mathematics, algorithms, and data manipulation.",
            "Le Texte Structuré est un langage impératif de haut niveau défini dans la norme CEI 61131-3, idéal pour les calculs mathématiques complexes, les algorithmes et la manipulation de données."
        )),
        section_title(lang.tr("Operators & Operator Precedence", "Opérateurs & Précédence")),
        table_card(
            &[lang.tr("Category", "Catégorie"), lang.tr("Operators", "Opérateurs"), lang.tr("Description & Examples", "Description & Exemples")],
            &[
                &[lang.tr("Assignment", "Affectation"), ":=", lang.tr("Assigns expression result to variable: Target := Source + 1;", "Affecte le résultat à la variable : Target := Source + 1;")],
                &[lang.tr("Arithmetic", "Arithmétique"), "+, -, *, /, MOD, **", lang.tr("Addition, subtraction, multiplication, division, modulo, exponentiation.", "Addition, soustraction, multiplication, division, modulo, puissance.")],
                &[lang.tr("Comparison", "Comparaison"), "=, <>, <, <=, >, >=", lang.tr("Equal, Not Equal, Less Than, Less/Equal, Greater Than, Greater/Equal.", "Égal, Différent, Inférieur, Inférieur ou Égal, Supérieur, Supérieur ou Égal.")],
                &[lang.tr("Logical / Bitwise", "Logique / Binaire"), "AND, OR, XOR, NOT", lang.tr("Boolean logic and bitwise mask operations (e.g. Flag AND NOT Mask).", "Logique booléenne et masques binaires (ex. Flag AND NOT Mask).")],
            ]
        ),
        section_title(lang.tr("Literals & Data Formatting", "Littéraux & Formatage")),
        code_snippet(
            "(* Boolean literals *)\n\
             bFlag := TRUE;\n\
             \n\
             (* Integer & Hexadecimal *)\n\
             nCount := 142;\n\
             nMask  := 16#FF00;\n\
             nBin   := 2#1010_1100;\n\
             \n\
             (* Floating Point Real *)\n\
             fPressure := 101.325;\n\
             \n\
             (* IEC Duration Literal *)\n\
             tDelay := T#5s250ms;\n\
             \n\
             (* String Literals *)\n\
             sMessage := 'FerritePLC Ready';"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_st_control_flow<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Control Flow Structures (IF, CASE, FOR, WHILE)", "Structures de Contrôle (IF, CASE, FOR, WHILE)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Structured Text provides full structured control flow statements for branching, selection, and bounded loops.",
            "Le Texte Structuré fournit des structures de contrôle complètes pour les branchements conditionnels, sélections et boucles bornées."
        )),
        section_title("IF ... ELSIF ... ELSE ... END_IF"),
        code_snippet(
            "IF Tank_Level > 95.0 THEN\n\
             \x20   Inlet_Valve := FALSE;\n\
             \x20   High_Alarm := TRUE;\n\
             ELSIF Tank_Level < 20.0 THEN\n\
             \x20   Inlet_Valve := TRUE;\n\
             \x20   Low_Alarm := TRUE;\n\
             ELSE\n\
             \x20   High_Alarm := FALSE;\n\
             \x20   Low_Alarm := FALSE;\n\
             END_IF;"
        ),
        section_title("CASE ... OF ... ELSE ... END_CASE"),
        code_snippet(
            "CASE Operating_Mode OF\n\
             \x20   1: System_State := 'Manual Jog';\n\
             \x20   2: System_State := 'Semi-Automatic';\n\
             \x20   3..5: System_State := 'Fully Automated Batch';\n\
             ELSE\n\
             \x20   System_State := 'Standby / E-Stop';\n\
             END_CASE;"
        ),
        section_title("FOR ... TO ... BY ... DO & WHILE ... DO"),
        code_snippet(
            "(* Deterministic bounded loop *)\n\
             FOR i := 0 TO 9 BY 1 DO\n\
             \x20   Buffer[i] := 0;\n\
             END_FOR;\n\
             \n\
             (* Monitored condition loop *)\n\
             WHILE Sensor_Busy AND Timeout_Counter < 100 DO\n\
             \x20   Timeout_Counter := Timeout_Counter + 1;\n\
             END_WHILE;"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_st_math<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Standard Math Functions (ABS, SQRT, etc.)", "Fonctions Mathématiques Standards (ABS, SQRT, etc.)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC includes a rich library of IEEE 754 floating point mathematical routines conforming to IEC 61131-3 Section 6.5.",
            "FerritePLC inclut une bibliothèque complète de fonctions mathématiques IEEE 754 conformes à la section 6.5 de la norme CEI 61131-3."
        )),
        table_card(
            &[lang.tr("Function", "Fonction"), lang.tr("Signature", "Signature"), lang.tr("Description", "Description")],
            &[
                &["ABS(x)", "REAL -> REAL", lang.tr("Absolute value |x|", "Valeur absolue |x|")],
                &["SQRT(x)", "REAL -> REAL", lang.tr("Square root of non-negative x", "Racine carrée de x non négatif")],
                &["LN(x)", "REAL -> REAL", lang.tr("Natural logarithm (base e)", "Logarithme naturel (base e)")],
                &["LOG(x)", "REAL -> REAL", lang.tr("Common logarithm (base 10)", "Logarithme décimal (base 10)")],
                &["EXP(x)", "REAL -> REAL", lang.tr("Exponential e^x", "Exponentielle e^x")],
                &["SIN(x), COS(x), TAN(x)", "REAL -> REAL", lang.tr("Trigonometric functions (radians)", "Fonctions trigonométriques (en radians)")],
                &["ASIN(x), ACOS(x), ATAN(x)", "REAL -> REAL", lang.tr("Inverse trigonometric functions", "Fonctions trigonométriques inverses")],
                &["MIN(a, b), MAX(a, b)", "REAL, REAL -> REAL", lang.tr("Minimum / Maximum of two values", "Minimum / Maximum de deux valeurs")],
                &["LIMIT(min, in, max)", "REAL, REAL, REAL -> REAL", lang.tr("Clamps in between min and max bounds", "Écrête in entre les bornes min et max")],
            ]
        ),
        code_snippet(
            "(* Scaling analog 4-20mA sensor to 0-100% *)\n\
             Raw_Clamped := LIMIT(4.0, Raw_Input_mA, 20.0);\n\
             Percentage  := ((Raw_Clamped - 4.0) / 16.0) * 100.0;"
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 5. Function Block Diagram
// ---------------------------------------------------------------------------

fn render_fbd_standard_blocks<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Standard Function Blocks (TON, TOF, TP, CTU, etc.)", "Blocs Fonctionnels Standards (TON, TOF, TP, CTU, etc.)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Function Block Diagram (FBD) represents control logic as graphical signal flows connecting input/output pins of reusable function blocks with internal state.",
            "Le Diagramme de Blocs Fonctionnels (FBD) représente la logique sous forme de flux de signaux graphiques reliant des broches de blocs réutilisables dotés d'état interne."
        )),
        section_title(lang.tr("Timer Blocks (IEC 61131-3)", "Blocs Temporisateurs (CEI 61131-3)")),
        symbol_card(
            "TON",
            lang.tr("On-Delay Timer", "Temporisateur au Travail (On-Delay)"),
            "IN: BOOL, PT: TIME -> Q: BOOL, ET: TIME",
            lang.tr("Delays setting output Q to TRUE by preset duration PT while input IN remains TRUE. ET accumulates elapsed time.", "Retarde le passage de la sortie Q à VRAI de la durée PT tant que l'entrée IN reste à VRAI. ET indique le temps écoulé."),
            "Timer1(IN := Start_Sensor, PT := T#5s);"
        ),
        symbol_card(
            "TOF",
            lang.tr("Off-Delay Timer", "Temporisateur au Repos (Off-Delay)"),
            "IN: BOOL, PT: TIME -> Q: BOOL, ET: TIME",
            lang.tr("Keeps output Q TRUE for duration PT after input IN falls to FALSE. Extends actuator running time.", "Maintient la sortie Q à VRAI pendant la durée PT après que l'entrée IN soit retombée à FAUX. Prolonge la marche d'un organe."),
            "Fan_Timer(IN := Run_Cmd, PT := T#10s);"
        ),
        symbol_card(
            "TP",
            lang.tr("Pulse Timer", "Temporisateur à Impulsion Calibrée (TP)"),
            "IN: BOOL, PT: TIME -> Q: BOOL, ET: TIME",
            lang.tr("Generates an output pulse of exact duration PT upon rising edge of IN, regardless of how long IN stays active.", "Génère une impulsion de sortie de durée fixe PT dès le front montant de IN, quelle que soit la durée de maintien de IN."),
            "Lube_Pulse(IN := Cycle_End, PT := T#2s);"
        ),
        section_title(lang.tr("Counters & Edge Triggers", "Compteurs & Détecteurs de Front")),
        symbol_card(
            "CTU / CTD",
            lang.tr("Up / Down Counter", "Compteur / Décompteur (CTU / CTD)"),
            "CU/CD: BOOL, R/LD: BOOL, PV: INT -> Q: BOOL, CV: INT",
            lang.tr("Increments (CTU) or decrements (CTD) current value CV on rising edges. Q turns TRUE when threshold PV is reached.", "Incrémente (CTU) ou décrémente (CTD) la valeur courante CV à chaque front montant. Q passe à VRAI au seuil PV."),
            "Batch_Counter(CU := Part_Sensor, PV := 50);"
        ),
        symbol_card(
            "R_TRIG / F_TRIG",
            lang.tr("Edge Trigger Detectors", "Détecteurs de Front Montant / Descendant"),
            "CLK: BOOL -> Q: BOOL",
            lang.tr("Detects transitions on signal CLK. Generates a 1-cycle boolean pulse on output Q on rising edge (R_TRIG) or falling edge (F_TRIG).", "Détecte les transitions du signal CLK. Émet une impulsion booléenne d'un cycle sur Q lors d'un front montant ou descendant."),
            "Edge(CLK := Button_Pin);"
        ),
        symbol_card(
            "SR / RS",
            lang.tr("Bistable Latches", "Basculeurs Bistables (SR / RS)"),
            "SR: S1, R -> Q1  |  RS: S, R1 -> Q1",
            lang.tr("Bistable memory blocks. SR is Set-dominant (if both S1 and R are TRUE, Q1 = TRUE). RS is Reset-dominant (if both S and R1 are TRUE, Q1 = FALSE).", "Mémoires bistables. SR est à priorité Enclenchement (Set). RS est à priorité Déclenchement (Reset)."),
            "Safety_FlipFlop(S := Set_Cmd, R1 := E_Stop);"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_fbd_pid<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("PID Closed-Loop Controller", "Régulateur Boucle Fermée PID"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC incorporates an industrial-grade Proportional-Integral-Derivative (PID) function block with selectable form, anti-windup clamping, and bumpless manual/auto transfer.",
            "FerritePLC intègre un bloc de régulation Proportionnel-Intégral-Dérivé (PID) de niveau industriel avec choix d'architecture, anti-emballement d'intégrale et transfert sans à-coup manuel/auto."
        )),
        section_title(lang.tr("Block Pinout & Parameters", "Brochage & Paramètres du Bloc")),
        table_card(
            &[lang.tr("Pin", "Broche"), lang.tr("Direction", "Sens"), lang.tr("Type", "Type"), lang.tr("Description", "Description")],
            &[
                &["SP", lang.tr("Input", "Entrée"), "REAL", lang.tr("Setpoint target value", "Consigne souhaitée")],
                &["PV", lang.tr("Input", "Entrée"), "REAL", lang.tr("Process Variable (measured sensor value)", "Mesure du procédé (retour capteur)")],
                &["KP", lang.tr("Input", "Entrée"), "REAL", lang.tr("Proportional Gain factor", "Gain proportionnel")],
                &["TI", lang.tr("Input", "Entrée"), "TIME", lang.tr("Integral Reset Time (0 = disable integral)", "Temps d'intégration (0 = sans intégrale)")],
                &["TD", lang.tr("Input", "Entrée"), "TIME", lang.tr("Derivative Rate Time", "Temps de dérivation")],
                &["MAN", lang.tr("Input", "Entrée"), "BOOL", lang.tr("Manual mode override toggle", "Commutateur de marche manuelle")],
                &["MAN_VAL", lang.tr("Input", "Entrée"), "REAL", lang.tr("Manual output value when MAN is TRUE", "Valeur de sortie manuelle forcée")],
                &["OUT", lang.tr("Output", "Sortie"), "REAL", lang.tr("Calculated Control Output (MV)", "Grandeur réglante calculée (MV)")],
            ]
        ),
        callout_box(
            lang.tr("Anti-Windup Protection", "Protection Anti-Emballement d'Intégrale (Anti-Windup)"),
            lang.tr(
                "When actuator limits (e.g. 0.0% to 100.0% valve opening) are reached, the integral accumulator is clamped to prevent saturation delay when the error changes sign.",
                "Lorsque les limites de l'actionneur (ex. 0.0% à 100.0% d'ouverture vanne) sont atteintes, l'intégrateur est verrouillé pour éviter les retards de désaturation."
            ),
            StudioTheme::ACCENT_CYAN,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_fbd_logic_math<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("FBD Logic, Comparison & Math Blocks", "Blocs Logiques, Comparaisons & Calculs FBD"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "The FBD canvas includes elementary logic gates, relational comparators, and arithmetic math blocks that evaluate topologically every scan cycle.",
            "Le canvas FBD comprend des portes logiques élémentaires, des comparateurs relationnels et des blocs de calcul arithmétique évalués par tri topologique à chaque cycle."
        )),
        section_title(lang.tr("Boolean Logic Gates", "Portes Logiques Booléennes")),
        table_card(
            &[lang.tr("Gate", "Porte"), lang.tr("Inputs -> Output", "Entrées -> Sortie"), lang.tr("Behavior", "Comportement")],
            &[
                &["AND", "IN1: BOOL, IN2: BOOL -> OUT: BOOL", lang.tr("Logical Conjunction: TRUE only if all inputs are TRUE.", "Conjonction Logique : VRAI seulement si toutes les entrées sont VRAIES.")],
                &["OR", "IN1: BOOL, IN2: BOOL -> OUT: BOOL", lang.tr("Logical Disjunction: TRUE if at least one input is TRUE.", "Disjonction Logique : VRAI si au moins une entrée est VRAIE.")],
                &["XOR", "IN1: BOOL, IN2: BOOL -> OUT: BOOL", lang.tr("Exclusive OR: TRUE if exactly one input is TRUE.", "OU Exclusif : VRAI si exactement une entrée est VRAIE.")],
                &["NOT", "IN: BOOL -> OUT: BOOL", lang.tr("Logical Inversion: Inverts boolean signal state.", "Inversion Logique : Inverse l'état du signal booléen.")],
            ]
        ),
        section_title(lang.tr("Relational Comparators", "Comparateurs Relationnels")),
        table_card(
            &[lang.tr("Comparator", "Comparateur"), lang.tr("Relation", "Relation"), lang.tr("Output Condition", "Condition de Sortie")],
            &[
                &["EQ / NE", "IN1 == IN2 / IN1 <> IN2", lang.tr("Tests equality or inequality of numbers or timers.", "Teste l'égalité ou l'inégalité de valeurs numériques ou durées.")],
                &["GT / GE", "IN1 > IN2 / IN1 >= IN2", lang.tr("Tests if IN1 is strictly greater (GT) or greater/equal (GE) than IN2.", "Teste si IN1 est strictement supérieur (GT) ou supérieur ou égal (GE) à IN2.")],
                &["LT / LE", "IN1 < IN2 / IN1 <= IN2", lang.tr("Tests if IN1 is strictly less (LT) or less/equal (LE) than IN2.", "Teste si IN1 est strictement inférieur (LT) ou inférieur ou égal (LE) à IN2.")],
            ]
        ),
        section_title(lang.tr("Arithmetic Math Blocks", "Blocs Arithmétiques")),
        symbol_card(
            "[ADD / SUB]",
            lang.tr("Addition / Subtraction Blocks", "Blocs Addition / Soustraction"),
            "IN1, IN2 -> OUT (INT / REAL)",
            lang.tr("Computes the sum (ADD) or difference (SUB) of input values.", "Calcule la somme (ADD) ou la différence (SUB) des grandeurs en entrée."),
            "Analog scaling offset: ADD(Raw_Pressure, Offset_Bar)"
        ),
        symbol_card(
            "[MUL / DIV]",
            lang.tr("Multiplication / Division Blocks", "Blocs Multiplication / Division"),
            "IN1, IN2 -> OUT (INT / REAL)",
            lang.tr("Multiplies or divides signals. Includes safety guards against division by zero (returning 0.0 or saturated bounds).", "Multiplie ou divise les signaux. Protégé contre les divisions par zéro."),
            "Gain calculation: MUL(Sensor_Voltage, Calibration_Gain)"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_fbd_custom_composite<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Custom & Composite User Function Blocks", "Blocs Fonctionnels Utilisateur & Composites"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC allows engineers to build reusable, encapsulated Composite Function Blocks from existing FBD networks. These composite blocks can be packaged, stored in the Custom FBD Library, and instantiated across any project diagram.",
            "FerritePLC permet aux automaticiens de concevoir des Blocs Fonctionnels Composites encapsulés et réutilisables à partir de réseaux FBD. Ces blocs peuvent être archivés dans la bibliothèque personnalisée et instanciés à volonté."
        )),
        section_title(lang.tr("Composite Block Concepts", "Concepts des Blocs Composites")),
        symbol_card(
            "COMPOSITE_FB",
            lang.tr("Sub-Diagram Encapsulation", "Encapsulation de Sous-Schéma"),
            lang.tr("Reusable Multi-Block Network", "Réseau Multi-Blocs Réutilisable"),
            lang.tr(
                "Encapsulates an entire internal sub-graph of logic, math, and timer blocks into a single block with exposed input and output connector pins.",
                "Encapsule tout un sous-graphe de logique, calculs et temporisateurs en un bloc unique doté de broches d'entrées et sorties configurables."
            ),
            "Instance: LeadLagPumpController(Run_Cmd, Flow_Sens -> Pump1_Start, Pump2_Start)"
        ),
        section_title(lang.tr("Custom FBD Library Management", "Gestion de la Bibliothèque Personnalisée")),
        table_card(
            &[lang.tr("Feature", "Fonctionnalité"), lang.tr("Capability Description", "Description de la Capacité")],
            &[
                &[lang.tr("Library Palette", "Palette Bibliothèque"), lang.tr("Displays all user-authored blocks in a dedicated studio toolbox panel.", "Affiche tous les blocs créés par l'utilisateur dans un panneau dédié du studio.")],
                &[lang.tr("Pin Configuration", "Configuration des Broches"), lang.tr("Customize names, types (BOOL, REAL, TIME), and descriptions for input/output connectors.", "Personnalise les noms, types (BOOL, REAL, TIME) et infobulles des broches.")],
                &[lang.tr("Versioned Storage", "Stockage Versionné"), lang.tr("Saved cleanly into the project's symbols and POU registry for seamless Git version control.", "Enregistré de manière lisible dans les registres du projet pour le suivi Git.")],
            ]
        ),
        callout_box(
            lang.tr("Clean Hierarchy & Debugging", "Hiérarchie Claire & Débogage"),
            lang.tr(
                "Double-clicking a composite function block on the canvas opens its internal schematic in an isolated drill-down tab with live signal probes.",
                "Un double-clic sur un bloc composite ouvre son schéma interne dans un onglet dédié permettant de visualiser les signaux en direct."
            ),
            StudioTheme::ACCENT_INDIGO,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 6. Memory & Tags
// ---------------------------------------------------------------------------

fn render_mem_direct<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Direct Memory Addressing (%I, %Q, %M)", "Adressage Direct Mémoire (%I, %Q, %M)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "In accordance with IEC 61131-3, FerritePLC provides a standardized direct memory mapping schema using prefix notation: %[Area][Size][Byte].[Bit].",
            "Conformément à la norme CEI 61131-3, FerritePLC offre un schéma d'adressage mémoire direct standardisé sous la forme %[Zone][Taille][Octet].[Bit]."
        )),
        section_title(lang.tr("Addressing Syntax Breakdown", "Décomposition de la Syntaxe")),
        table_card(
            &[lang.tr("Prefix", "Préfixe"), lang.tr("Meaning", "Signification"), lang.tr("Examples", "Exemples")],
            &[
                &["%I", lang.tr("Physical / Simulated Digital & Analog Inputs", "Entrées Physiques / Simulées (Tor & Analogiques)"), "%IX0.0 (Bit 0 of Input Byte 0), %IW2 (Input Word 2)"],
                &["%Q", lang.tr("Physical / Simulated Outputs to Actuators", "Sorties Physiques / Simulées vers Actionneurs"), "%QX0.1 (Bit 1 of Output Byte 0), %QW0 (Output Word 0)"],
                &["%M", lang.tr("Internal Marker / Flag Memory Space", "Mémoire Interne / Drapeaux / Mémentos"), "%MX10.0 (Internal Bit Flag), %MW100 (Internal Word)"],
            ]
        ),
        section_title(lang.tr("Size Specifiers", "Spécificateurs de Taille")),
        table_card(
            &[lang.tr("Code", "Code"), lang.tr("Data Size", "Taille"), lang.tr("Bits", "Bits"), lang.tr("Example Address", "Exemple d'Adresse")],
            &[
                &["X", lang.tr("Discrete Bit", "Bit Unique"), "1 bit", "%IX0.0, %QX1.7, %MX5.2"],
                &["B", lang.tr("Byte", "Octet"), "8 bits", "%IB0, %QB2, %MB10"],
                &["W", lang.tr("Word", "Mot (Word)"), "16 bits", "%IW0, %QW2, %MW100"],
                &["D", lang.tr("Double Word", "Double Mot (DWord)"), "32 bits", "%ID0, %QD4, %MD20"],
            ]
        ),
    ]
    .spacing(12)
    .into()
}

fn render_mem_tags<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Symbolic Tag Table & Data Types", "Table des Variables & Types IEC"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "Modern automation relies on meaningful symbolic tags rather than raw memory addresses. The TagTable provides strong typing, cross-referencing, and persistence configuration.",
            "L'automatisation moderne privilégie les variables symboliques explicites aux adresses brutes. La table TagTable garantit un typage fort, la détection des conflits et la configuration de rémanence."
        )),
        section_title(lang.tr("Supported IEC Data Types", "Types de Données CEI Supportés")),
        table_card(
            &[lang.tr("Type", "Type"), lang.tr("Bit Width", "Largeur"), lang.tr("Value Range", "Plage de Valeurs"), lang.tr("Default Initial Value", "Valeur Initiale")],
            &[
                &["BOOL", "1 bit", "TRUE (1) / FALSE (0)", "FALSE"],
                &["BYTE", "8 bits", "0 to 255 (Unsigned)", "0"],
                &["WORD", "16 bits", "0 to 65,535 (Unsigned)", "0"],
                &["DWORD", "32 bits", "0 to 4,294,967,295", "0"],
                &["INT", "16 bits", "-32,768 to +32,767 (Signed)", "0"],
                &["DINT", "32 bits", "-2,147,483,648 to +2,147,483,647", "0"],
                &["REAL", "32 bits", "IEEE 754 Single-Precision Float", "0.0"],
                &["TIME", "64 bits", "Duration in ns (T#0ms to T#1000h)", "T#0s"],
                &["STRING", "Variable", "UTF-8 Encoded Character Sequence", "''"],
            ]
        ),
        callout_box(
            lang.tr("Retentive Variables (RETAIN)", "Variables Rémanentes (RETAIN)"),
            lang.tr(
                "Tags marked with the RETAIN attribute preserve their values across CPU restarts, simulated power cycles, and program reloads via the local retain snapshot journal.",
                "Les variables marquées de l'attribut RETAIN conservent leur valeur lors des redémarrages de l'automate, coupures d'alimentation simulées et rechargements de programme."
            ),
            StudioTheme::ACCENT_AMBER,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 7. Simulation & Forcing
// ---------------------------------------------------------------------------

fn render_sim_exec_forces<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Scan Engine, Virtual Clock & I/O Forcing", "Moteur de Scan, Horloge Virtuelle & Forçage E/S"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "The FerritePLC simulation runtime gives control engineers complete interactive mastery over execution speed, single stepping, and diagnostic input/output forcing.",
            "Le moteur de simulation FerritePLC offre aux automaticiens un contrôle interactif complet : réglage de cadence, avance pas-à-pas et forçage de diagnostic des entrées/sorties."
        )),
        section_title(lang.tr("Execution Controls", "Commandes d'Exécution")),
        table_card(
            &[lang.tr("Command", "Commande"), lang.tr("Shortcut", "Raccourci"), lang.tr("Function", "Fonction")],
            &[
                &[lang.tr("Run / Pause", "Exécuter / Pause"), "Space", lang.tr("Toggles continuous scan execution at the configured cycle rate (default 10 ms).", "Bascule l'exécution cyclique continue selon la période configurée (10 ms par défaut).")],
                &[lang.tr("Single Step", "Pas Unique"), "F10", lang.tr("Advances the PLC runtime by exactly ONE scan cycle and pauses.", "Fait progresser l'automate d'exactement UN cycle de scan puis se remet en pause.")],
                &[lang.tr("Cycle Speed Slider", "Curseur de Vitesse"), "-", lang.tr("Adjusts virtual cycle duration from 1 ms up to 1000 ms.", "Ajuste la durée de cycle virtuelle de 1 ms jusqu'à 1000 ms.")],
            ]
        ),
        section_title(lang.tr("I/O Forcing Mechanism", "Mécanisme de Forçage E/S")),
        paragraph(lang.tr(
            "Forcing allows an engineer to override any physical or simulated input/output with a locked boolean value. While forced, normal logic execution or hardware signals cannot change the forced value.",
            "Le forçage permet de verrouiller toute entrée/sortie physique ou simulée à une valeur booléenne fixe. Tant qu'elle est forcée, la logique normale du programme ne peut plus la modifier."
        )),
        callout_box(
            lang.tr("[!] Forces Active Warning", "[!] Avertissement Forçages Actifs"),
            lang.tr(
                "Whenever at least one variable is forced, FerritePLC displays a prominent amber warning indicator '[!] Forces Active' in the top status bar. Use 'Unforce All' to restore normal deterministic scan control.",
                "Dès qu'au moins une variable est forcée, FerritePLC affiche une bannière d'alerte ambrée '[!] Forçages Actifs' dans la barre supérieure. Utilisez 'Déforcer Tout' pour rétablir la conduite normale."
            ),
            StudioTheme::ACCENT_AMBER,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_sim_alarms_retain<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("ISA 18.2 Alarm Lifecycle & Retain Snapshots", "Cycle de Vie Alarmes ISA 18.2 & Mémoire Rémanente"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC adheres to the international ISA 18.2 Management of Alarm Systems standard, managing state transitions from detection through acknowledgement and clearing.",
            "FerritePLC implémente la norme internationale ISA 18.2 pour la gestion des alarmes industrielles, régissant le cycle d'apparition, d'acquittement et de disparition."
        )),
        section_title(lang.tr("ISA 18.2 State Machine", "Machine d'États ISA 18.2")),
        table_card(
            &[lang.tr("State", "État"), lang.tr("Condition", "Condition"), lang.tr("Visual Indication", "Indicateur Visuel")],
            &[
                &["Normal", lang.tr("Process within safe limits, no alarm condition.", "Procédé nominal, aucune condition d'alarme active."), lang.tr("Green / No badge", "Vert / Aucun badge")],
                &["Active Unacknowledged", lang.tr("Alarm tripped; operator has NOT acknowledged yet.", "Alarme déclenchée ; opérateur n'a pas encore acquitté."), lang.tr("Flashing Red / High priority alert", "Rouge Clignotant / Alerte haute")],
                &["Active Acknowledged", lang.tr("Alarm condition still present; operator confirmed awareness.", "Condition toujours présente ; opérateur a confirmé."), lang.tr("Solid Amber / Steady warning", "Ambre Fixe / Avertissement continu")],
                &["Cleared Unacknowledged", lang.tr("Process returned to normal, but prior alarm was never acknowledged.", "Procédé redevenu normal mais l'alarme passée n'a pas été acquittée."), lang.tr("Dim Flashing / Record pending", "Clignotement Atténué / Historique en attente")],
            ]
        ),
    ]
    .spacing(12)
    .into()
}

fn render_sim_trend_plotter<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Real-Time Trend Plotter & Oscilloscope", "Traceur de Tendances Temps Réel & Oscilloscope"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC features an integrated high-speed multi-channel trend plotter and software oscilloscope, allowing live visual inspection of analog curves (%IW, %QW, PID variables) and discrete pulses.",
            "FerritePLC intègre un traceur de tendances multi-voies haute vitesse et un oscilloscope logiciel, permettant d'observer en direct les courbes analogiques (%IW, %QW, variables PID) et signaux tores."
        )),
        section_title(lang.tr("Plotter Capabilities & Modes", "Fonctionnalités & Modes du Traceur")),
        table_card(
            &[lang.tr("Capability", "Fonctionnalité"), lang.tr("Settings / Range", "Réglages / Plage"), lang.tr("Description", "Description")],
            &[
                &[lang.tr("Multi-Channel Tracking", "Suivi Multi-Voies"), "Up to 8 concurrent tags", lang.tr("Color-coded traces with individual scale, offset, and zero-reference lines.", "Tracés avec codes couleurs distincts, échelles individuelles et repères de zéro.")],
                &[lang.tr("Timebase / Window", "Base de Temps"), "100 ms to 60 s per division", lang.tr("Adjustable horizontal sweep rate with smooth circular buffer streaming.", "Vitesse de balayage horizontal réglable avec défilement fluide par tampon circulaire.")],
                &[lang.tr("Trigger Modes", "Modes de Déclenchement"), "Auto, Normal, Single Edge", lang.tr("Rising edge, falling edge, or threshold crossing on selected trigger channel.", "Front montant, descendant ou franchissement de seuil sur la voie de référence.")],
                &[lang.tr("Freeze & Cursors", "Gel & Curseurs de Mesure"), "Pause & Dual Cursors", lang.tr("Freeze live acquisition to inspect delta-time (Δt) and delta-amplitude (ΔV).", "Fige l'acquisition pour mesurer les écarts temporels (Δt) et d'amplitude (ΔV).")],
            ]
        ),
        callout_box(
            lang.tr("Zero Heap-Allocation Acquisition", "Acquisition sans Allocation Tas"),
            lang.tr(
                "Sample buffers use pre-allocated ring buffers. Enabling live plotting incurs negligible scan cycle overhead, preserving real-time deterministic execution.",
                "Les tampons d'échantillonnage reposent sur des buffers circulaires pré-alloués. L'enregistrement n'induit aucun ralentissement sur le temps de cycle déterministe."
            ),
            StudioTheme::ACCENT_CYAN,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 8. Fieldbus & Protocols
// ---------------------------------------------------------------------------

fn render_fieldbus_tcp_iot<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Modbus TCP, OPC UA & MQTT Industrial Telemetry", "Modbus TCP, OPC UA & Télémesure Industrielle MQTT"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC connects natively to industrial Ethernet networks and SCADA/MES/Cloud systems via open standards.",
            "FerritePLC communique nativement avec les réseaux industriels Ethernet et les systèmes SCADA/MES/Cloud via des standards ouverts."
        )),
        section_title(lang.tr("Supported Ethernet & IoT Protocols", "Protocoles Ethernet & IoT Supportés")),
        symbol_card(
            "Modbus",
            "Modbus TCP (Port 502)",
            lang.tr("Industrial Master / Slave Server", "Serveur Industriel Maître / Esclave"),
            lang.tr(
                "Exposes coils (%QX), discrete inputs (%IX), input registers (%IW), and holding registers (%MW / %QW) using standard function codes FC1, FC2, FC3, FC4, FC5, FC6, FC15, FC16.",
                "Expose les bobines (%QX), entrées tores (%IX), registres d'entrée (%IW) et registres de maintien (%MW/%QW) avec les codes fonctions standards FC1 à FC16."
            ),
            "Connect SCADA / HMI to localhost:502"
        ),
        symbol_card(
            "OPC UA",
            "OPC Unified Architecture (IEC 62541)",
            lang.tr("Secure Semantic Industrial Server", "Serveur Sémantique Sécurisé"),
            lang.tr(
                "Publishes the symbolic TagTable as a browsable hierarchical address space with real-time subscriptions, timestamped telemetry, and type metadata.",
                "Publie la table de variables symboliques sous forme d'espace d'adressage hiérarchique avec abonnements temps réel et métadonnées."
            ),
            "opc.tcp://127.0.0.1:4840"
        ),
        symbol_card(
            "MQTT",
            "MQTT IoT Telemetry Publisher",
            lang.tr("Cloud & Edge Pub/Sub Gateway", "Passerelle Pub/Sub IoT & Edge"),
            lang.tr(
                "Publishes JSON-encoded scan snapshots and ISA 18.2 alarm events to remote message brokers (e.g., Mosquitto, EMQX, HiveMQ) on configurable topics.",
                "Publie des instantanés JSON de télémesure et les événements d'alarme vers un courtier MQTT distant sur des sujets configurables."
            ),
            "Topic: ferrite/telemetry/scandata"
        ),
    ]
    .spacing(12)
    .into()
}

fn render_fieldbus_modbus_rtu<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Modbus RTU Serial (RS-485 / RS-232)", "Modbus RTU Série (RS-485 / RS-232)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC includes a complete industrial-grade Modbus RTU serial engine conforming to the Modbus Serial Line Protocol specification v1.02. It supports both Master and Slave roles over physical UART / RS-485 transceivers.",
            "FerritePLC intègre un moteur Modbus RTU série complet conforme à la norme Modbus Serial Line v1.02, supportant les rôles Maître et Esclave sur liaisons physiques UART / RS-485 / RS-232."
        )),
        section_title(lang.tr("Serial Framing & Frame Delimitation", "Cadrage Série & Délimitation de Trame")),
        table_card(
            &[lang.tr("Parameter", "Paramètre"), lang.tr("Standard Requirement", "Spécification"), lang.tr("Role in Communication", "Rôle dans la Liaison")],
            &[
                &[lang.tr("Baud Rate", "Vitesse (Bauds)"), "1200 to 115200 bps (default 9600)", lang.tr("Data transmission speed across the serial bus.", "Cadence de modulation des données sur le bus série.")],
                &[lang.tr("Parity & Framing", "Parité & Format"), "8 Data Bits, Even/None/Odd, 1 or 2 Stop Bits", lang.tr("Configured in SerialPortConfig to match connected field instruments.", "Configurable pour correspondre aux instruments de terrain.")],
                &[lang.tr("CRC-16 Checksum", "Somme de Contrôle CRC-16"), "Polynom 0xA001 (Table & Bitwise)", lang.tr("Appended to every frame (Low byte, High byte) for high data integrity.", "Calculé sur chaque trame pour garantir l'intégrité face aux parasites industriels.")],
                &[lang.tr("Inter-character Gap (t1.5)", "Silence Inter-caractères (t1.5)"), "1.5 character times (min 750 µs)", lang.tr("If a gap exceeds t1.5 during message reception, the incomplete frame is aborted.", "Si un silence dépasse t1.5 pendant la réception, la trame est annulée.")],
                &[lang.tr("Frame Delimiter (t3.5)", "Silence Séparateur (t3.5)"), "3.5 character times (min 1.75 ms)", lang.tr("A silent interval of at least t3.5 marks the end of a message and clears the bus.", "Un silence d'au moins t3.5 marque la fin du message et libère le bus.")],
            ]
        ),
        code_snippet(
            "+-------------+-------------------+----------------+----------------+\n\
             | Slave Addr  | Function Code     | Data Field     | CRC-16 Check   |\n\
             |  (1 Byte)   | (FC 01..16, 1 B)  | (0..252 Bytes) | (2 Bytes, L+H) |\n\
             +-------------+-------------------+----------------+----------------+\n\
             |<--------------------------- ADU Frame -------------------------->|"
        ),
        callout_box(
            lang.tr("RS-485 Multi-Drop Topology", "Topologie Multipoint RS-485"),
            lang.tr(
                "Up to 247 slave devices can be addressed on a single differential 2-wire RS-485 bus with terminating 120-ohm resistors. FerritePLC handles silent timing deterministically.",
                "Jusqu'à 247 esclaves peuvent être adressés sur une ligne différentielle 2 fils RS-485 avec résistances de fin de ligne 120 ohms. FerritePLC gère rigoureusement les silences t1.5 et t3.5."
            ),
            StudioTheme::ACCENT_AMBER,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 9. Hardware & Embedded HAL
// ---------------------------------------------------------------------------

fn render_hardware_rpi<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Raspberry Pi 40-Pin GPIO Header Visualizer", "Visualiseur GPIO 40 Broches Raspberry Pi"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "When running on embedded hardware (e.g. Raspberry Pi 4 / 5 / CM4), FerritePLC's Hardware Abstraction Layer (HAL) directly maps %IX and %QX memory addresses to physical 40-pin GPIO headers.",
            "Sur cible matérielle embarquée (ex. Raspberry Pi 4 / 5 / CM4), la couche HAL relie directement les adresses %IX et %QX aux broches physiques du connecteur 40 broches."
        )),
        section_title(lang.tr("Interactive 40-Pin Pinout Diagram", "Schéma Interactif du Connecteur 40 Broches")),
        code_snippet(
            "               3.3V DC (1)  (2)  5V DC\n\
             (GPIO 2 / I2C SDA) (3)  (4)  5V DC\n\
             (GPIO 3 / I2C SCL) (5)  (6)  Ground\n\
                      (GPIO 4) (7)  (8)  (GPIO 14 / UART TX)\n\
                        Ground (9)  (10) (GPIO 15 / UART RX)\n\
                     (GPIO 17) (11) (12) (GPIO 18 / PWM0)\n\
                     (GPIO 27) (13) (14) Ground\n\
                     (GPIO 22) (15) (16) (GPIO 23)\n\
                       3.3V DC (17) (18) (GPIO 24)\n\
            (GPIO 10 / SPI MOSI) (19) (20) Ground\n\
            (GPIO 9  / SPI MISO) (21) (22) (GPIO 25)\n\
            (GPIO 11 / SPI SCLK) (23) (24) (GPIO 8 / SPI CE0)\n\
                        Ground (25) (26) (GPIO 7 / SPI CE1)\n\
                     (ID_SD I2C) (27) (28) (ID_SC I2C)\n\
                      (GPIO 5) (29) (30) Ground\n\
                      (GPIO 6) (31) (32) (GPIO 12 / PWM0)\n\
                     (GPIO 13) (33) (34) Ground\n\
                     (GPIO 19) (35) (36) (GPIO 16)\n\
                     (GPIO 26) (37) (38) (GPIO 20)\n\
                        Ground (39) (40) (GPIO 21)"
        ),
        callout_box(
            lang.tr("Live Pin State Color Coding", "Code Couleur d'État des Broches"),
            lang.tr(
                "The Studio GPIO tab displays live logic states: Green for HIGH (3.3V active), Dark Gray for LOW (0V inactive), Red for 5V power rails, and Black for Ground (GND).",
                "L'onglet GPIO affiche l'état temps réel : Vert pour l'état HAUT (3.3V actif), Gris foncé pour BAS (0V inactif), Rouge pour l'alimentation 5V et Noir pour la masse (GND)."
            ),
            StudioTheme::ACCENT_EMERALD,
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 10. Digital Twin
// ---------------------------------------------------------------------------

fn render_digital_twin<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Digital Twin 2D Simulation Canvas", "Canvas de Simulation Digital Twin 2D"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "The 2D Digital Twin simulates realistic industrial plant components in real time. It binds PLC actuator outputs (%Q) to physical component simulations and feeds simulated sensor states back into PLC inputs (%I), providing both passive monitoring and interactive manual HMI control.",
            "Le Jumeau Numérique 2D simule en temps réel le comportement physique d'installations industrielles. Il relie les sorties automate (%Q) aux actionneurs et renvoie l'état des capteurs vers les entrées (%I), assurant à la fois supervision passive et conduite manuelle interactive."
        )),
        section_title(lang.tr("Simulated Physical Components", "Composants Physiques Simulés")),
        symbol_card(
            "[TANK]",
            lang.tr("Liquid Storage Tank", "Cuve de Stockage de Fluide"),
            lang.tr("Continuously Animated Fluid Level & Limit Switches", "Niveau de Fluide Animé & Capteurs de Niveaux"),
            lang.tr("Models fluid mass balance: Level(t) += (Inflow - Outflow) * dt. Displays visual liquid color/animation and high/low level limit switches.", "Modélise le bilan de masse : Niveau(t) += (DébitEntrant - DébitSortant) * dt. Affiche la jauge de liquide et déclenche les capteurs de seuil haut/bas."),
            "Tag bindings: Inlet_Valve -> Fill Rate, Tank_Full -> %IX0.2"
        ),
        symbol_card(
            "[ACTUATOR]",
            lang.tr("Industrial Actuators (Motor, Pump, Agitator)", "Actionneurs Industriels (Moteur, Pompe, Agitateur)"),
            lang.tr("ElectricMotor / CentrifugalPump / AgitatorMixer", "Moteur Électrique / Pompe / Agitateur"),
            lang.tr("Displays dynamic rotational and fluid flow animations when energized (%QX). Visualizes running, tripped, and standby states.", "Affiche une animation dynamique de rotation et de refoulement dès l'activation (%QX). Visualise les états marche, défaut et arrêt."),
            "Tag binding: Motor_Start -> %QX0.0"
        ),
        symbol_card(
            "[VALVE / PIPE]",
            lang.tr("Solenoid Valves & Flow Conduits", "Électrovannes & Conduites de Fluide"),
            lang.tr("Fluid Flow Regulation", "Régulation d'Écoulement de Fluide"),
            lang.tr("Controls pipe conductance. Supports binary open/closed solenoids or continuous percentage throttling (0..100%). Animates liquid transit inside connected pipes.", "Pilote la conductance de tuyauterie (tout-ou-rien ou vanne proportionnelle 0-100%). Anime le transit de fluide dans les tuyaux connectés."),
            "Tag binding: Solenoid_Valve_1 -> %QX0.1"
        ),
        symbol_card(
            "[HMI CONTROLS]",
            lang.tr("Interactive HMI (PushButtons & Switches)", "Commandes IHM Interactives (Boutons-Poussoirs & Commutateurs)"),
            lang.tr("Momentary & Maintained Operator Inputs", "Entrées Opérateur Fugitives & Maintenues"),
            lang.tr("Pushbuttons (momentary press/release or toggle) and 2-position selector switches allow operators to interact with simulated PLC inputs (%IX) in real time during simulation.", "Boutons-poussoirs (impulsionnels ou bistables) et commutateurs permettant à l'opérateur d'actionner les entrées automate (%IX) en direct."),
            "Tag binding: Start_PB -> %IX0.0 (Momentary PushButton)"
        ),
        symbol_card(
            "[LED / BLINK]",
            lang.tr("Indicator LEDs & Alarm Beacons", "Voyants Lumineux & Balises d'Alarme"),
            lang.tr("Configurable Blinking & Color States", "Clignotements & États Colorés Configurables"),
            lang.tr("Visual indicator lamps with configurable active/inactive colors and automatic blinking periods (e.g. 500 ms flash on alarm trip).", "Voyants d'état avec couleurs personnalisables et cadence de clignotement paramétrable (ex. flash 500 ms sur déclenchement d'alarme)."),
            "Tag binding: Alarm_Beacon -> %QX2.0 (Blink 500ms)"
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 11. Tools & Shortcuts
// ---------------------------------------------------------------------------

fn render_tools_plcopen_xml<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("PLCopen TC6 XML Export & Import", "Import & Export PLCopen TC6 XML"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC natively supports the international standard PLCopen TC6 XML schema (IEC 61131-10 / TC6 XML v2.01). This guarantees complete vendor-independent project interchange with major industrial automation platforms such as Codesys, TwinCAT, B&R Automation Studio, and Schneider Machine Expert.",
            "FerritePLC implémente nativement le schéma d'échange international PLCopen TC6 XML (CEI 61131-10 / TC6 XML v2.01). Cela garantit une interopérabilité totale avec les suites logicielles majeures du marché (Codesys, TwinCAT, B&R, Schneider)."
        )),
        section_title(lang.tr("Supported PLCopen TC6 Features", "Fonctionnalités PLCopen TC6 Supportées")),
        table_card(
            &[lang.tr("Feature", "Fonctionnalité"), lang.tr("Direction", "Sens"), lang.tr("Interchange Description", "Description de l'Échange")],
            &[
                &[lang.tr("Project Header & Info", "En-tête & Métadonnées"), "Import / Export", lang.tr("Transfers project author, organization, creation timestamps, and tool version.", "Transfère l'auteur, l'organisation, l'horodatage et la version de l'outil.")],
                &[lang.tr("Data Types & Variables", "Types & Variables Globales"), "Import / Export", lang.tr("Converts global tag tables (%I, %Q, %M) and strong IEC types into <globalVars> elements.", "Convertit les variables globales (%I, %Q, %M) et types CEI en balises <globalVars>.")],
                &[lang.tr("Structured Text POUs", "POUs Structured Text"), "Import / Export", lang.tr("Exports and parses ST algorithmic bodies enclosed in standard <ST> XML containers.", "Exporte et analyse le code source ST encapsulé dans les balises XML <ST>.")],
                &[lang.tr("Graphical POUs (LD, SFC, FBD)", "POUs Graphiques (LD, SFC, FBD)"), "Import / Export", lang.tr("Translates rungs, contacts, steps, transitions, and function block coordinates seamlessly.", "Convertit les rungs, contacts, étapes, transitions et blocs graphiques avec coordonnées.")],
            ]
        ),
        code_snippet(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
             <project xmlns=\"http://www.plcopen.org/xml/tc6_0201\">\n\
             \x20 <fileHeader companyName=\"FerritePLC\" productName=\"FerriteStudio\" />\n\
             \x20 <contentHeader name=\"PlantAutomation\">\n\
             \x20   <coordinateInfo><fbd><scaling x=\"1\" y=\"1\"/></fbd></coordinateInfo>\n\
             \x20 </contentHeader>\n\
             \x20 <types><pous><pou name=\"MainLogic\" pouType=\"program\">...</pou></pous></types>\n\
             </project>"
        ),
        callout_box(
            lang.tr("Cross-Platform Migration", "Migration Inter-Plateformes"),
            lang.tr(
                "Use 'File -> Export PLCopen XML' to share programs with clients using third-party controllers, or 'File -> Import PLCopen XML' to instantly migrate legacy PLC programs into FerritePLC.",
                "Utilisez 'Fichier -> Exporter PLCopen XML' pour partager vos programmes avec des automates tiers, ou 'Fichier -> Importer PLCopen XML' pour migrer instantanément vos applications vers FerritePLC."
            ),
            StudioTheme::ACCENT_CYAN,
        ),
    ]
    .spacing(12)
    .into()
}

fn render_canvas_shortcuts<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Studio Navigation & Keyboard Shortcuts", "Navigation Studio & Raccourcis Clavier"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC provides intuitive mouse gesture controls and industry-standard keyboard shortcuts for high-velocity schematic editing.",
            "FerritePLC offre une navigation intuitive à la souris et des raccourcis clavier standards pour une édition rapide de schémas."
        )),
        section_title(lang.tr("Canvas Pan, Zoom & Selection", "Déplacement, Zoom & Sélection sur le Canvas")),
        table_card(
            &[lang.tr("Action", "Action"), lang.tr("Mouse / Keyboard Input", "Action Souris / Clavier"), lang.tr("Behavior", "Comportement")],
            &[
                &[lang.tr("Pan Canvas", "Déplacer la Vue"), lang.tr("Middle-Click Drag  OR  Space + Left Drag", "Clic Molette Glisser  OU  Espace + Clic Gauche"), lang.tr("Smoothly moves the viewport in any direction.", "Fait défiler la vue fluidement dans toutes les directions.")],
                &[lang.tr("Zoom In / Out", "Zoom Avant / Arrière"), lang.tr("Mouse Wheel  OR  Ctrl + Wheel", "Molette Souris  OU  Ctrl + Molette"), lang.tr("Zooms in/out anchored around the current mouse cursor.", "Zoome/dézoome centré sur la position du curseur souris.")],
                &[lang.tr("Marquee Selection", "Sélection par Cadre"), lang.tr("Left-Click Drag in empty space", "Clic Gauche Glisser dans le vide"), lang.tr("Draws a selection rectangle to select multiple steps, rungs, or blocks.", "Trace un rectangle de sélection pour attraper plusieurs éléments.")],
                &[lang.tr("Toggle Selection", "Sélection Multiple"), lang.tr("Shift + Click", "Maj + Clic"), lang.tr("Adds or removes an element from the active selection set.", "Ajoute ou retire un élément de la sélection active.")],
                &[lang.tr("Wiring / Connections", "Câblage / Connexions"), lang.tr("Click Output Port -> Drag to Input Port", "Clic Port Sortie -> Glisser vers Port Entrée"), lang.tr("Draws an intelligent orthogonal signal wire between blocks.", "Trace un fil de signal orthogonal intelligent entre les blocs.")],
                &[lang.tr("Auto-Layout", "Disposition Auto"), lang.tr("Click 'Auto-Layout' Button", "Clic sur le Bouton 'Disposition Auto'"), lang.tr("Applies Sugiyama hierarchical layout to organize nodes neatly.", "Applique un algorithme hiérarchique pour réorganiser proprement le graphe.")],
                &[lang.tr("Delete Element", "Supprimer un Élément"), "Delete  OR  Backspace", lang.tr("Removes selected elements (prompts confirmation for safety).", "Supprime les éléments sélectionnés (avec confirmation de sécurité).")],
            ]
        ),
        section_title(lang.tr("Global Studio Shortcuts", "Raccourcis Clavier Généraux")),
        table_card(
            &[lang.tr("Shortcut", "Raccourci"), lang.tr("Command Name", "Nom de la Commande"), lang.tr("Description", "Description")],
            &[
                &["F1", lang.tr("Help & Manual", "Aide & Manuel"), lang.tr("Toggles this floating documentation and symbol reference window.", "Ouvre/ferme cette fenêtre flottante d'aide et de référence.")],
                &["Ctrl + S", lang.tr("Save Project", "Sauvegarder Projet"), lang.tr("Atomically writes all modified POUs and symbol tables to disk.", "Sauvegarde atomiquement toutes les POUs et tables de variables sur le disque.")],
                &["Ctrl + O", lang.tr("Open Project", "Ouvrir Projet"), lang.tr("Opens a native file picker to load an existing .fplc project.", "Ouvre un sélecteur de fichiers pour charger un projet .fplc.")],
                &["Ctrl + Z", lang.tr("Undo", "Annuler"), lang.tr("Reverts the last schematic edit or parameter change.", "Annule la dernière modification sur le schéma ou les paramètres.")],
                &["Ctrl + Y", lang.tr("Redo", "Rétablir"), lang.tr("Restores previously undone operations.", "Rétablit les modifications précédemment annulées.")],
                &["Space", lang.tr("Run / Pause PLC", "Exécuter / Pause API"), lang.tr("Toggles deterministic simulation scan cycle execution.", "Bascule l'exécution cyclique du moteur de simulation.")],
                &["F10", lang.tr("Single Step", "Pas Unique"), lang.tr("Executes exactly ONE scan cycle tick.", "Exécute exactement UN cycle de scan de l'automate.")],
            ]
        ),
    ]
    .spacing(12)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_catalog_completeness() {
        let topics = all_topics();
        assert!(topics.len() >= 29);
        for t in topics {
            assert!(!t.title_en.is_empty());
            assert!(!t.title_fr.is_empty());
            assert!(!t.keywords.is_empty());
        }
    }

    #[test]
    fn test_new_industrial_topics_exist_and_searchable() {
        let rtu_results = filter_topics("rtu");
        assert!(rtu_results.contains(&HelpTopic::FieldbusModbusRtu));

        let plcopen_results = filter_topics("plcopen");
        assert!(plcopen_results.contains(&HelpTopic::ToolsPlcOpenXml));

        let compare_results = filter_topics("compare");
        assert!(compare_results.contains(&HelpTopic::LdCompareMath));

        let composite_results = filter_topics("composite");
        assert!(composite_results.contains(&HelpTopic::FbdCustomComposite));

        let trend_results = filter_topics("oscilloscope");
        assert!(trend_results.contains(&HelpTopic::SimTrendPlotter));
    }

    #[test]
    fn test_help_filter_topics_by_query() {
        let ld_results = filter_topics("contact");
        assert!(ld_results.contains(&HelpTopic::LdContacts));

        let sfc_results = filter_topics("receptivity");
        assert!(
            sfc_results.contains(&HelpTopic::SfcDivergenceConvergence)
                || sfc_results.contains(&HelpTopic::SfcSteps)
                || sfc_results.contains(&HelpTopic::SfcIntrospection)
        );

        let mem_results = filter_topics("%MW");
        assert!(mem_results.contains(&HelpTopic::MemDirectAddressing));

        let empty_results = filter_topics("xyznonexistentterm123");
        assert!(empty_results.is_empty());
    }

    #[test]
    fn test_all_topics_render_without_panic() {
        let languages = [crate::i18n::UiLanguage::En, crate::i18n::UiLanguage::Fr];
        for topic_meta in all_topics() {
            for lang in languages {
                let _el: iced::Element<'_, ()> = render_topic_content(topic_meta.id, lang);
            }
        }
    }
}
