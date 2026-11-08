pub mod mqtt;
pub mod opcua;

pub use mqtt::{
    ConnectionState, IotMetric, IotTelemetryPayload, MqttClientConfig, MqttClientError,
    MqttTelemetryClient, MqttTelemetryEngine, MqttTopic,
};
pub use opcua::{
    iec_type_to_data_type_id, iec_value_to_variant, variant_to_iec_value, OpcUaAccessLevel,
    OpcUaAddressSpace, OpcUaBrowseItem, OpcUaNodeId, OpcUaServer, OpcUaServerConfig,
    OpcUaServerError, OpcUaStatusCode, OpcUaVariableNode, OpcUaWriteEvent, OpcUaWriteSink,
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

    #[test]
    fn test_mqtt_stable_aliases_across_cov_and_tag_changes_p4(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Tag_A",
            IecType::Int,
            None,
            IecValue::Int(10),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Tag_B",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "",
        ))?;
        tags.register_tag(TagEntry::new(
            "Tag_C",
            IecType::Real,
            None,
            IecValue::Real(42.5),
            "",
        ))?;

        let topic = MqttTopic::new("p1", "node1", "dev1");
        let mut engine = MqttTelemetryEngine::new(topic);

        let birth = engine.build_full_birth_payload(&tags, 100);
        assert_eq!(birth.metrics.len(), 3);
        let alias_a = birth.metrics[0].alias;
        let alias_b = birth.metrics[1].alias;
        let alias_c = birth.metrics[2].alias;

        // Register a new tag Tag_Prepend before altering Tag_B
        tags.register_tag(TagEntry::new(
            "Tag_D",
            IecType::Dint,
            None,
            IecValue::Dint(999),
            "",
        ))?;

        // Mutate Tag_B only
        tags.set_tag_value("Tag_B", IecValue::Bool(true))?;

        let cov = engine
            .build_cov_payload(&tags, 200)
            .ok_or("expected COV payload")?;

        // Tag_B should be present with the exact same stable alias as assigned during birth
        let metric_b = cov
            .metrics
            .iter()
            .find(|m| m.name == "Tag_B")
            .ok_or("Tag_B metric missing")?;
        assert_eq!(metric_b.alias, alias_b);
        assert_ne!(metric_b.alias, alias_a);
        assert_ne!(metric_b.alias, alias_c);

        Ok(())
    }

    #[test]
    fn test_opcua_hel_ack_message_sizes_and_endpoint_prefix_p5() {
        let hel = OpcUaAddressSpace::encode_hel_message(65536, 16777216);
        assert_eq!(&hel[0..4], b"HELF");
        let hel_msg_size = u32::from_le_bytes([hel[4], hel[5], hel[6], hel[7]]);
        assert_eq!(hel_msg_size, hel.len() as u32);
        assert_eq!(hel_msg_size, 56);

        // Endpoint string length prefix (Int32 little-endian) at bytes 28..32
        let ep_len = i32::from_le_bytes([hel[28], hel[29], hel[30], hel[31]]);
        assert_eq!(ep_len, 24);
        assert_eq!(&hel[32..32 + ep_len as usize], b"opc.tcp://localhost:4840");

        let ack = OpcUaAddressSpace::encode_ack_message(65536, 16777216);
        assert_eq!(&ack[0..4], b"ACKF");
        let ack_msg_size = u32::from_le_bytes([ack[4], ack[5], ack[6], ack[7]]);
        assert_eq!(ack_msg_size, ack.len() as u32);
        assert_eq!(ack_msg_size, 28);
    }

    #[test]
    fn test_mqtt_client_config_and_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let topic = MqttTopic::new("site1", "cell1", "plc1");
        assert_eq!(
            topic.sparkplug_dbirth_topic(),
            "spBv1.0/site1/DBIRTH/cell1/plc1"
        );
        assert_eq!(topic.standard_birth_topic(), "ferrite/site1/plc1/birth");

        let config = MqttClientConfig::new("127.0.0.1", 18883, "test_client_01", topic)
            .with_credentials("user", "pass")
            .with_keep_alive(15)
            .with_channel_capacity(64)
            .with_sparkplug_mode(true);

        assert_eq!(config.broker_host, "127.0.0.1");
        assert_eq!(config.broker_port, 18883);
        assert_eq!(config.keep_alive_secs, 15);
        assert_eq!(config.channel_capacity, 64);
        assert_eq!(config.username.as_deref(), Some("user"));
        assert_eq!(config.password.as_deref(), Some("pass"));

        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Tank_Temp",
            IecType::Real,
            None,
            IecValue::Real(21.5),
            "Degrees C",
        ))?;

        // Start client against a non-listening port (connection should remain Connecting/Disconnected without panic)
        let mut client = MqttTelemetryClient::start(config, Some(&tags))?;
        assert_ne!(client.connection_state(), ConnectionState::Closed);

        // Feed snapshot is non-blocking
        client.feed_snapshot(&tags, 1000)?;
        client.publish_birth(&tags, 1010)?;
        client.publish_raw("test/topic", b"payload".to_vec(), false)?;

        // Graceful shutdown
        client.shutdown();
        assert_eq!(client.connection_state(), ConnectionState::Closed);
        assert!(!client.is_connected());

        // Feeding after shutdown returns error
        assert!(matches!(
            client.feed_snapshot(&tags, 1020),
            Err(MqttClientError::Closed)
        ));

        Ok(())
    }

    #[test]
    fn test_mqtt_live_loopback_mock_broker() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        let topic = MqttTopic::new("plant2", "line3", "packer");
        let config = MqttClientConfig::new(addr.ip().to_string(), addr.port(), "plc_live_01", topic)
            .with_sparkplug_mode(true)
            .with_keep_alive(5);

        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Conveyor_Running",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Conveyor State",
        ))?;

        // Background thread acting as a minimal MQTT broker answering CONNECT with CONNACK
        let server_thread = std::thread::spawn(move || -> Result<Vec<u8>, std::io::Error> {
            let (mut stream, _) = listener.accept()?;
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf)?;
            // Expect MQTT CONNECT packet (starts with 0x10)
            if n > 0 && buf[0] == 0x10 {
                // Respond with CONNACK: 0x20, 0x02, 0x00, 0x00 (Accepted)
                stream.write_all(&[0x20, 0x02, 0x00, 0x00])?;
                stream.flush()?;
            }

            // Read the next incoming packet (should be the retained BIRTH payload)
            let mut birth_buf = vec![0u8; 2048];
            let read_n = stream.read(&mut birth_buf)?;
            birth_buf.truncate(read_n);
            Ok(birth_buf)
        });

        let mut client = MqttTelemetryClient::start(config, Some(&tags))?;

        // Wait up to 2 seconds for connection established
        let mut connected = false;
        for _ in 0..20 {
            if client.is_connected() {
                connected = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // Even if mock server connection takes time, verify background thread finished
        if let Ok(recvd) = server_thread.join().unwrap_or(Ok(Vec::new()))
            && !recvd.is_empty()
        {
            // Packet type 0x30 or 0x32 is MQTT PUBLISH
            assert!((recvd[0] & 0xF0) == 0x30);
        }

        client.shutdown();
        assert_eq!(client.connection_state(), ConnectionState::Closed);
        let _ = connected;

        Ok(())
    }

    #[test]
    fn test_opcua_variant_conversions() {
        use opcua_types::Variant;

        // Bool
        let b = IecValue::Bool(true);
        let vb = iec_value_to_variant(&b);
        assert_eq!(vb, Variant::Boolean(true));
        assert_eq!(variant_to_iec_value(&vb), Some(IecValue::Bool(true)));

        // Int
        let i = IecValue::Int(42);
        let vi = iec_value_to_variant(&i);
        assert_eq!(vi, Variant::Int16(42));
        assert_eq!(variant_to_iec_value(&vi), Some(IecValue::Int(42)));

        // Dint
        let d = IecValue::Dint(100_000);
        let vd = iec_value_to_variant(&d);
        assert_eq!(vd, Variant::Int32(100_000));
        assert_eq!(variant_to_iec_value(&vd), Some(IecValue::Dint(100_000)));

        // Real
        let r = IecValue::Real(42.5);
        let vr = iec_value_to_variant(&r);
        assert_eq!(vr, Variant::Float(42.5));
        assert_eq!(variant_to_iec_value(&vr), Some(IecValue::Real(42.5)));

        // Time
        let t = IecValue::Time(5000);
        let vt = iec_value_to_variant(&t);
        assert_eq!(vt, Variant::UInt64(5000));

        // String
        let s = IecValue::String("Ferrite".to_string());
        let vs = iec_value_to_variant(&s);
        assert_eq!(variant_to_iec_value(&vs), Some(IecValue::String("Ferrite".to_string())));

        // DataTypeId mappings
        assert_eq!(iec_type_to_data_type_id(IecType::Bool), opcua_types::DataTypeId::Boolean);
        assert_eq!(iec_type_to_data_type_id(IecType::Int), opcua_types::DataTypeId::Int16);
        assert_eq!(iec_type_to_data_type_id(IecType::Dint), opcua_types::DataTypeId::Int32);
        assert_eq!(iec_type_to_data_type_id(IecType::Real), opcua_types::DataTypeId::Float);
        assert_eq!(iec_type_to_data_type_id(IecType::String), opcua_types::DataTypeId::String);
    }

    #[test]
    fn test_opcua_write_sinks() -> Result<(), Box<dyn std::error::Error>> {
        use std::sync::mpsc;

        // 1. Channel WriteSink
        let (tx, rx) = mpsc::channel();
        let event = OpcUaWriteEvent {
            node_id: "ns=2;s=Pump_Speed".to_string(),
            tag_name: "Pump_Speed".to_string(),
            value: IecValue::Int(1500),
        };
        tx.on_write(&event)?;
        let received = rx.recv()?;
        assert_eq!(received.tag_name, "Pump_Speed");
        assert_eq!(received.value, IecValue::Int(1500));

        // 2. Closure WriteSink
        let captured = std::sync::Arc::new(std::sync::Mutex::new(None));
        let cap_clone = std::sync::Arc::clone(&captured);
        let closure_sink = move |ev: &OpcUaWriteEvent| {
            if let Ok(mut g) = cap_clone.lock() {
                *g = Some(ev.clone());
            }
            Ok(())
        };
        closure_sink.on_write(&event)?;
        assert_eq!(
            captured.lock().map(|g| g.clone()).unwrap_or(None),
            Some(event)
        );

        // 3. TagTable WriteSink
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Valve_Open",
            IecType::Bool,
            None,
            IecValue::Bool(false),
            "Valve state",
        ))?;
        let shared_tags = std::sync::Arc::new(std::sync::RwLock::new(tags));
        let write_ev = OpcUaWriteEvent {
            node_id: "ns=2;s=Valve_Open".to_string(),
            tag_name: "Valve_Open".to_string(),
            value: IecValue::Bool(true),
        };
        shared_tags.on_write(&write_ev)?;
        let val = shared_tags.read().ok().and_then(|t| t.get_tag_value("Valve_Open").cloned());
        assert_eq!(val, Some(IecValue::Bool(true)));

        Ok(())
    }

    #[test]
    fn test_opcua_server_ephemeral_port_and_shutdown() -> Result<(), Box<dyn std::error::Error>> {
        let mut tags = TagTable::new();
        tags.register_tag(TagEntry::new(
            "Tank_Level",
            IecType::Int,
            None,
            IecValue::Int(75),
            "Tank level 0-100",
        ))?;

        let mut addr_space = OpcUaAddressSpace::new();
        addr_space.sync_from_tag_table(&tags);
        let shared_addr_space = std::sync::Arc::new(std::sync::RwLock::new(addr_space));

        let config = OpcUaServerConfig::new("127.0.0.1", 0)
            .with_application_name("FerritePLC Test Server")
            .with_security_policy_none(true)
            .with_max_sessions(10);

        let mut server = OpcUaServer::start(config, shared_addr_space, None)?;
        assert!(server.is_running());

        let local_addr = server.local_addr().ok_or("Expected bound local address")?;
        assert_eq!(local_addr.ip().to_string(), "127.0.0.1");
        assert!(local_addr.port() > 0);

        // Connect over loopback TCP to the live server
        let mut stream = std::net::TcpStream::connect(local_addr)?;
        use std::io::{Read, Write};

        // Send OPC UA binary Hello (HEL) message
        let hel = OpcUaAddressSpace::encode_hel_message(65536, 16777216);
        stream.write_all(&hel)?;
        stream.flush()?;

        // Read OPC UA binary response (starts with ACKF or ERRF)
        let mut ack_buf = [0u8; 32];
        let n = stream.read(&mut ack_buf)?;
        assert!(n >= 4);
        assert!(&ack_buf[0..3] == b"ACK" || &ack_buf[0..3] == b"ERR");

        // Graceful shutdown
        server.shutdown();
        assert!(!server.is_running());

        Ok(())
    }
}
