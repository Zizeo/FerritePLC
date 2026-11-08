use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self, JoinHandle};

use async_trait::async_trait;
use ferrite_core::{CoreError, IecType, IecValue, TagTable};
use opcua_core::sync::RwLock as OpcRwLock;
use opcua_server::address_space::{AddressSpace, VariableBuilder};
use opcua_server::diagnostics::NamespaceMetadata;
use opcua_server::node_manager::memory::{
    InMemoryNodeManagerBuilder, InMemoryNodeManagerImpl, InMemoryNodeManagerImplBuilder,
};
use opcua_server::node_manager::{ParsedReadValueId, RequestContext, ServerContext, WriteNode};
use opcua_server::{ServerBuilder, ServerEndpoint, ServerHandle, ANONYMOUS_USER_TOKEN_ID};
use opcua_types::{DataValue, NodeId, StatusCode, TimestampsToReturn, Variant};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Standard IEC 62541 OPC UA Status Codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcUaStatusCode(pub u32);

impl std::fmt::Display for OpcUaStatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OPC UA StatusCode(0x{:08X})", self.0)
    }
}

impl std::error::Error for OpcUaStatusCode {}

impl OpcUaStatusCode {
    pub const GOOD: Self = Self(0x0000_0000);
    pub const BAD_UNEXPECTED_ERROR: Self = Self(0x8001_0000);
    pub const BAD_NODE_ID_UNKNOWN: Self = Self(0x8034_0000);
    pub const BAD_NOT_WRITABLE: Self = Self(0x803B_0000);
    pub const BAD_TYPE_MISMATCH: Self = Self(0x8074_0000);

    pub fn is_good(self) -> bool {
        (self.0 & 0x8000_0000) == 0
    }
}

/// Node identifier according to IEC 62541.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpcUaNodeId {
    Numeric { namespace: u16, id: u32 },
    String { namespace: u16, id: String },
}

impl OpcUaNodeId {
    pub fn ns2_string(tag_name: &str) -> Self {
        Self::String {
            namespace: 2,
            id: tag_name.to_string(),
        }
    }

    pub fn to_standard_string(&self) -> String {
        match self {
            Self::Numeric { namespace, id } => format!("ns={namespace};i={id}"),
            Self::String { namespace, id } => format!("ns={namespace};s={id}"),
        }
    }
}

/// Access level permissions for an OPC UA Variable Node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcUaAccessLevel {
    pub readable: bool,
    pub writable: bool,
}

impl Default for OpcUaAccessLevel {
    fn default() -> Self {
        Self {
            readable: true,
            writable: true,
        }
    }
}

/// Variable Node inside the IEC 62541 OPC UA Address Space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpcUaVariableNode {
    pub node_id: OpcUaNodeId,
    pub browse_name: String,
    pub display_name: String,
    pub data_type: IecType,
    pub value: IecValue,
    pub access_level: OpcUaAccessLevel,
    pub description: String,
}

/// Browsing item returned by an OPC UA Browse Service call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpcUaBrowseItem {
    pub node_id: String,
    pub browse_name: String,
    pub is_folder: bool,
}

/// OPC UA Server Address Space exposing FerritePLC symbols and memory.
pub struct OpcUaAddressSpace {
    nodes: HashMap<String, OpcUaVariableNode>,
}

impl Default for OpcUaAddressSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl OpcUaAddressSpace {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Synchronizes address space nodes from a FerritePLC `TagTable`.
    pub fn sync_from_tag_table(&mut self, tags: &TagTable) {
        for tag in tags.tags() {
            let node_id = OpcUaNodeId::ns2_string(&tag.name);
            let key = node_id.to_standard_string();

            let access_level = OpcUaAccessLevel {
                readable: true,
                writable: !tag.read_only_outside_sfc,
            };

            let node = OpcUaVariableNode {
                node_id,
                browse_name: tag.name.clone(),
                display_name: tag.name.clone(),
                data_type: tag.data_type,
                value: tag.current_value().clone(),
                access_level,
                description: tag.comment.clone(),
            };

            self.nodes.insert(key, node);
        }
    }

    /// Reads a node's current value by string NodeId (e.g., `"ns=2;s=Tank_Level"`).
    pub fn read_node(&self, node_id: &str) -> Result<IecValue, OpcUaStatusCode> {
        let node = self
            .nodes
            .get(node_id)
            .ok_or(OpcUaStatusCode::BAD_NODE_ID_UNKNOWN)?;
        if !node.access_level.readable {
            return Err(OpcUaStatusCode::BAD_UNEXPECTED_ERROR);
        }
        Ok(node.value.clone())
    }

    /// Writes a value to a node in the address space and commits to the `TagTable`.
    pub fn write_node(
        &mut self,
        node_id: &str,
        val: IecValue,
        tags: &mut TagTable,
    ) -> Result<(), OpcUaStatusCode> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or(OpcUaStatusCode::BAD_NODE_ID_UNKNOWN)?;

        if !node.access_level.writable {
            return Err(OpcUaStatusCode::BAD_NOT_WRITABLE);
        }

        let coerced = val
            .coerce_to(node.data_type)
            .map_err(|_| OpcUaStatusCode::BAD_TYPE_MISMATCH)?;

        node.value = coerced.clone();
        tags.set_tag_value(&node.browse_name, coerced)
            .map_err(|e| match e {
                CoreError::TypeMismatch { .. } => OpcUaStatusCode::BAD_TYPE_MISMATCH,
                _ => OpcUaStatusCode::BAD_NODE_ID_UNKNOWN,
            })?;

        Ok(())
    }

    /// Returns a list of all browsable variable nodes under `ns=2`.
    pub fn browse_nodes(&self) -> Vec<OpcUaBrowseItem> {
        self.nodes
            .values()
            .map(|n| OpcUaBrowseItem {
                node_id: n.node_id.to_standard_string(),
                browse_name: n.browse_name.clone(),
                is_folder: false,
            })
            .collect()
    }

    /// Formats an OPC UA binary protocol Hello message (`HEL`).
    pub fn encode_hel_message(buffer_size: u32, max_msg_size: u32) -> Vec<u8> {
        let endpoint = b"opc.tcp://localhost:4840";
        // Header (8 bytes) + 5 x UInt32 fields (20 bytes) + String length prefix (4 bytes) + endpoint bytes
        let total_len = (8 + 20 + 4 + endpoint.len()) as u32;
        let mut bytes = Vec::with_capacity(total_len as usize);
        bytes.extend_from_slice(b"HEL");
        bytes.push(b'F'); // Final chunk
        bytes.extend_from_slice(&total_len.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // Protocol version
        bytes.extend_from_slice(&buffer_size.to_le_bytes()); // Receive buffer size
        bytes.extend_from_slice(&buffer_size.to_le_bytes()); // Send buffer size
        bytes.extend_from_slice(&max_msg_size.to_le_bytes()); // Max message size
        bytes.extend_from_slice(&100u32.to_le_bytes()); // Max chunk count
        let ep_len = endpoint.len() as i32;
        bytes.extend_from_slice(&ep_len.to_le_bytes());
        bytes.extend_from_slice(endpoint);
        bytes
    }

    /// Formats an OPC UA binary protocol Acknowledge message (`ACK`).
    pub fn encode_ack_message(buffer_size: u32, max_msg_size: u32) -> Vec<u8> {
        let total_len = 28u32;
        let mut bytes = Vec::with_capacity(total_len as usize);
        bytes.extend_from_slice(b"ACK");
        bytes.push(b'F');
        bytes.extend_from_slice(&total_len.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // Protocol version
        bytes.extend_from_slice(&buffer_size.to_le_bytes());
        bytes.extend_from_slice(&buffer_size.to_le_bytes());
        bytes.extend_from_slice(&max_msg_size.to_le_bytes());
        bytes.extend_from_slice(&100u32.to_le_bytes());
        bytes
    }
}

/// Converts a Ferrite `IecValue` to an OPC UA `Variant`.
pub fn iec_value_to_variant(val: &IecValue) -> Variant {
    match val {
        IecValue::Bool(b) => Variant::Boolean(*b),
        IecValue::Sint(i) => Variant::SByte(*i),
        IecValue::Int(i) => Variant::Int16(*i),
        IecValue::Dint(d) => Variant::Int32(*d),
        IecValue::Lint(li) => Variant::Int64(*li),
        IecValue::Usint(u) | IecValue::Byte(u) => Variant::Byte(*u),
        IecValue::Uint(u) | IecValue::Word(u) => Variant::UInt16(*u),
        IecValue::Udint(u) | IecValue::Dword(u) => Variant::UInt32(*u),
        IecValue::Ulint(u) | IecValue::Lword(u) => Variant::UInt64(*u),
        IecValue::Real(r) => Variant::Float(*r),
        IecValue::Lreal(lr) => Variant::Double(*lr),
        IecValue::Time(ms) => Variant::UInt64(*ms),
        IecValue::String(s) => Variant::from(s.as_str()),
    }
}

/// Converts an OPC UA `Variant` to a Ferrite `IecValue`.
pub fn variant_to_iec_value(variant: &Variant) -> Option<IecValue> {
    match variant {
        Variant::Boolean(b) => Some(IecValue::Bool(*b)),
        Variant::SByte(i) => Some(IecValue::Sint(*i)),
        Variant::Byte(u) => Some(IecValue::Byte(*u)),
        Variant::Int16(i) => Some(IecValue::Int(*i)),
        Variant::UInt16(u) => Some(IecValue::Word(*u)),
        Variant::Int32(i) => Some(IecValue::Dint(*i)),
        Variant::UInt32(u) => Some(IecValue::Dword(*u)),
        Variant::Int64(i) => Some(IecValue::Lint(*i)),
        Variant::UInt64(u) => Some(IecValue::Lword(*u)),
        Variant::Float(f) => Some(IecValue::Real(*f)),
        Variant::Double(d) => Some(IecValue::Lreal(*d)),
        Variant::String(s) => Some(IecValue::String(s.as_ref().to_string())),
        _ => None,
    }
}

/// Maps a Ferrite `IecType` to the corresponding OPC UA `DataTypeId`.
pub fn iec_type_to_data_type_id(data_type: IecType) -> opcua_types::DataTypeId {
    match data_type {
        IecType::Bool => opcua_types::DataTypeId::Boolean,
        IecType::Sint => opcua_types::DataTypeId::SByte,
        IecType::Int => opcua_types::DataTypeId::Int16,
        IecType::Dint => opcua_types::DataTypeId::Int32,
        IecType::Lint => opcua_types::DataTypeId::Int64,
        IecType::Usint | IecType::Byte => opcua_types::DataTypeId::Byte,
        IecType::Uint | IecType::Word => opcua_types::DataTypeId::UInt16,
        IecType::Udint | IecType::Dword => opcua_types::DataTypeId::UInt32,
        IecType::Ulint | IecType::Lword => opcua_types::DataTypeId::UInt64,
        IecType::Real => opcua_types::DataTypeId::Float,
        IecType::Lreal => opcua_types::DataTypeId::Double,
        IecType::Time => opcua_types::DataTypeId::UInt64,
        IecType::String => opcua_types::DataTypeId::String,
    }
}

/// Event triggered when an OPC UA client writes to a variable node.
#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaWriteEvent {
    pub node_id: String,
    pub tag_name: String,
    pub value: IecValue,
}

/// Sink for receiving write-back requests from external OPC UA clients.
///
/// Mirrors how Modbus server reports writes via `ProcessImage` / `TagTable`.
pub trait OpcUaWriteSink: Send + Sync {
    fn on_write(&self, event: &OpcUaWriteEvent) -> Result<(), OpcUaStatusCode>;
}

impl OpcUaWriteSink for mpsc::Sender<OpcUaWriteEvent> {
    fn on_write(&self, event: &OpcUaWriteEvent) -> Result<(), OpcUaStatusCode> {
        self.send(event.clone())
            .map_err(|_| OpcUaStatusCode::BAD_UNEXPECTED_ERROR)
    }
}

impl OpcUaWriteSink for mpsc::SyncSender<OpcUaWriteEvent> {
    fn on_write(&self, event: &OpcUaWriteEvent) -> Result<(), OpcUaStatusCode> {
        self.try_send(event.clone())
            .map_err(|_| OpcUaStatusCode::BAD_UNEXPECTED_ERROR)
    }
}

impl<F> OpcUaWriteSink for F
where
    F: Fn(&OpcUaWriteEvent) -> Result<(), OpcUaStatusCode> + Send + Sync,
{
    fn on_write(&self, event: &OpcUaWriteEvent) -> Result<(), OpcUaStatusCode> {
        (self)(event)
    }
}

impl OpcUaWriteSink for Arc<RwLock<TagTable>> {
    fn on_write(&self, event: &OpcUaWriteEvent) -> Result<(), OpcUaStatusCode> {
        let mut guard = self
            .write()
            .map_err(|_| OpcUaStatusCode::BAD_UNEXPECTED_ERROR)?;
        guard
            .set_tag_value(&event.tag_name, event.value.clone())
            .map_err(|_| OpcUaStatusCode::BAD_TYPE_MISMATCH)
    }
}

/// Server configuration for the live OPC UA binary server.
#[derive(Debug, Clone)]
pub struct OpcUaServerConfig {
    pub application_name: String,
    pub application_uri: String,
    pub product_uri: String,
    pub host: String,
    pub port: u16,
    pub endpoint_path: String,
    pub security_policy_none: bool,
    pub max_sessions: usize,
    pub max_subscriptions: usize,
}

impl Default for OpcUaServerConfig {
    fn default() -> Self {
        Self {
            application_name: "FerritePLC OPC UA Server".to_string(),
            application_uri: "urn:ferrite-plc".to_string(),
            product_uri: "urn:ferrite-plc:server".to_string(),
            host: "127.0.0.1".to_string(),
            port: 4840,
            endpoint_path: "/".to_string(),
            security_policy_none: true,
            max_sessions: 100,
            max_subscriptions: 100,
        }
    }
}

impl OpcUaServerConfig {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            ..Default::default()
        }
    }

    pub fn with_application_name(mut self, name: impl Into<String>) -> Self {
        self.application_name = name.into();
        self
    }

    pub fn with_security_policy_none(mut self, allow: bool) -> Self {
        self.security_policy_none = allow;
        self
    }

    pub fn with_max_sessions(mut self, max: usize) -> Self {
        self.max_sessions = max;
        self
    }
}

/// Errors originating from the live OPC UA Server.
#[derive(Debug, Error)]
pub enum OpcUaServerError {
    #[error("OPC UA server build error: {0}")]
    Build(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Execution runtime error: {0}")]
    Execution(String),

    #[error("Server is not running")]
    NotRunning,
}

struct FerriteNodeManagerBuilder {
    address_space: Arc<RwLock<OpcUaAddressSpace>>,
    write_sink: Option<Arc<dyn OpcUaWriteSink>>,
}

impl InMemoryNodeManagerImplBuilder for FerriteNodeManagerBuilder {
    type Impl = FerriteNodeManager;

    fn build(self, context: ServerContext, address_space: &mut AddressSpace) -> Self::Impl {
        let ns_index = {
            let mut type_tree = context.type_tree.write();
            type_tree.namespaces_mut().add_namespace("urn:ferrite-plc")
        };
        address_space.add_namespace("urn:ferrite-plc", ns_index);

        FerriteNodeManager {
            address_space: self.address_space,
            write_sink: self.write_sink,
            ns_index,
        }
    }
}

struct FerriteNodeManager {
    address_space: Arc<RwLock<OpcUaAddressSpace>>,
    write_sink: Option<Arc<dyn OpcUaWriteSink>>,
    ns_index: u16,
}

#[async_trait]
impl InMemoryNodeManagerImpl for FerriteNodeManager {
    async fn init(&self, address_space: &mut AddressSpace, _context: ServerContext) {
        if let Ok(guard) = self.address_space.read() {
            for node in guard.nodes.values() {
                let opc_node_id = NodeId::new(self.ns_index, node.browse_name.clone());
                let mut var_builder =
                    VariableBuilder::new(&opc_node_id, &node.browse_name, &node.display_name)
                        .value(iec_value_to_variant(&node.value))
                        .data_type(iec_type_to_data_type_id(node.data_type))
                        .description(&node.description);
                if node.access_level.writable {
                    var_builder = var_builder.writable();
                }
                let var = var_builder.build();
                address_space.add_variables(vec![var], &NodeId::objects_folder_id());
            }
        }
    }

    fn name(&self) -> &str {
        "ferrite"
    }

    fn namespaces(&self) -> Vec<NamespaceMetadata> {
        vec![NamespaceMetadata {
            namespace_uri: "urn:ferrite-plc".to_string(),
            namespace_index: self.ns_index,
            ..Default::default()
        }]
    }

    async fn read_values(
        &self,
        context: &RequestContext,
        address_space: &OpcRwLock<AddressSpace>,
        nodes: &[&ParsedReadValueId],
        max_age: f64,
        timestamps_to_return: TimestampsToReturn,
    ) -> Vec<DataValue> {
        let guard = self.address_space.read().ok();
        let as_guard = address_space.read();
        let mut results = Vec::with_capacity(nodes.len());

        for n in nodes {
            if n.node_id.namespace == self.ns_index
                && let opcua_types::Identifier::String(ref s) = n.node_id.identifier
                && let Some(ref g) = guard
            {
                let standard_id = format!("ns={};s={}", self.ns_index, s.as_ref());
                if let Ok(val) = g.read_node(&standard_id) {
                    results.push(DataValue::new_now(iec_value_to_variant(&val)));
                    continue;
                }
            }
            results.push(as_guard.read(context, n, max_age, timestamps_to_return));
        }
        results
    }

    async fn write(
        &self,
        context: &RequestContext,
        address_space: &OpcRwLock<AddressSpace>,
        nodes_to_write: &mut [&mut WriteNode],
    ) -> Result<(), StatusCode> {
        let mut as_guard = address_space.write();
        let type_tree = context.type_tree.read();

        for write in nodes_to_write.iter_mut() {
            let write_val = write.value();
            let node_id = &write_val.node_id;

            if node_id.namespace != self.ns_index {
                write.set_status(StatusCode::BadNodeIdUnknown);
                continue;
            }

            let tag_name = match &node_id.identifier {
                opcua_types::Identifier::String(s) => s.as_ref().to_string(),
                _ => {
                    write.set_status(StatusCode::BadNodeIdUnknown);
                    continue;
                }
            };

            let node = match as_guard.validate_node_write(context, write_val, &*type_tree) {
                Ok(n) => n,
                Err(code) => {
                    write.set_status(code);
                    continue;
                }
            };

            if write_val.attribute_id != opcua_types::AttributeId::Value {
                write.set_status(StatusCode::BadNotWritable);
                continue;
            }

            let new_variant = match &write_val.value.value {
                Some(v) => v,
                None => {
                    write.set_status(StatusCode::BadNothingToDo);
                    continue;
                }
            };

            let iec_val = match variant_to_iec_value(new_variant) {
                Some(v) => v,
                None => {
                    write.set_status(StatusCode::BadTypeMismatch);
                    continue;
                }
            };

            let standard_id = format!("ns={};s={tag_name}", self.ns_index);

            if let Some(sink) = &self.write_sink {
                let event = OpcUaWriteEvent {
                    node_id: standard_id.clone(),
                    tag_name: tag_name.clone(),
                    value: iec_val.clone(),
                };
                if sink.on_write(&event).is_err() {
                    write.set_status(StatusCode::BadTypeMismatch);
                    continue;
                }
            }

            if let Ok(mut g) = self.address_space.write()
                && let Some(n) = g.nodes.get_mut(&standard_id)
            {
                n.value = iec_val;
            }

            match opcua_server::address_space::write_node_value(node, write.value()) {
                Ok(()) => {
                    write.set_status(StatusCode::Good);
                }
                Err(code) => {
                    write.set_status(code);
                }
            }
        }

        Ok(())
    }
}

/// Live Industrial OPC UA Binary Server.
///
/// Exposes FerritePLC symbols and memory under Namespace 2 (`ns=2;s=<Tag_Name>`).
/// External OPC UA clients (SCADA, MES, Ignition, Kepware) can Browse, Read, and Write
/// PLC variables in real time.
///
/// Write-backs from clients flow asynchronously through an [`OpcUaWriteSink`] callback or channel,
/// ensuring the deterministic scan loop is notified cleanly without blocking socket calls.
pub struct OpcUaServer {
    config: OpcUaServerConfig,
    local_addr: Option<SocketAddr>,
    server_handle: Option<ServerHandle>,
    worker_handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
}

impl OpcUaServer {
    /// Starts the live OPC UA server in a background thread.
    ///
    /// Binds to `config.host` and `config.port` (port `0` automatically allocates an ephemeral port).
    pub fn start(
        config: OpcUaServerConfig,
        address_space: Arc<RwLock<OpcUaAddressSpace>>,
        write_sink: Option<Arc<dyn OpcUaWriteSink>>,
    ) -> Result<Self, OpcUaServerError> {
        let running = Arc::new(AtomicBool::new(true));
        let bind_addr = format!("{}:{}", config.host, config.port);
        let (init_tx, init_rx) = mpsc::channel();
        let server_cfg = config.clone();

        let worker_handle = thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = init_tx.send(Err(OpcUaServerError::Execution(e.to_string())));
                    return;
                }
            };

            let _guard = rt.enter();

            let mut builder = ServerBuilder::new()
                .application_name(&server_cfg.application_name)
                .application_uri(&server_cfg.application_uri)
                .product_uri(&server_cfg.product_uri)
                .host(&server_cfg.host)
                .port(server_cfg.port)
                .max_sessions(server_cfg.max_sessions);

            if server_cfg.security_policy_none {
                builder = builder
                    .add_endpoint(
                        "none",
                        ServerEndpoint::new_none(
                            &server_cfg.endpoint_path,
                            &[ANONYMOUS_USER_TOKEN_ID.to_string()],
                        ),
                    )
                    .discovery_urls(vec![server_cfg.endpoint_path.clone()]);
            } else {
                builder = builder.discovery_urls(vec![server_cfg.endpoint_path.clone()]);
            }

            let node_mgr_builder = FerriteNodeManagerBuilder {
                address_space,
                write_sink,
            };
            let in_memory_builder = InMemoryNodeManagerBuilder::new(node_mgr_builder);
            let builder = builder.with_node_manager(in_memory_builder);

            let (server, server_handle) = match builder.build() {
                Ok(pair) => pair,
                Err(e) => {
                    let _ = init_tx.send(Err(OpcUaServerError::Build(e)));
                    return;
                }
            };

            let listener = match rt.block_on(tokio::net::TcpListener::bind(&bind_addr)) {
                Ok(l) => l,
                Err(e) => {
                    let _ = init_tx.send(Err(OpcUaServerError::Io(e)));
                    return;
                }
            };

            let local_addr = match listener.local_addr() {
                Ok(a) => a,
                Err(e) => {
                    let _ = init_tx.send(Err(OpcUaServerError::Io(e)));
                    return;
                }
            };

            let _ = init_tx.send(Ok((server_handle, local_addr)));

            rt.block_on(async move {
                let _ = server.run_with(listener).await;
            });
        });

        let (server_handle, local_addr) = match init_rx.recv() {
            Ok(Ok(pair)) => (Some(pair.0), Some(pair.1)),
            Ok(Err(e)) => return Err(e),
            Err(_) => (None, None),
        };

        Ok(Self {
            config,
            local_addr,
            server_handle,
            worker_handle: Some(worker_handle),
            running,
        })
    }

    /// Returns the bound local socket address (useful when dynamic port 0 was selected).
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// Returns the server configuration.
    pub fn config(&self) -> &OpcUaServerConfig {
        &self.config
    }

    /// Returns `true` if the server is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Signals graceful shutdown and terminates the background worker thread.
    pub fn shutdown(&mut self) {
        if self.running.swap(false, Ordering::SeqCst) {
            if let Some(handle) = self.server_handle.take() {
                handle.cancel();
            }
            if let Some(worker) = self.worker_handle.take() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for OpcUaServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}
