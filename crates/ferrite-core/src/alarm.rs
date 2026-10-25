use serde::{Deserialize, Serialize};

use crate::types::IecValue;

/// Standard ISA-18.2 Alarm Operational States.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlarmState {
    #[default]
    Normal,
    ActiveUnacknowledged,
    ActiveAcknowledged,
    ClearedUnacknowledged,
}

impl AlarmState {
    pub fn is_active(self) -> bool {
        matches!(self, Self::ActiveUnacknowledged | Self::ActiveAcknowledged)
    }

    pub fn is_unacknowledged(self) -> bool {
        matches!(
            self,
            Self::ActiveUnacknowledged | Self::ClearedUnacknowledged
        )
    }
}

/// Alarm Urgency / Severity according to ISA-18.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AlarmSeverity {
    #[default]
    Warning,
    Minor,
    Major,
    Critical,
}

/// Condition that triggers the alarm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlarmCondition {
    High { limit: f32 },
    Low { limit: f32 },
    Digital { trigger_value: bool },
}

/// Configured alarm definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlarmDefinition {
    pub id: String,
    pub tag: String,
    pub condition: AlarmCondition,
    pub severity: AlarmSeverity,
    pub message: String,
}

/// Live Alarm Instance tracked inside the PLC Runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveAlarm {
    pub definition: AlarmDefinition,
    pub state: AlarmState,
    pub last_transition_ms: u64,
    pub trigger_count: u64,
}

impl ActiveAlarm {
    pub fn new(definition: AlarmDefinition) -> Self {
        Self {
            definition,
            state: AlarmState::Normal,
            last_transition_ms: 0,
            trigger_count: 0,
        }
    }

    /// Evaluates the current tag value and updates the ISA-18.2 alarm state.
    pub fn evaluate(&mut self, current_val: &IecValue, time_ms: u64) -> bool {
        let condition_met = match &self.definition.condition {
            AlarmCondition::High { limit } => {
                current_val.as_real().map(|v| v >= *limit).unwrap_or(false)
            }
            AlarmCondition::Low { limit } => {
                current_val.as_real().map(|v| v <= *limit).unwrap_or(false)
            }
            AlarmCondition::Digital { trigger_value } => {
                current_val.as_bool().map(|v| v == *trigger_value).unwrap_or(false)
            }
        };

        let old_state = self.state;
        match (self.state, condition_met) {
            (AlarmState::Normal, true) => {
                self.state = AlarmState::ActiveUnacknowledged;
                self.trigger_count = self.trigger_count.saturating_add(1);
                self.last_transition_ms = time_ms;
            }
            (AlarmState::ActiveAcknowledged, false) => {
                self.state = AlarmState::Normal;
                self.last_transition_ms = time_ms;
            }
            (AlarmState::ActiveUnacknowledged, false) => {
                self.state = AlarmState::ClearedUnacknowledged;
                self.last_transition_ms = time_ms;
            }
            (AlarmState::ClearedUnacknowledged, true) => {
                self.state = AlarmState::ActiveUnacknowledged;
                self.last_transition_ms = time_ms;
            }
            _ => {}
        }
        old_state != self.state
    }

    /// Operator acknowledgement (ISA-18.2 Ack).
    pub fn acknowledge(&mut self, time_ms: u64) {
        match self.state {
            AlarmState::ActiveUnacknowledged => {
                self.state = AlarmState::ActiveAcknowledged;
                self.last_transition_ms = time_ms;
            }
            AlarmState::ClearedUnacknowledged => {
                self.state = AlarmState::Normal;
                self.last_transition_ms = time_ms;
            }
            _ => {}
        }
    }
}

/// Historical Alarm Journal Event (ISA-18.2 Sequence of Events).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmEventRecord {
    pub timestamp_ms: u64,
    pub alarm_id: String,
    pub state: AlarmState,
    pub severity: AlarmSeverity,
    pub message: String,
}
