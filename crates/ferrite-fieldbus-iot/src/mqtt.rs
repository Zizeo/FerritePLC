use std::collections::HashMap;

use ferrite_core::{IecValue, TagTable};
use serde::{Deserialize, Serialize};

/// Metric entry following Sparkplug B / Industrial IoT tag telemetry standards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IotMetric {
    pub name: String,
    pub alias: Option<u64>,
    pub timestamp_ms: u64,
    pub datatype: String,
    pub value: IecValue,
}

/// Industrial IoT Telemetry Payload (JSON / Sparkplug B compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IotTelemetryPayload {
    pub timestamp_ms: u64,
    pub seq: u64,
    pub metrics: Vec<IotMetric>,
}

/// MQTT topic structure for industrial PLC deployments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttTopic {
    pub group_id: String,
    pub edge_node_id: String,
    pub device_id: String,
}

impl MqttTopic {
    pub fn new(group: impl Into<String>, node: impl Into<String>, device: impl Into<String>) -> Self {
        Self {
            group_id: group.into(),
            edge_node_id: node.into(),
            device_id: device.into(),
        }
    }

    /// Returns Sparkplug B DDATA (Device Data) topic:
    /// `spBv1.0/{group_id}/DDATA/{edge_node_id}/{device_id}`
    pub fn sparkplug_ddata_topic(&self) -> String {
        format!(
            "spBv1.0/{}/DDATA/{}/{}",
            self.group_id, self.edge_node_id, self.device_id
        )
    }

    /// Returns standard JSON telemetry topic:
    /// `ferrite/{group_id}/{device_id}/telemetry`
    pub fn standard_telemetry_topic(&self) -> String {
        format!("ferrite/{}/{}/telemetry", self.group_id, self.device_id)
    }
}

/// Edge telemetry publisher engine with change-of-value (COV) deadband filtering.
pub struct MqttTelemetryEngine {
    pub topic: MqttTopic,
    seq: u64,
    last_published: HashMap<String, IecValue>,
}

impl MqttTelemetryEngine {
    pub fn new(topic: MqttTopic) -> Self {
        Self {
            topic,
            seq: 0,
            last_published: HashMap::new(),
        }
    }

    /// Generates a full snapshot payload containing all variables in `TagTable`.
    pub fn build_full_birth_payload(&mut self, tags: &TagTable, timestamp_ms: u64) -> IotTelemetryPayload {
        self.seq = self.seq.saturating_add(1);
        let mut metrics = Vec::new();

        for (idx, tag) in tags.tags().iter().enumerate() {
            let val = tag.current_value().clone();
            self.last_published.insert(tag.name.clone(), val.clone());
            metrics.push(IotMetric {
                name: tag.name.clone(),
                alias: Some(idx as u64),
                timestamp_ms,
                datatype: format!("{:?}", tag.data_type),
                value: val,
            });
        }

        IotTelemetryPayload {
            timestamp_ms,
            seq: self.seq,
            metrics,
        }
    }

    /// Generates a delta payload containing only changed tags (Report by Exception / COV).
    pub fn build_cov_payload(&mut self, tags: &TagTable, timestamp_ms: u64) -> Option<IotTelemetryPayload> {
        let mut metrics = Vec::new();

        for (idx, tag) in tags.tags().iter().enumerate() {
            let val = tag.current_value();
            let changed = match self.last_published.get(&tag.name) {
                Some(prev) => prev != val,
                None => true,
            };

            if changed {
                self.last_published.insert(tag.name.clone(), val.clone());
                metrics.push(IotMetric {
                    name: tag.name.clone(),
                    alias: Some(idx as u64),
                    timestamp_ms,
                    datatype: format!("{:?}", tag.data_type),
                    value: val.clone(),
                });
            }
        }

        if metrics.is_empty() {
            None
        } else {
            self.seq = self.seq.saturating_add(1);
            Some(IotTelemetryPayload {
                timestamp_ms,
                seq: self.seq,
                metrics,
            })
        }
    }

    /// Serializes a telemetry payload to JSON for MQTT publishing.
    pub fn serialize_json(payload: &IotTelemetryPayload) -> Result<String, serde_json::Error> {
        serde_json::to_string(payload)
    }
}
