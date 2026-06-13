use crate::models::server::deserialize_uuid;
use chrono::Utc;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    EnumIter, DeriveActiveEnum, Debug, Clone, PartialEq, Eq, ToSchema, Deserialize, Serialize,
)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum LogAction {
    #[sea_orm(num_value = 0)]
    ServerAdded,
    #[sea_orm(num_value = 1)]
    ServerDeleted,
    #[sea_orm(num_value = 2)]
    ServerStarted,
    #[sea_orm(num_value = 3)]
    ServerUpdated,
    #[sea_orm(num_value = 4)]
    ServerStopped,
    #[sea_orm(num_value = 5)]
    ServerLogDeleted,
    #[sea_orm(num_value = 6)]
    ImagePullStarted,
}

#[sea_orm::model]
#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, ToSchema, Deserialize, Serialize)]
#[sea_orm(table_name = "log")]
#[serde(rename_all = "camelCase")]
#[schema(as = GlobalLog)]
pub struct Model {
    #[sea_orm(primary_key)]
    #[serde(default = "Uuid::new_v4", deserialize_with = "deserialize_uuid")]
    pub id: Uuid,

    pub action: LogAction,

    pub target: Option<Uuid>,

    pub timestamp: chrono::DateTime<Utc>,
}

impl ActiveModelBehavior for ActiveModel {}
