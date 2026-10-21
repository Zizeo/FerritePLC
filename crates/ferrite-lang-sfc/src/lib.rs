use std::collections::HashMap;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Initial,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcStep {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub kind: StepKind,
    #[serde(default)]
    pub actions: Vec<SfcAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn compile(&self, tags: &TagTable) -> Result<CompiledSfcPou, SfcError> {
        if !self.steps.iter().any(|s| s.kind == StepKind::Initial) {
            return Err(SfcError::MissingInitialStep(self.name.clone()));
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
                    let stmts =
                        parse_st_statements(&act.target, &self.name).map_err(|source| {
                            SfcError::St {
                                pou: self.name.clone(),
                                source,
                            }
                        })?;
                    for s in &stmts {
                        s.validate(tags, &self.name, true).map_err(|source| {
                            SfcError::Ir {
                                pou: self.name.clone(),
                                source,
                            }
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
                let cleaned = trimmed.strip_prefix('X').or_else(|| trimmed.strip_prefix('x')).unwrap_or(trimmed);
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
                let cleaned = trimmed.strip_prefix('X').or_else(|| trimmed.strip_prefix('x')).unwrap_or(trimmed);
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
            let rty = expr.infer_type(tags, &self.name).map_err(|source| SfcError::Ir {
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
}

impl ExecutablePou for CompiledSfcPou {
    fn id(&self) -> PouId {
        self.id.clone()
    }

    fn kind(&self) -> LanguageKind {
        LanguageKind::Sfc
    }

    fn execute(&mut self, ctx: &mut ScanContext<'_>) -> Result<(), RuntimeError> {
        if !self.initialized {
            for step in &mut self.steps {
                step.active = step.initial;
                step.prev_active = false;
                step.elapsed_ms = 0;
            }
            // Honor any external Set/Force applied before first scan
            self.sync_external_step_overrides(ctx);
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
                    ConvergenceKind::And => {
                        tr.from_indices.iter().all(|&s_idx| self.steps[s_idx].active)
                    }
                    ConvergenceKind::Or => {
                        tr.from_indices.iter().any(|&s_idx| self.steps[s_idx].active)
                    }
                };
                // If this transition is part of an exclusive OR divergence, skip if its upstream step was already claimed
                let already_claimed = tr.divergence == DivergenceKind::Or
                    && tr
                        .from_indices
                        .iter()
                        .any(|&s_idx| self.clear_scratch[s_idx]);

                if enabled && !already_claimed {
                    let cond = tr
                        .receptivity
                        .evaluate(ctx, self.id.as_str())?
                        .as_bool()?;
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
                },
                SfcTransition {
                    id: "T2".to_string(),
                    from_steps: vec!["1".to_string()],
                    to_steps: vec!["0".to_string()],
                    receptivity: "X1.t >= T#100ms".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
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
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Branch_A".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Branch_B".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
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
                },
                // AND Convergence: [1, 2] -> 3
                SfcTransition {
                    id: "T_AndConv".to_string(),
                    from_steps: vec!["1".to_string(), "2".to_string()],
                    to_steps: vec!["3".to_string()],
                    receptivity: "Join_Ready".to_string(),
                    convergence: ConvergenceKind::And,
                    divergence: DivergenceKind::And,
                },
                // OR Convergence: [3] -> 4
                SfcTransition {
                    id: "T_ToForce".to_string(),
                    from_steps: vec!["3".to_string()],
                    to_steps: vec!["4".to_string()],
                    receptivity: "TRUE".to_string(),
                    convergence: ConvergenceKind::Or,
                    divergence: DivergenceKind::Or,
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
                },
                SfcStep {
                    id: "1".to_string(),
                    name: "Slave_Work".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                },
                SfcStep {
                    id: "2".to_string(),
                    name: "Slave_Override".to_string(),
                    kind: StepKind::Normal,
                    actions: vec![],
                },
            ],
            transitions: vec![SfcTransition {
                id: "TS1".to_string(),
                from_steps: vec!["0".to_string()],
                to_steps: vec!["1".to_string()],
                receptivity: "master_seq.X1.state".to_string(),
                convergence: ConvergenceKind::And,
                divergence: DivergenceKind::And,
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
}
