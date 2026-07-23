use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    endpoints::server::Branch,
    models::server::{ServerConfig, StartupParameterWrapper},
};

/// Singleton row id — there is only ever one set of global server defaults.
pub const DEFAULTS_ID: i32 = 1;

#[sea_orm::model]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, DeriveEntityModel, ToSchema)]
#[sea_orm(table_name = "server_defaults")]
#[serde(rename_all = "camelCase")]
#[schema(as = ServerDefaults)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[serde(skip_deserializing, default)]
    pub id: i32,
    pub name: String,

    #[schema(default = "Branch::Stable")]
    pub branch: Branch,

    pub config: ServerConfig,

    #[serde(flatten)]
    pub startup_parameters_wrapper: StartupParameterWrapper,
}

impl ActiveModelBehavior for ActiveModel {}
