use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Singleton row id — there is only ever one set of host settings.
pub const SETTINGS_ID: i32 = 1;

/// Host-wide settings, changed by admins in ARS Service.
#[sea_orm::model]
#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, ToSchema, Deserialize, Serialize)]
#[sea_orm(table_name = "settings")]
#[serde(rename_all = "camelCase")]
#[schema(as = Settings)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[serde(skip)]
    #[schema(ignore)]
    pub id: i32,
    /// When on, servers must use the ports of a port group; custom ports are rejected.
    #[serde(default)]
    pub port_groups_only: bool,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            id: SETTINGS_ID,
            port_groups_only: false,
        }
    }
}

impl ActiveModelBehavior for ActiveModel {}
