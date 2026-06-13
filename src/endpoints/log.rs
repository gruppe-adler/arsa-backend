use std::sync::Arc;

use axum::extract::{Query, State};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};

use crate::{
    AppState,
    models::{self, responses::ErrorResponse},
    shared::{AppJson, ArsaError, PaginatedResponse, PaginationParams},
};

#[utoipa::path(
    get,
    tag = "log",
    path = "/logs",
    params(PaginationParams),
    responses(
        (status = OK, description = "Logs returned successfully", body = PaginatedResponse<models::log::Model>),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to get logs", body = ErrorResponse),
    )
)]
pub async fn get_global_logs(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
) -> Result<AppJson<PaginatedResponse<models::log::Model>>, ArsaError> {
    let page = pagination.page.unwrap_or(1);
    let limit = pagination.limit.unwrap_or(50);

    let paginator = models::log::Entity::find()
        .order_by_desc(models::log::Column::Timestamp)
        .paginate(&state.db, limit);

    let total = paginator.num_items().await?;
    let total_pages = paginator.num_pages().await?;

    let data = paginator.fetch_page(page - 1).await?;

    Ok(AppJson(PaginatedResponse {
        data,
        page,
        limit,
        total,
        total_pages,
    }))
}
