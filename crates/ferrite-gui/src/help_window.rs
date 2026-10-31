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
    FieldbusHardware,
    DigitalTwin,
    Shortcuts,
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
            HelpCategory::FieldbusHardware,
            HelpCategory::DigitalTwin,
            HelpCategory::Shortcuts,
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
            HelpCategory::Simulation => lang.tr("7. Simulator & Forcing", "7. Simulateur & Forçages"),
            HelpCategory::FieldbusHardware => {
                lang.tr("8. Fieldbus & Hardware", "8. Bus de Terrain & Matériel")
            }
            HelpCategory::DigitalTwin => lang.tr("9. 2D Digital Twin", "9. Jumeau Numérique 2D"),
            HelpCategory::Shortcuts => lang.tr("10. Shortcuts & Controls", "10. Raccourcis & Édition"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpTopic {
    ArchScanLoop,
    ArchProjectFiles,
    LdContacts,
    LdCoils,
    LdBranches,
    SfcSteps,
    SfcActions,
    SfcDivergenceConvergence,
    SfcIntrospection,
    StSyntax,
    StControlFlow,
    StMathFunctions,
    FbdStandardBlocks,
    FbdPidController,
    MemDirectAddressing,
    MemTagTable,
    SimExecutionAndForces,
    SimAlarmsAndRetain,
    FieldbusProtocols,
    HardwareRpiGpio,
    DigitalTwinSimulation,
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
    // Architecture
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
    // Ladder Diagram
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
    // SFC / Grafcet
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
    // Structured Text
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
    // Function Block Diagram
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
    // Memory & Addressing
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
    // Simulation
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
    // Fieldbus & Hardware
    HelpTopicMeta {
        id: HelpTopic::FieldbusProtocols,
        category: HelpCategory::FieldbusHardware,
        title_en: "Industrial Fieldbus (Modbus TCP, OPC UA, MQTT)",
        title_fr: "Bus de Terrain Industriels (Modbus TCP, OPC UA, MQTT)",
        keywords: &["fieldbus", "modbus", "modbus tcp", "opc ua", "mqtt", "telemetry", "holding registers", "coils", "address space"],
    },
    HelpTopicMeta {
        id: HelpTopic::HardwareRpiGpio,
        category: HelpCategory::FieldbusHardware,
        title_en: "Raspberry Pi 40-Pin GPIO Header Visualizer",
        title_fr: "Visualiseur GPIO 40 Broches Raspberry Pi",
        keywords: &["gpio", "raspberry pi", "rpi", "hardware", "hal", "40-pin", "pinout", "levels"],
    },
    // Digital Twin
    HelpTopicMeta {
        id: HelpTopic::DigitalTwinSimulation,
        category: HelpCategory::DigitalTwin,
        title_en: "Digital Twin 2D Simulation Canvas",
        title_fr: "Canvas de Simulation Digital Twin 2D",
        keywords: &["digital twin", "twin", "simulation", "physics", "tanks", "pumps", "valves", "conveyors", "sensors", "schematic"],
    },
    // Shortcuts
    HelpTopicMeta {
        id: HelpTopic::CanvasShortcuts,
        category: HelpCategory::Shortcuts,
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
use iced::widget::{column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::i18n::UiLanguage;
use crate::theme::StudioTheme;

pub fn render_topic_content<'a, Message: 'a>(
    topic: HelpTopic,
    lang: UiLanguage,
) -> Element<'a, Message> {
    match topic {
        HelpTopic::ArchScanLoop => render_arch_scan_loop(lang),
        HelpTopic::ArchProjectFiles => render_arch_project_files(lang),
        HelpTopic::LdContacts => render_ld_contacts(lang),
        HelpTopic::LdCoils => render_ld_coils(lang),
        HelpTopic::LdBranches => render_ld_branches(lang),
        HelpTopic::SfcSteps => render_sfc_steps(lang),
        HelpTopic::SfcActions => render_sfc_actions(lang),
        HelpTopic::SfcDivergenceConvergence => render_sfc_div_conv(lang),
        HelpTopic::SfcIntrospection => render_sfc_introspection(lang),
        HelpTopic::StSyntax => render_st_syntax(lang),
        HelpTopic::StControlFlow => render_st_control_flow(lang),
        HelpTopic::StMathFunctions => render_st_math(lang),
        HelpTopic::FbdStandardBlocks => render_fbd_standard_blocks(lang),
        HelpTopic::FbdPidController => render_fbd_pid(lang),
        HelpTopic::MemDirectAddressing => render_mem_direct(lang),
        HelpTopic::MemTagTable => render_mem_tags(lang),
        HelpTopic::SimExecutionAndForces => render_sim_exec_forces(lang),
        HelpTopic::SimAlarmsAndRetain => render_sim_alarms_retain(lang),
        HelpTopic::FieldbusProtocols => render_fieldbus(lang),
        HelpTopic::HardwareRpiGpio => render_hardware_rpi(lang),
        HelpTopic::DigitalTwinSimulation => render_digital_twin(lang),
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
    .style(move |_theme| container::Style {
        background: Some(Background::Color(StudioTheme::BG_SURFACE)),
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
    .style(|_theme| container::Style {
        background: Some(Background::Color(StudioTheme::BG_BASE)),
        border: Border {
            color: StudioTheme::BORDER_CARD,
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
            .style(|_theme| container::Style {
                background: Some(Background::Color(StudioTheme::BG_BASE)),
                border: Border {
                    color: StudioTheme::BORDER_HAIRLINE,
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
    .style(|_theme| container::Style {
        background: Some(Background::Color(StudioTheme::BG_BASE)),
        border: Border {
            color: StudioTheme::BORDER_HAIRLINE,
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

// ---------------------------------------------------------------------------
// 8. Fieldbus & Hardware
// ---------------------------------------------------------------------------

fn render_fieldbus<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Industrial Fieldbus (Modbus TCP, OPC UA, MQTT)", "Bus de Terrain Industriels (Modbus TCP, OPC UA, MQTT)"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "FerritePLC connects natively to industrial networks and SCADA/MES systems via industry-standard open communication protocols.",
            "FerritePLC communique nativement avec les réseaux industriels et systèmes SCADA/MES via les protocoles ouverts standards du marché."
        )),
        section_title(lang.tr("Supported Protocols", "Protocoles Supportés")),
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
// 9. Digital Twin
// ---------------------------------------------------------------------------

fn render_digital_twin<'a, Message: 'a>(lang: UiLanguage) -> Element<'a, Message> {
    column![
        text(lang.tr("Digital Twin 2D Simulation Canvas", "Canvas de Simulation Digital Twin 2D"))
            .size(18)
            .color(StudioTheme::TEXT_PRIMARY),
        paragraph(lang.tr(
            "The 2D Digital Twin simulates realistic industrial plant components in real time. It binds PLC actuator outputs (%Q) to physical component simulations and feeds simulated sensor states back into PLC inputs (%I).",
            "Le Jumeau Numérique 2D simule en temps réel le comportement physique d'installations industrielles. Il relie les sorties automate (%Q) aux actionneurs et renvoie l'état des capteurs vers les entrées (%I)."
        )),
        section_title(lang.tr("Simulated Physical Components", "Composants Physiques Simulés")),
        symbol_card(
            "[TANK]",
            lang.tr("Liquid Storage Tank", "Cuve de Stockage de Fluide"),
            lang.tr("Continuously Animated Fluid Level", "Niveau de Fluide Animé en Continu"),
            lang.tr("Models fluid mass balance: Level(t) += (Inflow - Outflow) * dt. Displays visual liquid animation and high/low level limit switches.", "Modélise le bilan de masse : Niveau(t) += (DébitEntrant - DébitSortant) * dt. Affiche une jauge de niveau et des capteurs de seuil haut/bas."),
            "Tag bindings: Inlet_Valve -> Fill Rate, Tank_Full -> %IX0.2"
        ),
        symbol_card(
            "[PUMP]",
            lang.tr("Centrifugal Pump", "Pompe Centrifuge"),
            lang.tr("Dynamic Flow Rate Actuator", "Actionneur de Débit Dynamique"),
            lang.tr("Generates fluid flow when energized (%QX). Displays running rotation and output pressure.", "Génère un écoulement lorsqu'elle est activée (%QX). Affiche la rotation du rotor et la pression de refoulement."),
            "Tag binding: Pump_Start -> %QX0.0"
        ),
        symbol_card(
            "[VALVE]",
            lang.tr("Solenoid Valve", "Électrovanne Proportionnelle / Tout-ou-Rien"),
            lang.tr("Flow Control Valve", "Vanne de Contrôle de Débit"),
            lang.tr("Controls pipe conductance. Supports binary open/closed solenoids or continuous percentage throttling.", "Pilote la conductance de tuyauterie. Supporte le tout-ou-rien ou la régulation proportionnelle 0-100%."),
            "Tag binding: Solenoid_Valve_1 -> %QX0.1"
        ),
        symbol_card(
            "[CONVEYOR]",
            lang.tr("Conveyor Belt & Optical Sensors", "Convoyeur à Bande & Capteurs Optiques"),
            lang.tr("Part Transport & Presence Detection", "Transport de Pièces & Détection de Présence"),
            lang.tr("Moves physical parts across the canvas. When a part enters a photo-eye beam, the corresponding discrete input (%I) is asserted.", "Déplace des pièces le long de la ligne. Dès qu'une pièce coupe le faisceau d'une cellule, l'entrée (%I) associée s'active."),
            "Tag bindings: Conveyor_Run -> %QX1.0, Part_Detect -> %IX1.0"
        ),
    ]
    .spacing(12)
    .into()
}

// ---------------------------------------------------------------------------
// 10. Shortcuts & Navigation
// ---------------------------------------------------------------------------

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
        assert!(topics.len() >= 20);
        for t in topics {
            assert!(!t.title_en.is_empty());
            assert!(!t.title_fr.is_empty());
            assert!(!t.keywords.is_empty());
        }
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
