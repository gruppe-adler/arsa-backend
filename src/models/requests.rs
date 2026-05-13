use sea_orm::prelude::Uuid;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

#[derive(IntoParams, Deserialize, ToSchema)]
pub struct IdParams {
    /// Id of the server
    #[param(required, example = "f30b8424-28d6-4b0a-9348-9f05327fa886")]
    pub id: Uuid,
}
