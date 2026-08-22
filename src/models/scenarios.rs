use crate::endpoints::server::Branch;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[sea_orm::model]
#[derive(
    Debug, Default, Clone, PartialEq, Eq, DeriveEntityModel, ToSchema, Deserialize, Serialize,
)]
#[sea_orm(table_name = "scenario")]
#[serde(rename_all = "camelCase")]
#[schema(as = Scenario)]
pub struct Model {
    #[sea_orm(primary_key)]
    pub path: String,

    pub name: String,
    pub branch: Branch,
}

impl ActiveModelBehavior for ActiveModel {}
