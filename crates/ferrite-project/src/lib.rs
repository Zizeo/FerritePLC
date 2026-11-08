use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use ferrite_core::{
    CoreError, ExecutablePou, FbInstance, IecType, IecValue, LanguageKind, StandardFbKind,
    TagEntry, TagTable,
};
use ferrite_ir::{DiagnosticSeverity, ValidationDiagnostic};
use ferrite_lang_fbd::{FbdBlock, FbdBlockKind, FbdError, FbdInputSource, FbdProgram};
use ferrite_lang_ld::{CoilKind, ContactKind, LdCoil, LdElement, LdError, LdProgram, LdRung};
use ferrite_lang_sfc::{
    ActionQualifier, ConvergenceKind, DivergenceKind, SfcAction, SfcError, SfcProgram, SfcStep,
    SfcTransition, StepKind,
};
use ferrite_lang_st::{StError, StProgram};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MANIFEST_FILE_NAME: &str = "FerriteProject.fplc";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("I/O error on '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("TOML deserialize error in '{path}': {message}")]
    TomlDe { path: PathBuf, message: String },

    #[error("TOML serialize error: {0}")]
    TomlSer(String),

    #[error("Unsupported project schema version {0}")]
    UnsupportedSchema(u32),

    #[error("Duplicate POU '{0}'")]
    DuplicatePou(String),

    #[error("Core error: {0}")]
    Core(#[from] CoreError),

    #[error("Ladder error: {0}")]
    Ld(#[from] LdError),

    #[error("Grafcet/SFC error: {0}")]
    Sfc(#[from] SfcError),

    #[error("Structured Text error: {0}")]
    St(#[from] StError),

    #[error("Function Block Diagram error: {0}")]
    Fbd(#[from] FbdError),

    #[error("Project validation failed with {0} error(s)")]
    ValidationFailed(usize),

    #[error("PLCopen XML error: {0}")]
    Xml(String),
}

pub mod plcopen;
pub use plcopen::PlcOpenTc6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Cyclic,
    Continuous,
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskConfig {
    pub name: String,
    pub kind: TaskKind,
    pub priority: u32,
    pub interval_ms: u64,
    #[serde(default)]
    pub trigger_symbol: Option<String>,
    pub pou_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PouManifestEntry {
    pub name: String,
    pub language: LanguageKind,
    /// Relative path inside the project folder, e.g., `"pous/main_safety.ld.json"`
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub scan_cycle_ms: u64,
    pub watchdog_us: u64,
    pub tasks: Vec<TaskConfig>,
    pub pous: Vec<PouManifestEntry>,
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

impl ProjectManifest {
    pub fn migrate_if_needed(&mut self) -> Result<(), ProjectError> {
        if self.schema_version == 0 {
            self.schema_version = CURRENT_SCHEMA_VERSION;
        }
        if self.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedSchema(self.schema_version));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GlobalTagsFile {
    #[serde(default)]
    pub tags: Vec<TagEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FbInstancesFile {
    #[serde(default)]
    pub instances: Vec<FbInstance>,
}

/// Comparison operators for scenario assertions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssertOp {
    #[default]
    #[serde(rename = "==", alias = "=", alias = "eq", alias = "Equal")]
    Equal,
    #[serde(rename = "!=", alias = "ne", alias = "NotEqual")]
    NotEqual,
    #[serde(rename = "<", alias = "lt", alias = "LessThan")]
    LessThan,
    #[serde(rename = "<=", alias = "le", alias = "LessEqual")]
    LessEqual,
    #[serde(rename = ">", alias = "gt", alias = "GreaterThan")]
    GreaterThan,
    #[serde(rename = ">=", alias = "ge", alias = "GreaterEqual")]
    GreaterEqual,
    #[serde(rename = "in_range", alias = "range", alias = "between")]
    InRange,
}

impl std::fmt::Display for AssertOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Equal => write!(f, "=="),
            Self::NotEqual => write!(f, "!="),
            Self::LessThan => write!(f, "<"),
            Self::LessEqual => write!(f, "<="),
            Self::GreaterThan => write!(f, ">"),
            Self::GreaterEqual => write!(f, ">="),
            Self::InRange => write!(f, "in_range"),
        }
    }
}

/// Structured assertion targeting shared FB instances or SFC step states in simulation scenarios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioAssert {
    /// Shared FB instance name (e.g. "Timer_1") or composite path (e.g. "Timer_1.CV")
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fb: String,
    /// Field or pin name on the FB (e.g. "CV", "Q", "ET")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Comparison operator: ==, !=, <, <=, >, >=, in_range (defaults to ==)
    #[serde(default)]
    pub op: AssertOp,
    /// Expected value for binary comparison operators
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<IecValue>,
    /// Range-gate minimum value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<IecValue>,
    /// Range-gate maximum value
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<IecValue>,
    /// Optional SFC step assert target (e.g. "batch_seq.X0")
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "step")]
    pub sfc_step: Option<String>,
}

impl ScenarioAssert {
    /// Resolves the FB instance name and pin/field name.
    /// Handles both `fb = "Timer_1", field = "CV"` and composite `fb = "Timer_1.CV"`.
    pub fn target_fb_and_field(&self) -> Option<(&str, &str)> {
        if self.fb.is_empty() {
            return None;
        }
        if let Some((fb, pin)) = self.fb.split_once('.') {
            let field = self.field.as_deref().unwrap_or(pin);
            Some((fb, field))
        } else if let Some(field) = self.field.as_deref() {
            Some((&self.fb, field))
        } else {
            None
        }
    }
}

fn deserialize_scenario_asserts<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<ScenarioAssert>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(ScenarioAssert),
        Many(Vec<ScenarioAssert>),
    }

    match Option::<OneOrMany>::deserialize(deserializer)? {
        Some(OneOrMany::One(a)) => Ok(vec![a]),
        Some(OneOrMany::Many(vec)) => Ok(vec),
        None => Ok(Vec::new()),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SimStep {
    pub scan: u64,
    #[serde(default)]
    pub description: String,
    /// Map of symbol or direct input address (`%IX0.0`, `Sensor_1`) to value before scan
    #[serde(default)]
    pub set: HashMap<String, IecValue>,
    /// Map of symbol to expected value after scan
    #[serde(default)]
    pub expect: HashMap<String, IecValue>,
    /// Structured FB assertions and range-gates
    #[serde(
        default,
        deserialize_with = "deserialize_scenario_asserts",
        alias = "assert",
        alias = "assertions",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub asserts: Vec<ScenarioAssert>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SimScenario {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub total_scans: u64,
    #[serde(default)]
    pub steps: Vec<SimStep>,
    /// Scenario-level assertions and range-gates
    #[serde(
        default,
        deserialize_with = "deserialize_scenario_asserts",
        alias = "assert",
        alias = "assertions",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub asserts: Vec<ScenarioAssert>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScenariosFile {
    #[serde(default)]
    pub scenarios: Vec<SimScenario>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PouSource {
    Ld(LdProgram),
    Sfc(SfcProgram),
    St(StProgram),
    Fbd(FbdProgram),
}

impl PouSource {
    pub fn name(&self) -> &str {
        match self {
            Self::Ld(p) => &p.name,
            Self::Sfc(p) => &p.name,
            Self::St(p) => &p.name,
            Self::Fbd(p) => &p.name,
        }
    }

    pub fn language(&self) -> LanguageKind {
        match self {
            Self::Ld(_) => LanguageKind::Ld,
            Self::Sfc(_) => LanguageKind::Sfc,
            Self::St(_) => LanguageKind::St,
            Self::Fbd(_) => LanguageKind::Fbd,
        }
    }
}

/// Self-contained FerritePLC project representation.
#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub root_dir: PathBuf,
    pub manifest: ProjectManifest,
    pub global_tags: GlobalTagsFile,
    pub fb_instances: FbInstancesFile,
    pub pous: HashMap<String, PouSource>,
    pub scenarios: ScenariosFile,
}

impl Project {
    /// Atomically writes `content` to `dest` via a temporary file in the same directory + rename.
    pub fn atomic_write_str(dest: &Path, content: &str) -> Result<(), ProjectError> {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|source| ProjectError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let file_name = dest
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ferrite.tmp");
        let tmp_path = dest.with_file_name(format!(".{file_name}.tmp"));

        let write_and_sync = || -> std::io::Result<()> {
            let mut file = fs::File::create(&tmp_path)?;
            use std::io::Write;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            drop(file);
            Ok(())
        };

        if let Err(source) = write_and_sync() {
            let _ = fs::remove_file(&tmp_path);
            return Err(ProjectError::Io {
                path: tmp_path,
                source,
            });
        }

        if let Err(source) = fs::rename(&tmp_path, dest) {
            let _ = fs::remove_file(&tmp_path);
            return Err(ProjectError::Io {
                path: dest.to_path_buf(),
                source,
            });
        }
        Ok(())
    }

    /// Scaffolds a complete multi-language project on disk showcasing all FerritePLC features:
    /// - Ladder (`main_safety.ld.json`): NO, NC, Rising Edge (`|P|`), Falling Edge (`|N|`) contacts + Normal, Set (`S`), Reset (`R`) coils
    /// - Master Grafcet (`batch_seq.sfc.json`): AND Divergence (`===`), AND Convergence (`===`), OR Divergence (`---`), OR Convergence (`---`), Cross-SFC Transition Receptivity (`cip_clean.X2.state`), and Cross-SFC Step Set/Reset (`S`/`R`)
    /// - Subordinate Grafcet (`cip_clean.sfc.json`): Cross-SFC Transition Receptivity (`batch_seq.X6.state`) and Clean-In-Place sequence
    /// - Structured Text (`math_pid.st`): Process simulation physics, timers, and cross-SFC state inspection
    /// - Function Block Diagram (`pump_ctrl.fbd.json`): Multi-block signal flow network (`AND`, `OR`, `GT`, `GE`)
    pub fn scaffold(target_dir: &Path, project_name: &str) -> Result<Self, ProjectError> {
        let tags = vec![
            TagEntry::new(
                "EStop_OK",
                IecType::Bool,
                Some("%IX0.0".parse()?),
                IecValue::Bool(true),
                "Normally-closed emergency stop safety input",
            ),
            TagEntry::new(
                "Start_Button",
                IecType::Bool,
                Some("%IX0.1".parse()?),
                IecValue::Bool(false),
                "Operator cycle start pushbutton (rising edge |P| trigger)",
            ),
            TagEntry::new(
                "Stop_Button",
                IecType::Bool,
                Some("%IX0.2".parse()?),
                IecValue::Bool(true),
                "Operator cycle stop pushbutton (falling edge |N| release trigger)",
            ),
            TagEntry::new(
                "Guard_Door_Closed",
                IecType::Bool,
                Some("%IX0.3".parse()?),
                IecValue::Bool(true),
                "Safety guard door interlock switch",
            ),
            TagEntry::new(
                "Fast_Recipe_Mode",
                IecType::Bool,
                Some("%IX0.4".parse()?),
                IecValue::Bool(true),
                "Selector switch: TRUE = Fast Mix branch (OR Div), FALSE = Slow Cure branch",
            ),
            TagEntry::new(
                "Permissive_OK",
                IecType::Bool,
                Some("%MX0.0".parse()?),
                IecValue::Bool(false),
                "Master safety interlock output from Ladder -> Grafcet & FBD",
            ),
            TagEntry::new(
                "Start_Pulse",
                IecType::Bool,
                Some("%MX0.1".parse()?),
                IecValue::Bool(false),
                "One-scan rising-edge start pulse from Ladder |P| contact",
            ),
            TagEntry::new(
                "Cycle_Latched",
                IecType::Bool,
                Some("%MX0.2".parse()?),
                IecValue::Bool(false),
                "Set/Reset (S/R) latched run status from Ladder",
            ),
            TagEntry::new(
                "Reagent_Ready",
                IecType::Bool,
                Some("%MX0.3".parse()?),
                IecValue::Bool(false),
                "Reagent dosing complete flag for AND convergence",
            ),
            TagEntry::new(
                "Mix_Done",
                IecType::Bool,
                Some("%MX0.4".parse()?),
                IecValue::Bool(false),
                "Mixing complete flag for OR convergence",
            ),
            TagEntry::new(
                "Fill_Valve",
                IecType::Bool,
                Some("%QX0.0".parse()?),
                IecValue::Bool(false),
                "Water inlet valve driven by parallel branch Step 1",
            ),
            TagEntry::new(
                "Dosing_Valve",
                IecType::Bool,
                Some("%QX0.1".parse()?),
                IecValue::Bool(false),
                "Chemical reagent dosing valve driven by parallel branch Step 2",
            ),
            TagEntry::new(
                "Agitator_Fast",
                IecType::Bool,
                Some("%QX0.2".parse()?),
                IecValue::Bool(false),
                "High-speed mixer motor driven by OR branch Step 4",
            ),
            TagEntry::new(
                "Agitator_Slow",
                IecType::Bool,
                Some("%QX0.3".parse()?),
                IecValue::Bool(false),
                "Low-speed curing mixer driven by OR branch Step 5",
            ),
            TagEntry::new(
                "Heater_On",
                IecType::Bool,
                Some("%QX0.4".parse()?),
                IecValue::Bool(false),
                "Tank jacket heater contactor",
            ),
            TagEntry::new(
                "Drain_Valve",
                IecType::Bool,
                Some("%QX0.5".parse()?),
                IecValue::Bool(false),
                "Tank bottom drain valve",
            ),
            TagEntry::new(
                "Pump_Active",
                IecType::Bool,
                Some("%QX0.6".parse()?),
                IecValue::Bool(false),
                "Main circulation pump output from FBD",
            ),
            TagEntry::new(
                "CIP_Pump",
                IecType::Bool,
                Some("%QX0.7".parse()?),
                IecValue::Bool(false),
                "Clean-In-Place pump driven by subordinate cip_clean SFC",
            ),
            TagEntry::new(
                "Sanitizer_Valve",
                IecType::Bool,
                Some("%QX1.0".parse()?),
                IecValue::Bool(false),
                "Sanitizer spray valve driven by cip_clean SFC Step 1",
            ),
            TagEntry::new(
                "Main_Header_Active",
                IecType::Bool,
                Some("%QX1.1".parse()?),
                IecValue::Bool(false),
                "Combined pump header status from FBD OR block",
            ),
            TagEntry::new(
                "High_Temp_Alarm",
                IecType::Bool,
                Some("%QX1.2".parse()?),
                IecValue::Bool(false),
                "High temperature alarm from FBD GT comparator -> breaks Ladder NC interlock",
            ),
            TagEntry::new(
                "Overpressure_Fault",
                IecType::Bool,
                Some("%QX1.3".parse()?),
                IecValue::Bool(false),
                "Tank overfill/overpressure fault from FBD GE comparator",
            ),
            TagEntry::new(
                "Tank_Level",
                IecType::Int,
                Some("%MW10".parse()?),
                IecValue::Int(0),
                "Tank level percentage (0..100) computed in ST",
            ),
            TagEntry::new(
                "Batch_Count",
                IecType::Int,
                Some("%MW12".parse()?),
                IecValue::Int(0),
                "Completed batches counter",
            ),
            TagEntry::new(
                "Sanitize_Count",
                IecType::Int,
                Some("%MW14".parse()?),
                IecValue::Int(0),
                "Completed Clean-In-Place (CIP) cycles counter",
            ),
            TagEntry::new(
                "Tank_Temp",
                IecType::Real,
                None,
                IecValue::Real(25.0),
                "Process fluid temperature in Celsius",
            ),
            TagEntry::new(
                "Temp_Setpoint",
                IecType::Real,
                None,
                IecValue::Real(60.0),
                "Target temperature setpoint for P/PI/PID corrector",
            ),
            TagEntry::new(
                "PID_Output",
                IecType::Real,
                None,
                IecValue::Real(0.0),
                "Control output from P/PI/PID corrector (0..100%)",
            ),
        ];

        let fb_instances = vec![
            FbInstance::new("Fill_Timer", StandardFbKind::Ton),
            FbInstance::new("Alarm_Debounce", StandardFbKind::Ton),
            FbInstance::new("Temp_PID", StandardFbKind::PidMixed),
        ];

        // 1. Ladder Diagram POU: main_safety.ld.json
        // Demonstrates NO, NC, Rising Edge (|P|), Falling Edge (|N|) contacts + Normal, Set (S), Reset (R) coils
        let ld_pou = LdProgram {
            name: "main_safety".to_string(),
            comment: "Safety interlocks + Rising/Falling Edge Start/Stop latching".to_string(),
            rungs: vec![
                LdRung {
                    id: "Rung_Safety".to_string(),
                    comment: "Master safety permissive: EStop_OK & Guard_Door_Closed & !High_Temp_Alarm & !Overpressure_Fault".to_string(),
                    elements: vec![
                        LdElement::Contact {
                            symbol: "EStop_OK".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                        LdElement::Contact {
                            symbol: "Guard_Door_Closed".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                        LdElement::Contact {
                            symbol: "High_Temp_Alarm".to_string(),
                            kind: ContactKind::NormallyClosed,
                        },
                        LdElement::Contact {
                            symbol: "Overpressure_Fault".to_string(),
                            kind: ContactKind::NormallyClosed,
                        },
                    ],
                    coils: vec![LdCoil {
                        symbol: "Permissive_OK".to_string(),
                        kind: CoilKind::Normal,
                    }],
                },
                LdRung {
                    id: "Rung_Start_RisingEdge".to_string(),
                    comment: "Rising edge |P| on Start_Button generates 1-scan Start_Pulse and latches Cycle_Latched (S)".to_string(),
                    elements: vec![
                        LdElement::Contact {
                            symbol: "Permissive_OK".to_string(),
                            kind: ContactKind::NormallyOpen,
                        },
                        LdElement::Contact {
                            symbol: "Start_Button".to_string(),
                            kind: ContactKind::RisingEdge,
                        },
                    ],
                    coils: vec![
                        LdCoil {
                            symbol: "Start_Pulse".to_string(),
                            kind: CoilKind::Normal,
                        },
                        LdCoil {
                            symbol: "Cycle_Latched".to_string(),
                            kind: CoilKind::Set,
                        },
                    ],
                },
                LdRung {
                    id: "Rung_Stop_FallingEdge".to_string(),
                    comment: "Falling edge |N| on Stop_Button release unlatches Cycle_Latched (R)".to_string(),
                    elements: vec![LdElement::Contact {
                        symbol: "Stop_Button".to_string(),
                        kind: ContactKind::FallingEdge,
                    }],
                    coils: vec![LdCoil {
                        symbol: "Cycle_Latched".to_string(),
                        kind: CoilKind::Reset,
                    }],
                },
            ],
        };

        // 2. Master Grafcet (SFC) POU: batch_seq.sfc.json
        // Demonstrates AND Divergence (===), AND Convergence (===), OR Divergence (---), OR Convergence (---),
        // Cross-SFC Step Set/Reset (S/R), and Cross-SFC Active Step Transition Receptivity (cip_clean.X2.state)
        let sfc_pou = SfcProgram {
            name: "batch_seq".to_string(),
            comment: "Master batch sequence with AND/OR parallel branches & cross-SFC CIP synchronization".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_Idle".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::R,
                            target: "cip_clean.X2.state".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::S,
                            target: "cip_clean.X0.state".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Branch_Fill_Water".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Fill_Valve".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::P,
                            target: "Batch_Count := Batch_Count + 1;".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Branch_Dose_Reagent".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Dosing_Valve".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::L,
                            target: "Agitator_Fast".to_string(),
                            duration: Some("T#150ms".to_string()),
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "3".to_string(),
                    name: "Step_Recipe_Select".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "4".to_string(),
                    name: "Alt_Fast_Mix".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Agitator_Fast".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Heater_On".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "5".to_string(),
                    name: "Alt_Slow_Cure".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Heater_On".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::D,
                            target: "Agitator_Slow".to_string(),
                            duration: Some("T#100ms".to_string()),
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "6".to_string(),
                    name: "Step_Wait_CIP".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::N,
                        target: "Drain_Valve".to_string(),
                        duration: None,
                    }],
                    ..Default::default()
                },
            ],
            transitions: vec![
                SfcTransition {
                    id: "T1_AndDiv".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string(), "2".to_string()],
                    receptivity: "Permissive_OK AND (Start_Pulse OR Start_Button)".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T2_AndConv".to_string(),
                    from_steps: vec!["1".to_string(), "2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Tank_Level >= 50 AND Reagent_Ready".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T3_Or_Fast".to_string(),
                    from_steps: vec!["3".to_string()],
                    to_steps: vec!["4".to_string()],
                    receptivity: "Fast_Recipe_Mode AND Permissive_OK".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::Or,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T4_Or_Slow".to_string(),
                    from_steps: vec!["3".to_string()],
                    to_steps: vec!["5".to_string()],
                    receptivity: "NOT Fast_Recipe_Mode AND Permissive_OK".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::Or,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T5a_Merge_Fast".to_string(),
                    from_steps: vec!["4".to_string()],
                    to_steps: vec!["6".to_string()],
                    receptivity: "Mix_Done".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T5b_Merge_Slow".to_string(),
                    from_steps: vec!["5".to_string()],
                    to_steps: vec!["6".to_string()],
                    receptivity: "Tank_Temp >= 60.0".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T6_SubSfc_Done".to_string(),
                    from_steps: vec!["6".to_string()],
                    to_steps: vec!["0".to_string()],
                    receptivity: "cip_clean.X2.state OR NOT Permissive_OK".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
            ],
        };

        // 3. Subordinate Grafcet (SFC) POU: cip_clean.sfc.json
        // Triggered automatically when `batch_seq.X6.state` becomes active!
        let cip_sfc_pou = SfcProgram {
            name: "cip_clean".to_string(),
            comment: "Subordinate Clean-In-Place (CIP) Grafcet controlled by batch_seq.X6.state"
                .to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "CIP_Standby".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "CIP_Rinse_Active".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "CIP_Pump".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Sanitizer_Valve".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "CIP_Complete".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::P,
                        target: "Sanitize_Count := Sanitize_Count + 1;".to_string(),
                        duration: None,
                    }],
                    ..Default::default()
                },
            ],
            transitions: vec![
                SfcTransition {
                    id: "T_CIP_Start".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string()],
                    receptivity: "batch_seq.X6.state AND Permissive_OK".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T_CIP_Done".to_string(),
                    from_steps: vec!["1".to_string()],
                    to_steps: vec!["2".to_string()],
                    receptivity: "Tank_Level = 0".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T_CIP_Reset".to_string(),
                    from_steps: vec!["2".to_string()],
                    to_steps: vec!["0".to_string()],
                    receptivity: "batch_seq.X0.state".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
            ],
        };

        // 4. Structured Text POU: math_pid.st
        let st_source = r#"PROGRAM math_pid
    (* Update Tank_Level while Fill_Valve is open in Grafcet Step 1 *)
    IF Fill_Valve AND Permissive_OK THEN
        Tank_Level := Tank_Level + 25;
    END_IF;

    (* Mark reagent ready when parallel branch Step 2 opens Dosing_Valve *)
    IF Dosing_Valve AND Permissive_OK THEN
        Reagent_Ready := TRUE;
    END_IF;

    (* Heat and mix batch during OR-branch Step 4 or Step 5 *)
    IF Heater_On AND Permissive_OK THEN
        Tank_Temp := Tank_Temp + 20.0;
        Mix_Done := TRUE;
    END_IF;

    (* Drain tank and reset flags during Step 6 / CIP Rinse *)
    IF Drain_Valve OR cip_clean.X1.state THEN
        Tank_Level := 0;
        Reagent_Ready := FALSE;
        Mix_Done := FALSE;
        Tank_Temp := 25.0;
    END_IF;

    Fill_Timer(IN := batch_seq.X1.state, PT := T#200ms);
END_PROGRAM
"#;
        let st_pou = StProgram::parse("math_pid", st_source)?;

        // 5. Function Block Diagram POU: pump_ctrl.fbd.json
        let mut pump_and_in = HashMap::new();
        pump_and_in.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "Permissive_OK".to_string(),
            },
        );
        pump_and_in.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "Fill_Valve".to_string(),
            },
        );
        let mut pump_and_out = HashMap::new();
        pump_and_out.insert("OUT".to_string(), "Pump_Active".to_string());

        let mut header_or_in = HashMap::new();
        header_or_in.insert(
            "IN1".to_string(),
            FbdInputSource::Link {
                from_block: "Blk_Pump_Gate".to_string(),
                from_pin: "OUT".to_string(),
            },
        );
        header_or_in.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "CIP_Pump".to_string(),
            },
        );
        let mut header_or_out = HashMap::new();
        header_or_out.insert("OUT".to_string(), "Main_Header_Active".to_string());

        let mut temp_gt_in = HashMap::new();
        temp_gt_in.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "Tank_Temp".to_string(),
            },
        );
        temp_gt_in.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "80.0".to_string(),
            },
        );
        let mut temp_gt_out = HashMap::new();
        temp_gt_out.insert("OUT".to_string(), "High_Temp_Alarm".to_string());

        let mut lvl_ge_in = HashMap::new();
        lvl_ge_in.insert(
            "IN1".to_string(),
            FbdInputSource::Value {
                expr: "Tank_Level".to_string(),
            },
        );
        lvl_ge_in.insert(
            "IN2".to_string(),
            FbdInputSource::Value {
                expr: "95".to_string(),
            },
        );
        let mut lvl_ge_out = HashMap::new();
        lvl_ge_out.insert("OUT".to_string(), "Overpressure_Fault".to_string());

        let fbd_pou = FbdProgram {
            name: "pump_ctrl".to_string(),
            comment: "Pump activation, CIP header OR gate, and temperature/level comparators"
                .to_string(),
            composites: vec![],
            blocks: vec![
                FbdBlock {
                    id: "Blk_Pump_Gate".to_string(),
                    kind: FbdBlockKind::And,
                    inputs: pump_and_in,
                    outputs: pump_and_out,
                },
                FbdBlock {
                    id: "Blk_CIP_Header".to_string(),
                    kind: FbdBlockKind::Or,
                    inputs: header_or_in,
                    outputs: header_or_out,
                },
                FbdBlock {
                    id: "Blk_Temp_Comparator".to_string(),
                    kind: FbdBlockKind::Gt,
                    inputs: temp_gt_in,
                    outputs: temp_gt_out,
                },
                FbdBlock {
                    id: "Blk_Level_Guard".to_string(),
                    kind: FbdBlockKind::Ge,
                    inputs: lvl_ge_in,
                    outputs: lvl_ge_out,
                },
            ],
        };

        let manifest = ProjectManifest {
            schema_version: CURRENT_SCHEMA_VERSION,
            name: project_name.to_string(),
            version: "0.2.0".to_string(),
            scan_cycle_ms: 50,
            watchdog_us: 10_000,
            tasks: vec![TaskConfig {
                name: "MainTask".to_string(),
                kind: TaskKind::Cyclic,
                priority: 1,
                interval_ms: 50,
                trigger_symbol: None,
                pou_order: vec![
                    "main_safety".to_string(),
                    "batch_seq".to_string(),
                    "cip_clean".to_string(),
                    "math_pid".to_string(),
                    "pump_ctrl".to_string(),
                ],
            }],
            pous: vec![
                PouManifestEntry {
                    name: "main_safety".to_string(),
                    language: LanguageKind::Ld,
                    path: "pous/main_safety.ld.json".to_string(),
                },
                PouManifestEntry {
                    name: "batch_seq".to_string(),
                    language: LanguageKind::Sfc,
                    path: "pous/batch_seq.sfc.json".to_string(),
                },
                PouManifestEntry {
                    name: "cip_clean".to_string(),
                    language: LanguageKind::Sfc,
                    path: "pous/cip_clean.sfc.json".to_string(),
                },
                PouManifestEntry {
                    name: "math_pid".to_string(),
                    language: LanguageKind::St,
                    path: "pous/math_pid.st".to_string(),
                },
                PouManifestEntry {
                    name: "pump_ctrl".to_string(),
                    language: LanguageKind::Fbd,
                    path: "pous/pump_ctrl.fbd.json".to_string(),
                },
            ],
        };

        let mut step1_set = HashMap::new();
        step1_set.insert("EStop_OK".to_string(), IecValue::Bool(true));
        step1_set.insert("Guard_Door_Closed".to_string(), IecValue::Bool(true));
        step1_set.insert("Stop_Button".to_string(), IecValue::Bool(true));
        step1_set.insert("Fast_Recipe_Mode".to_string(), IecValue::Bool(true));
        let mut step1_expect = HashMap::new();
        step1_expect.insert("Permissive_OK".to_string(), IecValue::Bool(true));
        step1_expect.insert("batch_seq.X0.state".to_string(), IecValue::Bool(true));
        step1_expect.insert("cip_clean.X0.state".to_string(), IecValue::Bool(true));

        let mut step2_set = HashMap::new();
        step2_set.insert("Start_Button".to_string(), IecValue::Bool(true));
        let mut step2_expect = HashMap::new();
        step2_expect.insert("Start_Pulse".to_string(), IecValue::Bool(true));
        step2_expect.insert("Cycle_Latched".to_string(), IecValue::Bool(true));
        step2_expect.insert("batch_seq.X1.state".to_string(), IecValue::Bool(true));
        step2_expect.insert("batch_seq.X2.state".to_string(), IecValue::Bool(true));
        step2_expect.insert("Fill_Valve".to_string(), IecValue::Bool(true));
        step2_expect.insert("Dosing_Valve".to_string(), IecValue::Bool(true));
        step2_expect.insert("Pump_Active".to_string(), IecValue::Bool(true));
        step2_expect.insert("Main_Header_Active".to_string(), IecValue::Bool(true));
        step2_expect.insert("Tank_Level".to_string(), IecValue::Int(25));
        step2_expect.insert("Reagent_Ready".to_string(), IecValue::Bool(true));
        step2_expect.insert("Batch_Count".to_string(), IecValue::Int(1));

        let mut step3_set = HashMap::new();
        step3_set.insert("Start_Button".to_string(), IecValue::Bool(false));
        let mut step3_expect = HashMap::new();
        step3_expect.insert("Start_Pulse".to_string(), IecValue::Bool(false));
        step3_expect.insert("Cycle_Latched".to_string(), IecValue::Bool(true));
        step3_expect.insert("Tank_Level".to_string(), IecValue::Int(50));

        let mut step4_expect = HashMap::new();
        step4_expect.insert("batch_seq.X1.state".to_string(), IecValue::Bool(false));
        step4_expect.insert("batch_seq.X2.state".to_string(), IecValue::Bool(false));
        step4_expect.insert("batch_seq.X3.state".to_string(), IecValue::Bool(true));
        step4_expect.insert("Fill_Valve".to_string(), IecValue::Bool(false));

        let mut step5_expect = HashMap::new();
        step5_expect.insert("batch_seq.X4.state".to_string(), IecValue::Bool(true));
        step5_expect.insert("batch_seq.X5.state".to_string(), IecValue::Bool(false));
        step5_expect.insert("Heater_On".to_string(), IecValue::Bool(true));
        step5_expect.insert("Mix_Done".to_string(), IecValue::Bool(true));
        step5_expect.insert("Tank_Temp".to_string(), IecValue::Real(45.0));

        let mut step6_expect = HashMap::new();
        step6_expect.insert("batch_seq.X6.state".to_string(), IecValue::Bool(true));
        step6_expect.insert("cip_clean.X1.state".to_string(), IecValue::Bool(true));
        step6_expect.insert("CIP_Pump".to_string(), IecValue::Bool(true));
        step6_expect.insert("Sanitizer_Valve".to_string(), IecValue::Bool(true));
        step6_expect.insert("Main_Header_Active".to_string(), IecValue::Bool(true));
        step6_expect.insert("Tank_Level".to_string(), IecValue::Int(0));

        let mut step7_set = HashMap::new();
        step7_set.insert("Stop_Button".to_string(), IecValue::Bool(false));
        let mut step7_expect = HashMap::new();
        step7_expect.insert("Cycle_Latched".to_string(), IecValue::Bool(false));
        step7_expect.insert("cip_clean.X2.state".to_string(), IecValue::Bool(true));
        step7_expect.insert("Sanitize_Count".to_string(), IecValue::Int(1));

        let mut step8_expect = HashMap::new();
        step8_expect.insert("batch_seq.X0.state".to_string(), IecValue::Bool(true));
        step8_expect.insert("cip_clean.X0.state".to_string(), IecValue::Bool(true));
        step8_expect.insert("CIP_Pump".to_string(), IecValue::Bool(false));
        step8_expect.insert("Main_Header_Active".to_string(), IecValue::Bool(false));

        let scenarios = ScenariosFile {
            scenarios: vec![SimScenario {
                name: "complex_parallel_and_or_cross_sfc_cycle".to_string(),
                description: "Verifies LD edges (|P|/|N|) -> SFC AND/OR Divergence & Convergence -> Cross-SFC CIP sequence -> ST & FBD".to_string(),
                total_scans: 8,
                steps: vec![
                    SimStep {
                        scan: 1,
                        description: "Scan 1: Safety interlocks healthy -> Permissive_OK, batch_seq.X0 & cip_clean.X0 initialized".to_string(),
                        set: step1_set,
                        expect: step1_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 2,
                        description: "Scan 2: Start_Button rising edge |P| -> T1_AndDiv splits into parallel Step 1 (Water) & Step 2 (Reagent)".to_string(),
                        set: step2_set,
                        expect: step2_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 3,
                        description: "Scan 3: Start_Pulse clears after 1 scan while Cycle_Latched stays SET; Tank_Level reaches 50%".to_string(),
                        set: step3_set,
                        expect: step3_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 4,
                        description: "Scan 4: T2_AndConv synchronizes Step 1 & Step 2 into Step 3 (Recipe Selection)".to_string(),
                        set: HashMap::new(),
                        expect: step4_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 5,
                        description: "Scan 5: T3_Or_Fast (OR Divergence) selects Step 4 (Alt_Fast_Mix) based on Fast_Recipe_Mode".to_string(),
                        set: HashMap::new(),
                        expect: step5_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 6,
                        description: "Scan 6: T5a_Merge_Fast / T5b_Merge_Slow (OR Convergence) merges into Step 6 -> triggers subordinate cip_clean.X1 via cross-SFC transition".to_string(),
                        set: HashMap::new(),
                        expect: step6_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 7,
                        description: "Scan 7: cip_clean advances to X2 (CIP_Complete); Stop_Button falling edge |N| unlatches Cycle_Latched".to_string(),
                        set: step7_set,
                        expect: step7_expect,
                        asserts: Vec::new(),
                    },
                    SimStep {
                        scan: 8,
                        description: "Scan 8: batch_seq sees cip_clean.X2.state -> returns to X0 and resets cip_clean to X0 via S/R actions".to_string(),
                        set: HashMap::new(),
                        expect: step8_expect,
                        asserts: Vec::new(),
                    },
                ],
                asserts: Vec::new(),
            }],
        };

        let mut sfc_pou = sfc_pou;
        sfc_pou.assign_default_grid_positions();
        let mut cip_sfc_pou = cip_sfc_pou;
        cip_sfc_pou.assign_default_grid_positions();

        let mut pous = HashMap::new();
        pous.insert("main_safety".to_string(), PouSource::Ld(ld_pou));
        pous.insert("batch_seq".to_string(), PouSource::Sfc(sfc_pou));
        pous.insert("cip_clean".to_string(), PouSource::Sfc(cip_sfc_pou));
        pous.insert("math_pid".to_string(), PouSource::St(st_pou));
        pous.insert("pump_ctrl".to_string(), PouSource::Fbd(fbd_pou));

        let proj = Self {
            root_dir: target_dir.to_path_buf(),
            manifest,
            global_tags: GlobalTagsFile { tags },
            fb_instances: FbInstancesFile {
                instances: fb_instances,
            },
            pous,
            scenarios,
        };
        proj.save(target_dir)?;
        Ok(proj)
    }

    /// Loads a `Project` from a directory or `FerriteProject.fplc` file path.
    pub fn load(path: &Path) -> Result<Self, ProjectError> {
        let (root_dir, manifest_path) = if path.is_dir() {
            (path.to_path_buf(), path.join(MANIFEST_FILE_NAME))
        } else {
            let parent = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            (parent, path.to_path_buf())
        };

        let manifest_raw =
            fs::read_to_string(&manifest_path).map_err(|source| ProjectError::Io {
                path: manifest_path.clone(),
                source,
            })?;
        let mut manifest: ProjectManifest =
            toml::from_str(&manifest_raw).map_err(|e| ProjectError::TomlDe {
                path: manifest_path.clone(),
                message: e.to_string(),
            })?;
        manifest.migrate_if_needed()?;

        let tags_path = root_dir.join("symbols").join("global_tags.toml");
        let global_tags: GlobalTagsFile = if tags_path.exists() {
            let raw = fs::read_to_string(&tags_path).map_err(|source| ProjectError::Io {
                path: tags_path.clone(),
                source,
            })?;
            toml::from_str(&raw).map_err(|e| ProjectError::TomlDe {
                path: tags_path,
                message: e.to_string(),
            })?
        } else {
            GlobalTagsFile::default()
        };

        let fbs_path = root_dir.join("symbols").join("fb_instances.toml");
        let fb_instances: FbInstancesFile = if fbs_path.exists() {
            let raw = fs::read_to_string(&fbs_path).map_err(|source| ProjectError::Io {
                path: fbs_path.clone(),
                source,
            })?;
            toml::from_str(&raw).map_err(|e| ProjectError::TomlDe {
                path: fbs_path,
                message: e.to_string(),
            })?
        } else {
            FbInstancesFile::default()
        };

        let scenarios_path = root_dir.join("sim").join("scenarios.toml");
        let scenarios: ScenariosFile = if scenarios_path.exists() {
            let raw = fs::read_to_string(&scenarios_path).map_err(|source| ProjectError::Io {
                path: scenarios_path.clone(),
                source,
            })?;
            toml::from_str(&raw).map_err(|e| ProjectError::TomlDe {
                path: scenarios_path,
                message: e.to_string(),
            })?
        } else {
            ScenariosFile::default()
        };

        let canonical_root = root_dir.canonicalize().map_err(|source| ProjectError::Io {
            path: root_dir.clone(),
            source,
        })?;

        let mut pous = HashMap::new();
        for entry in &manifest.pous {
            let rel = Path::new(&entry.path);
            if rel.is_absolute()
                || rel.components().any(|c| {
                    matches!(
                        c,
                        std::path::Component::ParentDir
                            | std::path::Component::Prefix(_)
                            | std::path::Component::RootDir
                    )
                })
            {
                return Err(ProjectError::ValidationFailed(1));
            }

            let pou_path = root_dir.join(&entry.path);
            let canonical_pou = pou_path.canonicalize().map_err(|source| ProjectError::Io {
                path: pou_path.clone(),
                source,
            })?;
            if !canonical_pou.starts_with(&canonical_root) {
                return Err(ProjectError::ValidationFailed(1));
            }

            let raw = fs::read_to_string(&canonical_pou).map_err(|source| ProjectError::Io {
                path: canonical_pou,
                source,
            })?;
            let src = match entry.language {
                LanguageKind::Ld => PouSource::Ld(LdProgram::from_json(&raw)?),
                LanguageKind::Sfc => {
                    let mut sfc = SfcProgram::from_json(&raw)?;
                    if sfc.steps.iter().any(|s| s.grid_pos.is_none())
                        || sfc.transitions.iter().any(|t| t.grid_pos.is_none())
                    {
                        sfc.assign_default_grid_positions();
                    }
                    PouSource::Sfc(sfc)
                }
                LanguageKind::St => PouSource::St(StProgram::parse(&entry.name, raw)?),
                LanguageKind::Fbd => PouSource::Fbd(FbdProgram::from_json(&raw)?),
            };
            pous.insert(entry.name.clone(), src);
        }

        Ok(Self {
            root_dir,
            manifest,
            global_tags,
            fb_instances,
            pous,
            scenarios,
        })
    }

    /// Atomically saves only `FerriteProject.fplc`.
    pub fn save_manifest(&self, target_dir: &Path) -> Result<(), ProjectError> {
        let manifest_toml = toml::to_string_pretty(&self.manifest)
            .map_err(|e| ProjectError::TomlSer(e.to_string()))?;
        Self::atomic_write_str(&target_dir.join(MANIFEST_FILE_NAME), &manifest_toml)?;
        Ok(())
    }

    /// Atomically saves a single POU file on disk (`pous/<name>.<ext>`) without writing other POUs.
    pub fn save_pou(&self, target_dir: &Path, pou_name: &str) -> Result<PathBuf, ProjectError> {
        let entry = self
            .manifest
            .pous
            .iter()
            .find(|e| e.name == pou_name)
            .ok_or(ProjectError::ValidationFailed(1))?;
        let pou = self
            .pous
            .get(pou_name)
            .ok_or(ProjectError::ValidationFailed(1))?;

        let pou_lang = match pou {
            PouSource::Ld(_) => LanguageKind::Ld,
            PouSource::Sfc(_) => LanguageKind::Sfc,
            PouSource::St(_) => LanguageKind::St,
            PouSource::Fbd(_) => LanguageKind::Fbd,
        };
        if entry.language != pou_lang {
            return Err(ProjectError::ValidationFailed(1));
        }

        let pou_path = target_dir.join(&entry.path);
        let serialized = match pou {
            PouSource::Ld(p) => p.to_json_pretty()?,
            PouSource::Sfc(p) => p.to_json_pretty()?,
            PouSource::St(p) => p.source.clone(),
            PouSource::Fbd(p) => p.to_json_pretty()?,
        };
        Self::atomic_write_str(&pou_path, &serialized)?;
        Ok(pou_path)
    }

    /// Atomically saves the entire project (`FerriteProject.fplc`, `symbols/*`, `pous/*`, `sim/scenarios.toml`).
    pub fn save(&self, target_dir: &Path) -> Result<(), ProjectError> {
        self.save_manifest(target_dir)?;

        let tags_toml = toml::to_string_pretty(&self.global_tags)
            .map_err(|e| ProjectError::TomlSer(e.to_string()))?;
        Self::atomic_write_str(
            &target_dir.join("symbols").join("global_tags.toml"),
            &tags_toml,
        )?;

        let fbs_toml = toml::to_string_pretty(&self.fb_instances)
            .map_err(|e| ProjectError::TomlSer(e.to_string()))?;
        Self::atomic_write_str(
            &target_dir.join("symbols").join("fb_instances.toml"),
            &fbs_toml,
        )?;

        let sim_toml = toml::to_string_pretty(&self.scenarios)
            .map_err(|e| ProjectError::TomlSer(e.to_string()))?;
        Self::atomic_write_str(&target_dir.join("sim").join("scenarios.toml"), &sim_toml)?;

        for entry in &self.manifest.pous {
            self.save_pou(target_dir, &entry.name)?;
        }

        Ok(())
    }
}

fn validate_pou_name(name: &str) -> Result<&str, ProjectError> {
    let clean = name.trim();
    if clean.is_empty()
        || clean.contains(['/', '\\', ':', '.', '\0'])
        || !clean.chars().all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(ProjectError::ValidationFailed(1));
    }
    Ok(clean)
}

fn rewrite_qualified_symbol(text: &str, old_prefix: &str, new_prefix: &str) -> String {
    let old_needle = format!("{old_prefix}.");
    let new_replacement = format!("{new_prefix}.");
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;

    while let Some(rel_pos) = text[cursor..].find(&old_needle) {
        let match_start = cursor + rel_pos;
        let is_prev_ident = if match_start == 0 {
            false
        } else {
            text[..match_start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        };

        if !is_prev_ident {
            result.push_str(&text[cursor..match_start]);
            result.push_str(&new_replacement);
            cursor = match_start + old_needle.len();
        } else {
            result.push_str(&text[cursor..match_start + old_needle.len()]);
            cursor = match_start + old_needle.len();
        }
    }
    result.push_str(&text[cursor..]);
    result
}

fn rewrite_ld_element(elem: &mut LdElement, old_name: &str, new_name: &str) {
    match elem {
        LdElement::Contact { symbol, .. } => {
            *symbol = rewrite_qualified_symbol(symbol, old_name, new_name);
        }
        LdElement::Branch { paths } => {
            for path in paths {
                for el in path {
                    rewrite_ld_element(el, old_name, new_name);
                }
            }
        }
        LdElement::InlineFb {
            enable_pin,
            inputs,
            outputs,
            ..
        } => {
            if let Some(ep) = enable_pin {
                *ep = rewrite_qualified_symbol(ep, old_name, new_name);
            }
            for (_pin, sym) in inputs {
                *sym = rewrite_qualified_symbol(sym, old_name, new_name);
            }
            for (_pin, sym) in outputs {
                *sym = rewrite_qualified_symbol(sym, old_name, new_name);
            }
        }
        LdElement::Compare { left, right, .. } => {
            *left = rewrite_qualified_symbol(left, old_name, new_name);
            *right = rewrite_qualified_symbol(right, old_name, new_name);
        }
        LdElement::MathMove {
            input1,
            input2,
            dest,
            ..
        } => {
            *input1 = rewrite_qualified_symbol(input1, old_name, new_name);
            if let Some(i2) = input2 {
                *i2 = rewrite_qualified_symbol(i2, old_name, new_name);
            }
            *dest = rewrite_qualified_symbol(dest, old_name, new_name);
        }
    }
}

impl Project {
    /// Adds a new POU in `pous/` and registers it in the manifest and first task's execution order.
    pub fn add_pou(&mut self, name: &str, lang: LanguageKind) -> Result<PathBuf, ProjectError> {
        let clean_name = validate_pou_name(name)?;
        if self.pous.contains_key(clean_name) {
            return Err(ProjectError::DuplicatePou(clean_name.to_string()));
        }
        let rel_path = match lang {
            LanguageKind::Ld => format!("pous/{clean_name}.ld.json"),
            LanguageKind::Sfc => format!("pous/{clean_name}.sfc.json"),
            LanguageKind::St => format!("pous/{clean_name}.st"),
            LanguageKind::Fbd => format!("pous/{clean_name}.fbd.json"),
        };

        let pou_src = match lang {
            LanguageKind::Ld => PouSource::Ld(LdProgram {
                name: clean_name.to_string(),
                comment: format!("Ladder Diagram POU {clean_name}"),
                rungs: vec![],
            }),
            LanguageKind::Sfc => PouSource::Sfc(SfcProgram {
                name: clean_name.to_string(),
                comment: format!("Grafcet POU {clean_name}"),
                steps: vec![SfcStep {
                    id: "0".to_string(),
                    name: format!("{clean_name}_Init"),
                    kind: StepKind::Initial,
                    actions: vec![],
                    grid_pos: Some((60, 40)),
                    ..Default::default()
                }],
                transitions: vec![],
            }),
            LanguageKind::St => {
                let src =
                    format!("PROGRAM {clean_name}\n    (* Structured Text routine *)\nEND_PROGRAM\n");
                PouSource::St(StProgram::parse(clean_name, src)?)
            }
            LanguageKind::Fbd => PouSource::Fbd(FbdProgram {
                name: clean_name.to_string(),
                comment: format!("Function Block Diagram POU {clean_name}"),
                composites: vec![],
                blocks: vec![],
            }),
        };

        let root = self.root_dir.clone();
        let pou_path = root.join(&rel_path);
        if root.exists() {
            let serialized = match &pou_src {
                PouSource::Ld(p) => p.to_json_pretty()?,
                PouSource::Sfc(p) => p.to_json_pretty()?,
                PouSource::St(p) => p.source.clone(),
                PouSource::Fbd(p) => p.to_json_pretty()?,
            };
            Self::atomic_write_str(&pou_path, &serialized)?;
        }

        self.pous.insert(clean_name.to_string(), pou_src);
        self.manifest.pous.push(PouManifestEntry {
            name: clean_name.to_string(),
            language: lang,
            path: rel_path.clone(),
        });
        if let Some(first_task) = self.manifest.tasks.first_mut()
            && !first_task.pou_order.contains(&clean_name.to_string())
        {
            first_task.pou_order.push(clean_name.to_string());
        }

        if root.exists()
            && let Err(err) = self.save_manifest(&root)
        {
            let _ = fs::remove_file(&pou_path);
            self.pous.remove(clean_name);
            self.manifest.pous.retain(|p| p.name != clean_name);
            if let Some(first_task) = self.manifest.tasks.first_mut() {
                first_task.pou_order.retain(|p| p != clean_name);
            }
            return Err(err);
        }
        Ok(root.join(rel_path))
    }

    /// Renames a POU across manifest, pous map, task execution orders, and on-disk files.
    pub fn rename_pou(&mut self, old_name: &str, new_name: &str) -> Result<(), ProjectError> {
        let clean_new = validate_pou_name(new_name)?;
        if old_name == clean_new {
            return Ok(());
        }
        if self.pous.contains_key(clean_new) {
            return Err(ProjectError::DuplicatePou(clean_new.to_string()));
        }
        let mut pou_src = self
            .pous
            .remove(old_name)
            .ok_or(ProjectError::ValidationFailed(1))?;

        // Update the internal name within the POU data structure
        match &mut pou_src {
            PouSource::Ld(ld) => ld.name = clean_new.to_string(),
            PouSource::Sfc(sfc) => sfc.name = clean_new.to_string(),
            PouSource::St(st) => {
                st.name = clean_new.to_string();
                let old_prog = format!("PROGRAM {old_name}");
                let new_prog = format!("PROGRAM {clean_new}");
                if st.source.contains(&old_prog) {
                    st.source = st.source.replace(&old_prog, &new_prog);
                    *st = StProgram::parse(clean_new, &st.source)?;
                }
            }
            PouSource::Fbd(fbd) => fbd.name = clean_new.to_string(),
        }

        // Rewrite qualified references across all other POUs
        for pou in self.pous.values_mut() {
            match pou {
                PouSource::Ld(ld) => {
                    ld.comment = rewrite_qualified_symbol(&ld.comment, old_name, clean_new);
                    for rung in &mut ld.rungs {
                        rung.comment = rewrite_qualified_symbol(&rung.comment, old_name, clean_new);
                        for el in &mut rung.elements {
                            rewrite_ld_element(el, old_name, clean_new);
                        }
                        for coil in &mut rung.coils {
                            coil.symbol =
                                rewrite_qualified_symbol(&coil.symbol, old_name, clean_new);
                        }
                    }
                }
                PouSource::Sfc(sfc) => {
                    sfc.comment = rewrite_qualified_symbol(&sfc.comment, old_name, clean_new);
                    for step in &mut sfc.steps {
                        for action in &mut step.actions {
                            action.target =
                                rewrite_qualified_symbol(&action.target, old_name, clean_new);
                        }
                    }
                    for trans in &mut sfc.transitions {
                        trans.receptivity =
                            rewrite_qualified_symbol(&trans.receptivity, old_name, clean_new);
                    }
                }
                PouSource::Fbd(fbd) => {
                    fbd.comment = rewrite_qualified_symbol(&fbd.comment, old_name, clean_new);
                    for block in &mut fbd.blocks {
                        for input in block.inputs.values_mut() {
                            if let FbdInputSource::Value { expr } = input {
                                *expr = rewrite_qualified_symbol(expr, old_name, clean_new);
                            }
                        }
                        for target in block.outputs.values_mut() {
                            *target = rewrite_qualified_symbol(target, old_name, clean_new);
                        }
                    }
                    for comp in &mut fbd.composites {
                        for block in &mut comp.blocks {
                            for input in block.inputs.values_mut() {
                                if let FbdInputSource::Value { expr } = input {
                                    *expr = rewrite_qualified_symbol(expr, old_name, clean_new);
                                }
                            }
                            for target in block.outputs.values_mut() {
                                *target = rewrite_qualified_symbol(target, old_name, clean_new);
                            }
                        }
                    }
                }
                PouSource::St(st) => {
                    st.source = rewrite_qualified_symbol(&st.source, old_name, clean_new);
                    *st = StProgram::parse(&st.name, &st.source)?;
                }
            }
        }

        // Rewrite in scenarios
        for sc in &mut self.scenarios.scenarios {
            for a in &mut sc.asserts {
                if let Some(ref mut step) = a.sfc_step {
                    *step = rewrite_qualified_symbol(step, old_name, clean_new);
                }
            }
            for step in &mut sc.steps {
                for a in &mut step.asserts {
                    if let Some(ref mut step) = a.sfc_step {
                        *step = rewrite_qualified_symbol(step, old_name, clean_new);
                    }
                }
                step.set = std::mem::take(&mut step.set)
                    .into_iter()
                    .map(|(k, v)| (rewrite_qualified_symbol(&k, old_name, clean_new), v))
                    .collect();
                step.expect = std::mem::take(&mut step.expect)
                    .into_iter()
                    .map(|(k, v)| (rewrite_qualified_symbol(&k, old_name, clean_new), v))
                    .collect();
            }
        }

        // Rewrite trigger_symbol in tasks
        for task in &mut self.manifest.tasks {
            if let Some(ref mut sym) = task.trigger_symbol {
                *sym = rewrite_qualified_symbol(sym, old_name, clean_new);
            }
        }

        // Check if any old qualified references remain in any POU
        let needle = format!("{old_name}.");
        for pou in self.pous.values() {
            let has_ref = match pou {
                PouSource::Ld(ld) => {
                    serde_json::to_string(ld).map(|s| s.contains(&needle)).unwrap_or(false)
                }
                PouSource::Sfc(sfc) => {
                    serde_json::to_string(sfc).map(|s| s.contains(&needle)).unwrap_or(false)
                }
                PouSource::St(st) => st.source.contains(&needle),
                PouSource::Fbd(fbd) => {
                    serde_json::to_string(fbd).map(|s| s.contains(&needle)).unwrap_or(false)
                }
            };
            if has_ref {
                return Err(ProjectError::ValidationFailed(1));
            }
        }

        // Locate manifest entry
        let mut old_file_path = None;
        let mut new_file_path = None;
        if let Some(entry) = self.manifest.pous.iter_mut().find(|p| p.name == old_name) {
            entry.name = clean_new.to_string();
            let old_path_str = entry.path.clone();
            let ext = match entry.language {
                LanguageKind::Ld => "ld.json",
                LanguageKind::Sfc => "sfc.json",
                LanguageKind::St => "st",
                LanguageKind::Fbd => "fbd.json",
            };
            let new_path_str = format!("pous/{clean_new}.{ext}");
            entry.path = new_path_str.clone();

            if self.root_dir.exists() {
                old_file_path = Some(self.root_dir.join(old_path_str));
                new_file_path = Some(self.root_dir.join(new_path_str));
            }
        }

        // Rename or remove old on-disk file
        if let (Some(old_path), Some(new_path)) = (old_file_path, new_file_path)
            && old_path.exists()
        {
            if new_path.exists() && old_path != new_path {
                return Err(ProjectError::DuplicatePou(clean_new.to_string()));
            }
            let _ = std::fs::remove_file(&old_path);
        }

        // Update tasks pou_order
        for task in &mut self.manifest.tasks {
            for name in &mut task.pou_order {
                if name == old_name {
                    *name = clean_new.to_string();
                }
            }
        }

        self.pous.insert(clean_new.to_string(), pou_src);

        // Save entire project to disk if root_dir exists (persisting all updated cross-references)
        if self.root_dir.exists() {
            let root = self.root_dir.clone();
            self.save(&root)?;
        }
        Ok(())
    }

    /// Moves a POU up by one position in the manifest and updates task execution orders.
    pub fn move_pou_up(&mut self, name: &str) -> bool {
        let Some(idx) = self.manifest.pous.iter().position(|p| p.name == name) else {
            return false;
        };
        if idx == 0 {
            return false;
        }
        let prev_name = self.manifest.pous[idx - 1].name.clone();
        self.manifest.pous.swap(idx, idx - 1);

        // Update task pou_orders where both appear adjacently
        for task in &mut self.manifest.tasks {
            if let Some(pos_cur) = task.pou_order.iter().position(|p| p == name)
                && pos_cur > 0
                && task.pou_order[pos_cur - 1] == prev_name
            {
                task.pou_order.swap(pos_cur, pos_cur - 1);
            }
        }
        true
    }

    /// Moves a POU down by one position in the manifest and updates task execution orders.
    pub fn move_pou_down(&mut self, name: &str) -> bool {
        let Some(idx) = self.manifest.pous.iter().position(|p| p.name == name) else {
            return false;
        };
        if idx + 1 >= self.manifest.pous.len() {
            return false;
        }
        let next_name = self.manifest.pous[idx + 1].name.clone();
        self.manifest.pous.swap(idx, idx + 1);

        // Update task pou_orders where both appear adjacently
        for task in &mut self.manifest.tasks {
            if let Some(pos_cur) = task.pou_order.iter().position(|p| p == name)
                && pos_cur + 1 < task.pou_order.len()
                && task.pou_order[pos_cur + 1] == next_name
            {
                task.pou_order.swap(pos_cur, pos_cur + 1);
            }
        }
        true
    }

    /// Reorders a POU from `from_name`'s position to `to_name`'s position.
    pub fn reorder_pou(&mut self, from_name: &str, to_name: &str) -> bool {
        if from_name == to_name {
            return false;
        }
        let Some(from_idx) = self.manifest.pous.iter().position(|p| p.name == from_name) else {
            return false;
        };
        let Some(to_idx) = self.manifest.pous.iter().position(|p| p.name == to_name) else {
            return false;
        };

        let item = self.manifest.pous.remove(from_idx);
        self.manifest.pous.insert(to_idx, item);

        // Update tasks pou_order
        for task in &mut self.manifest.tasks {
            if let Some(t_from) = task.pou_order.iter().position(|p| p == from_name)
                && let Some(t_to) = task.pou_order.iter().position(|p| p == to_name)
            {
                let t_item = task.pou_order.remove(t_from);
                task.pou_order.insert(t_to, t_item);
            }
        }

        if self.root_dir.exists() {
            let root = self.root_dir.clone();
            if self.save_manifest(&root).is_err() {
                // Revert in-memory changes on save error
                let item = self.manifest.pous.remove(to_idx);
                self.manifest.pous.insert(from_idx, item);
                for task in &mut self.manifest.tasks {
                    if let Some(t_to) = task.pou_order.iter().position(|p| p == from_name)
                        && let Some(t_from) = task.pou_order.iter().position(|p| p == to_name)
                    {
                        let t_item = task.pou_order.remove(t_to);
                        task.pou_order.insert(t_from, t_item);
                    }
                }
                return false;
            }
        }

        true
    }

    /// Deletes a POU across manifest, pous map, task execution orders, and on-disk files.
    pub fn delete_pou(&mut self, name: &str) -> Result<(), ProjectError> {
        self.pous.remove(name);
        if let Some(pos) = self.manifest.pous.iter().position(|p| p.name == name) {
            let entry = self.manifest.pous.remove(pos);
            if self.root_dir.exists() {
                let file_path = self.root_dir.join(&entry.path);
                if file_path.exists() {
                    std::fs::remove_file(&file_path).map_err(|source| ProjectError::Io {
                        path: file_path,
                        source,
                    })?;
                }
            }
        }
        for task in &mut self.manifest.tasks {
            task.pou_order.retain(|p| p != name);
        }
        if self.root_dir.exists() {
            let root = self.root_dir.clone();
            self.save_manifest(&root)?;
        }
        Ok(())
    }

    /// Builds the unified `TagTable` (including all Grafcet `X<i>.state` & `X<i>.t` introspection symbols).
    pub fn build_tag_table(&self) -> Result<TagTable, ProjectError> {
        let mut table = TagTable::new();
        for tag in &self.global_tags.tags {
            table.register_tag(tag.clone())?;
        }
        for fb in &self.fb_instances.instances {
            table.register_fb(fb.clone());
        }
        for entry in &self.manifest.pous {
            if let Some(pou) = self.pous.get(&entry.name) {
                match pou {
                    PouSource::Sfc(sfc) => {
                        sfc.register_introspection_symbols(&mut table)?;
                    }
                    PouSource::Fbd(fbd) => {
                        for blk in &fbd.blocks {
                            if let FbdBlockKind::StandardFb { instance, fb_type } = &blk.kind {
                                if let Some(existing) = table.get_fb_mut(instance) {
                                    if let Some(k) = fb_type {
                                        existing.kind = *k;
                                    }
                                } else {
                                    table.register_fb(FbInstance::new(
                                        instance.clone(),
                                        fb_type.unwrap_or(StandardFbKind::Ton),
                                    ));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(table)
    }

    /// Validates all POUs, cross-language symbol references, types, and combinational loops.
    pub fn check(&self) -> Result<Vec<ValidationDiagnostic>, ProjectError> {
        let tags = self.build_tag_table()?;
        let mut diagnostics = Vec::new();

        for entry in &self.manifest.pous {
            let Some(pou) = self.pous.get(&entry.name) else {
                diagnostics.push(ValidationDiagnostic {
                    pou: entry.name.clone(),
                    language: entry.language,
                    severity: DiagnosticSeverity::Error,
                    message: format!("POU '{}' listed in manifest is missing", entry.name),
                });
                continue;
            };
            let res: Result<(), String> = match pou {
                PouSource::Ld(p) => p.validate(&tags).map_err(|e| e.to_string()),
                PouSource::Sfc(p) => p.compile(&tags).map(|_| ()).map_err(|e| e.to_string()),
                PouSource::St(p) => p.validate(&tags).map_err(|e| e.to_string()),
                PouSource::Fbd(p) => p.validate(&tags).map(|_| ()).map_err(|e| e.to_string()),
            };
            if let Err(msg) = res {
                diagnostics.push(ValidationDiagnostic {
                    pou: entry.name.clone(),
                    language: entry.language,
                    severity: DiagnosticSeverity::Error,
                    message: msg,
                });
            }
        }

        // Scenario assertions validation
        for sc in &self.scenarios.scenarios {
            let mut check_assert = |assert: &ScenarioAssert| {
                if let Some((fb_name, pin_name)) = assert.target_fb_and_field() {
                    if let Some(fb_inst) = tags.get_fb(fb_name) {
                        if !pin_name.is_empty() && fb_inst.read_pin(pin_name).is_err() {
                            diagnostics.push(ValidationDiagnostic {
                                pou: sc.name.clone(),
                                language: LanguageKind::St,
                                severity: DiagnosticSeverity::Error,
                                message: format!(
                                    "Scenario '{}': unknown pin/field '{}' for FB instance '{}'",
                                    sc.name, pin_name, fb_name
                                ),
                            });
                        }
                    } else {
                        diagnostics.push(ValidationDiagnostic {
                            pou: sc.name.clone(),
                            language: LanguageKind::St,
                            severity: DiagnosticSeverity::Error,
                            message: format!(
                                "Scenario '{}': unknown Function Block instance '{}'",
                                sc.name, fb_name
                            ),
                        });
                    }
                } else if !assert.fb.is_empty() && assert.sfc_step.is_none() {
                    diagnostics.push(ValidationDiagnostic {
                        pou: sc.name.clone(),
                        language: LanguageKind::St,
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Scenario '{}': assertion on FB '{}' missing field/pin",
                            sc.name, assert.fb
                        ),
                    });
                }

                if let Some(ref sfc_step) = assert.sfc_step
                    && let Some((pou_name, _)) = sfc_step.split_once('.')
                    && !self.pous.contains_key(pou_name)
                {
                    diagnostics.push(ValidationDiagnostic {
                        pou: sc.name.clone(),
                        language: LanguageKind::Sfc,
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Scenario '{}': SFC step assertion targets unknown POU '{}'",
                            sc.name, pou_name
                        ),
                    });
                }

                if assert.op == AssertOp::InRange && assert.min.is_none() && assert.max.is_none() {
                    diagnostics.push(ValidationDiagnostic {
                        pou: sc.name.clone(),
                        language: LanguageKind::St,
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "Scenario '{}': in_range assertion requires min or max",
                            sc.name
                        ),
                    });
                }
            };

            for a in &sc.asserts {
                check_assert(a);
            }
            for step in &sc.steps {
                for a in &step.asserts {
                    check_assert(a);
                }
            }
        }

        Ok(diagnostics)
    }
}

pub struct CompiledTask {
    pub name: String,
    pub kind: TaskKind,
    pub priority: u32,
    pub interval_ms: u64,
    pub trigger_symbol: Option<String>,
    pub pous: Vec<Box<dyn ExecutablePou>>,
}

impl Project {
    /// Compiles all POUs grouped into their configured scheduled tasks.
    pub fn compile_tasks(&self) -> Result<(TagTable, Vec<CompiledTask>), ProjectError> {
        let tags = self.build_tag_table()?;
        let mut tasks_sorted = self.manifest.tasks.clone();
        tasks_sorted.sort_by_key(|t| t.priority);

        let mut compiled_tasks = Vec::new();
        for task in &tasks_sorted {
            let mut pous = Vec::new();
            for pou_name in &task.pou_order {
                if let Some(pou) = self.pous.get(pou_name) {
                    let exec: Box<dyn ExecutablePou> = match pou {
                        PouSource::Ld(p) => Box::new(p.compile(&tags)?),
                        PouSource::Sfc(p) => Box::new(p.compile(&tags)?),
                        PouSource::St(p) => Box::new(p.compile(&tags)?),
                        PouSource::Fbd(p) => Box::new(p.compile(&tags)?),
                    };
                    pous.push(exec);
                }
            }
            compiled_tasks.push(CompiledTask {
                name: task.name.clone(),
                kind: task.kind,
                priority: task.priority,
                interval_ms: task.interval_ms,
                trigger_symbol: task.trigger_symbol.clone(),
                pous,
            });
        }
        Ok((tags, compiled_tasks))
    }

    /// Exports the project to a standard PLCopen TC6 XML string.
    pub fn export_plcopen_xml(&self) -> Result<String, ProjectError> {
        PlcOpenTc6::export_project(self)
    }

    /// Imports a project from a PLCopen TC6 XML string into a target directory.
    pub fn import_plcopen_xml(xml: &str, target_dir: &Path) -> Result<Self, ProjectError> {
        PlcOpenTc6::import_project(xml, target_dir)
    }

    /// Compiles all POUs in configured task priority and execution order.
    pub fn compile(&self) -> Result<(TagTable, Vec<Box<dyn ExecutablePou>>), ProjectError> {
        let tags = self.build_tag_table()?;
        let mut tasks_sorted = self.manifest.tasks.clone();
        tasks_sorted.sort_by_key(|t| t.priority);

        let mut compiled_pous: Vec<Box<dyn ExecutablePou>> = Vec::new();
        for task in &tasks_sorted {
            for pou_name in &task.pou_order {
                if let Some(pou) = self.pous.get(pou_name) {
                    let exec: Box<dyn ExecutablePou> = match pou {
                        PouSource::Ld(p) => Box::new(p.compile(&tags)?),
                        PouSource::Sfc(p) => Box::new(p.compile(&tags)?),
                        PouSource::St(p) => Box::new(p.compile(&tags)?),
                        PouSource::Fbd(p) => Box::new(p.compile(&tags)?),
                    };
                    compiled_pous.push(exec);
                }
            }
        }
        Ok((tags, compiled_pous))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_project_scaffold_load_check_and_add_pou() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join("ferrite_plc_test_project");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        let proj = Project::scaffold(&temp_dir, "test_plant")?;
        let diags = proj.check()?;
        assert!(diags.is_empty(), "Expected 0 diagnostics, got {diags:?}");

        let mut loaded = Project::load(&temp_dir)?;
        assert_eq!(loaded.manifest.name, "test_plant");
        assert_eq!(loaded.pous.len(), 5);

        loaded.add_pou("extra_logic", LanguageKind::St)?;
        let diags_after = loaded.check()?;
        assert!(diags_after.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_plcopen_tc6_xml_export_and_import_roundtrip() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = env::temp_dir().join("ferrite_plc_test_plcopen_orig");
        let import_dir = env::temp_dir().join("ferrite_plc_test_plcopen_imported");
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }
        if import_dir.exists() {
            let _ = fs::remove_dir_all(&import_dir);
        }

        let proj = Project::scaffold(&temp_dir, "plcopen_plant")?;
        let xml = proj.export_plcopen_xml()?;
        assert!(xml.contains("<project xmlns=\"http://www.plcopen.org/xml/tc6_0201\">"));
        assert!(xml.contains("<fileHeader companyName=\"FerritePLC\""));
        assert!(xml.contains("<pou name=\"main_safety\""));
        assert!(xml.contains("<pou name=\"math_pid\""));
        assert!(xml.contains("<variable name=\"Tank_Level\""));

        let imported = Project::import_plcopen_xml(&xml, &import_dir)?;
        assert_eq!(imported.manifest.name, "plcopen_plant");
        assert!(
            imported
                .global_tags
                .tags
                .iter()
                .any(|t| t.name == "Tank_Level")
        );
        assert!(imported.pous.contains_key("math_pid"));

        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&import_dir);
        Ok(())
    }

    #[test]
    fn test_examples_sfc_auto_sort_persisted() -> Result<(), Box<dyn std::error::Error>> {
        let candidates = [
            PathBuf::from("../../examples/my_automation_plant"),
            PathBuf::from("examples/my_automation_plant"),
            PathBuf::from("../../examples/complex_batch_plant"),
            PathBuf::from("examples/complex_batch_plant"),
        ];

        for path in &candidates {
            if path.exists() {
                let proj = Project::load(path)?;
                for (name, pou) in &proj.pous {
                    if let PouSource::Sfc(sfc) = pou {
                        let report = ferrite_lang_sfc::validate_sfc_topology(sfc);
                        assert!(
                            report.is_valid,
                            "SFC validation errors in {}::{}: {:?}",
                            path.display(),
                            name,
                            report.errors
                        );
                        assert!(
                            sfc.steps.iter().all(|s| s.grid_pos.is_some()),
                            "Step without grid_pos in {}::{}",
                            path.display(),
                            name
                        );
                        assert!(
                            sfc.transitions.iter().all(|t| t.grid_pos.is_some()),
                            "Transition without grid_pos in {}::{}",
                            path.display(),
                            name
                        );
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_project_rename_pou() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_rename_pou_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_rename")?;
        assert!(proj.pous.contains_key("pump_ctrl"));

        // Rename pump_ctrl -> boiler_fbd
        proj.rename_pou("pump_ctrl", "boiler_fbd")?;
        assert!(!proj.pous.contains_key("pump_ctrl"));
        assert!(proj.pous.contains_key("boiler_fbd"));

        // Verify manifest updated
        let manifest_entry = proj
            .manifest
            .pous
            .iter()
            .find(|p| p.name == "boiler_fbd")
            .ok_or("Manifest entry boiler_fbd not found")?;
        assert_eq!(manifest_entry.path, "pous/boiler_fbd.fbd.json");
        assert!(!proj.manifest.pous.iter().any(|p| p.name == "pump_ctrl"));

        // Verify tasks updated
        for task in &proj.manifest.tasks {
            assert!(!task.pou_order.contains(&"pump_ctrl".to_string()));
            assert!(task.pou_order.contains(&"boiler_fbd".to_string()));
        }

        // Test error on duplicate name
        let err = proj.rename_pou("boiler_fbd", "main_safety");
        assert!(err.is_err());

        // Test error on empty name
        let err_empty = proj.rename_pou("boiler_fbd", "   ");
        assert!(err_empty.is_err());

        // Test error on path traversal / invalid chars
        assert!(proj.rename_pou("boiler_fbd", "../evil").is_err());
        assert!(proj.rename_pou("boiler_fbd", "evil/nested").is_err());
        assert!(proj.rename_pou("boiler_fbd", "evil\\nested").is_err());
        assert!(proj.rename_pou("boiler_fbd", "name.with.dots").is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_project_reorder_pous() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_reorder_pou_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_reorder")?;
        let orig_order: Vec<String> = proj.manifest.pous.iter().map(|p| p.name.clone()).collect();
        assert!(orig_order.len() >= 2);

        // Move first item up should return false (boundary)
        assert!(!proj.move_pou_up(&orig_order[0]));

        // Move second item up should swap with first
        let second = orig_order[1].clone();
        let first = orig_order[0].clone();
        assert!(proj.move_pou_up(&second));
        assert_eq!(proj.manifest.pous[0].name, second);
        assert_eq!(proj.manifest.pous[1].name, first);

        // Move it down should restore
        assert!(proj.move_pou_down(&second));
        assert_eq!(proj.manifest.pous[0].name, first);
        assert_eq!(proj.manifest.pous[1].name, second);

        // Move last item down should return false (boundary)
        let last = orig_order
            .last()
            .ok_or("orig_order should not be empty")?
            .clone();
        assert!(!proj.move_pou_down(&last));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_project_delete_pou() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_delete_pou_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_delete")?;
        assert!(proj.pous.contains_key("pump_ctrl"));

        proj.delete_pou("pump_ctrl")?;
        assert!(!proj.pous.contains_key("pump_ctrl"));
        assert!(!proj.manifest.pous.iter().any(|p| p.name == "pump_ctrl"));
        for task in &proj.manifest.tasks {
            assert!(!task.pou_order.contains(&"pump_ctrl".to_string()));
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_project_save_single_pou() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_save_single_pou_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_save_pou")?;

        // Modify both `main_safety` and `math_pid` in memory
        if let Some(PouSource::Ld(ld)) = proj.pous.get_mut("main_safety") {
            ld.comment = "Updated main_safety comment".to_string();
        }
        if let Some(PouSource::St(st)) = proj.pous.get_mut("math_pid") {
            st.source = "PROGRAM math_pid\n    (* Modified unsaved ST *)\nEND_PROGRAM\n".to_string();
        }

        // Save only `main_safety`
        proj.save_pou(&temp_dir, "main_safety")?;

        // Reload from disk: `main_safety` must have the update, while `math_pid` must remain untouched
        let reloaded = Project::load(&temp_dir)?;
        let Some(PouSource::Ld(reloaded_ld)) = reloaded.pous.get("main_safety") else {
            return Err("main_safety missing".into());
        };
        assert_eq!(reloaded_ld.comment, "Updated main_safety comment");

        let Some(PouSource::St(reloaded_st)) = reloaded.pous.get("math_pid") else {
            return Err("math_pid missing".into());
        };
        assert!(!reloaded_st.source.contains("Modified unsaved ST"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_project_reorder_pou_arbitrary() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "ferrite_test_reorder_arbitrary_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_reorder_arb")?;
        assert!(proj.manifest.pous.len() >= 3);
        let orig_first = proj.manifest.pous[0].name.clone();
        let orig_third = proj.manifest.pous[2].name.clone();

        // Reorder: move first POU to third position
        assert!(proj.reorder_pou(&orig_first, &orig_third));
        assert_eq!(proj.manifest.pous[2].name, orig_first);

        // Reorder onto itself is no-op returning false
        assert!(!proj.reorder_pou(&orig_first, &orig_first));

        // Reorder unknown POU returns false
        assert!(!proj.reorder_pou("non_existent", &orig_first));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_cr2_plcopen_collision_and_parse_failure() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_cr2_collision_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let bad_dir = env::temp_dir().join(format!(
            "ferrite_test_cr2_bad_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));

        // Colliding POU: batch_seq in scaffold is SFC, but imported XML defines it as ST
        let colliding_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="FerritePLC" productName="Test"/>
  <contentHeader name="collision_plant"/>
  <types>
    <pous>
      <pou name="batch_seq" pouType="program">
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml"><![CDATA[PROGRAM batch_seq
    (* Replaced batch_seq as ST *)
END_PROGRAM
]]></xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations/>
  </instances>
</project>"#;

        let imported = Project::import_plcopen_xml(colliding_xml, &temp_dir)?;
        let entry = imported
            .manifest
            .pous
            .iter()
            .find(|p| p.name == "batch_seq")
            .ok_or("batch_seq entry not found")?;
        assert_eq!(entry.language, LanguageKind::St);
        assert_eq!(entry.path, "pous/batch_seq.st");

        // Old SFC file must not exist on disk, ST file must exist
        assert!(!temp_dir.join("pous/batch_seq.sfc.json").exists());
        assert!(temp_dir.join("pous/batch_seq.st").exists());

        // Reload from disk must succeed
        let reloaded = Project::load(&temp_dir)?;
        let pou = reloaded.pous.get("batch_seq").ok_or("batch_seq missing")?;
        assert!(matches!(pou, PouSource::St(_)));

        // Unparseable POU must fail import
        let bad_xml = r#"<?xml version="1.0" encoding="utf-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="FerritePLC" productName="Test"/>
  <contentHeader name="bad_plant"/>
  <types>
    <pous>
      <pou name="broken_pou" pouType="program">
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml"><![CDATA[NOT VALID ST CODE ??? &&&]]></xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
  <instances>
    <configurations/>
  </instances>
</project>"#;

        let err = Project::import_plcopen_xml(bad_xml, &bad_dir);
        assert!(err.is_err(), "Unparseable POU must fail import");

        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::remove_dir_all(&bad_dir);
        Ok(())
    }

    #[test]
    fn test_i19_i20_atomic_write_and_add_pou_validation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_i19_i20_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_add_pou")?;

        // I-20: Test validation of invalid names
        assert!(proj.add_pou("fo:o", LanguageKind::St).is_err());
        assert!(proj.add_pou("../evil", LanguageKind::St).is_err());
        assert!(proj.add_pou("", LanguageKind::St).is_err());
        assert!(proj.add_pou("   ", LanguageKind::St).is_err());
        assert!(proj.add_pou("invalid name", LanguageKind::St).is_err());
        assert!(proj.add_pou("evil/nested", LanguageKind::St).is_err());
        assert!(proj.add_pou("evil\\nested", LanguageKind::St).is_err());

        // Ensure no files created outside pous/
        assert!(!temp_dir.join("evil.st").exists());
        if let Some(parent) = temp_dir.parent() {
            assert!(!parent.join("evil.st").exists());
        }

        // Valid add_pou creates file before manifest entry
        let pou_path = proj.add_pou("safety_gate", LanguageKind::St)?;
        assert!(pou_path.exists());
        assert!(proj.manifest.pous.iter().any(|p| p.name == "safety_gate"));

        // Reload project
        let reloaded = Project::load(&temp_dir)?;
        assert!(reloaded.pous.contains_key("safety_gate"));

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_i21_rename_pou_rewrites_cross_references() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_i21_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_rename_refs")?;

        // Before rename, project check passes
        assert!(proj.check()?.is_empty());

        // Rename cip_clean to cip_clean_renamed
        proj.rename_pou("cip_clean", "cip_clean_renamed")?;

        // Check that batch_seq (SFC) references to cip_clean were rewritten
        let batch_sfc = match proj.pous.get("batch_seq") {
            Some(PouSource::Sfc(s)) => s,
            _ => return Err("batch_seq missing or not SFC".into()),
        };
        // Check action target
        let has_renamed_action = batch_sfc.steps.iter().any(|s| {
            s.actions
                .iter()
                .any(|a| a.target.contains("cip_clean_renamed.X2.state"))
        });
        assert!(has_renamed_action, "batch_seq action target not rewritten");

        // Check transition receptivity
        let has_renamed_trans = batch_sfc
            .transitions
            .iter()
            .any(|t| t.receptivity.contains("cip_clean_renamed.X2.state"));
        assert!(has_renamed_trans, "batch_seq transition not rewritten");

        // Check that math_pid (ST) reference was rewritten
        let math_st = match proj.pous.get("math_pid") {
            Some(PouSource::St(s)) => s,
            _ => return Err("math_pid missing or not ST".into()),
        };
        assert!(
            math_st.source.contains("cip_clean_renamed.X1.state"),
            "math_pid ST source not rewritten: {}",
            math_st.source
        );

        // Check scenarios rewritten
        let sc = &proj.scenarios.scenarios[0];
        let has_renamed_sc = sc.steps.iter().any(|s| {
            s.expect
                .keys()
                .any(|k| k.starts_with("cip_clean_renamed."))
        });
        assert!(has_renamed_sc, "scenarios not rewritten");

        // Check project validation passes with 0 errors!
        let diags = proj.check()?;
        assert!(diags.is_empty(), "Project check failed after rename: {diags:?}");

        // Reload from disk and verify disk files were updated
        let reloaded = Project::load(&temp_dir)?;
        let reloaded_diags = reloaded.check()?;
        assert!(reloaded_diags.is_empty(), "Reloaded check failed: {reloaded_diags:?}");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_c15_manifest_path_sandboxing() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_c15_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let proj = Project::scaffold(&temp_dir, "test_c15")?;
        drop(proj);

        // Tamper with manifest to have a path with traversal
        let manifest_path = temp_dir.join(MANIFEST_FILE_NAME);
        let raw = fs::read_to_string(&manifest_path)?;
        let tampered = raw.replace("pous/math_pid.st", "../math_pid.st");
        fs::write(&manifest_path, tampered)?;

        // Project::load must fail sandboxing check
        let res = Project::load(&temp_dir);
        assert!(res.is_err(), "Project::load must reject traversal path in manifest");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_c16_swallowed_errors_save_manifest_pou_missing() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_c16_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_c16")?;

        // Remove a POU from in-memory map without updating manifest
        proj.pous.remove("pump_ctrl");

        // proj.save must fail instead of silently skipping
        let res = proj.save(&temp_dir);
        assert!(res.is_err(), "save() must fail when manifest POU is missing from map");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_c17_plcopen_rejects_byte_word() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_c17_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));

        let xml_byte = r#"<?xml version="1.0" encoding="utf-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="FerritePLC" productName="Test"/>
  <contentHeader name="byte_plant"/>
  <types><pous/></types>
  <instances>
    <configurations>
      <configuration name="Config">
        <resource name="Res">
          <globalVars>
            <variable name="ByteVar">
              <type><BYTE/></type>
            </variable>
          </globalVars>
        </resource>
      </configuration>
    </configurations>
  </instances>
</project>"#;

        let res = Project::import_plcopen_xml(xml_byte, &temp_dir);
        assert!(res.is_err(), "importing BYTE must fail with unsupported type error");

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }

    #[test]
    fn test_scenario_fb_assertions_schema_and_validation() -> Result<(), Box<dyn std::error::Error>> {
        // 1. Legacy scenarios TOML parses without error
        let legacy_toml = r#"
[[scenarios]]
name = "legacy_test"
total_scans = 5

[[scenarios.steps]]
scan = 1
description = "Legacy step"
[scenarios.steps.set]
Start_Button = true
[scenarios.steps.expect]
Permissive_OK = true
"#;
        let file: ScenariosFile = toml::from_str(legacy_toml)?;
        assert_eq!(file.scenarios.len(), 1);
        assert_eq!(file.scenarios[0].steps.len(), 1);
        assert!(file.scenarios[0].steps[0].asserts.is_empty());
        assert_eq!(
            file.scenarios[0].steps[0].expect.get("Permissive_OK"),
            Some(&IecValue::Bool(true))
        );

        // 2. New FB assert with fb, field, op, value
        let fb_assert_toml = r#"
[[scenarios]]
name = "fb_test"
total_scans = 10

[[scenarios.steps]]
scan = 2
description = "Check timer CV"
assert = { fb = "Timer_1", field = "CV", op = ">=", value = 50 }
"#;
        let file_fb: ScenariosFile = toml::from_str(fb_assert_toml)?;
        assert_eq!(file_fb.scenarios[0].steps[0].asserts.len(), 1);
        let a = &file_fb.scenarios[0].steps[0].asserts[0];
        assert_eq!(a.fb, "Timer_1");
        assert_eq!(a.field.as_deref(), Some("CV"));
        assert_eq!(a.op, AssertOp::GreaterEqual);
        assert_eq!(a.value, Some(IecValue::Int(50)));
        assert_eq!(a.target_fb_and_field(), Some(("Timer_1", "CV")));

        // 3. Composite fb notation "Timer_1.CV"
        let composite_toml = r#"
[[scenarios]]
name = "composite_test"
total_scans = 3

[[scenarios.steps]]
scan = 1
assert = { fb = "Timer_1.CV", op = ">=", value = 100 }
"#;
        let file_comp: ScenariosFile = toml::from_str(composite_toml)?;
        let a_comp = &file_comp.scenarios[0].steps[0].asserts[0];
        assert_eq!(a_comp.target_fb_and_field(), Some(("Timer_1", "CV")));

        // 4. Range-gate assertion
        let range_toml = r#"
[[scenarios]]
name = "range_test"
total_scans = 3

[[scenarios.steps]]
scan = 1
assert = { fb = "Level_PID.OUT", op = "in_range", min = 10.0, max = 90.0 }
"#;
        let file_range: ScenariosFile = toml::from_str(range_toml)?;
        let a_range = &file_range.scenarios[0].steps[0].asserts[0];
        assert_eq!(a_range.op, AssertOp::InRange);
        assert_eq!(a_range.min, Some(IecValue::Real(10.0)));
        assert_eq!(a_range.max, Some(IecValue::Real(90.0)));

        // 5. Multiple assertions list
        let multi_toml = r#"
[[scenarios]]
name = "multi_test"
total_scans = 2

[[scenarios.steps]]
scan = 1
assert = [
    { fb = "Timer_1", field = "CV", op = ">=", value = 10 },
    { fb = "Timer_1", field = "Q", op = "==", value = false },
    { sfc_step = "batch_seq.X0", op = "==", value = true }
]
"#;
        let file_multi: ScenariosFile = toml::from_str(multi_toml)?;
        assert_eq!(file_multi.scenarios[0].steps[0].asserts.len(), 3);

        // 6. Project::check() validates FB instance existence and pin names
        let temp_dir = env::temp_dir().join(format!(
            "ferrite_test_fb_asserts_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()
        ));
        let mut proj = Project::scaffold(&temp_dir, "test_fb_check")?;

        // Valid scenario assert on Fill_Timer.ET (which is a TON instance in scaffold)
        let valid_step = SimStep {
            scan: 1,
            asserts: vec![ScenarioAssert {
                fb: "Fill_Timer".to_string(),
                field: Some("ET".to_string()),
                op: AssertOp::GreaterEqual,
                value: Some(IecValue::Time(100)),
                min: None,
                max: None,
                sfc_step: None,
            }],
            ..Default::default()
        };
        proj.scenarios.scenarios.push(SimScenario {
            name: "valid_fb_scenario".to_string(),
            description: "valid".to_string(),
            total_scans: 1,
            steps: vec![valid_step],
            asserts: Vec::new(),
        });
        let diags = proj.check()?;
        assert!(diags.is_empty(), "Valid FB assert should produce 0 diagnostics, got {diags:?}");

        // Invalid scenario assert on unknown FB instance
        let bad_step = SimStep {
            scan: 1,
            asserts: vec![ScenarioAssert {
                fb: "NonExistent_FB".to_string(),
                field: Some("CV".to_string()),
                op: AssertOp::Equal,
                value: Some(IecValue::Int(0)),
                min: None,
                max: None,
                sfc_step: None,
            }],
            ..Default::default()
        };
        proj.scenarios.scenarios.push(SimScenario {
            name: "bad_fb_scenario".to_string(),
            description: "bad fb".to_string(),
            total_scans: 1,
            steps: vec![bad_step],
            asserts: Vec::new(),
        });
        let diags_bad_fb = proj.check()?;
        assert!(
            diags_bad_fb.iter().any(|d| d.message.contains("unknown Function Block instance 'NonExistent_FB'")),
            "Expected unknown FB error, got {diags_bad_fb:?}"
        );

        // Invalid scenario assert on unknown pin for valid FB
        proj.scenarios.scenarios.pop();
        let bad_pin_step = SimStep {
            scan: 1,
            asserts: vec![ScenarioAssert {
                fb: "Fill_Timer.UNKNOWN_PIN".to_string(),
                field: None,
                op: AssertOp::Equal,
                value: Some(IecValue::Bool(true)),
                min: None,
                max: None,
                sfc_step: None,
            }],
            ..Default::default()
        };
        proj.scenarios.scenarios.push(SimScenario {
            name: "bad_pin_scenario".to_string(),
            description: "bad pin".to_string(),
            total_scans: 1,
            steps: vec![bad_pin_step],
            asserts: Vec::new(),
        });
        let diags_bad_pin = proj.check()?;
        assert!(
            diags_bad_pin.iter().any(|d| d.message.contains("unknown pin/field 'UNKNOWN_PIN'")),
            "Expected unknown pin error, got {diags_bad_pin:?}"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
        Ok(())
    }
}
