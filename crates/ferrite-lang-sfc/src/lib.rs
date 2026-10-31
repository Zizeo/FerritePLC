use std::collections::{HashMap, HashSet, VecDeque};

use ferrite_core::{
    CoreError, ExecutablePou, IecType, IecValue, LanguageKind, PouId, RuntimeError, ScanContext,
    TagTable, parse_iec_time_literal,
};
use ferrite_ir::{IrError, IrExpr, IrStmt, SymbolResolver};
use ferrite_lang_st::{StError, parse_st_expression, parse_st_statements};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum SfcError {
    #[error("JSON parse error in Grafcet/SFC POU: {0}")]
    Json(String),

    #[error("Unknown step reference '{step}' in SFC POU '{pou}'")]
    UnknownStep { pou: String, step: String },

    #[error("Missing initial step in SFC POU '{0}'")]
    MissingInitialStep(String),

    #[error("Invalid duration '{duration}' for qualifier {qualifier:?} in SFC POU '{pou}'")]
    InvalidQualifierDuration {
        pou: String,
        qualifier: ActionQualifier,
        duration: String,
    },

    #[error("Expression/Action syntax error in SFC POU '{pou}': {source}")]
    St {
        pou: String,
        #[source]
        source: StError,
    },

    #[error("Validation error in SFC POU '{pou}': {source}")]
    Ir {
        pou: String,
        #[source]
        source: IrError,
    },

    #[error("Core error in SFC POU '{pou}': {source}")]
    Core {
        pou: String,
        #[source]
        source: CoreError,
    },

    #[error("Topology validation error in SFC POU '{pou}': {}", format_sfc_validation_errors(.errors))]
    TopologyValidation {
        pou: String,
        errors: Vec<SfcValidationError>,
    },
}

fn format_sfc_validation_errors(errors: &[SfcValidationError]) -> String {
    errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Initial,
    #[default]
    Normal,
    Macro,
}

/// IEC 60848 / IEC 61131-3 step action qualifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionQualifier {
    /// Non-stored (active while step is active)
    N,
    /// Set (stored / latched active when step activates; can also set another SFC step state)
    S,
    /// Reset (clears stored variable or deactivates another SFC step state when step activates)
    R,
    /// Time-Limited (active while step is active and `t < duration`)
    L,
    /// Time-Delayed (active while step is active and `t >= duration`)
    D,
    /// Pulse (active for one scan cycle on step rising edge)
    P,
    /// Force ON (IEC 60848 forcing order: forces target symbol or SFC step state TRUE while step is active)
    F,
    /// Force OFF (IEC 60848 forcing order: forces target symbol or SFC step state FALSE while step is active)
    FZ,
}

/// Convergence mode when a transition has multiple upstream `from_steps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceKind {
    /// Simultaneous (AND) convergence (`===`): ALL upstream `from_steps` must be active.
    #[default]
    And,
    /// Selection (OR) convergence (`---`): ANY upstream `from_steps` being active enables the transition.
    Or,
}

/// Divergence mode for outgoing transition branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceKind {
    /// Simultaneous (AND) divergence (`===`): activates all `to_steps` simultaneously.
    #[default]
    And,
    /// Exclusive Selection (OR) divergence (`---`): if multiple OR transitions from the same step are true, priority order applies.
    Or,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcAction {
    pub qualifier: ActionQualifier,
    /// Either a boolean symbol/pin/step-state name (`Fill_Valve`, `sub_seq.X1.state`) or an ST statement (`Setpoint := 100;`).
    pub target: String,
    /// Required for `L` and `D` qualifiers (e.g. `"T#500ms"` or `"T#2s"`).
    #[serde(default)]
    pub duration: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SfcStep {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub kind: StepKind,
    #[serde(default)]
    pub actions: Vec<SfcAction>,
    #[serde(default)]
    pub grid_pos: Option<(i32, i32)>,
}

impl SfcStep {
    pub fn new(id: impl Into<String>, name: impl Into<String>, kind: StepKind) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            actions: Vec::new(),
            grid_pos: None,
        }
    }

    pub fn with_pos(mut self, x: i32, y: i32) -> Self {
        self.grid_pos = Some((x, y));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SfcTransition {
    pub id: String,
    pub from_steps: Vec<String>,
    pub to_steps: Vec<String>,
    /// Boolean/comparison receptivity expression (e.g., `"Permissive_OK AND Sensor_1"` or `"master_seq.X1.state"`).
    pub receptivity: String,
    /// Convergence behavior when `from_steps` has multiple steps (`And` = simultaneous `===`, `Or` = selection `---`).
    #[serde(default)]
    pub convergence: ConvergenceKind,
    /// Divergence behavior (`And` = simultaneous `===`, `Or` = exclusive selection `---`).
    #[serde(default)]
    pub divergence: DivergenceKind,
    #[serde(default)]
    pub grid_pos: Option<(i32, i32)>,
}

impl SfcTransition {
    pub fn new(id: impl Into<String>, receptivity: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            from_steps: Vec::new(),
            to_steps: Vec::new(),
            receptivity: receptivity.into(),
            convergence: ConvergenceKind::And,
            divergence: DivergenceKind::And,
            grid_pos: None,
        }
    }

    pub fn with_pos(mut self, x: i32, y: i32) -> Self {
        self.grid_pos = Some((x, y));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcTreeInfo {
    pub tree_id: usize,
    pub step_ids: Vec<String>,
    pub transition_ids: Vec<String>,
    pub initial_step_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SfcValidationError {
    MissingInitialStepInTree {
        tree_id: usize,
        step_ids: Vec<String>,
    },
    DanglingStepReference {
        transition_id: String,
        referenced_step: String,
        is_from_step: bool,
    },
    InvalidAlternation {
        element_id: String,
        reason: String,
    },
    OrphanElement {
        element_id: String,
        is_step: bool,
    },
    UnclosedAndBranch {
        transition_id: String,
        reason: String,
    },
    InvalidAndDivergence {
        transition_id: String,
        reason: String,
    },
    InvalidAndConvergence {
        transition_id: String,
        reason: String,
    },
    InvalidOrDivergence {
        element_id: String,
        reason: String,
    },
    InvalidOrConvergence {
        element_id: String,
        reason: String,
    },
}

impl std::fmt::Display for SfcValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingInitialStepInTree { tree_id, step_ids } => {
                write!(
                    f,
                    "Tree #{tree_id} (steps: [{}]) is missing an Initial Step",
                    step_ids.join(", ")
                )
            }
            Self::DanglingStepReference {
                transition_id,
                referenced_step,
                is_from_step,
            } => {
                let dir = if *is_from_step { "source" } else { "target" };
                write!(
                    f,
                    "Transition '{transition_id}' references unknown {dir} step '{referenced_step}'"
                )
            }
            Self::InvalidAlternation { element_id, reason } => {
                write!(f, "Alternation violation at '{element_id}': {reason}")
            }
            Self::OrphanElement {
                element_id,
                is_step,
            } => {
                let kind = if *is_step { "Step" } else { "Transition" };
                write!(
                    f,
                    "Orphan {kind} '{element_id}' is disconnected from any link"
                )
            }
            Self::UnclosedAndBranch {
                transition_id,
                reason,
            } => {
                write!(
                    f,
                    "Unclosed AND divergence at transition '{transition_id}': {reason}"
                )
            }
            Self::InvalidAndDivergence {
                transition_id,
                reason,
            } => {
                write!(
                    f,
                    "Invalid AND divergence at transition '{transition_id}': {reason}"
                )
            }
            Self::InvalidAndConvergence {
                transition_id,
                reason,
            } => {
                write!(
                    f,
                    "Invalid AND convergence at transition '{transition_id}': {reason}"
                )
            }
            Self::InvalidOrDivergence { element_id, reason } => {
                write!(f, "Invalid OR divergence at '{element_id}': {reason}")
            }
            Self::InvalidOrConvergence { element_id, reason } => {
                write!(f, "Invalid OR convergence at '{element_id}': {reason}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcValidationReport {
    pub is_valid: bool,
    pub errors: Vec<SfcValidationError>,
    pub warnings: Vec<String>,
    pub trees: Vec<SfcTreeInfo>,
}

/// Validates the graph topology of an SFC program:
/// 1. Discovers independent parallel trees (connected components).
/// 2. Verifies each non-empty tree has at least one initial step.
/// 3. Detects dangling step references in transitions.
/// 4. Checks for orphan steps and transitions.
pub fn validate_sfc_topology(program: &SfcProgram) -> SfcValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if program.steps.is_empty() && program.transitions.is_empty() {
        return SfcValidationReport {
            is_valid: true,
            errors,
            warnings,
            trees: Vec::new(),
        };
    }

    // 1. Build step lookup map
    let mut step_map: HashMap<String, usize> = HashMap::new();
    for (idx, step) in program.steps.iter().enumerate() {
        step_map.insert(step.id.clone(), idx);
        step_map.insert(step.name.clone(), idx);
        let trimmed = step.id.trim();
        let stripped = trimmed
            .strip_prefix('X')
            .or_else(|| trimmed.strip_prefix('x'))
            .unwrap_or(trimmed);
        step_map.insert(stripped.to_string(), idx);
    }

    // 2. Validate step references in transitions
    for tr in &program.transitions {
        for fs in &tr.from_steps {
            let trimmed = fs.trim();
            let stripped = trimmed
                .strip_prefix('X')
                .or_else(|| trimmed.strip_prefix('x'))
                .unwrap_or(trimmed);
            if !step_map.contains_key(trimmed) && !step_map.contains_key(stripped) {
                errors.push(SfcValidationError::DanglingStepReference {
                    transition_id: tr.id.clone(),
                    referenced_step: fs.clone(),
                    is_from_step: true,
                });
            }
        }
        for ts in &tr.to_steps {
            let trimmed = ts.trim();
            let stripped = trimmed
                .strip_prefix('X')
                .or_else(|| trimmed.strip_prefix('x'))
                .unwrap_or(trimmed);
            if !step_map.contains_key(trimmed) && !step_map.contains_key(stripped) {
                errors.push(SfcValidationError::DanglingStepReference {
                    transition_id: tr.id.clone(),
                    referenced_step: ts.clone(),
                    is_from_step: false,
                });
            }
        }

        // 2b. Validate IEC 60848 Divergence & Convergence structural rules
        // Rule 1: AND Divergence (===) must be preceded by an upstream step/transition
        if tr.divergence == DivergenceKind::And && tr.to_steps.len() > 1 {
            if tr.from_steps.is_empty() {
                errors.push(SfcValidationError::InvalidAndDivergence {
                    transition_id: tr.id.clone(),
                    reason: "IEC 60848: AND divergence must be preceded by an upstream step/transition".to_string(),
                });
            }
            let unique_targets: HashSet<&String> = tr.to_steps.iter().collect();
            if unique_targets.len() < tr.to_steps.len() {
                errors.push(SfcValidationError::InvalidAndDivergence {
                    transition_id: tr.id.clone(),
                    reason: "IEC 60848: AND divergence targets contain duplicate steps".to_string(),
                });
            }
        }

        // Rule 2: AND Convergence (===) must synchronize multiple steps into a transition that targets a step
        if tr.convergence == ConvergenceKind::And && tr.from_steps.len() > 1 {
            if tr.to_steps.is_empty() {
                errors.push(SfcValidationError::InvalidAndConvergence {
                    transition_id: tr.id.clone(),
                    reason: "IEC 60848: AND convergence must transition into a target step".to_string(),
                });
            }
            let unique_sources: HashSet<&String> = tr.from_steps.iter().collect();
            if unique_sources.len() < tr.from_steps.len() {
                errors.push(SfcValidationError::InvalidAndConvergence {
                    transition_id: tr.id.clone(),
                    reason: "IEC 60848: AND convergence sources contain duplicate steps".to_string(),
                });
            }
        }

        // Rule 3: OR Divergence (—) cannot be driven by a single transition; each alternative branch must have its own transition
        if tr.divergence == DivergenceKind::Or && tr.to_steps.len() > 1 {
            errors.push(SfcValidationError::InvalidOrDivergence {
                element_id: tr.id.clone(),
                reason: "IEC 60848: OR divergence cannot be driven by a single transition; each alternative branch must have its own transition".to_string(),
            });
        }

        // Rule 4: OR Convergence (—) cannot merge multiple steps into a single transition; each alternative branch must end in its own transition into the destination step
        if tr.convergence == ConvergenceKind::Or && tr.from_steps.len() > 1 {
            errors.push(SfcValidationError::InvalidOrConvergence {
                element_id: tr.id.clone(),
                reason: "IEC 60848: OR convergence cannot merge multiple steps into a single transition; each alternative branch must end in its own transition into the destination step".to_string(),
            });
        }
    }

    // 3. Graph connected components
    let n_steps = program.steps.len();
    let n_trans = program.transitions.len();

    let mut step_adj: Vec<Vec<usize>> = vec![Vec::new(); n_steps];
    let mut trans_adj: Vec<Vec<usize>> = vec![Vec::new(); n_trans];

    for (t_idx, tr) in program.transitions.iter().enumerate() {
        for fs in &tr.from_steps {
            let trimmed = fs.trim();
            let stripped = trimmed
                .strip_prefix('X')
                .or_else(|| trimmed.strip_prefix('x'))
                .unwrap_or(trimmed);
            if let Some(&s_idx) = step_map.get(trimmed).or_else(|| step_map.get(stripped)) {
                step_adj[s_idx].push(t_idx);
                trans_adj[t_idx].push(s_idx);
            }
        }
        for ts in &tr.to_steps {
            let trimmed = ts.trim();
            let stripped = trimmed
                .strip_prefix('X')
                .or_else(|| trimmed.strip_prefix('x'))
                .unwrap_or(trimmed);
            if let Some(&s_idx) = step_map.get(trimmed).or_else(|| step_map.get(stripped)) {
                step_adj[s_idx].push(t_idx);
                trans_adj[t_idx].push(s_idx);
            }
        }
    }

    let mut visited_steps = HashSet::new();
    let mut visited_trans = HashSet::new();
    let mut trees = Vec::new();

    for s_start in 0..n_steps {
        if visited_steps.contains(&s_start) {
            continue;
        }

        let mut queue_steps = VecDeque::new();
        let mut queue_trans = VecDeque::new();

        queue_steps.push_back(s_start);
        visited_steps.insert(s_start);

        let mut comp_steps = Vec::new();
        let mut comp_trans = Vec::new();

        while !queue_steps.is_empty() || !queue_trans.is_empty() {
            while let Some(s) = queue_steps.pop_front() {
                comp_steps.push(s);
                for &t in &step_adj[s] {
                    if visited_trans.insert(t) {
                        queue_trans.push_back(t);
                    }
                }
            }
            while let Some(t) = queue_trans.pop_front() {
                comp_trans.push(t);
                for &s in &trans_adj[t] {
                    if visited_steps.insert(s) {
                        queue_steps.push_back(s);
                    }
                }
            }
        }

        let tree_id = trees.len() + 1;
        let step_ids: Vec<String> = comp_steps
            .iter()
            .map(|&si| program.steps[si].id.clone())
            .collect();
        let transition_ids: Vec<String> = comp_trans
            .iter()
            .map(|&ti| program.transitions[ti].id.clone())
            .collect();
        let initial_step_ids: Vec<String> = comp_steps
            .iter()
            .filter(|&&si| program.steps[si].kind == StepKind::Initial)
            .map(|&si| program.steps[si].id.clone())
            .collect();

        // A sequence tree (having transitions) MUST contain at least one initial step!
        if !comp_trans.is_empty() && initial_step_ids.is_empty() {
            errors.push(SfcValidationError::MissingInitialStepInTree {
                tree_id,
                step_ids: step_ids.clone(),
            });
        }

        // Check orphan step (isolated from any transitions)
        if comp_trans.is_empty() && (n_steps > 1 || n_trans > 0) {
            warnings.push(format!(
                "Step '{}' is an orphan (not connected to any transition)",
                step_ids[0]
            ));
            // Do not treat standalone orphan steps as independent parallel execution trees
            continue;
        }

        let tree_id = trees.len() + 1;
        trees.push(SfcTreeInfo {
            tree_id,
            step_ids,
            transition_ids,
            initial_step_ids,
        });
    }

    // Check any transitions not connected to any step
    for t_start in 0..n_trans {
        if !visited_trans.contains(&t_start) {
            visited_trans.insert(t_start);
            let tr_id = program.transitions[t_start].id.clone();
            errors.push(SfcValidationError::OrphanElement {
                element_id: tr_id.clone(),
                is_step: false,
            });
            // Do not treat standalone orphan transitions as independent parallel execution trees
        }
    }

    let is_valid = errors.is_empty();
    SfcValidationReport {
        is_valid,
        errors,
        warnings,
        trees,
    }
}

/// Computes visual rows of step indices so that parallel branches created by AND Divergence
/// (`to_steps.len() > 1`) or OR Divergence (multiple transitions sharing `from_steps`) render side-by-side.
pub fn compute_sfc_step_rows(program: &SfcProgram) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut placed: HashSet<usize> = HashSet::new();

    let mut idx = 0;
    while idx < program.steps.len() {
        if placed.contains(&idx) {
            idx += 1;
            continue;
        }
        let step_id = &program.steps[idx].id;
        let step_name = &program.steps[idx].name;
        let mut sibling_row: Vec<usize> = Vec::new();

        // Case A: Single transition with multiple `to_steps` (AND divergence) containing this step
        if let Some(and_tr) = program.transitions.iter().find(|t| {
            t.to_steps.len() > 1
                && (t.to_steps.contains(step_id) || t.to_steps.iter().any(|s| s == step_name))
        }) {
            for tid in &and_tr.to_steps {
                if let Some(s_pos) = program
                    .steps
                    .iter()
                    .position(|s| &s.id == tid || &s.name == tid)
                    && !placed.contains(&s_pos)
                    && !sibling_row.contains(&s_pos)
                {
                    sibling_row.push(s_pos);
                }
            }
        }

        // Case B: Multiple transitions sharing the same `from_steps` (OR divergence) targeting this step
        if sibling_row.len() <= 1
            && let Some(parent_tr) = program.transitions.iter().find(|t| {
                (t.to_steps.contains(step_id) || t.to_steps.iter().any(|s| s == step_name))
                    && !t.from_steps.is_empty()
            })
        {
            let shared_from = &parent_tr.from_steps;
            let or_siblings: Vec<&SfcTransition> = program
                .transitions
                .iter()
                .filter(|t| &t.from_steps == shared_from)
                .collect();
            if or_siblings.len() > 1 {
                sibling_row.clear();
                for otr in or_siblings {
                    for tid in &otr.to_steps {
                        if let Some(s_pos) = program
                            .steps
                            .iter()
                            .position(|s| &s.id == tid || &s.name == tid)
                            && !placed.contains(&s_pos)
                            && !sibling_row.contains(&s_pos)
                        {
                            sibling_row.push(s_pos);
                        }
                    }
                }
            }
        }

        if sibling_row.is_empty() {
            sibling_row.push(idx);
        }
        for &s_pos in &sibling_row {
            placed.insert(s_pos);
        }
        rows.push(sibling_row);
        idx += 1;
    }

    rows
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcProgram {
    pub name: String,
    #[serde(default)]
    pub comment: String,
    pub steps: Vec<SfcStep>,
    pub transitions: Vec<SfcTransition>,
}

impl SfcProgram {
    pub fn from_json(json: &str) -> Result<Self, SfcError> {
        serde_json::from_str(json).map_err(|e| SfcError::Json(e.to_string()))
    }

    pub fn to_json_pretty(&self) -> Result<String, SfcError> {
        serde_json::to_string_pretty(self).map_err(|e| SfcError::Json(e.to_string()))
    }

    /// Registers `X<i>.state` (`BOOL`), `<pou>.X<i>.state` (`BOOL`), `<step_name>.state` (`BOOL`),
    /// and their `.t` (`TIME`) counterparts for every step in `TagTable`.
    pub fn register_introspection_symbols(&self, tags: &mut TagTable) -> Result<(), SfcError> {
        for step in &self.steps {
            tags.register_sfc_step_full(
                &self.name,
                &step.id,
                &step.name,
                step.kind == StepKind::Initial,
            )
            .map_err(|source| SfcError::Core {
                pou: self.name.clone(),
                source,
            })?;
        }
        Ok(())
    }

    /// Computes and assigns orderly, non-overlapping grid positions `(grid_x, grid_y)`
    /// for all steps and transitions that do not yet have explicit coordinates.
    pub fn assign_default_grid_positions(&mut self) {
        let report = validate_sfc_topology(self);
        let mut current_tree_x = 60;
        let mut placed_steps = HashSet::new();
        let mut placed_trans = HashSet::new();

        // 1. Arrange each independent tree in its own non-overlapping vertical column band
        for tree in &report.trees {
            let mut tree_steps: Vec<usize> = Vec::new();
            for sid in &tree.step_ids {
                if let Some(pos) = self
                    .steps
                    .iter()
                    .position(|s| &s.id == sid || &s.name == sid)
                {
                    tree_steps.push(pos);
                }
            }

            let all_rows = compute_sfc_step_rows(self);
            let tree_rows: Vec<Vec<usize>> = all_rows
                .into_iter()
                .map(|r| r.into_iter().filter(|si| tree_steps.contains(si)).collect())
                .filter(|r: &Vec<usize>| !r.is_empty())
                .collect();

            let max_cols = tree_rows.iter().map(|r| r.len()).max().unwrap_or(1);

            // Compute dynamic column widths taking into account step actions and transition receptivities
            let mut col_widths = vec![280_i32; max_cols];
            for row in &tree_rows {
                for (c_idx, &s_idx) in row.iter().enumerate() {
                    if let Some(step) = self.steps.get(s_idx) {
                        let mut w = 180;
                        if !step.actions.is_empty() {
                            let max_act_len = step
                                .actions
                                .iter()
                                .map(|a| a.target.len())
                                .max()
                                .unwrap_or(0);
                            let target_w = (max_act_len as i32 * 8 + 20).max(110);
                            w += 14 + 26 + target_w;
                        }
                        // Add spacing margin between columns (at least 60px) and round up to 20px grid
                        let req_w = ((w + 60 + 19) / 20) * 20;
                        if c_idx < max_cols && req_w > col_widths[c_idx] {
                            col_widths[c_idx] = req_w;
                        }
                    }
                }
            }

            // Also check transitions in this tree
            for tr in &self.transitions {
                if tree.transition_ids.contains(&tr.id) {
                    let rec_chars = if tr.receptivity.is_empty() {
                        4
                    } else {
                        tr.receptivity.len()
                    };
                    let rec_w = 80 + 10 + (rec_chars as i32 * 8) + 20;
                    let req_w = ((rec_w + 60 + 19) / 20) * 20;
                    for row in &tree_rows {
                        for (c_idx, &s_idx) in row.iter().enumerate() {
                            if let Some(s) = self.steps.get(s_idx)
                                && (tr.from_steps.iter().any(|fs| fs == &s.id || fs == &s.name)
                                    || tr.to_steps.iter().any(|ts| ts == &s.id || ts == &s.name))
                                && c_idx < max_cols
                                && req_w > col_widths[c_idx]
                            {
                                col_widths[c_idx] = req_w;
                            }
                        }
                    }
                }
            }

            let mut col_offsets = vec![0_i32; max_cols];
            for c in 1..max_cols {
                col_offsets[c] = col_offsets[c - 1] + col_widths[c - 1];
            }
            let total_tree_cols_w: i32 = col_widths.iter().sum();

            let mid_span_center = (col_offsets[0] + 90 + col_offsets[max_cols - 1] + 90) / 2;
            let single_step_center_x = ((mid_span_center - 90 + 10) / 20) * 20;
            let single_trans_center_x = single_step_center_x + 50;

            let mut y = 40;

            for row in &tree_rows {
                let cols = row.len();

                for (c_idx, &s_idx) in row.iter().enumerate() {
                    placed_steps.insert(s_idx);
                    if let Some(step) = self.steps.get_mut(s_idx)
                        && step.grid_pos.is_none()
                    {
                        let x = if cols == 1 && max_cols > 1 {
                            current_tree_x + single_step_center_x
                        } else {
                            current_tree_x + col_offsets[c_idx]
                        };
                        step.grid_pos = Some((x, y));
                    }
                }

                // Find transitions originating from steps in this row
                let from_step_ids: Vec<String> = row
                    .iter()
                    .filter_map(|&si| self.steps.get(si).map(|s| s.id.clone()))
                    .collect();

                let mut trans_in_row = Vec::new();
                for (t_idx, tr) in self.transitions.iter().enumerate() {
                    if !placed_trans.contains(&t_idx)
                        && tree.transition_ids.contains(&tr.id)
                        && tr.from_steps.iter().any(|fs| {
                            from_step_ids.contains(fs)
                                || from_step_ids
                                    .iter()
                                    .any(|sid| sid == fs.strip_prefix('X').unwrap_or(fs))
                        })
                    {
                        trans_in_row.push(t_idx);
                    }
                }

                let max_acts = row
                    .iter()
                    .filter_map(|&si| self.steps.get(si))
                    .map(|s| s.actions.len())
                    .max()
                    .unwrap_or(0);
                let step_h = 60.max(if max_acts > 0 {
                    10 + (max_acts as i32 * 25)
                } else {
                    60
                });
                y += ((step_h + 60 + 19) / 20) * 20;

                if !trans_in_row.is_empty() {
                    let t_cols = trans_in_row.len();
                    for (c_idx, &t_idx) in trans_in_row.iter().enumerate() {
                        placed_trans.insert(t_idx);
                        if let Some(tr) = self.transitions.get_mut(t_idx)
                            && tr.grid_pos.is_none()
                        {
                            let x = if t_cols == 1 && max_cols > 1 {
                                current_tree_x + single_trans_center_x
                            } else if c_idx < max_cols {
                                current_tree_x + col_offsets[c_idx] + 50
                            } else {
                                current_tree_x
                                    + col_offsets[max_cols - 1]
                                    + 50
                                    + ((c_idx - max_cols + 1) as i32 * 280)
                            };
                            tr.grid_pos = Some((x, y));
                        }
                    }
                    y += 100;
                }
            }

            // Any unlinked transitions belonging to this tree
            for (t_idx, tr) in self.transitions.iter_mut().enumerate() {
                if tree.transition_ids.contains(&tr.id) && !placed_trans.contains(&t_idx) {
                    placed_trans.insert(t_idx);
                    if tr.grid_pos.is_none() {
                        let x = current_tree_x + 50;
                        tr.grid_pos = Some((x, y));
                        y += 80;
                    }
                }
            }

            current_tree_x += total_tree_cols_w + 140;
        }

        // 2. Any leftover unplaced steps or transitions (e.g. isolated or unreferenced)
        let mut unlinked_y = 40;
        for (s_idx, step) in self.steps.iter_mut().enumerate() {
            if !placed_steps.contains(&s_idx) && step.grid_pos.is_none() {
                step.grid_pos = Some((current_tree_x, unlinked_y));
                unlinked_y += 120;
            }
        }
        for (t_idx, tr) in self.transitions.iter_mut().enumerate() {
            if !placed_trans.contains(&t_idx) && tr.grid_pos.is_none() {
                tr.grid_pos = Some((current_tree_x + 50, unlinked_y));
                unlinked_y += 100;
            }
        }
    }

    pub fn compile(&self, tags: &TagTable) -> Result<CompiledSfcPou, SfcError> {
        let report = validate_sfc_topology(self);
        if !report.is_valid {
            // Check if entirely missing initial step (preserves exact SfcError::MissingInitialStep)
            if !self.steps.iter().any(|s| s.kind == StepKind::Initial) {
                return Err(SfcError::MissingInitialStep(self.name.clone()));
            }
            // Check for dangling steps first to preserve UnknownStep errors
            for err in &report.errors {
                if let SfcValidationError::DanglingStepReference {
                    referenced_step, ..
                } = err
                {
                    return Err(SfcError::UnknownStep {
                        pou: self.name.clone(),
                        step: referenced_step.clone(),
                    });
                }
            }
            return Err(SfcError::TopologyValidation {
                pou: self.name.clone(),
                errors: report.errors,
            });
        }

        let mut step_map: HashMap<String, usize> = HashMap::new();
        for (idx, step) in self.steps.iter().enumerate() {
            step_map.insert(step.id.clone(), idx);
        }

        let mut compiled_steps = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let mut compiled_actions = Vec::with_capacity(step.actions.len());
            for act in &step.actions {
                let duration_ms = match act.qualifier {
                    ActionQualifier::L | ActionQualifier::D => {
                        let raw = act.duration.as_deref().ok_or_else(|| {
                            SfcError::InvalidQualifierDuration {
                                pou: self.name.clone(),
                                qualifier: act.qualifier,
                                duration: String::new(),
                            }
                        })?;
                        parse_iec_time_literal(raw).ok_or_else(|| {
                            SfcError::InvalidQualifierDuration {
                                pou: self.name.clone(),
                                qualifier: act.qualifier,
                                duration: raw.to_string(),
                            }
                        })?
                    }
                    _ => 0,
                };

                // Allow SFC actions (`in_sfc = true`) to set, reset, or force step states of other SFC programs
                let kind = if act.target.contains(":=") || act.target.contains('(') {
                    let stmts = parse_st_statements(&act.target, &self.name).map_err(|source| {
                        SfcError::St {
                            pou: self.name.clone(),
                            source,
                        }
                    })?;
                    for s in &stmts {
                        s.validate(tags, &self.name, true)
                            .map_err(|source| SfcError::Ir {
                                pou: self.name.clone(),
                                source,
                            })?;
                    }
                    CompiledActionKind::Statements(stmts)
                } else {
                    let ty = SymbolResolver::validate_writable_symbol(
                        tags,
                        &act.target,
                        &self.name,
                        true,
                    )
                    .map_err(|source| SfcError::Ir {
                        pou: self.name.clone(),
                        source,
                    })?;
                    if ty != IecType::Bool {
                        return Err(SfcError::Ir {
                            pou: self.name.clone(),
                            source: IrError::TypeMismatch {
                                pou: self.name.clone(),
                                expected: IecType::Bool,
                                actual: ty,
                                context: format!("boolean step action '{}'", act.target),
                            },
                        });
                    }
                    CompiledActionKind::BooleanSymbol(act.target.trim().to_string())
                };

                compiled_actions.push(CompiledAction {
                    qualifier: act.qualifier,
                    kind,
                    duration_ms,
                });
            }

            let short_prefix = if step.id.starts_with('X') || step.id.starts_with('x') {
                step.id.clone()
            } else {
                format!("X{}", step.id)
            };
            let state_sym = if self.name.is_empty() {
                format!("{short_prefix}.state")
            } else {
                format!("{}.{short_prefix}.state", self.name)
            };
            let time_sym = if self.name.is_empty() {
                format!("{short_prefix}.t")
            } else {
                format!("{}.{short_prefix}.t", self.name)
            };

            compiled_steps.push(CompiledStep {
                id: step.id.clone(),
                name: step.name.clone(),
                state_sym,
                time_sym,
                initial: step.kind == StepKind::Initial,
                active: step.kind == StepKind::Initial,
                prev_active: false,
                elapsed_ms: 0,
                actions: compiled_actions,
            });
        }

        let mut compiled_transitions = Vec::with_capacity(self.transitions.len());
        for tr in &self.transitions {
            let mut from_indices = Vec::with_capacity(tr.from_steps.len());
            for fs in &tr.from_steps {
                let trimmed = fs.trim();
                let cleaned = trimmed
                    .strip_prefix('X')
                    .or_else(|| trimmed.strip_prefix('x'))
                    .unwrap_or(trimmed);
                let idx = step_map
                    .get(trimmed)
                    .or_else(|| step_map.get(cleaned))
                    .copied()
                    .ok_or_else(|| SfcError::UnknownStep {
                        pou: self.name.clone(),
                        step: fs.clone(),
                    })?;
                from_indices.push(idx);
            }
            let mut to_indices = Vec::with_capacity(tr.to_steps.len());
            for ts in &tr.to_steps {
                let trimmed = ts.trim();
                let cleaned = trimmed
                    .strip_prefix('X')
                    .or_else(|| trimmed.strip_prefix('x'))
                    .unwrap_or(trimmed);
                let idx = step_map
                    .get(trimmed)
                    .or_else(|| step_map.get(cleaned))
                    .copied()
                    .ok_or_else(|| SfcError::UnknownStep {
                        pou: self.name.clone(),
                        step: ts.clone(),
                    })?;
                to_indices.push(idx);
            }
            let expr = parse_st_expression(&tr.receptivity, &self.name).map_err(|source| {
                SfcError::St {
                    pou: self.name.clone(),
                    source,
                }
            })?;
            let rty = expr
                .infer_type(tags, &self.name)
                .map_err(|source| SfcError::Ir {
                    pou: self.name.clone(),
                    source,
                })?;
            if rty != IecType::Bool {
                return Err(SfcError::Ir {
                    pou: self.name.clone(),
                    source: IrError::TypeMismatch {
                        pou: self.name.clone(),
                        expected: IecType::Bool,
                        actual: rty,
                        context: format!("transition '{}' receptivity", tr.id),
                    },
                });
            }
            compiled_transitions.push(CompiledTransition {
                from_indices,
                to_indices,
                receptivity: expr,
                convergence: tr.convergence,
                divergence: tr.divergence,
            });
        }

        let trans_len = compiled_transitions.len();
        let steps_len = compiled_steps.len();

        Ok(CompiledSfcPou {
            id: PouId::new(self.name.clone()),
            initialized: false,
            steps: compiled_steps,
            transitions: compiled_transitions,
            fired_scratch: vec![false; trans_len],
            clear_scratch: vec![false; steps_len],
            activate_scratch: vec![false; steps_len],
        })
    }
}

#[derive(Debug, Clone)]
enum CompiledActionKind {
    BooleanSymbol(String),
    Statements(Vec<IrStmt>),
}

#[derive(Debug, Clone)]
struct CompiledAction {
    qualifier: ActionQualifier,
    kind: CompiledActionKind,
    duration_ms: u64,
}

#[derive(Debug, Clone)]
struct CompiledStep {
    id: String,
    name: String,
    state_sym: String,
    time_sym: String,
    initial: bool,
    active: bool,
    prev_active: bool,
    elapsed_ms: u64,
    actions: Vec<CompiledAction>,
}

#[derive(Debug, Clone)]
struct CompiledTransition {
    from_indices: Vec<usize>,
    to_indices: Vec<usize>,
    receptivity: IrExpr,
    convergence: ConvergenceKind,
    divergence: DivergenceKind,
}

/// Real-time safe Grafcet / Sequential Function Chart executable POU.
/// Pre-allocates evolution scratch buffers so `execute()` performs zero heap allocations.
#[derive(Debug, Clone)]
pub struct CompiledSfcPou {
    id: PouId,
    initialized: bool,
    steps: Vec<CompiledStep>,
    transitions: Vec<CompiledTransition>,
    fired_scratch: Vec<bool>,
    clear_scratch: Vec<bool>,
    activate_scratch: Vec<bool>,
}

impl CompiledSfcPou {
    fn sync_introspection_symbols(&self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
        ctx.with_sfc_engine(|sfc_ctx| -> Result<(), RuntimeError> {
            for step in &self.steps {
                let _ = sfc_ctx.write_symbol(&step.state_sym, IecValue::Bool(step.active));
                let _ = sfc_ctx.write_symbol(&step.time_sym, IecValue::Time(step.elapsed_ms));
            }
            Ok(())
        })
    }

    /// Reads any external Set / Reset / Force applied to this POU's step `.state` symbols in `TagTable`
    /// (for example by another SFC program's action or by interactive GUI Set/Force).
    fn sync_external_step_overrides(&mut self, ctx: &ScanContext<'_>) {
        for step in &mut self.steps {
            if let Some(tag) = ctx.tags.get_tag(&step.state_sym) {
                if let Some(IecValue::Bool(forced_b)) = tag.forced {
                    if step.active != forced_b {
                        step.active = forced_b;
                        step.elapsed_ms = 0;
                    }
                } else if let Ok(tag_b) = tag.current_value().as_bool()
                    && step.active != tag_b
                {
                    step.active = tag_b;
                    step.elapsed_ms = 0;
                }
            }
        }
    }

    /// Enforces active Forces (`tag.forced`) on step states after transition evolution.
    fn enforce_forced_steps(&mut self, ctx: &ScanContext<'_>) {
        for step in &mut self.steps {
            if let Some(tag) = ctx.tags.get_tag(&step.state_sym)
                && let Some(IecValue::Bool(forced_b)) = tag.forced
            {
                step.active = forced_b;
            }
        }
    }
    /// Resets all SFC steps back to initial configuration and clears elapsed active timers.
    pub fn reset(&mut self) {
        self.initialized = false;
        for step in &mut self.steps {
            step.active = step.initial;
            step.prev_active = false;
            step.elapsed_ms = 0;
        }
        self.fired_scratch.fill(false);
        self.clear_scratch.fill(false);
        self.activate_scratch.fill(false);
    }
}

impl ExecutablePou for CompiledSfcPou {
    fn id(&self) -> PouId {
        self.id.clone()
    }

    fn kind(&self) -> LanguageKind {
        LanguageKind::Sfc
    }

    fn reset(&mut self) {
        self.reset();
    }

    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
        if !self.initialized {
            for step in &mut self.steps {
                step.active = step.initial;
                step.prev_active = false;
                step.elapsed_ms = 0;
            }
            // Honor any external Set/Force applied before first scan
            self.enforce_forced_steps(ctx);
            self.initialized = true;
            self.sync_introspection_symbols(ctx)?;
        } else {
            // Pull in any external Set/Reset/Force from other SFC programs or GUI watch/force table
            self.sync_external_step_overrides(ctx);

            // Advance elapsed duration for active steps prior to evaluating receptivities
            for step in &mut self.steps {
                if step.active {
                    step.elapsed_ms = step.elapsed_ms.saturating_add(ctx.dt_ms);
                } else {
                    step.elapsed_ms = 0;
                }
            }
            self.sync_introspection_symbols(ctx)?;

            // IEC 60848 Phase 1: Evaluate all transition receptivities (supporting AND & OR convergence/divergence)
            self.fired_scratch.fill(false);
            self.clear_scratch.fill(false); // temporarily tracks steps claimed by an exclusive OR divergence
            for (t_idx, tr) in self.transitions.iter().enumerate() {
                if tr.from_indices.is_empty() {
                    continue;
                }
                let enabled = match tr.convergence {
                    ConvergenceKind::And => tr
                        .from_indices
                        .iter()
                        .all(|&s_idx| self.steps[s_idx].active),
                    ConvergenceKind::Or => tr
                        .from_indices
                        .iter()
                        .any(|&s_idx| self.steps[s_idx].active),
                };
                // If this transition is part of an exclusive OR divergence, skip if its upstream step was already claimed
                let already_claimed = tr.divergence == DivergenceKind::Or
                    && tr
                        .from_indices
                        .iter()
                        .any(|&s_idx| self.clear_scratch[s_idx]);

                if enabled && !already_claimed {
                    let cond = tr.receptivity.evaluate(ctx, self.id.as_str())?.as_bool()?;
                    if cond {
                        self.fired_scratch[t_idx] = true;
                        if tr.divergence == DivergenceKind::Or {
                            for &s_idx in &tr.from_indices {
                                self.clear_scratch[s_idx] = true;
                            }
                        }
                    }
                }
            }

            // IEC 60848 Phase 2 & 3: Simultaneous clearing of active steps & activation of target steps
            self.clear_scratch.fill(false);
            self.activate_scratch.fill(false);

            for (t_idx, tr) in self.transitions.iter().enumerate() {
                if self.fired_scratch[t_idx] {
                    for &s_idx in &tr.from_indices {
                        if self.steps[s_idx].active {
                            self.clear_scratch[s_idx] = true;
                        }
                    }
                    for &s_idx in &tr.to_indices {
                        self.activate_scratch[s_idx] = true;
                    }
                }
            }

            for (s_idx, step) in self.steps.iter_mut().enumerate() {
                step.prev_active = step.active;
                if self.clear_scratch[s_idx] {
                    step.active = false;
                    step.elapsed_ms = 0;
                }
                if self.activate_scratch[s_idx] {
                    step.active = true;
                    step.elapsed_ms = 0;
                }
            }

            // Enforce any active forces on steps (`F` / `FZ` or manual Force)
            self.enforce_forced_steps(ctx);
            self.sync_introspection_symbols(ctx)?;
        }

        // IEC 60848 Phase 4: Evaluate step actions (with SFC privilege so actions can Set/Reset/Force other SFC steps)
        let pou_name = self.id.as_str();
        ctx.with_sfc_engine(|sfc_ctx| -> Result<(), RuntimeError> {
            for step in &self.steps {
                let _ = (&step.id, &step.name);
                let just_deactivated = !step.active && step.prev_active;
                if just_deactivated {
                    for act in &step.actions {
                        match act.qualifier {
                            ActionQualifier::N
                            | ActionQualifier::L
                            | ActionQualifier::D
                            | ActionQualifier::P => {
                                if let CompiledActionKind::BooleanSymbol(ref sym) = act.kind {
                                    sfc_ctx.write_symbol(sym, IecValue::Bool(false))?;
                                }
                            }
                            ActionQualifier::F | ActionQualifier::FZ => {
                                if let CompiledActionKind::BooleanSymbol(ref sym) = act.kind {
                                    sfc_ctx.unforce_symbol(sym)?;
                                }
                            }
                            ActionQualifier::S | ActionQualifier::R => {}
                        }
                    }
                }
            }

            for step in &self.steps {
                if !step.active {
                    continue;
                }
                let rising_edge = step.active && !step.prev_active;
                for act in &step.actions {
                    match act.qualifier {
                        ActionQualifier::N => match &act.kind {
                            CompiledActionKind::BooleanSymbol(sym) => {
                                sfc_ctx.write_symbol(sym, IecValue::Bool(true))?;
                            }
                            CompiledActionKind::Statements(stmts) => {
                                for s in stmts {
                                    s.execute(sfc_ctx, pou_name)?;
                                }
                            }
                        },
                        ActionQualifier::S => match &act.kind {
                            CompiledActionKind::BooleanSymbol(sym) => {
                                sfc_ctx.write_symbol(sym, IecValue::Bool(true))?;
                            }
                            CompiledActionKind::Statements(stmts) => {
                                for s in stmts {
                                    s.execute(sfc_ctx, pou_name)?;
                                }
                            }
                        },
                        ActionQualifier::R => match &act.kind {
                            CompiledActionKind::BooleanSymbol(sym) => {
                                sfc_ctx.write_symbol(sym, IecValue::Bool(false))?;
                            }
                            CompiledActionKind::Statements(stmts) => {
                                for s in stmts {
                                    s.execute(sfc_ctx, pou_name)?;
                                }
                            }
                        },
                        ActionQualifier::F => {
                            if let CompiledActionKind::BooleanSymbol(sym) = &act.kind {
                                sfc_ctx.force_symbol(sym, IecValue::Bool(true))?;
                            }
                        }
                        ActionQualifier::FZ => {
                            if let CompiledActionKind::BooleanSymbol(sym) = &act.kind {
                                sfc_ctx.force_symbol(sym, IecValue::Bool(false))?;
                            }
                        }
                        ActionQualifier::L => {
                            let active_now = step.elapsed_ms < act.duration_ms;
                            match &act.kind {
                                CompiledActionKind::BooleanSymbol(sym) => {
                                    sfc_ctx.write_symbol(sym, IecValue::Bool(active_now))?;
                                }
                                CompiledActionKind::Statements(stmts) if active_now => {
                                    for s in stmts {
                                        s.execute(sfc_ctx, pou_name)?;
                                    }
                                }
                                _ => {}
                            }
                        }
                        ActionQualifier::D => {
                            let active_now = step.elapsed_ms >= act.duration_ms;
                            match &act.kind {
                                CompiledActionKind::BooleanSymbol(sym) => {
                                    sfc_ctx.write_symbol(sym, IecValue::Bool(active_now))?;
                                }
                                CompiledActionKind::Statements(stmts) if active_now => {
                                    for s in stmts {
                                        s.execute(sfc_ctx, pou_name)?;
                                    }
                                }
                                _ => {}
                            }
                        }
                        ActionQualifier::P => match &act.kind {
                            CompiledActionKind::BooleanSymbol(sym) => {
                                sfc_ctx.write_symbol(sym, IecValue::Bool(rising_edge))?;
                            }
                            CompiledActionKind::Statements(stmts) if rising_edge => {
                                for s in stmts {
                                    s.execute(sfc_ctx, pou_name)?;
                                }
                            }
                            _ => {}
                        },
                    }
                }
            }
            Ok(())
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::{ProcessImage, TagEntry};

    #[test]
    fn test_grafcet_iec60848_evolution_and_cross_language_receptivity()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        tags.register_tag(TagEntry::new(
            "Permissive_OK",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "From Ladder safety POU",
        ))?;
        tags.register_tag(TagEntry::new(
            "Sensor_1",
            IecType::Bool,
            Some("%IX0.0".parse()?),
            IecValue::Bool(false),
            "Start sensor",
        ))?;
        tags.register_tag(TagEntry::new(
            "Fill_Valve",
            IecType::Bool,
            Some("%QX0.0".parse()?),
            IecValue::Bool(false),
            "Valve output controlled by SFC N action",
        ))?;
        tags.register_tag(TagEntry::new(
            "Batch_Counter",
            IecType::Int,
            None,
            IecValue::Int(0),
            "Incremented by P action",
        ))?;

        let sfc = SfcProgram {
            name: "batch_seq".to_string(),
            comment: "Batch filling sequence".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step_Idle".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step_Fill".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::N,
                            target: "Fill_Valve".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::P,
                            target: "Batch_Counter := Batch_Counter + 1;".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
            ],
            transitions: vec![
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string()],
                    receptivity: "Permissive_OK AND Sensor_1".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T2".to_string(),
                    from_steps: vec!["1".to_string()],
                    to_steps: vec!["0".to_string()],
                    receptivity: "X1.t >= T#100ms".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
            ],
        };

        sfc.register_introspection_symbols(&mut tags)?;
        let mut compiled = sfc.compile(&tags)?;

        // Scan 1: Initialize initial step 0
        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("X0.state")?);
        assert!(!ctx.read_bool("X1.state")?);
        assert!(!ctx.read_bool("Fill_Valve")?);

        // Scan 2: Ladder sets Permissive_OK = TRUE, hardware sets Sensor_1 = TRUE
        ctx.write_symbol("Permissive_OK", IecValue::Bool(true))?;
        ctx.write_symbol("Sensor_1", IecValue::Bool(true))?;
        compiled.execute(&mut ctx)?;
        assert!(!ctx.read_bool("X0.state")?);
        assert!(ctx.read_bool("X1.state")?);
        assert!(ctx.read_bool("Fill_Valve")?);
        assert_eq!(ctx.read_symbol("Batch_Counter")?, IecValue::Int(1));

        // Scan 3 (+50ms): Step 1 remains active (X1.t = 50ms < 100ms); P action does NOT re-trigger
        ctx.write_symbol("Sensor_1", IecValue::Bool(false))?;
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("X1.state")?);
        assert_eq!(ctx.read_symbol("X1.t")?, IecValue::Time(50));
        assert_eq!(ctx.read_symbol("Batch_Counter")?, IecValue::Int(1));

        // Scan 4 (+50ms): X1.t reaches 100ms -> T2 fires -> Step 1 clears, Step 0 activates, Fill_Valve resets to FALSE
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("X0.state")?);
        assert!(!ctx.read_bool("X1.state")?);
        assert!(!ctx.read_bool("Fill_Valve")?);

        Ok(())
    }

    #[test]
    fn test_sfc_and_or_divergence_convergence_and_cross_sfc_set_force()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        tags.register_tag(TagEntry::new(
            "Start_Split",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Triggers AND divergence",
        ))?;
        tags.register_tag(TagEntry::new(
            "Join_Ready",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Triggers AND convergence",
        ))?;

        // 1. Master SFC with AND divergence (0 -> 1, 2) and AND convergence (1, 2 -> 3)
        // Step 1 controls Slave SFC via transition receptivity `master_seq.X1.state`
        // Step 3 sets `slave_seq.X2.state` (S) and resets `slave_seq.X1.state` (R)
        // Step 4 forces `slave_seq.X0.state` ON (F) and `slave_seq.X2.state` OFF (FZ)
        let master_sfc = SfcProgram {
            name: "master_seq".to_string(),
            comment: "Master Grafcet with AND/OR divergence & cross-SFC control".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Master_Idle".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Branch_A".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Branch_B".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "3".to_string(),
                    name: "Master_Set_Slave".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::R,
                            target: "slave_seq.X1.state".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::S,
                            target: "slave_seq.X2.state".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
                SfcStep {
                    id: "4".to_string(),
                    name: "Master_Force_Slave".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![
                        SfcAction {
                            qualifier: ActionQualifier::F,
                            target: "slave_seq.X0.state".to_string(),
                            duration: None,
                        },
                        SfcAction {
                            qualifier: ActionQualifier::FZ,
                            target: "slave_seq.X2.state".to_string(),
                            duration: None,
                        },
                    ],
                    ..Default::default()
                },
            ],
            transitions: vec![
                // AND Divergence: 0 -> [1, 2]
                SfcTransition {
                    id: "T_AndDiv".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string(), "2".to_string()],
                    receptivity: "Start_Split".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                // AND Convergence: [1, 2] -> 3
                SfcTransition {
                    id: "T_AndConv".to_string(),
                    from_steps: vec!["1".to_string(), "2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Join_Ready".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                // OR Convergence: [3] -> 4
                SfcTransition {
                    id: "T_ToForce".to_string(),
                    from_steps: vec!["3".to_string()],
                    to_steps: vec!["4".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::Or,
                    divergence: DivergenceKind::Or,
                    ..Default::default()
                },
            ],
        };

        // 2. Slave SFC controlled by Master SFC's active step state (`master_seq.X1.state`)
        let slave_sfc = SfcProgram {
            name: "slave_seq".to_string(),
            comment: "Slave Grafcet controlled by master_seq".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Slave_Idle".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Slave_Work".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Slave_Override".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "TS1".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string()],
                receptivity: "master_seq.X1.state".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };

        master_sfc.register_introspection_symbols(&mut tags)?;
        slave_sfc.register_introspection_symbols(&mut tags)?;

        let mut compiled_master = master_sfc.compile(&tags)?;
        let mut compiled_slave = slave_sfc.compile(&tags)?;

        // Scan 1: Initialize both SFCs
        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);
        compiled_master.execute(&mut ctx)?;
        compiled_slave.execute(&mut ctx)?;
        assert!(ctx.read_bool("master_seq.X0.state")?);
        assert!(ctx.read_bool("slave_seq.X0.state")?);

        // Scan 2: Trigger AND divergence in master_seq (0 -> 1 & 2 simultaneously)
        // Slave SFC's TS1 sees `master_seq.X1.state == TRUE` and transitions 0 -> 1!
        ctx.write_symbol("Start_Split", IecValue::Bool(true))?;
        compiled_master.execute(&mut ctx)?;
        compiled_slave.execute(&mut ctx)?;
        assert!(!ctx.read_bool("master_seq.X0.state")?);
        assert!(ctx.read_bool("master_seq.X1.state")?);
        assert!(ctx.read_bool("master_seq.X2.state")?);
        assert!(!ctx.read_bool("slave_seq.X0.state")?);
        assert!(ctx.read_bool("slave_seq.X1.state")?);

        // Scan 3: Trigger AND convergence in master_seq ([1, 2] -> 3)
        // Step 3 executes [R] slave_seq.X1.state and [S] slave_seq.X2.state!
        ctx.write_symbol("Join_Ready", IecValue::Bool(true))?;
        compiled_master.execute(&mut ctx)?;
        compiled_slave.execute(&mut ctx)?;
        assert!(!ctx.read_bool("master_seq.X1.state")?);
        assert!(!ctx.read_bool("master_seq.X2.state")?);
        assert!(ctx.read_bool("master_seq.X3.state")?);
        assert!(!ctx.read_bool("slave_seq.X1.state")?);
        assert!(ctx.read_bool("slave_seq.X2.state")?);

        // Scan 4: master_seq transitions 3 -> 4, executing [F] slave_seq.X0.state and [FZ] slave_seq.X2.state
        compiled_master.execute(&mut ctx)?;
        compiled_slave.execute(&mut ctx)?;
        assert!(ctx.read_bool("master_seq.X4.state")?);
        assert!(ctx.read_bool("slave_seq.X0.state")?);
        assert!(!ctx.read_bool("slave_seq.X2.state")?);

        Ok(())
    }

    #[test]
    fn test_sfc_multi_tree_detection_and_validation() {
        // 1. Two valid independent parallel trees in the same POU
        let multi_tree_prog = SfcProgram {
            name: "parallel_trees".to_string(),
            comment: "Two concurrent SFC trees".to_string(),
            steps: vec![
                // Tree 1: X0 -> T0 -> X1
                SfcStep::new("0", "Tree1_Init", StepKind::Initial),
                SfcStep::new("1", "Tree1_Step1", StepKind::Normal),
                // Tree 2: X10 -> T10 -> X11
                SfcStep::new("10", "Tree2_Init", StepKind::Initial),
                SfcStep::new("11", "Tree2_Step1", StepKind::Normal),
            ],
            transitions: vec![
                SfcTransition {
                    id: "T0".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
                SfcTransition {
                    id: "T10".to_string(),
                    from_steps: vec!["10".to_string()],
                    to_steps: vec!["11".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
            ],
        };

        let report = validate_sfc_topology(&multi_tree_prog);
        assert!(report.is_valid);
        assert_eq!(report.trees.len(), 2);
        assert_eq!(report.trees[0].initial_step_ids.len(), 1);
        assert_eq!(report.trees[1].initial_step_ids.len(), 1);

        // 2. Tree 2 is missing an initial step -> reports MissingInitialStepInTree
        let mut invalid_tree = multi_tree_prog.clone();
        invalid_tree.steps[2].kind = StepKind::Normal; // X10 is no longer Initial
        let report2 = validate_sfc_topology(&invalid_tree);
        assert!(!report2.is_valid);
        assert!(
            report2
                .errors
                .iter()
                .any(|e| matches!(e, SfcValidationError::MissingInitialStepInTree { .. }))
        );

        // 3. Dangling reference in a transition
        let mut dangling_tree = multi_tree_prog.clone();
        dangling_tree.transitions[0].to_steps = vec!["999".to_string()];
        let report3 = validate_sfc_topology(&dangling_tree);
        assert!(!report3.is_valid);
        assert!(
            report3
                .errors
                .iter()
                .any(|e| matches!(e, SfcValidationError::DanglingStepReference { .. }))
        );

        // 4. Spawning an unconnected orphan step does not create a fake parallel tree
        let mut with_orphan = multi_tree_prog.clone();
        with_orphan
            .steps
            .push(SfcStep::new("99", "OrphanStep", StepKind::Normal));
        let report4 = validate_sfc_topology(&with_orphan);
        assert_eq!(
            report4.trees.len(),
            2,
            "Orphan step must not be counted as a parallel tree"
        );
        assert!(report4.warnings.iter().any(|w| w.contains("is an orphan")));

        // 5. Spawning an unconnected orphan transition does not create a fake parallel tree
        let mut with_orphan_tr = multi_tree_prog;
        with_orphan_tr.transitions.push(SfcTransition {
            id: "T99".to_string(),
            from_steps: vec![],
            to_steps: vec![],
            receptivity: "TRUE".to_string(),
            convergence: ConvergenceKind::And,
            divergence: DivergenceKind::And,
            grid_pos: None,
        });
        let report5 = validate_sfc_topology(&with_orphan_tr);
        assert_eq!(
            report5.trees.len(),
            2,
            "Orphan transition must not be counted as a parallel tree"
        );
    }

    #[test]
    fn test_sfc_grid_pos_json_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        // 1. JSON without grid_pos defaults cleanly to None
        let raw_json = r#"{
            "name": "legacy_seq",
            "comment": "",
            "steps": [
                { "id": "0", "name": "Init", "kind": "initial", "actions": [] }
            ],
            "transitions": [
                { "id": "T0", "from_steps": ["0"], "to_steps": ["0"], "receptivity": "TRUE" }
            ]
        }"#;

        let mut prog = SfcProgram::from_json(raw_json)?;
        assert_eq!(prog.steps[0].grid_pos, None);
        assert_eq!(prog.transitions[0].grid_pos, None);

        // 2. assign_default_grid_positions computes valid coordinates
        prog.assign_default_grid_positions();
        assert!(prog.steps[0].grid_pos.is_some());
        assert!(prog.transitions[0].grid_pos.is_some());

        // 3. Custom coordinates serialize and deserialize accurately
        prog.steps[0].grid_pos = Some((160, 320));
        let serialized = prog.to_json_pretty()?;
        assert!(serialized.contains("\"grid_pos\""));

        let reloaded = SfcProgram::from_json(&serialized)?;
        assert_eq!(reloaded.steps[0].grid_pos, Some((160, 320)));

        Ok(())
    }

    #[test]
    fn test_sfc_multi_tree_parallel_execution() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();

        tags.register_tag(TagEntry::new(
            "Go_Tree1",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Go_Tree2",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Act_Tree1",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Act_Tree2",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;

        let prog = SfcProgram {
            name: "dual_trees".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "T1_Idle", StepKind::Initial),
                SfcStep {
                    id: "1".to_string(),
                    name: "T1_Active".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::N,
                        target: "Act_Tree1".to_string(),
                        duration: None,
                    }],
                    grid_pos: Some((100, 100)),
                },
                SfcStep::new("10", "T2_Idle", StepKind::Initial),
                SfcStep {
                    id: "11".to_string(),
                    name: "T2_Active".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::N,
                        target: "Act_Tree2".to_string(),
                        duration: None,
                    }],
                    grid_pos: Some((400, 100)),
                },
            ],
            transitions: vec![
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string()],
                    receptivity: "Go_Tree1".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: Some((100, 200)),
                },
                SfcTransition {
                    id: "T2".to_string(),
                    from_steps: vec!["10".to_string()],
                    to_steps: vec!["11".to_string()],
                    receptivity: "Go_Tree2".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: Some((400, 200)),
                },
            ],
        };

        prog.register_introspection_symbols(&mut tags)?;
        let mut compiled = prog.compile(&tags)?;

        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);
        compiled.execute(&mut ctx)?;
        // Both initial steps are active simultaneously!
        assert!(ctx.read_bool("dual_trees.X0.state")?);
        assert!(ctx.read_bool("dual_trees.X10.state")?);
        assert!(!ctx.read_bool("Act_Tree1")?);
        assert!(!ctx.read_bool("Act_Tree2")?);

        // Advance Tree 1 only
        ctx.write_symbol("Go_Tree1", IecValue::Bool(true))?;
        compiled.execute(&mut ctx)?;
        assert!(!ctx.read_bool("dual_trees.X0.state")?);
        assert!(ctx.read_bool("dual_trees.X1.state")?);
        assert!(ctx.read_bool("Act_Tree1")?);
        // Tree 2 remains unaffected in its initial state!
        assert!(ctx.read_bool("dual_trees.X10.state")?);
        assert!(!ctx.read_bool("Act_Tree2")?);

        // Advance Tree 2
        ctx.write_symbol("Go_Tree2", IecValue::Bool(true))?;
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("dual_trees.X11.state")?);
        assert!(ctx.read_bool("Act_Tree2")?);
        assert!(ctx.read_bool("Act_Tree1")?);

        Ok(())
    }

    #[test]
    fn test_sfc_auto_sort_layout_multi_tree_and_divergence()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut prog = SfcProgram {
            name: "multi_auto_sort".to_string(),
            comment: String::new(),
            steps: vec![
                SfcStep::new("0", "Init1", StepKind::Initial),
                SfcStep::new("1", "BranchA", StepKind::Normal),
                SfcStep::new("2", "BranchB", StepKind::Normal),
                SfcStep::new("3", "Join", StepKind::Normal),
                // Tree 2
                SfcStep::new("10", "Init2", StepKind::Initial),
                SfcStep::new("11", "Run2", StepKind::Normal),
            ],
            transitions: vec![
                // T1: AND divergence 0 -> 1, 2
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string(), "2".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
                // T2: AND convergence 1, 2 -> 3
                SfcTransition {
                    id: "T2".to_string(),
                    from_steps: vec!["1".to_string(), "2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
                // Tree 2 transition
                SfcTransition {
                    id: "T3".to_string(),
                    from_steps: vec!["10".to_string()],
                    to_steps: vec!["11".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    grid_pos: None,
                },
            ],
        };

        // All grid_pos are initially None
        assert!(prog.steps.iter().all(|s| s.grid_pos.is_none()));
        assert!(prog.transitions.iter().all(|t| t.grid_pos.is_none()));

        // Run auto-sort
        prog.assign_default_grid_positions();

        // Every step and transition now has a valid grid position
        assert!(prog.steps.iter().all(|s| s.grid_pos.is_some()));
        assert!(prog.transitions.iter().all(|t| t.grid_pos.is_some()));

        let get_step_pos = |id: &str| -> Result<(i32, i32), Box<dyn std::error::Error>> {
            prog.steps
                .iter()
                .find(|s| s.id == id)
                .and_then(|s| s.grid_pos)
                .ok_or_else(|| format!("missing grid_pos for step {id}").into())
        };
        let get_trans_pos = |id: &str| -> Result<(i32, i32), Box<dyn std::error::Error>> {
            prog.transitions
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.grid_pos)
                .ok_or_else(|| format!("missing grid_pos for transition {id}").into())
        };

        let s0 = get_step_pos("0")?;
        let s1 = get_step_pos("1")?;
        let s2 = get_step_pos("2")?;
        let s3 = get_step_pos("3")?;
        let s10 = get_step_pos("10")?;
        let s11 = get_step_pos("11")?;

        let t1 = get_trans_pos("T1")?;
        let t2 = get_trans_pos("T2")?;
        let t3 = get_trans_pos("T3")?;

        // Tree 1: 2 parallel branches (max_cols = 2).
        // Branch A (s1) at x=60, Branch B (s2) at x=340 (spaced by min col width 280).
        // Single steps s0 and s3 are horizontally centered between the branches:
        // center = (150 + 430) / 2 = 290 -> step_x = 290 - 90 = 200.
        assert_eq!(s1.0, 60);
        assert_eq!(s2.0, 340);
        assert_eq!(s0.0, 200);
        assert_eq!(s3.0, 200);

        // T1 and T2 are centered at 290 -> trans_x = 290 - 40 = 250.
        assert_eq!(t1.0, 250);
        assert_eq!(t2.0, 250);

        // Y order in Tree 1: s0 < t1 < s1 & s2 < t2 < s3
        assert!(s0.1 < t1.1);
        assert!(t1.1 < s1.1);
        assert_eq!(s1.1, s2.1);
        assert!(s1.1 < t2.1);
        assert!(t2.1 < s3.1);

        // Tree 2 must be separated into a distinct horizontal band (x > 340 + 280)
        assert!(s10.0 > s2.0);
        assert_eq!(s10.0, s11.0);
        assert_eq!(t3.0, s10.0 + 50);
        assert!(s10.1 < t3.1);
        assert!(t3.1 < s11.1);

        Ok(())
    }

    #[test]
    fn test_sfc_auto_sort_expands_columns_for_actions_and_text()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut prog = SfcProgram {
            name: "wide_actions_sort".to_string(),
            comment: String::new(),
            steps: vec![
                SfcStep::new("0", "Init", StepKind::Initial),
                SfcStep {
                    id: "1".to_string(),
                    name: "Branch_Long_Action".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![SfcAction {
                        qualifier: ActionQualifier::P,
                        target: "Batch_Count := Batch_Count + 1;".to_string(), // 32 chars
                        duration: None,
                    }],
                    grid_pos: None,
                },
                SfcStep::new("2", "Branch_B", StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T1".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string(), "2".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                grid_pos: None,
            }],
        };

        prog.assign_default_grid_positions();

        let s1 = prog
            .steps
            .iter()
            .find(|s| s.id == "1")
            .and_then(|s| s.grid_pos)
            .unwrap_or((0, 0));
        let s2 = prog
            .steps
            .iter()
            .find(|s| s.id == "2")
            .and_then(|s| s.grid_pos)
            .unwrap_or((0, 0));

        // Column 0 must be expanded significantly beyond 280 to accommodate the 32-character action text
        let col_gap = s2.0 - s1.0;
        assert!(
            col_gap >= 500,
            "Expected column gap >= 500 to fit actions, got {col_gap}"
        );

        Ok(())
    }

    #[test]
    fn test_compiled_sfc_pou_reset() -> Result<(), Box<dyn std::error::Error>> {
        let sfc = SfcProgram {
            name: "test_reset".to_string(),
            comment: "Test reset".to_string(),
            steps: vec![
                SfcStep {
                    id: "0".to_string(),
                    name: "Step0".to_string(),
                    kind: StepKind::Initial,
                    actions: vec![],
                    ..Default::default()
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Step1".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                    ..Default::default()
                },
            ],
            transitions: vec![SfcTransition {
                id: "T0".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };
        let mut tags = TagTable::new();
        sfc.register_introspection_symbols(&mut tags)?;
        let mut compiled = sfc.compile(&tags)?;
        let mut pi = ProcessImage::new(128);
        // Scan 1: Initial step 0 is active
        let mut ctx1 = ScanContext::new(&mut pi, &mut tags, 50, 1);
        compiled.execute(&mut ctx1)?;
        assert!(ctx1.read_bool("test_reset.X0.state")?);
        assert!(!ctx1.read_bool("test_reset.X1.state")?);

        // Scan 2: T0 fires, activating step 1
        let mut ctx2 = ScanContext::new(&mut pi, &mut tags, 50, 2);
        compiled.execute(&mut ctx2)?;
        assert!(!ctx2.read_bool("test_reset.X0.state")?);
        assert!(ctx2.read_bool("test_reset.X1.state")?);

        // Reset: returns to initial state
        compiled.reset();
        let mut ctx3 = ScanContext::new(&mut pi, &mut tags, 50, 3);
        compiled.execute(&mut ctx3)?;
        assert!(ctx3.read_bool("test_reset.X0.state")?);
        assert!(!ctx3.read_bool("test_reset.X1.state")?);
        Ok(())
    }

    #[test]
    fn test_sfc_validation_and_divergence_rule() {
        // Valid AND divergence: Transition T0 connects Step 0 to Steps 1 and 2
        let valid_and_div = SfcProgram {
            name: "valid_and_div".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T0".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string(), "2".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };
        let rep_valid = validate_sfc_topology(&valid_and_div);
        assert!(
            rep_valid.errors.iter().all(|e| !matches!(e, SfcValidationError::InvalidAndDivergence { .. })),
            "Valid AND divergence should not have InvalidAndDivergence errors"
        );

        // Invalid AND divergence: T0 has multiple to_steps but empty from_steps
        let invalid_and_div = SfcProgram {
            name: "invalid_and_div".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T0".to_string(),
                from_steps: vec![],
                to_steps: vec!["1".to_string(), "2".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };
        let rep_invalid = validate_sfc_topology(&invalid_and_div);
        assert!(
            rep_invalid.errors.iter().any(|e| matches!(e, SfcValidationError::InvalidAndDivergence { transition_id, .. } if transition_id == "T0")),
            "Expected InvalidAndDivergence error on T0"
        );
    }

    #[test]
    fn test_sfc_validation_and_convergence_rule() {
        // Valid AND convergence: Steps 1 and 2 synchronize into Transition T0 -> Step 3
        let valid_and_conv = SfcProgram {
            name: "valid_and_conv".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
                SfcStep::new("3", "Step3", StepKind::Normal),
            ],
            transitions: vec![
                SfcTransition {
                    id: "T_start".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string(), "2".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T_conv".to_string(),
                    from_steps: vec!["1".to_string(), "2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
            ],
        };
        let rep_valid = validate_sfc_topology(&valid_and_conv);
        assert!(
            rep_valid.errors.iter().all(|e| !matches!(e, SfcValidationError::InvalidAndConvergence { .. })),
            "Valid AND convergence should not have InvalidAndConvergence errors"
        );

        // Invalid AND convergence: T_conv has multiple from_steps but empty to_steps
        let invalid_and_conv = SfcProgram {
            name: "invalid_and_conv".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T_conv".to_string(),
                from_steps: vec!["1".to_string(), "2".to_string()],
                to_steps: vec![],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };
        let rep_invalid = validate_sfc_topology(&invalid_and_conv);
        assert!(
            rep_invalid.errors.iter().any(|e| matches!(e, SfcValidationError::InvalidAndConvergence { transition_id, .. } if transition_id == "T_conv")),
            "Expected InvalidAndConvergence error on T_conv"
        );
    }

    #[test]
    fn test_sfc_validation_or_divergence_rule() {
        // Valid OR divergence: Step 0 diverges to two branches, each having its own transition
        let valid_or_div = SfcProgram {
            name: "valid_or_div".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
            ],
            transitions: vec![
                SfcTransition {
                    id: "T0_A".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string()],
                    receptivity: "BranchA".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::Or,
                    ..Default::default()
                },
                SfcTransition {
                    id: "T0_B".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["2".to_string()],
                    receptivity: "BranchB".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::Or,
                    ..Default::default()
                },
            ],
        };
        let rep_valid = validate_sfc_topology(&valid_or_div);
        assert!(
            rep_valid.errors.iter().all(|e| !matches!(e, SfcValidationError::InvalidOrDivergence { .. })),
            "Valid OR divergence should not have InvalidOrDivergence errors"
        );

        // Invalid OR divergence: A single transition T0 attempts an OR divergence to multiple target steps
        let invalid_or_div = SfcProgram {
            name: "invalid_or_div".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T0".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string(), "2".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::Or,
                ..Default::default()
            }],
        };
        let rep_invalid = validate_sfc_topology(&invalid_or_div);
        assert!(
            rep_invalid.errors.iter().any(|e| matches!(e, SfcValidationError::InvalidOrDivergence { element_id, .. } if element_id == "T0")),
            "Expected InvalidOrDivergence error on single transition T0 branching to multiple target steps"
        );
    }

    #[test]
    fn test_sfc_validation_or_convergence_rule() {
        // Valid OR convergence: Two independent transitions T1 and T2 converge into target Step 3
        let valid_or_conv = SfcProgram {
            name: "valid_or_conv".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
                SfcStep::new("3", "Step3", StepKind::Normal),
            ],
            transitions: vec![
                SfcTransition {
                    id: "T1".to_string(),
                    from_steps: vec!["1".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Done1".to_string(),
                    ..Default::default()
                },
                SfcTransition {
                    id: "T2".to_string(),
                    from_steps: vec!["2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Done2".to_string(),
                    ..Default::default()
                },
            ],
        };
        let rep_valid = validate_sfc_topology(&valid_or_conv);
        assert!(
            rep_valid.errors.iter().all(|e| !matches!(e, SfcValidationError::InvalidOrConvergence { .. })),
            "Valid OR convergence should not produce InvalidOrConvergence errors"
        );

        // Invalid OR convergence: A single transition attempting to merge multiple steps with ConvergenceKind::Or
        let invalid_single_or_conv = SfcProgram {
            name: "invalid_single_or_conv".to_string(),
            comment: "".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "Step1", StepKind::Normal),
                SfcStep::new("2", "Step2", StepKind::Normal),
                SfcStep::new("3", "Step3", StepKind::Normal),
            ],
            transitions: vec![SfcTransition {
                id: "T_conv".to_string(),
                from_steps: vec!["1".to_string(), "2".to_string()],
                to_steps: vec!["3".to_string()],
                receptivity: "TRUE".to_string(),
                convergence: ConvergenceKind::Or,
                divergence: DivergenceKind::And,
                ..Default::default()
            }],
        };
        let rep_invalid_single = validate_sfc_topology(&invalid_single_or_conv);
        assert!(
            rep_invalid_single.errors.iter().any(|e| matches!(e, SfcValidationError::InvalidOrConvergence { element_id, .. } if element_id == "T_conv")),
            "Expected InvalidOrConvergence error when a single transition has multiple from_steps with ConvergenceKind::Or"
        );
    }

    #[test]
    fn test_sfc_or_convergence_state_passes_when_variables_true() -> Result<(), Box<dyn std::error::Error>> {
        let mut img = ProcessImage::default();
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Merge_Permissive",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Merge condition",
        ))?;

        let sfc = SfcProgram {
            name: "or_conv_run".to_string(),
            comment: "Test OR convergence state transition".to_string(),
            steps: vec![
                SfcStep::new("0", "Initial", StepKind::Initial),
                SfcStep::new("1", "BranchA", StepKind::Normal),
                SfcStep::new("2", "BranchB", StepKind::Normal),
                SfcStep::new("3", "MergedStep", StepKind::Normal),
            ],
            transitions: vec![
                // Step 0 -> Step 1 (activated first)
                SfcTransition {
                    id: "T0".to_string(),
                    from_steps: vec!["0".to_string()],
                    to_steps: vec!["1".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                // Step 1 -> Step 3 (Branch A convergence transition)
                SfcTransition {
                    id: "T1_Merge".to_string(),
                    from_steps: vec!["1".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Merge_Permissive".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
                // Step 2 -> Step 3 (Branch B convergence transition)
                SfcTransition {
                    id: "T2_Merge".to_string(),
                    from_steps: vec!["2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Merge_Permissive".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                    ..Default::default()
                },
            ],
        };

        sfc.register_introspection_symbols(&mut tags)?;
        let mut compiled = sfc.compile(&tags)?;
        let mut ctx = ScanContext::new(&mut img, &mut tags, 50, 1);

        // Scan 1: Initialize initial step 0
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("or_conv_run.X0.state")?, "Step 0 should be active on first scan");
        assert!(!ctx.read_bool("or_conv_run.X1.state")?, "Step 1 should not be active yet");

        // Scan 2: T0 (TRUE) fires -> Step 0 clears, Step 1 activates
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("or_conv_run.X1.state")?, "Step 1 should be active after T0 fires");
        assert!(!ctx.read_bool("or_conv_run.X2.state")?, "Step 2 should be inactive");
        assert!(!ctx.read_bool("or_conv_run.X3.state")?, "Step 3 should not be active yet");

        // Scan 3: Merge_Permissive is still FALSE -> Step 1 remains active, Step 3 inactive.
        compiled.execute(&mut ctx)?;
        assert!(ctx.read_bool("or_conv_run.X1.state")?, "Step 1 should remain active");
        assert!(!ctx.read_bool("or_conv_run.X3.state")?, "Step 3 should not be active yet");

        // Set Merge_Permissive to TRUE:
        ctx.write_symbol("Merge_Permissive", IecValue::Bool(true))?;

        // Scan 4: OR convergence transition T1_Merge must fire because Step 1 is active and Merge_Permissive is TRUE!
        compiled.execute(&mut ctx)?;
        assert!(!ctx.read_bool("or_conv_run.X1.state")?, "Step 1 should clear when T1_Merge fires");
        assert!(ctx.read_bool("or_conv_run.X3.state")?, "State MUST pass to Step 3 through OR convergence!");

        Ok(())
    }
}
