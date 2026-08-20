use std::{collections::HashMap, sync::Arc, time::Duration};

use ::reqwest::{StatusCode, header};
use axum::{
    Extension,
    extract::{Query, State},
    http::HeaderValue,
    response::{IntoResponse, Redirect, Response},
};
use axum_cookie::CookieManager;
use axum_cookie::cookie::{Cookie, cookie::SameSite};
use chrono::Utc;
use openidconnect::{core::*, *};
use sea_orm::{ActiveValue::Set, EntityTrait, sea_query::OnConflict};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    config::AppConfig,
    models,
    shared::{AppJson, ArsaError},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KeycloakAdditionalClaims {
    #[serde(default)]
    pub realm_access: Option<RealmAccess>,
    #[serde(default)]
    pub resource_access: Option<HashMap<String, ResourceAccess>>,
}

impl AdditionalClaims for KeycloakAdditionalClaims {}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct RealmAccess {
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceAccess {
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct Claims {
    sub: String,
    // email: Option<String>,
    preferred_username: Option<String>,
    realm_access: Option<RealmAccess>,
    exp: usize,
    picture: Option<String>,
    #[serde(skip)]
    refresh_token: Option<String>,
}

impl Claims {
    pub fn has_role(&self, role: &str) -> bool {
        if let Some(ra) = &self.realm_access {
            return ra.roles.contains(&role.to_owned());
        }
        false
    }

    pub fn get_user(&self) -> String {
        self.sub.clone()
    }

    pub fn needs_refresh(&self, refresh_window_seconds: usize) -> bool {
        let now = Utc::now().timestamp() as usize;
        self.exp <= now + refresh_window_seconds
    }
}

pub const SESSION_REFRESH_WINDOW_SECONDS: usize = 300;

type KeycloakClient = Client<
    KeycloakAdditionalClaims,
    core::CoreAuthDisplay,
    core::CoreGenderClaim,
    core::CoreJweContentEncryptionAlgorithm,
    core::CoreJsonWebKey,
    core::CoreAuthPrompt,
    StandardErrorResponse<core::CoreErrorResponseType>,
    StandardTokenResponse<
        IdTokenFields<
            KeycloakAdditionalClaims,
            EmptyExtraTokenFields,
            core::CoreGenderClaim,
            core::CoreJweContentEncryptionAlgorithm,
            core::CoreJwsSigningAlgorithm,
        >,
        core::CoreTokenType,
    >,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, core::CoreTokenType>,
    core::CoreRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

fn build_http_client() -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Couldn't build http_client")
}

async fn build_oidc_client() -> Result<KeycloakClient, ArsaError> {
    let config = AppConfig::get();
    let http_client = build_http_client();

    let issuer_url = IssuerUrl::new(config.oidc_issuer.clone())
        .map_err(|err| ArsaError::UnknownError(err.to_string()))?;
    let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, &http_client)
        .await
        .map_err(|err| ArsaError::UnknownError(err.to_string()))?;

    let client = KeycloakClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(config.oidc_client_id.clone()),
        Some(ClientSecret::new(config.oidc_client_secret.clone())),
    )
    .set_redirect_uri(
        RedirectUrl::new(config.oidc_callback_uri.clone())
            .map_err(|err| ArsaError::UnknownError(err.to_string()))?,
    );

    Ok(client)
}

pub fn build_session_cookie(session_id: &str) -> Cookie<'static> {
    Cookie::new("session", session_id.to_string())
        .with_path("/")
        .with_http_only(true)
        .with_same_site(SameSite::None)
        .with_secure(true)
        .with_max_age(Duration::from_secs(86400))
}

pub async fn refresh_session(
    state: &Arc<AppState>,
    cookie_manager: &CookieManager,
    session_id: &str,
    claims: &Claims,
) -> Option<Claims> {
    let refresh_token = claims.refresh_token.as_ref()?;
    let client = build_oidc_client().await.ok()?;
    let http_client = build_http_client();

    let token_response = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.clone()))
        .map_err(|err| {
            eprintln!("Failed to build OIDC refresh request: {err}");
        })
        .ok()?
        .request_async(&http_client)
        .await
        .map_err(|err| {
            eprintln!("Failed to refresh OIDC session: {err}");
        })
        .ok()?;

    let new_exp = token_response
        .expires_in()
        .map(|duration| Utc::now().timestamp() as usize + duration.as_secs() as usize)
        .unwrap_or(claims.exp);

    let refreshed_claims = Claims {
        sub: claims.sub.clone(),
        preferred_username: claims.preferred_username.clone(),
        realm_access: claims.realm_access.clone(),
        exp: new_exp,
        picture: claims.picture.clone(),
        refresh_token: token_response
            .refresh_token()
            .map(|token| token.secret().to_string())
            .or_else(|| claims.refresh_token.clone()),
    };

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.to_string(), refreshed_claims.clone());
    }

    cookie_manager.add(build_session_cookie(session_id));

    Some(refreshed_claims)
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
}

pub struct OidcAuthState {
    pkce_verifier: String,
    nonce: String,
}

#[utoipa::path(
    get,
    tag = "auth",
    path = "/login",
    responses((status = SEE_OTHER, description = "Redirect to the configured OpenID provider"))
)]
pub async fn login(State(state): State<Arc<AppState>>) -> Result<Redirect, ArsaError> {
    let client = build_oidc_client().await?;
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    {
        let mut oauth_states = state.oauth_states.write().await;
        oauth_states.insert(
            csrf_token.secret().to_string(),
            OidcAuthState {
                pkce_verifier: pkce_verifier.secret().to_string(),
                nonce: nonce.secret().to_string(),
            },
        );
    }

    Ok(Redirect::to(auth_url.as_str()))
}

#[utoipa::path(
    get,
    tag = "auth",
    path = "/callback",
    responses((status = SEE_OTHER, description = "Redirect back to the app after completing the OpenID flow"))
)]
pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, ArsaError> {
    let code = query.code.ok_or(ArsaError::BadRequest)?;
    let csrf_state = query.state.ok_or(ArsaError::BadRequest)?;

    let auth_state = {
        let mut oauth_states = state.oauth_states.write().await;
        oauth_states.remove(&csrf_state)
    }
    .ok_or(ArsaError::BadRequest)?;

    let client = build_oidc_client().await?;
    let http_client = build_http_client();

    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|err| ArsaError::UnknownError(err.to_string()))?
        .set_pkce_verifier(PkceCodeVerifier::new(auth_state.pkce_verifier.clone()))
        .request_async(&http_client)
        .await
        .map_err(|err| ArsaError::UnknownError(err.to_string()))?;

    let id_token = token_response
        .extra_fields()
        .id_token()
        .ok_or_else(|| ArsaError::UnknownError("Server did not return an ID token".to_string()))?;
    let id_token_verifier = client.id_token_verifier();
    let nonce = Nonce::new(auth_state.nonce.clone());
    let id_token_claims = id_token
        .claims(&id_token_verifier, &nonce)
        .map_err(|err| ArsaError::UnknownError(err.to_string()))?;

    let add = id_token_claims.additional_claims();

    let session_id = Uuid::new_v4().to_string();
    let app_claims = Claims {
        sub: id_token_claims.subject().to_string(),
        // email: id_token_claims.email().map(|email| email.to_string()),
        preferred_username: id_token_claims
            .preferred_username()
            .map(|username| username.to_string()),
        realm_access: add.realm_access.clone().map(|realm_access| RealmAccess {
            roles: realm_access.roles,
        }),
        exp: id_token_claims.expiration().timestamp() as usize,
        picture: id_token_claims
            .picture()
            .and_then(|claim| claim.get(None))
            .map(|url| url.as_str().to_string()),
        refresh_token: token_response
            .refresh_token()
            .map(|token| token.secret().to_string()),
    };

    let user = models::user::ActiveModel {
        id: Set(app_claims.sub.clone()),
        name: Set(app_claims
            .preferred_username
            .as_deref()
            .unwrap_or("User")
            .to_string()),
    };

    let _ = models::user::Entity::insert(user)
        .on_conflict(
            OnConflict::column(models::user::Column::Id)
                .update_columns([models::user::Column::Name])
                .to_owned(),
        )
        .exec(&state.db)
        .await?;

    if !app_claims.has_role(&AppConfig::get().access_role) {
        return Ok(StatusCode::UNAUTHORIZED.into_response());
    }

    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.clone(), app_claims.clone());
    }

    if let Some(expected_access_token_hash) = id_token_claims.access_token_hash() {
        let actual_access_token_hash = AccessTokenHash::from_token(
            token_response.access_token(),
            id_token
                .signing_alg()
                .map_err(|err| ArsaError::UnknownError(err.to_string()))?,
            id_token
                .signing_key(&id_token_verifier)
                .map_err(|err| ArsaError::UnknownError(err.to_string()))?,
        )
        .map_err(|err| ArsaError::UnknownError(err.to_string()))?;
        if actual_access_token_hash != *expected_access_token_hash {
            return Err(ArsaError::Unauthorized);
        }
    }

    let cookie = build_session_cookie(&session_id);

    let mut response = Redirect::to(&AppConfig::get().oidc_frontend_uri).into_response();
    *response.status_mut() = StatusCode::MOVED_PERMANENTLY;
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string())
            .map_err(|err| ArsaError::UnknownError(err.to_string()))?,
    );

    Ok(response)
}

#[utoipa::path(
    get,
    tag = "auth",
    path = "/userclaims",
    responses(
        (status = OK, description = "User Claims", body = Claims),
    )
)]
pub async fn user_claims(
    Extension(claims): Extension<Claims>,
) -> Result<AppJson<Claims>, ArsaError> {
    Ok(AppJson(claims))
}

#[utoipa::path(
    get,
    tag = "auth",
    path = "/logout",
    responses((status = SEE_OTHER, description = "Redirect to frontend?"))
)]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    cookie_manager: CookieManager,
) -> Result<Redirect, ArsaError> {
    println!("in loguot");
    let session_id = cookie_manager
        .get("session")
        .map(|cookie| cookie.value().to_string())
        .ok_or(ArsaError::Unauthorized)?;
    {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&session_id);
    }

    cookie_manager.remove("session");
    Ok(Redirect::to(&AppConfig::get().oidc_frontend_uri))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_expired_tokens_for_refresh() {
        let claims = Claims {
            sub: "user".to_string(),
            preferred_username: None,
            realm_access: None,
            exp: (Utc::now().timestamp() - 60) as usize,
            picture: None,
            refresh_token: None,
        };

        assert!(claims.needs_refresh(300));
    }

    #[test]
    fn keeps_fresh_tokens_from_refresh() {
        let claims = Claims {
            sub: "user".to_string(),
            preferred_username: None,
            realm_access: None,
            exp: (Utc::now().timestamp() + 3600) as usize,
            picture: None,
            refresh_token: None,
        };

        assert!(!claims.needs_refresh(300));
    }
}
