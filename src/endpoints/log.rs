use std::sync::Arc;

use axum::extract::{Query, State};
use chrono::Utc;
use sea_orm::{EntityTrait, FromQueryResult, PaginatorTrait, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    models::{self, log::LogAction, responses::ErrorResponse},
    shared::{AppJson, ArsaError, PaginatedResponse, PaginationParams},
};

#[derive(Debug, ToSchema, FromQueryResult, Serialize, Deserialize)]
pub struct GlobalLog {
    pub id: Uuid,

    pub action: LogAction,

    pub target: Option<Uuid>,

    pub actor_id: String,
    pub actor: String,

    pub timestamp: chrono::DateTime<Utc>,
}

#[utoipa::path(
    get,
    tag = "log",
    path = "/logs",
    params(PaginationParams),
    responses(
        (status = OK, description = "Logs returned successfully", body = PaginatedResponse<GlobalLog>),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to get logs", body = ErrorResponse),
    )
)]
pub async fn get_global_logs(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
) -> Result<AppJson<PaginatedResponse<GlobalLog>>, ArsaError> {
    let page = pagination.page.unwrap_or(1);
    let limit = pagination.limit.unwrap_or(50);

    let paginator = models::log::Entity::find()
        .find_also_related(models::user::Entity)
        .order_by_desc(models::log::Column::Timestamp)
        .into_model::<models::log::Model, models::user::Model>()
        .paginate(&state.db, limit);

    let total = paginator.num_items().await?;
    let total_pages = paginator.num_pages().await?;

    let data = paginator
        .fetch_page(page - 1)
        .await?
        .into_iter()
        .map(|(log, user)| {
            let actor_id = user
                .as_ref()
                .map(|user| user.id.clone())
                .unwrap_or_else(|| log.actor_id.clone().unwrap_or_default());
            let actor = user
                .as_ref()
                .map(|user| user.name.clone())
                .unwrap_or_else(|| log.actor_id.clone().unwrap_or_default());

            GlobalLog {
                id: log.id,
                action: log.action,
                target: log.target,
                actor_id,
                actor,
                timestamp: log.timestamp,
            }
        })
        .collect::<Vec<_>>();

    Ok(AppJson(PaginatedResponse {
        data,
        page,
        limit,
        total,
        total_pages,
    }))
}
