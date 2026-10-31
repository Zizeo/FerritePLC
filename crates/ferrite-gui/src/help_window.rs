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
}
