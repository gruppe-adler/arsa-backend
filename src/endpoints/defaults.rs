use axum::extract::State;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use std::sync::Arc;

use crate::{
    AppState,
    models::{self, defaults::DEFAULTS_ID},
    shared::{AppJson, ArsaError},
};

/// The built-in seed used the first time defaults are requested and no row exists yet.
/// Mirrors arsa-frontend's `src/utils/defaults.ts`.
const SEED_JSON: &str = include_str!("./defaults_seed.json");

async fn get_or_seed(state: &Arc<AppState>) -> Result<models::defaults::Model, ArsaError> {
    if let Some(existing) = models::defaults::Entity::find_by_id(DEFAULTS_ID)
        .one(&state.db)
        .await?
    {
        return Ok(existing);
    }

    let seed: models::defaults::Model =
        serde_json::from_str(SEED_JSON).map_err(ArsaError::SerdeError)?;

    let inserted = models::defaults::ActiveModel {
        id: Set(DEFAULTS_ID),
        name: Set(seed.name),
        branch: Set(seed.branch),
        config: Set(seed.config),
        startup_parameters_wrapper: Set(seed.startup_parameters_wrapper),
    }
    .insert(&state.db)
    .await?;

    Ok(inserted)
}

#[utoipa::path(
    get,
    tag = "defaults",
    path = "/defaults",
    responses(
        (status = OK, description = "Global server defaults used to prefill new servers", body = models::defaults::Model)
    )
)]
pub async fn get_defaults(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<models::defaults::Model>, ArsaError> {
    Ok(AppJson(get_or_seed(&state).await?))
}

#[utoipa::path(
    put,
    tag = "defaults",
    path = "/defaults",
    request_body(
        description = "Global server defaults to save",
        content = inline(models::defaults::Model)
    ),
    responses(
        (status = OK, description = "Defaults were saved", body = models::defaults::Model)
    )
)]
pub async fn put_defaults(
    State(state): State<Arc<AppState>>,
    AppJson(params): AppJson<models::defaults::Model>,
) -> Result<AppJson<models::defaults::Model>, ArsaError> {
    // Ensure a row exists (and is seeded) before updating it, so PUT works
    // the same whether or not GET was ever called first.
    get_or_seed(&state).await?;

    let updated = models::defaults::ActiveModel {
        id: Set(DEFAULTS_ID),
        name: Set(params.name),
        branch: Set(params.branch),
        config: Set(params.config),
        startup_parameters_wrapper: Set(params.startup_parameters_wrapper),
    }
    .update(&state.db)
    .await?;

    Ok(AppJson(updated))
}
