use std::collections::HashMap;

use ferrite_core::{CoreError, IecType, IecValue, TagTable};
use serde::{Deserialize, Serialize};

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
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(b"HEL");
        bytes.push(b'F'); // Final chunk
        let total_len = 32u32;
        bytes.extend_from_slice(&total_len.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // Protocol version
        bytes.extend_from_slice(&buffer_size.to_le_bytes()); // Receive buffer size
        bytes.extend_from_slice(&buffer_size.to_le_bytes()); // Send buffer size
        bytes.extend_from_slice(&max_msg_size.to_le_bytes()); // Max message size
        bytes.extend_from_slice(&100u32.to_le_bytes()); // Max chunk count
        let endpoint = b"opc.tcp://localhost:4840";
        bytes.extend_from_slice(endpoint);
        bytes
    }

    /// Formats an OPC UA binary protocol Acknowledge message (`ACK`).
    pub fn encode_ack_message(buffer_size: u32, max_msg_size: u32) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(28);
        bytes.extend_from_slice(b"ACK");
        bytes.push(b'F');
        let total_len = 28u32;
        bytes.extend_from_slice(&total_len.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // Protocol version
        bytes.extend_from_slice(&buffer_size.to_le_bytes());
        bytes.extend_from_slice(&buffer_size.to_le_bytes());
        bytes.extend_from_slice(&max_msg_size.to_le_bytes());
        bytes.extend_from_slice(&100u32.to_le_bytes());
        bytes
    }
}
