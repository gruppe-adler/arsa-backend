use std::sync::OnceLock;

use axum::http::HeaderValue;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub allowed_origins: Vec<HeaderValue>,
    pub access_role: String,
    pub admin_role: String,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    pub oidc_client_secret: String,
    pub oidc_callback_uri: String,
    pub oidc_frontend_uri: String,
    pub bind_address: String,
    pub use_volume: bool,
    pub server_volume: String,
    pub repo_volume: String,
}

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

impl AppConfig {
    fn from_env() -> Self {
        AppConfig {
            allowed_origins: if let Ok(origins_env) = std::env::var("ARSA_ALLOWED_ORIGINS") {
                origins_env
                    .split(';')
                    .map(|x| x.to_string().parse::<HeaderValue>().unwrap())
                    .collect::<Vec<HeaderValue>>()
            } else {
                vec![]
            },
            access_role: std::env::var("ARSA_ACCESS_ROLE").unwrap_or_default(),
            admin_role: std::env::var("ARSA_ADMIN_ROLE").unwrap_or_default(),
            bind_address: std::env::var("ARSA_BIND_ADDRESS").unwrap_or("0.0.0.0:3000".to_string()),
            oidc_issuer: std::env::var("ARSA_OIDC_ISSUER").unwrap_or_default(),
            oidc_client_id: std::env::var("ARSA_OIDC_CLIENT_ID").unwrap_or_default(),
            oidc_client_secret: std::env::var("ARSA_OIDC_CLIENT_SECRET").unwrap_or_default(),
            oidc_callback_uri: std::env::var("ARSA_OIDC_CALLBACK_URI")
                .unwrap_or_else(|_| "http://localhost:3000/callback".to_string()),
            oidc_frontend_uri: std::env::var("ARSA_OIDC_FRONTEND_URI")
                .unwrap_or_else(|_| "http://localhost:5173/callback".to_string()),
            use_volume: std::env::var("ARSA_USE_VOLUME")
                .unwrap_or("false".to_string())
                .parse()
                .unwrap_or(true),
            server_volume: std::env::var("ARSA_SERVER_VOLUME")
                .unwrap_or("arsa-servers-volume".to_string()),
            repo_volume: std::env::var("ARSA_REPO_VOLUME")
                .unwrap_or("arsa-repo-volume".to_string()),
        }
    }

    /// Call this once at startup, before anything tries to use `get()`.
    pub fn init() {
        let config = AppConfig::from_env();
        CONFIG.set(config).expect("AppConfig already initialized");
    }

    /// Access from anywhere in the codebase.
    pub fn get() -> &'static AppConfig {
        CONFIG
            .get()
            .expect("AppConfig accessed before init() was called")
    }
}
