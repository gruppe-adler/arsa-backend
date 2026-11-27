use axum::{Extension, extract::State};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, PaginatorTrait};
use std::sync::Arc;

use crate::{
    AppState,
    config::AppConfig,
    endpoints::auth::Claims,
    models::{self, responses::ErrorResponse, settings::SETTINGS_ID},
    shared::{AppJson, ArsaError},
};

/// The stored settings, or the defaults when an admin never saved any.
pub async fn load_settings(state: &Arc<AppState>) -> Result<models::settings::Model, ArsaError> {
    Ok(models::settings::Entity::find_by_id(SETTINGS_ID)
        .one(&state.db)
        .await?
        .unwrap_or_default())
}

#[utoipa::path(
    get,
    tag = "settings",
    path = "/settings",
    responses(
        (status = OK, description = "Host-wide settings", body = models::settings::Model)
    )
)]
pub async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<models::settings::Model>, ArsaError> {
    Ok(AppJson(load_settings(&state).await?))
}

#[utoipa::path(
    put,
    tag = "settings",
    path = "/settings",
    request_body = models::settings::Model,
    responses(
        (status = OK, description = "The settings were saved", body = models::settings::Model),
        (status = BAD_REQUEST, description = "Port groups only needs at least one port group", body = ErrorResponse),
        (status = UNAUTHORIZED, description = "Only admins can change settings", body = ErrorResponse)
    )
)]
pub async fn put_settings(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    AppJson(params): AppJson<models::settings::Model>,
) -> Result<AppJson<models::settings::Model>, ArsaError> {
    if !claims.has_role(&AppConfig::get().admin_role) {
        return Err(ArsaError::Unauthorized);
    }

    // Otherwise no server could be saved or started any more.
    if params.port_groups_only && models::port_group::Entity::find().count(&state.db).await? == 0 {
        return Err(ArsaError::BadRequestMessage(
            "Add a port group before allowing only port groups".to_string(),
        ));
    }

    let exists = models::settings::Entity::find_by_id(SETTINGS_ID)
        .one(&state.db)
        .await?
        .is_some();
    let settings = models::settings::ActiveModel {
        id: Set(SETTINGS_ID),
        port_groups_only: Set(params.port_groups_only),
    };
    let saved = if exists {
        settings.update(&state.db).await?
    } else {
        settings.insert(&state.db).await?
    };

    Ok(AppJson(saved))
}
