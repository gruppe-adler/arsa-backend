use crate::models::server::deserialize_uuid;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[sea_orm::model]
#[derive(
    Debug, Default, Clone, PartialEq, Eq, DeriveEntityModel, ToSchema, Deserialize, Serialize,
)]
#[sea_orm(table_name = "pull_log")]
#[serde(rename_all = "camelCase")]
#[schema(as = PullLog)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,

    #[sea_orm(index)]
    #[serde(default = "Uuid::new_v4", deserialize_with = "deserialize_uuid")]
    pub pull_id: Uuid,

    pub error_detail_code: i64,
    pub error_detail_message: String,

    pub status: String,

    pub progress_detail_current: i64,
    pub progress_detail_total: i64,
}

impl ActiveModelBehavior for ActiveModel {}
