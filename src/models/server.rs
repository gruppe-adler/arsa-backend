use sea_orm::entity::prelude::*;
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;

use crate::endpoints::server::Branch;

pub fn deserialize_uuid<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(Uuid::new_v4())
    } else {
        Uuid::parse_str(&s).map_err(serde::de::Error::custom)
    }
}

#[sea_orm::model]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, DeriveEntityModel, ToSchema)]
#[sea_orm(table_name = "server")]
#[serde(rename_all = "camelCase")]
#[schema(as = Server)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[serde(default = "Uuid::new_v4", deserialize_with = "deserialize_uuid")]
    pub uuid: Uuid,
    pub name: String,
    pub is_running: bool,

    #[schema(default = "Branch::Stable")]
    pub branch: Branch,

    pub player_count: u32,

    pub config: ServerConfig,

    #[serde(flatten)]
    pub startup_parameters_wrapper: StartupParameterWrapper,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartupParameterWrapper {
    pub startup_parameters: Vec<StartupParameter>,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Platform {
    #[serde(rename = "PLATFORM_PC")]
    Pc,
    #[serde(rename = "PLATFORM_XBL")]
    Xbl,
    #[serde(rename = "PLATFORM_PSN")]
    Psn,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub mod_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerIdentityId {
    pub name: String,
    pub identity_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartupParameter {
    pub parameter: String,
    pub tooltip: String,
    pub enabled: bool,
    #[serde(rename = "type")]
    pub param_type: String, // "number" | "string" | "select"
    pub value: Option<serde_json::Value>, // Can be number, string, or null
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_list: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_val: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_val: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[schema(example = "Available")]
pub enum ArsStatus {
    Unknown = 0,
    Available = 1,
    Recreating = 2,
    RecreatingFailure = 3,
    Unavailable = 4,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub last_tag_time: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
pub struct A2S {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rcon {
    pub address: String,
    pub port: u16,
    pub password: String,
    pub max_clients: u32,
    pub permission: String,
    pub blacklist: Vec<String>,
    pub whitelist: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GameProperties {
    pub server_max_view_distance: u32,
    pub server_min_grass_distance: u32,
    pub fast_validation: bool,
    pub network_view_distance: u32,
    pub battl_eye: bool,
    pub disable_third_person: bool,
    #[serde(rename = "VONDisableUI")]
    pub von_disable_ui: bool,
    #[serde(rename = "VONDisableDirectSpeechUI")]
    pub von_disable_direct_speech_ui: bool,
    #[serde(rename = "VONCanTransmitCrossFaction")]
    pub von_can_transmit_cross_faction: bool,
    pub mission_header: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub name: String,
    pub password: String,
    pub password_admin: String,
    pub admins: Vec<String>,
    pub scenario_id: String,
    pub max_players: u32,
    pub visible: bool,
    pub cross_platform: bool,
    pub supported_platforms: Vec<Platform>,
    pub game_properties: GameProperties,
    pub mods_required_by_default: bool,
    pub mods: Vec<Mod>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JoinQueue {
    pub max_size: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Operating {
    pub lobby_player_synchronise: bool,
    pub disable_crash_reporter: bool,
    #[serde(default = "Vec::new")]
    pub disable_navmesh_streaming: Vec<String>,
    pub disable_server_shutdown: bool,
    #[serde(rename = "disableAI")]
    pub disable_ai: bool,
    pub player_save_time: u32,
    pub ai_limit: i32,
    pub slot_reservation_timeout: u32,
    pub join_queue: JoinQueue,
}

#[derive(Clone, Debug, Serialize, Deserialize, FromJsonQueryResult, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub bind_address: String,
    pub bind_port: u16,
    pub public_address: String,
    pub public_port: u16,
    pub a2s: A2S,
    pub rcon: Rcon,
    pub game: Game,
    pub operating: Operating,
}
