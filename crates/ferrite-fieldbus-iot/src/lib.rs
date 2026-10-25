pub mod mqtt;
pub mod opcua;

pub use mqtt::{IotMetric, IotTelemetryPayload, MqttTelemetryEngine, MqttTopic};
pub use opcua::{
    OpcUaAccessLevel, OpcUaAddressSpace, OpcUaBrowseItem, OpcUaNodeId, OpcUaStatusCode,
    OpcUaVariableNode,
};

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_core::{IecType, IecValue, TagEntry, TagTable};

    #[test]
    fn test_mqtt_telemetry_engine_and_cov() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Tank_Level",
            IecType::Int,
            None,
            IecValue::Int(20),
            "Tank Level",
        ))?;
        tags.register_tag(TagEntry::new(
            "Pump_Active",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Pump Running",
        ))?;

        let topic = MqttTopic::new("plant1", "edge_node_01", "tank_controller");
        assert_eq!(
            topic.sparkplug_ddata_topic(),
            "spBv1.0/plant1/DDATA/edge_node_01/tank_controller"
        );

        let mut engine = MqttTelemetryEngine::new(topic);
        // Birth payload has all metrics
        let birth = engine.build_full_birth_payload(&tags, 1000);
        assert_eq!(birth.metrics.len(), 2);
        assert_eq!(birth.seq, 1);

        // Immediate COV payload with no changes -> None
        let no_change = engine.build_cov_payload(&tags, 1010);
        assert!(no_change.is_none());

        // Update tag value
        tags.set_tag_value("Tank_Level", IecValue::Int(45))?;
        let cov = engine.build_cov_payload(&tags, 1020);
        assert!(cov.is_some());
        let cov_payload = cov.ok_or("COV payload expected")?;
        assert_eq!(cov_payload.metrics.len(), 1);
        assert_eq!(cov_payload.metrics[0].name, "Tank_Level");
        assert_eq!(cov_payload.metrics[0].value, IecValue::Int(45));

        let json = MqttTelemetryEngine::serialize_json(&cov_payload)?;
        assert!(json.contains("\"Tank_Level\""));
        assert!(json.contains("\"value\":45"));

        Ok(())
    }

    #[test]
    fn test_opcua_address_space_read_write_browse() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Agitator_Speed",
            IecType::Real,
            None,
            IecValue::Real(120.0),
            "Agitator RPM",
        ))?;

        let mut addr_space = OpcUaAddressSpace::new();
        addr_space.sync_from_tag_table(&tags);

        let nodes = addr_space.browse_nodes();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].browse_name, "Agitator_Speed");
        assert_eq!(nodes[0].node_id, "ns=2;s=Agitator_Speed");

        let val = addr_space.read_node("ns=2;s=Agitator_Speed")?;
        assert_eq!(val, IecValue::Real(120.0));

        // Write via OPC UA node
        addr_space.write_node("ns=2;s=Agitator_Speed", IecValue::Real(150.0), &mut tags)?;
        assert_eq!(
            tags.get_tag_value("Agitator_Speed"),
            Some(&IecValue::Real(150.0))
        );

        // Bad NodeId returns BAD_NODE_ID_UNKNOWN
        let err = addr_space.read_node("ns=2;s=NonExistent");
        assert_eq!(err, Err(OpcUaStatusCode::BAD_NODE_ID_UNKNOWN));

        // HEL/ACK binary framing
        let hel = OpcUaAddressSpace::encode_hel_message(65536, 16777216);
        assert_eq!(&hel[0..4], b"HELF");
        let ack = OpcUaAddressSpace::encode_ack_message(65536, 16777216);
        assert_eq!(&ack[0..4], b"ACKF");

        Ok(())
    }
}
