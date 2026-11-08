use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ferrite_core::{IecValue, TagTable};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub fn new(
        group: impl Into<String>,
        node: impl Into<String>,
        device: impl Into<String>,
    ) -> Self {
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

    /// Returns Sparkplug B DBIRTH (Device Birth) topic:
    /// `spBv1.0/{group_id}/DBIRTH/{edge_node_id}/{device_id}`
    pub fn sparkplug_dbirth_topic(&self) -> String {
        format!(
            "spBv1.0/{}/DBIRTH/{}/{}",
            self.group_id, self.edge_node_id, self.device_id
        )
    }

    /// Returns standard JSON birth topic:
    /// `ferrite/{group_id}/{device_id}/birth`
    pub fn standard_birth_topic(&self) -> String {
        format!("ferrite/{}/{}/birth", self.group_id, self.device_id)
    }
}

/// Edge telemetry publisher engine with change-of-value (COV) deadband filtering.
pub struct MqttTelemetryEngine {
    pub topic: MqttTopic,
    seq: u64,
    last_published: HashMap<String, IecValue>,
    tag_aliases: HashMap<String, u64>,
}

impl MqttTelemetryEngine {
    pub fn new(topic: MqttTopic) -> Self {
        Self {
            topic,
            seq: 0,
            last_published: HashMap::new(),
            tag_aliases: HashMap::new(),
        }
    }

    fn get_or_assign_alias(&mut self, tag_name: &str) -> u64 {
        if let Some(&alias) = self.tag_aliases.get(tag_name) {
            alias
        } else {
            let alias = self.tag_aliases.len() as u64;
            self.tag_aliases.insert(tag_name.to_string(), alias);
            alias
        }
    }

    /// Generates a full snapshot payload containing all variables in `TagTable`.
    pub fn build_full_birth_payload(
        &mut self,
        tags: &TagTable,
        timestamp_ms: u64,
    ) -> IotTelemetryPayload {
        self.seq = self.seq.saturating_add(1);
        let mut metrics = Vec::new();

        for tag in tags.tags() {
            let val = tag.current_value().clone();
            self.last_published.insert(tag.name.clone(), val.clone());
            let alias = self.get_or_assign_alias(&tag.name);
            metrics.push(IotMetric {
                name: tag.name.clone(),
                alias: Some(alias),
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
    pub fn build_cov_payload(
        &mut self,
        tags: &TagTable,
        timestamp_ms: u64,
    ) -> Option<IotTelemetryPayload> {
        let mut metrics = Vec::new();

        for tag in tags.tags() {
            let val = tag.current_value();
            let changed = match self.last_published.get(&tag.name) {
                Some(prev) => prev != val,
                None => true,
            };

            if changed {
                self.last_published.insert(tag.name.clone(), val.clone());
                let alias = self.get_or_assign_alias(&tag.name);
                metrics.push(IotMetric {
                    name: tag.name.clone(),
                    alias: Some(alias),
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

/// Connection state of the live MQTT client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Closed,
}

/// Errors occurring in the MQTT telemetry client.
#[derive(Debug, Error)]
pub enum MqttClientError {
    #[error("MQTT client is closed/shut down")]
    Closed,

    #[error("Snapshot feed queue is full: {0}")]
    QueueFull(String),

    #[error("Background worker thread terminated")]
    WorkerTerminated,

    #[error("MQTT configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("MQTT client error: {0}")]
    Client(String),
}

/// Configuration for `MqttTelemetryClient`.
#[derive(Debug, Clone)]
pub struct MqttClientConfig {
    pub broker_host: String,
    pub broker_port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keep_alive_secs: u64,
    pub use_tls: bool,
    pub ca_cert: Option<Vec<u8>>,
    pub topic: MqttTopic,
    pub channel_capacity: usize,
    pub sparkplug_mode: bool,
}

impl MqttClientConfig {
    pub fn new(
        broker_host: impl Into<String>,
        broker_port: u16,
        client_id: impl Into<String>,
        topic: MqttTopic,
    ) -> Self {
        Self {
            broker_host: broker_host.into(),
            broker_port,
            client_id: client_id.into(),
            username: None,
            password: None,
            keep_alive_secs: 30,
            use_tls: false,
            ca_cert: None,
            topic,
            channel_capacity: 128,
            sparkplug_mode: true,
        }
    }

    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    pub fn with_tls(mut self, ca_cert: Option<Vec<u8>>) -> Self {
        self.use_tls = true;
        self.ca_cert = ca_cert;
        self
    }

    pub fn with_keep_alive(mut self, secs: u64) -> Self {
        self.keep_alive_secs = secs;
        self
    }

    pub fn with_channel_capacity(mut self, cap: usize) -> Self {
        self.channel_capacity = cap;
        self
    }

    pub fn with_sparkplug_mode(mut self, sparkplug: bool) -> Self {
        self.sparkplug_mode = sparkplug;
        self
    }
}

enum ClientCommand {
    Snapshot {
        tags: TagTable,
        timestamp_ms: u64,
    },
    PublishBirth {
        tags: TagTable,
        timestamp_ms: u64,
    },
    PublishRaw {
        topic: String,
        payload: Vec<u8>,
        retain: bool,
    },
    Shutdown,
}

/// Live Industrial IoT MQTT Telemetry Client with non-blocking snapshot feeding.
///
/// Designed specifically for deterministic PLC runtimes: network I/O and payload formatting
/// run entirely on dedicated background threads. The PLC scan loop feeds tag snapshots
/// via [`feed_snapshot`](MqttTelemetryClient::feed_snapshot) without blocking on socket operations.
///
/// ### Lifecycle & Telemetry Sequence
/// 1. Connects via TCP or TLS with keep-alive, authentication, and auto-reconnect.
/// 2. Publishes retained Sparkplug B `DBIRTH` or standard `birth` message upon connection.
/// 3. Publishes `DDATA` / COV delta telemetry as tags change value.
/// 4. Sequence numbers start at 1 for birth and increment monotonically across all publications.
/// 5. Gracefully disconnects on [`shutdown`](MqttTelemetryClient::shutdown).
pub struct MqttTelemetryClient {
    tx: mpsc::SyncSender<ClientCommand>,
    connection_state: Arc<RwLock<ConnectionState>>,
    running: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
    conn_handle: Option<JoinHandle<()>>,
}

impl MqttTelemetryClient {
    /// Starts the live MQTT client on dedicated background threads.
    ///
    /// If `initial_tags` are provided, the retained BIRTH sequence will be registered and
    /// dispatched immediately once connection to the broker is established.
    pub fn start(
        config: MqttClientConfig,
        initial_tags: Option<&TagTable>,
    ) -> Result<Self, MqttClientError> {
        let mut mqtt_options = rumqttc::MqttOptions::new(
            &config.client_id,
            &config.broker_host,
            config.broker_port,
        );
        mqtt_options.set_keep_alive(Duration::from_secs(config.keep_alive_secs));

        if let (Some(u), Some(p)) = (&config.username, &config.password) {
            mqtt_options.set_credentials(u, p);
        }

        if config.use_tls {
            let tls_config = if let Some(ca) = &config.ca_cert {
                rumqttc::TlsConfiguration::Simple {
                    ca: ca.clone(),
                    alpn: None,
                    client_auth: None,
                }
            } else {
                rumqttc::TlsConfiguration::Simple {
                    ca: Vec::new(),
                    alpn: None,
                    client_auth: None,
                }
            };
            mqtt_options.set_transport(rumqttc::Transport::Tls(tls_config));
        }

        let (client, mut connection) = rumqttc::Client::new(mqtt_options, 128);
        let running = Arc::new(AtomicBool::new(true));
        let connection_state = Arc::new(RwLock::new(ConnectionState::Connecting));
        let (tx, rx) = mpsc::sync_channel::<ClientCommand>(config.channel_capacity);

        let running_conn = Arc::clone(&running);
        let state_conn = Arc::clone(&connection_state);

        // Thread 1: Connection event loop & connection state monitor
        let conn_handle = thread::spawn(move || {
            while running_conn.load(Ordering::Relaxed) {
                match connection.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(event)) => {
                        if let rumqttc::Event::Incoming(rumqttc::Incoming::ConnAck(_)) = event
                            && let Ok(mut st) = state_conn.write()
                            && *st != ConnectionState::Closed
                        {
                            *st = ConnectionState::Connected;
                        }
                    }
                    Ok(Err(_err)) => {
                        if let Ok(mut st) = state_conn.write()
                            && *st != ConnectionState::Closed
                        {
                            *st = ConnectionState::Connecting;
                        }
                        thread::sleep(Duration::from_millis(250));
                    }
                    Err(rumqttc::RecvTimeoutError::Timeout) => {}
                    Err(rumqttc::RecvTimeoutError::Disconnected) => {
                        if let Ok(mut st) = state_conn.write()
                            && *st != ConnectionState::Closed
                        {
                            *st = ConnectionState::Disconnected;
                        }
                        break;
                    }
                }
            }
        });

        // Thread 2: Command & Snapshot processor (zero socket calls for callers)
        let running_worker = Arc::clone(&running);
        let state_worker = Arc::clone(&connection_state);
        let worker_config = config.clone();
        let initial_birth_tags = initial_tags.cloned();

        let worker_handle = thread::spawn(move || {
            let mut engine = MqttTelemetryEngine::new(worker_config.topic.clone());
            let mut birth_sent = false;
            let mut pending_birth = initial_birth_tags;

            while running_worker.load(Ordering::Relaxed) {
                // If we have an initial birth queued and we are connected, publish retained BIRTH
                let is_connected = state_worker
                    .read()
                    .map(|st| *st == ConnectionState::Connected)
                    .unwrap_or(false);

                if !birth_sent
                    && is_connected
                    && let Some(ref tags) = pending_birth
                {
                    let birth_payload = engine.build_full_birth_payload(tags, 1000);
                    if let Ok(json) = MqttTelemetryEngine::serialize_json(&birth_payload) {
                        let topic = if worker_config.sparkplug_mode {
                            worker_config.topic.sparkplug_dbirth_topic()
                        } else {
                            worker_config.topic.standard_birth_topic()
                        };
                        let _ = client.try_publish(
                            topic,
                            rumqttc::QoS::AtLeastOnce,
                            true,
                            json.into_bytes(),
                        );
                        birth_sent = true;
                        pending_birth = None;
                    }
                }

                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(ClientCommand::Snapshot { tags, timestamp_ms }) => {
                        // Ensure birth payload is published prior to any delta COV
                        if !birth_sent {
                            let birth_payload =
                                engine.build_full_birth_payload(&tags, timestamp_ms);
                            if let Ok(json) = MqttTelemetryEngine::serialize_json(&birth_payload) {
                                let topic = if worker_config.sparkplug_mode {
                                    worker_config.topic.sparkplug_dbirth_topic()
                                } else {
                                    worker_config.topic.standard_birth_topic()
                                };
                                let _ = client.try_publish(
                                    topic,
                                    rumqttc::QoS::AtLeastOnce,
                                    true,
                                    json.into_bytes(),
                                );
                                birth_sent = true;
                                pending_birth = None;
                            }
                        }

                        // Evaluate Report-by-Exception / Change-of-Value
                        if let Some(cov_payload) = engine.build_cov_payload(&tags, timestamp_ms)
                            && let Ok(json) = MqttTelemetryEngine::serialize_json(&cov_payload)
                        {
                            let topic = if worker_config.sparkplug_mode {
                                worker_config.topic.sparkplug_ddata_topic()
                            } else {
                                worker_config.topic.standard_telemetry_topic()
                            };
                            let _ = client.try_publish(
                                topic,
                                rumqttc::QoS::AtLeastOnce,
                                false,
                                json.into_bytes(),
                            );
                        }
                    }
                    Ok(ClientCommand::PublishBirth { tags, timestamp_ms }) => {
                        let birth_payload = engine.build_full_birth_payload(&tags, timestamp_ms);
                        if let Ok(json) = MqttTelemetryEngine::serialize_json(&birth_payload) {
                            let topic = if worker_config.sparkplug_mode {
                                worker_config.topic.sparkplug_dbirth_topic()
                            } else {
                                worker_config.topic.standard_birth_topic()
                            };
                            let _ = client.try_publish(
                                topic,
                                rumqttc::QoS::AtLeastOnce,
                                true,
                                json.into_bytes(),
                            );
                            birth_sent = true;
                            pending_birth = None;
                        }
                    }
                    Ok(ClientCommand::PublishRaw {
                        topic,
                        payload,
                        retain,
                    }) => {
                        let _ = client.try_publish(
                            topic,
                            rumqttc::QoS::AtLeastOnce,
                            retain,
                            payload,
                        );
                    }
                    Ok(ClientCommand::Shutdown) => {
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            let _ = client.try_disconnect();
        });

        Ok(Self {
            tx,
            connection_state,
            running,
            worker_handle: Some(worker_handle),
            conn_handle: Some(conn_handle),
        })
    }

    /// Non-blocking feed of a process snapshot from the PLC scan cycle.
    ///
    /// Copies tag values to the in-memory queue. The background worker evaluates COV deadbands
    /// and formats/publishes telemetry without delaying the scan cycle.
    pub fn feed_snapshot(
        &self,
        tags: &TagTable,
        timestamp_ms: u64,
    ) -> Result<(), MqttClientError> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(MqttClientError::Closed);
        }
        self.tx
            .try_send(ClientCommand::Snapshot {
                tags: tags.clone(),
                timestamp_ms,
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => {
                    MqttClientError::QueueFull("Telemetry snapshot queue full".to_string())
                }
                mpsc::TrySendError::Disconnected(_) => MqttClientError::WorkerTerminated,
            })
    }

    /// Explicitly triggers a full retained BIRTH payload for the given tags.
    pub fn publish_birth(
        &self,
        tags: &TagTable,
        timestamp_ms: u64,
    ) -> Result<(), MqttClientError> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(MqttClientError::Closed);
        }
        self.tx
            .try_send(ClientCommand::PublishBirth {
                tags: tags.clone(),
                timestamp_ms,
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => {
                    MqttClientError::QueueFull("Telemetry command queue full".to_string())
                }
                mpsc::TrySendError::Disconnected(_) => MqttClientError::WorkerTerminated,
            })
    }

    /// Non-blocking raw message publish to a custom topic.
    pub fn publish_raw(
        &self,
        topic: impl Into<String>,
        payload: Vec<u8>,
        retain: bool,
    ) -> Result<(), MqttClientError> {
        if !self.running.load(Ordering::Relaxed) {
            return Err(MqttClientError::Closed);
        }
        self.tx
            .try_send(ClientCommand::PublishRaw {
                topic: topic.into(),
                payload,
                retain,
            })
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => {
                    MqttClientError::QueueFull("Telemetry command queue full".to_string())
                }
                mpsc::TrySendError::Disconnected(_) => MqttClientError::WorkerTerminated,
            })
    }

    /// Returns the current connection state.
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_state
            .read()
            .map(|st| *st)
            .unwrap_or(ConnectionState::Closed)
    }

    /// Returns `true` if connected to the MQTT broker.
    pub fn is_connected(&self) -> bool {
        self.connection_state() == ConnectionState::Connected
    }

    /// Gracefully stops the background threads and disconnects from the MQTT broker.
    pub fn shutdown(&mut self) {
        if self.running.swap(false, Ordering::SeqCst) {
            if let Ok(mut st) = self.connection_state.write() {
                *st = ConnectionState::Closed;
            }
            let _ = self.tx.try_send(ClientCommand::Shutdown);
            if let Some(h) = self.worker_handle.take() {
                let _ = h.join();
            }
            if let Some(h) = self.conn_handle.take() {
                let _ = h.join();
            }
        }
    }
}

impl Drop for MqttTelemetryClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}
