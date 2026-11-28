use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models;

use super::player;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultLogs {
    pub success: bool,
    pub contains_crash_report_log: bool,
    pub logs: Vec<Log>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub dir: String,
    pub contains_console_log: bool,
    pub contains_script_log: bool,
    pub contains_error_log: bool,
    pub contains_crash_log: bool,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub enum LogType {
    #[serde(rename = "console.log")]
    Console,
    #[serde(rename = "error.log")]
    Error,
    #[serde(rename = "script.log")]
    Script,
    #[serde(rename = "crash.log")]
    Crash,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DockerStats {
    pub timestamp: DateTime<Utc>,
    pub block_io_read: u64,
    pub block_io_write: u64,
    pub cpu_percentage: f64,
    pub name: String,
    pub id: String,
    pub mem_usage: u64,
    pub mem_limit: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub pid_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerIdentityId {
    pub name: String,
    pub identity_id: String,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultSize {
    pub addons_size: u64,
    pub logs_size: u64,
    pub profile_size: u64,
    pub mods: Vec<String>,
    pub logs: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub struct IPv4Response {
    pub ipv4: String,
}

#[derive(Serialize, ToSchema)]
pub struct ImageVersionResponse {
    pub version: String,
}

#[derive(Serialize, ToSchema)]
pub struct SuccessResponse {
    pub success: bool,
}

#[derive(Serialize, ToSchema)]
pub struct FileContentResponse {
    pub file_content: String,
}

/// A chunk of a (possibly still growing) log file, read from a byte offset.
#[derive(Serialize, ToSchema)]
pub struct LogFileChunkResponse {
    /// File content from the requested offset up to the returned `offset`
    pub file_content: String,
    /// Byte offset to pass on the next request to receive only new content
    pub offset: u64,
    /// True when the file was truncated or replaced since the previous request (the
    /// offset is past its end, or its start no longer matches `head`), so the content
    /// was read from the start instead
    pub reset: bool,
    /// Fingerprint of the start of the file; pass it with `offset` on the next request
    pub head: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileFileListResponse {
    pub files: Vec<String>,
}

/// A running server holding some of the ports another server needs.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortConflict {
    pub uuid: String,
    pub name: String,
    /// The ports both servers want
    pub ports: Vec<u16>,
}

/// Why a server's ports can't be used.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum PortConflictReason {
    /// Another running server or a program on the host holds the ports.
    PortsInUse,
    /// Only port groups are allowed and the server uses custom ports.
    NotAPortGroup,
}

/// Returned with 409 when a server can't start because of its ports.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PortConflictResponse {
    pub reason: PortConflictReason,
    pub message: String,
    /// The ports that are taken
    pub ports: Vec<u16>,
    /// Running ARSA servers holding the ports. Empty when something outside ARSA holds them.
    pub conflicts: Vec<PortConflict>,
    /// Port groups no running server uses, to switch to
    pub free_port_groups: Vec<models::port_group::Model>,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
#[serde(rename_all_fields = "camelCase")]
pub enum ServerStatusUpdates {
    IsRunningUpdate {
        uuid: String,
        is_running: bool,
    },
    ArsStatusUpdate {
        ars_status: super::server::ArsStatus,
    },
    PlayerCountUpdate {
        uuid: String,
        player_count: u32,
    },
    LogUpdate {
        log: crate::endpoints::log::GlobalLog,
    },
    CreateImageProgress {
        info: models::pull_log::Model,
    },
    CreateImageFinished {
        pull_id: String,
    },
    ScenariosUpdateFinished {
        branch: crate::endpoints::server::Branch,
        success: bool,
        count: u32,
        error: Option<String>,
    },
    Error {
        error: String,
    },
}

impl From<player::Model> for PlayerIdentityId {
    fn from(model: player::Model) -> Self {
        PlayerIdentityId {
            name: model.name,
            identity_id: model.uuid.to_string(),
        }
    }
}
