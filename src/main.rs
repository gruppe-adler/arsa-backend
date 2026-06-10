use axum::{
    extract::{
        ConnectInfo, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket},
    },
    http::{
        HeaderValue, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    response::IntoResponse,
    routing::{any, get},
};
use axum_extra::{TypedHeader, headers};
use bollard::{
    Docker,
    query_parameters::{EventsOptionsBuilder, ListContainersOptionsBuilder},
};
use futures::{SinkExt, StreamExt};
use local_ip_address::local_ip;
use sea_orm::{ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    fs, signal,
    sync::{Mutex, broadcast},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;
use uuid::Uuid;

use crate::{
    endpoints::{image::*, server::*, workshop::*},
    models::{
        requests::EditProfileFileRequest,
        responses::{LogType, ProfileFileListResponse},
        server::ArsStatus,
    },
    shared::ArsaError,
};

mod endpoints;
mod models;
mod shared;

#[derive(OpenApi)]
#[openapi(
    components(schemas(LogType, Branch, ListScenariosResponse, ProfileFileListResponse, EditProfileFileRequest)),
    tags(
        (name = "server", description = "Server api endpoints"),
        (name = "workshop", description = "Workshop api"),
        (name = "image", description = "Docker image endpoints")
    ),
    paths(
        get_profile_file,
        put_profile_file
    )
)]
struct ApiDoc;

pub struct AppState {
    pub ip: IpAddr,
    pub db: DatabaseConnection,
    pub status: Mutex<ArsStatus>,
    pub channel: broadcast::Sender<String>,
    pub docker: Docker,
    pub watchers: tokio::sync::RwLock<
        std::collections::HashMap<uuid::Uuid, tokio_util::sync::CancellationToken>,
    >,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let docker = Docker::connect_with_defaults().expect("Couldn't connect to docker");
    let ip = local_ip().unwrap();

    let cors_layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
            Method::PUT,
        ])
        .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
        .allow_headers([AUTHORIZATION, CONTENT_TYPE]);

    println!("Local IP address: {:?}", ip);

    let ars_path = crate::endpoints::server::get_ars_path();
    if !ars_path.exists() {
        fs::create_dir_all(ars_path).await?;
    }

    let base_path = crate::endpoints::server::get_base_path()
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    if !base_path.exists() {
        fs::create_dir_all(base_path).await?;
    }

    let addon_path = crate::endpoints::server::get_addon_download_dir()
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    if !addon_path.exists() {
        fs::create_dir_all(addon_path).await?;
    }

    let db_dir = PathBuf::from("./db/");
    if !db_dir.exists() {
        fs::create_dir(&db_dir).await?;
    }
    let db_file_path = db_dir.join("arsa.sqlite");
    if !db_file_path.exists() {
        fs::File::create(&db_file_path).await?;
    }

    let db = Database::connect(format!(
        "sqlite://{}?mode=rwc",
        db_file_path
            .into_os_string()
            .into_string()
            .unwrap_or_default()
    ))
    .await?;

    db.get_schema_registry(module_path!().split("::").next().unwrap())
        .sync(&db)
        .await?;

    // Cleanup pull logs
    models::pull_log::Entity::delete_many()
        .filter(models::pull_log::Column::Id.is_not_null())
        .exec(&db)
        .await?;

    let server_id_routes = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_server, delete_server))
        .routes(routes!(start_server))
        .routes(routes!(stop_server))
        .routes(routes!(get_logs))
        .routes(routes!(get_log_file))
        .routes(routes!(get_crash_log))
        .routes(routes!(delete_log))
        .routes(routes!(get_player_log))
        .routes(routes!(get_stats))
        .routes(routes!(get_size_method))
        .routes(routes!(get_profile_files))
        .route(
            "/{id}/profile/{*path}",
            get(get_profile_file).put(put_profile_file),
        )
        .routes(routes!(is_server_running));

    let server_routes = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/server", server_id_routes)
        .routes(routes!(post_server, put_server))
        .routes(routes!(get_servers))
        .routes(routes!(get_public_ip))
        .routes(routes!(get_pull_image))
        .routes(routes!(get_pull_logs))
        .routes(routes!(get_image_version))
        .routes(routes!(update_scenarios_from_branch))
        .routes(routes!(get_scenarios_from_branch))
        .routes(routes!(get_status));

    let workshop_routes = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(get_workshop))
        .routes(routes!(get_workshop_detail))
        .routes(routes!(get_workshop_scenarios));

    let api_routes_v2 = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/v2", server_routes)
        .nest("/v2", workshop_routes);

    let (tx, _rx) = broadcast::channel(50);
    let app_state = Arc::new(AppState {
        db,
        ip,
        status: Mutex::new(ArsStatus::Available),
        channel: tx,
        docker,
        watchers: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });

    update_container_status(&app_state)
        .await
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/api", api_routes_v2)
        .route("/ws", any(ws_handler))
        .layer(cors_layer)
        .with_state(app_state.clone())
        .split_for_parts();

    let router = router.merge(SwaggerUi::new("/swagger-ui").url("/apidoc/openapi.json", api));

    let (watch_handle, watch_token) = watch_containers(&app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    println!();
    println!("Shutting down...");
    watch_token.cancel();
    let _ = watch_handle.await;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

fn watch_containers(state: &Arc<AppState>) -> (JoinHandle<()>, CancellationToken) {
    let token = CancellationToken::new();

    let cloned_token = token.clone();

    let mut filters = HashMap::new();
    filters.insert("type", vec!["container"]);
    filters.insert("label", vec!["de.grad.arsa.version"]);
    filters.insert("event", vec!["start", "die"]);
    let state = state.clone();

    let mut event_stream = state
        .docker
        .events(Some(EventsOptionsBuilder::new().filters(&filters).build()));

    let join_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                event = event_stream.next() => {
                    if let Some(event) = event
                        && let Ok(event_message) = event
                        && let Some(action) = event_message.action
                        && let Some(actor) = event_message.actor
                        && let Some(id) = actor.id
                        && let Ok(container_info) = state.docker.inspect_container(&id, None).await
                        && let Some(name) = container_info.name
                        && let Ok(id) = Uuid::parse_str(name.trim_start_matches('/'))
                    {
                        if action == "start" {
                            let _ = update_is_running_by_id(&state, &id, true).await;
                        } else if action == "die" {
                            let _ = update_is_running_by_id(&state, &id, false).await;
                        }
                    }
                },
                _ = cloned_token.cancelled() => {
                    println!("Container watch stopped.");
                    break;
                }
            }
        }
    });
    (join_handle, token)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");

    ws.on_upgrade(move |socket| handle_socket(socket, addr, state))
}

async fn handle_socket(socket: WebSocket, _who: SocketAddr, state: Arc<AppState>) {
    let (sender, mut receiver) = socket.split();
    let sender = Arc::new(tokio::sync::Mutex::new(sender));

    let mut rx = state.channel.subscribe();

    let sender_clone = sender.clone();
    let _send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            println!("Message received: {msg}");
            let mut s = sender_clone.lock().await;
            if let Err(err) = s.send(Message::text(msg)).await {
                println!("Error during ws send: {err}");
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        if let Ok(msg) = msg {
            if let Message::Text(txt) = msg {
                if txt == "ping" {
                    let _ = sender
                        .lock()
                        .await
                        .send(Message::Text("pong".to_string().into()))
                        .await;
                } else {
                    println!("Received unknown WebSocket message: {txt}");
                }
            }
        } else {
            let error = msg.err().unwrap();
            println!("Error receiving message: {:?}", error);
            let mut s = sender.lock().await;
            send_close_message(&mut s, 1011, &format!("Error occurred: {}", error)).await;
            break;
        }
    }
}

async fn send_close_message(
    socket: &mut futures::stream::SplitSink<WebSocket, Message>,
    code: u16,
    reason: &str,
) {
    _ = socket
        .send(Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })))
        .await;
}

async fn update_container_status(state: &Arc<AppState>) -> Result<(), ArsaError> {
    let mut filters = HashMap::new();
    filters.insert("label", vec!["de.grad.arsa.version"]);

    let containers = state
        .docker
        .list_containers(Some(
            ListContainersOptionsBuilder::new()
                .all(true)
                .filters(&filters)
                .build(),
        ))
        .await?;

    for container in containers {
        if let Some(names) = container.names
            && let Some(name) = names.first()
            && let Ok(id) = Uuid::parse_str(name.trim_start_matches('/'))
            && let Some(status) = container.status
        {
            let is_running = status == "running" && status == "restarting";
            update_is_running_by_id(state, &id, is_running).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::models::server::Model;
    use regex::Regex;
    use std::sync::LazyLock;

    static LOG_DIR_NAME_REGEX: LazyLock<Regex> = std::sync::LazyLock::new(|| {
        Regex::new("^logs_([0-9]{4})-([0-9]{2})-([0-9]{2})_([0-9]{2})-([0-9]{2})-([0-9]{2})$")
            .unwrap()
    });

    #[test]
    fn test_regex() {
        assert!(LOG_DIR_NAME_REGEX.is_match("logs_2026-03-14_15-21-11"));
        assert!(!LOG_DIR_NAME_REGEX.is_match("alogs_2026-03-14_15-21-11"));
        assert!(!LOG_DIR_NAME_REGEX.is_match("2026-03-14_15-21-11"));
        assert!(!LOG_DIR_NAME_REGEX.is_match("logs_2026_03-14_15-21-11"));
        assert!(!LOG_DIR_NAME_REGEX.is_match("../logs_2026_03-14_15-21-11"));
        assert!(!LOG_DIR_NAME_REGEX.is_match("/logs_2026_03-14_15-21-11/"));
    }

    #[test]
    fn test_json_deserialize() {
        let server: Model = serde_json::from_str(EXAMPLE_CONFIG).unwrap();
        dbg!(server);
    }

    #[test]
    fn test_json_serialize() {
        let server: Model = serde_json::from_str(EXAMPLE_CONFIG).unwrap();
        let json = serde_json::to_string(&server).unwrap();
        dbg!(json);
    }

    const EXAMPLE_CONFIG: &str = r#"{
  "uuid": "",
  "name": "my server's name",
  "branch": "stable",
  "isRunning": false,
  "playerCount": 1337,
  "config": {
    "bindAddress": "0.0.0.0",
    "bindPort": 2001,
    "publicAddress": "172.30.154.230",
    "publicPort": 2001,
    "a2s": {
      "address": "0.0.0.0",
      "port": 17777
    },
    "rcon": {
      "address": "0.0.0.0",
      "port": 19999,
      "password": "mE*mueuJnG27@LZrgK4pLjEL",
      "maxClients": 16,
      "permission": "monitor",
      "blacklist": [],
      "whitelist": []
    },
    "game": {
      "name": "my server's name",
      "password": "bEBArjt_Jt-A!Ce*xrr2huQo",
      "passwordAdmin": "gHuaVyZCFQ!Rg984HMj6aoHn",
      "admins": [],
      "scenarioId": "{59AD59368755F41A}Missions/21_GM_Eden.conf",
      "maxPlayers": 32,
      "visible": true,
      "crossPlatform": false,
      "supportedPlatforms": [
        "PLATFORM_PC",
        "PLATFORM_XBL",
        "PLATFORM_PSN"
      ],
      "gameProperties": {
        "serverMaxViewDistance": 2500,
        "serverMinGrassDistance": 50,
        "fastValidation": true,
        "networkViewDistance": 1000,
        "battlEye": true,
        "disableThirdPerson": true,
        "VONDisableUI": true,
        "VONDisableDirectSpeechUI": true,
        "VONCanTransmitCrossFaction": false,
        "missionHeader": {}
      },
      "modsRequiredByDefault": true,
      "mods": []
    },
    "operating": {
      "lobbyPlayerSynchronise": true,
      "disableCrashReporter": false,
      "disableServerShutdown": false,
      "disableAI": false,
      "playerSaveTime": 120,
      "aiLimit": -1,
      "slotReservationTimeout": 60,
      "joinQueue": {
        "maxSize": 0
      }
    }
  },
  "startupParameters": [
    {
      "parameter": "autoReload",
      "tooltip": "value is in seconds",
      "enabled": false,
      "type": "number",
      "value": 10,
      "minVal": 0
    },
    {
      "parameter": "loadSessionSave",
      "tooltip": "It can be used alone to load the latest save, or with a specific save file name.",
      "enabled": false,
      "type": "string",
      "value": ""
    },
    {
      "parameter": "logStats",
      "tooltip": "defined interval in milliseconds",
      "enabled": false,
      "type": "number",
      "value": 10000,
      "minVal": 1
    },
    {
      "parameter": "maxFPS",
      "tooltip": "should always be set to prevent high load on server",
      "enabled": true,
      "type": "number",
      "value": 30,
      "minVal": 1
    },
    {
      "parameter": "nds",
      "tooltip": "The provided value stands for diameter, or the number of cells which are being replicated - default is 2 in each direction.",
      "enabled": false,
      "type": "number",
      "value": 2,
      "minVal": 1
    },
    {
      "parameter": "nwkResolution",
      "tooltip": "defines what resolution Spatial Map cells should be set at in a 100..1000m range",
      "enabled": false,
      "type": "number",
      "value": 500,
      "minVal": 100,
      "maxVal": 1000
    },
    {
      "parameter": "rpl-timeout-ms",
      "tooltip": "sets the client/server timeout's value, in milliseconds",
      "enabled": false,
      "type": "number",
      "value": 10000,
      "minVal": 1
    },
    {
      "parameter": "staggeringBudget",
      "tooltip": "defines how many stationary spatial map cells are allowed to be processed in one tick in 1..10201 range",
      "enabled": false,
      "type": "number",
      "value": 5000,
      "minVal": 1,
      "maxVal": 10201
    },
    {
      "parameter": "streamingBudget",
      "tooltip": "The global streaming budget that is equally distributed between all connections. It cannot go under 100 to prevent the system stalling.",
      "enabled": false,
      "type": "number",
      "value": 500,
      "minVal": 100
    },
    {
      "parameter": "streamsDelta",
      "tooltip": "is a tool to limit the amount of streams being opened for a client in range 1..1000 (default 100)",
      "enabled": false,
      "type": "number",
      "value": 200,
      "minVal": 1,
      "maxVal": 1000
    },
    {
      "parameter": "keepNumOfLogs",
      "tooltip": "sets the maximum amount of logs to keep (default: 10)",
      "enabled": false,
      "type": "number",
      "value": 10,
      "minVal": 1
    },
    {
      "parameter": "logLevel",
      "tooltip": "allows for different log levels. Each level includes the ones below it (e.g error includes error and fatal). Possible values range from normal (where everything is logged) to fatal (where only extreme issues are logged)",
      "enabled": false,
      "type": "select",
      "value": "normal",
      "valueList": [
        "normal",
        "warning",
        "error",
        "fatal"
      ]
    }
  ]
}"#;
}
