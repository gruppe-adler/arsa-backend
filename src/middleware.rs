use std::sync::Arc;

use crate::{AppState, config::AppConfig};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_cookie::CookieManager;

pub async fn auth_middleware(mut req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let state = req
        .extensions()
        .get::<Arc<AppState>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .clone();

    let cookie_manager = req
        .extensions()
        .get::<Result<CookieManager, (StatusCode, String)>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?
        .as_ref()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let claims = get_claims(&state, cookie_manager)
        .await
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    if !claims.has_role(&AppConfig::get().access_role) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

pub async fn get_claims(
    state: &Arc<AppState>,
    cookie_manager: &CookieManager,
) -> Option<crate::endpoints::auth::Claims> {
    let session_id = cookie_manager
        .get("session")
        .map(|cookie| cookie.value().to_string())?;

    let sessions = state.sessions.read().await;
    let claims = sessions.get(&session_id)?.clone();
    drop(sessions);

    if claims.needs_refresh(crate::endpoints::auth::SESSION_REFRESH_WINDOW_SECONDS) {
        return crate::endpoints::auth::refresh_session(
            state,
            cookie_manager,
            &session_id,
            &claims,
        )
        .await;
    }

    Some(claims)
}
