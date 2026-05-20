use axum::extract::{Path, State};
use bollard::{
    query_parameters::{
        CreateContainerOptionsBuilder, CreateImageOptions, InspectContainerOptionsBuilder,
        StatsOptionsBuilder,
    },
    secret::{
        ContainerCreateBody, ContainerStateStatusEnum, ContainerStatsResponse, EndpointSettings,
        HostConfig, Mount, MountTypeEnum, NetworkingConfig, PortBinding,
    },
};
use chrono::DateTime;
use fs_extra::dir::get_size;
use futures::StreamExt;
use regex::Regex;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, EntityTrait, FromJsonQueryResult, prelude::Uuid, sea_query,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{Arc, LazyLock},
};
use tokio::{
    fs::{self, File},
    io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom},
};
use utoipa::{IntoParams, ToSchema};

use crate::{
    AppState,
    models::{self, player, requests::*, responses::*},
    shared::{AppJson, ArsaError},
};

#[derive(
    Debug, Clone, Copy, Deserialize, Serialize, ToSchema, Eq, PartialEq, FromJsonQueryResult,
)]
pub enum Branch {
    #[serde(rename = "stable")]
    Stable,
    #[serde(rename = "experimental")]
    Experimental,
}

impl Default for Branch {
    fn default() -> Self {
        Self::Stable
    }
}

#[derive(IntoParams, Deserialize)]
pub struct BranchParams {
    /// Branch to pull
    #[param(example = "stable")]
    pub branch: Branch,
}

#[allow(dead_code)] // Needed for utoipa
#[derive(IntoParams, Deserialize, ToSchema)]
pub struct LogFileParams {
    /// Log folder name
    #[param(example = "profileDir")]
    pub log: String,
    /// Log file type
    #[param(example = "console.log")]
    pub log_type: LogType,
}

async fn get_config_json_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_config_path(uuid).await?.join("config.json"))
}

async fn get_config_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_server_path(uuid).await?.join("config"))
}

async fn get_crash_log_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_log_path(uuid).await?.join("CrashReports.log"))
}

async fn get_logs_path(
    uuid: &Uuid,
    log: Option<String>,
    log_type: Option<LogType>,
) -> Result<PathBuf, ArsaError> {
    let mut server_path = get_log_path(uuid).await?;

    if let Some(log) = log {
        server_path = server_path.join(log);
        if let Some(log_type) = log_type {
            server_path = server_path.join(serde_json::to_string(&log_type)?.trim_matches('"'));
        }
    }

    Ok(server_path)
}

async fn get_log_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_profiles_path(uuid).await?.join("logs"))
}

async fn get_profiles_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_server_path(uuid).await?.join("profiles"))
}

async fn get_profile_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_profiles_path(uuid).await?.join("profile"))
}

async fn get_addons_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_profiles_path(uuid).await?.join("addons"))
}

async fn get_server_path(uuid: &Uuid) -> Result<PathBuf, ArsaError> {
    Ok(get_base_path().await?.join(uuid.to_string()))
}

pub async fn get_base_path() -> Result<PathBuf, ArsaError> {
    let mut base_path = get_ars_path();

    if base_path.is_relative() {
        base_path = fs::canonicalize(base_path).await?;
    }

    base_path = base_path.join("servers");

    Ok(base_path)
}

pub fn get_ars_path() -> PathBuf {
    PathBuf::from(env::var("ARSA_BASE_PATH").unwrap_or("./ars".to_string()))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/logs",
    params(IdParams),
    responses(
        (status = OK, description = "List of logs", body = ResultLogs),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]
pub async fn get_logs(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<ResultLogs>, ArsaError> {
    let _ = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    let mut result_logs = ResultLogs {
        success: false,
        contains_crash_report_log: false,
        logs: vec![],
    };

    let logs_dir = get_log_path(&id).await?;
    if !logs_dir.exists() {
        return Ok(AppJson(result_logs));
    }

    let log_dir = logs_dir.read_dir()?;

    for entry in log_dir {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            let entry_name = entry_path
                .file_name()
                .and_then(|x| x.to_os_string().into_string().ok())
                .unwrap_or_default();

            let log = Log {
                dir: entry_name,
                contains_console_log: entry_path.join("console.log").exists(),
                contains_script_log: entry_path.join("script.log").exists(),
                contains_error_log: entry_path.join("error.log").exists(),
                contains_crash_log: entry_path.join("crash.log").exists(),
            };
            result_logs.logs.push(log);
        }
    }

    if logs_dir.join("CrashReports.log").exists() {
        result_logs.contains_crash_report_log = true;
    }

    result_logs.success = true;

    Ok(AppJson(result_logs))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/logs/{log}/{log_type}",
    params(IdParams, LogFileParams),
    responses(
        (status = OK, description = "Log file content", body = FileContentResponse),
        (status = NOT_FOUND, description = "Server or log was not found", body = ErrorResponse),
        (status = BAD_REQUEST, description = "Invalid log directory name", body = ErrorResponse)
    )
)]
pub async fn get_log_file(
    State(state): State<Arc<AppState>>,
    Path((id, log, log_type)): Path<(Uuid, String, LogType)>,
) -> Result<AppJson<FileContentResponse>, ArsaError> {
    let _ = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    if !LOG_DIR_NAME_REGEX.is_match(&log) {
        return Err(ArsaError::BadRequest);
    }

    let log_path = get_logs_path(&id, Some(log), Some(log_type)).await?;

    let file_content = fs::read_to_string(log_path).await?;
    Ok(AppJson(FileContentResponse {
        file_content: file_content,
    }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/crash-reports-log",
    params(IdParams),
    responses(
        (status = OK, description = "Crash reports log content", body = FileContentResponse),
        (status = NOT_FOUND, description = "Server or log was not found", body = ErrorResponse)
    )
)]
pub async fn get_crash_log(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<FileContentResponse>, ArsaError> {
    let _ = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    let crash_report_path = get_crash_log_path(&id).await?;

    if crash_report_path.exists() {
        return Err(ArsaError::NotFound);
    }

    let crash_report_content = fs::read_to_string(crash_report_path).await?;

    Ok(AppJson(FileContentResponse {
        file_content: crash_report_content,
    }))
}

#[utoipa::path(
    delete,
    path = "/{id}/logs/{log}",
    tag = "arsa",
    params(IdParams),
    responses(
        (status = OK, description = "The log was deleted", body = SuccessResponse),
        (status = NOT_FOUND, description = "Server or log was not found", body = ErrorResponse),
        (status = BAD_REQUEST, description = "Invalid log directory name", body = ErrorResponse)
    )
)]
pub async fn delete_log(
    State(state): State<Arc<AppState>>,
    Path((uuid, log)): Path<(Uuid, String)>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let _ = models::server::Entity::find_by_id(uuid)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    if LOG_DIR_NAME_REGEX.is_match(&log) {
        return Err(ArsaError::BadRequest);
    }

    let log_path = get_logs_path(&uuid, Some(log), None).await?;
    if log_path.exists() || !log_path.is_dir() {
        return Err(ArsaError::NotFound);
    }

    fs::remove_dir_all(log_path).await?;

    Ok(AppJson(SuccessResponse { success: true }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/known-players",
    responses(
        (status = OK, description = "List of known players", body = Vec<PlayerIdentityId>),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]
pub async fn get_player_log(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<Vec<PlayerIdentityId>>, ArsaError> {
    let players = models::player::Entity::find().all(&state.db).await?;

    Ok(AppJson(players.into_iter().map(|x| x.into()).collect()))
}

fn calculate_cpu_percentage(stats: &ContainerStatsResponse) -> f64 {
    let cpu_delta = stats
        .cpu_stats
        .as_ref()
        .and_then(|x| x.cpu_usage.as_ref())
        .and_then(|x| x.total_usage)
        .unwrap_or_default() as f64
        - stats
            .precpu_stats
            .as_ref()
            .and_then(|x| x.cpu_usage.as_ref())
            .and_then(|x| x.total_usage)
            .unwrap_or_default() as f64;

    let system_delta = stats
        .cpu_stats
        .as_ref()
        .and_then(|x| x.system_cpu_usage)
        .unwrap_or_default() as f64
        - stats
            .precpu_stats
            .as_ref()
            .and_then(|x| x.system_cpu_usage)
            .unwrap_or_default() as f64;

    let mut online_cpus = stats
        .cpu_stats
        .as_ref()
        .and_then(|x| x.online_cpus)
        .unwrap_or_default() as f64;

    if online_cpus == 0.0 {
        online_cpus = stats
            .precpu_stats
            .as_ref()
            .and_then(|x| x.online_cpus)
            .unwrap_or_default() as f64;
    }

    if system_delta > 0.0 && cpu_delta > 0.0 {
        return (cpu_delta / system_delta) * online_cpus;
    }

    return 0.0;
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/stats",
    params(IdParams),
    responses(
        (status = OK, description = "Server statistics", body = DockerStats),
        (status = NOT_FOUND, description = "Server was not found or stats unavailable", body = ErrorResponse)
    )
)]
pub async fn get_stats(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<DockerStats>, ArsaError> {
    let _ = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    let container_name = id.to_string();
    let stats_response = state
        .docker
        .stats(
            &container_name,
            Some(StatsOptionsBuilder::new().stream(false).build()),
        )
        .next()
        .await
        .and_then(|x| x.ok());

    if let Some(stats) = stats_response {
        // https://github.com/docker/cli/blob/a6d013f4c9caabb22ab68225176f6d886d9e8a95/cli/command/container/stats_helpers.go#L183

        let timestamp = DateTime::parse_from_rfc3339(stats.read.as_deref().unwrap_or_default())
            .unwrap_or_default()
            .to_utc();

        let io_service_bytes_recursive = stats
            .blkio_stats
            .as_ref()
            .and_then(|x| x.io_service_bytes_recursive.clone())
            .unwrap_or_default();

        let block_io_read: u64 = io_service_bytes_recursive
            .iter()
            .filter(|x| x.op.as_deref().unwrap_or_default() == "read")
            .map(|x| x.value.unwrap_or_default())
            .sum();

        let block_io_write: u64 = io_service_bytes_recursive
            .iter()
            .filter(|x| x.op.as_deref().unwrap_or_default() == "write")
            .map(|x| x.value.unwrap_or_default())
            .sum();

        let cpu_percentage = calculate_cpu_percentage(&stats);

        let name = stats.name.unwrap_or_default().trim_matches('/').to_owned();

        let id = stats.id.unwrap_or_default();

        let mem_usage = stats
            .memory_stats
            .as_ref()
            .and_then(|x| x.usage)
            .unwrap_or_default();

        let mem_limit = stats
            .memory_stats
            .as_ref()
            .and_then(|x| x.limit)
            .unwrap_or_default();

        let (network_rx_bytes, network_tx_bytes): (u64, u64) = stats
            .networks
            .unwrap_or_default()
            .values()
            .map(|x| {
                (
                    x.rx_bytes.unwrap_or_default(),
                    x.tx_bytes.unwrap_or_default(),
                )
            })
            .fold((0, 0), |acc, (a, b)| (acc.0 + a, acc.1 + b));

        let pid_count = stats
            .pids_stats
            .as_ref()
            .and_then(|x| x.current)
            .unwrap_or_default();

        return Ok(AppJson(DockerStats {
            timestamp,
            block_io_read,
            block_io_write,
            cpu_percentage,
            name,
            id,
            mem_usage,
            mem_limit,
            network_rx_bytes,
            network_tx_bytes,
            pid_count,
        }));
    }

    return Err(ArsaError::NotFound);
}

async fn get_size_from_path(path: &PathBuf) -> Result<u64, ArsaError> {
    Ok(if path.exists() { get_size(path)? } else { 0 })
}

async fn get_all_files_from_path(path: &PathBuf) -> Result<Vec<String>, ArsaError> {
    Ok(if path.exists() {
        get_names_in_dir(path).await?
    } else {
        Vec::new()
    })
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/size",
    params(IdParams),
    responses(
        (status = OK, description = "Server directory sizes", body = ResultSize)
    )
)]
pub async fn get_size_method(
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<ResultSize>, ArsaError> {
    let path = get_server_path(&id).await?;
    if !path.exists() {
        return Ok(AppJson(ResultSize::default()));
    }

    let addons_path = get_addons_path(&id).await?;
    let logs_path = get_logs_path(&id, None, None).await?;

    Ok(AppJson(ResultSize {
        addons_size: get_size_from_path(&addons_path).await?,
        logs_size: get_size_from_path(&logs_path).await?,
        profile_size: get_size_from_path(&get_profile_path(&id).await?).await?,
        mods: get_all_files_from_path(&addons_path).await?,
        logs: get_all_files_from_path(&logs_path).await?,
    }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/start", params(IdParams),
    responses(
        (status = OK, description = "Server was started and/or is running", body = SuccessResponse),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]
pub async fn start_server(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let server = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    let container_name = id.to_string();

    create_dirs(id).await?;

    let inspect = state
        .clone()
        .docker
        .inspect_container(&container_name, None)
        .await;

    match inspect {
        Ok(res) => {
            if let Some(container_status) = res.state.and_then(|x| x.status) {
                if container_status == ContainerStateStatusEnum::RUNNING
                    || container_status == ContainerStateStatusEnum::RESTARTING
                {
                    return Ok(AppJson(SuccessResponse { success: true }));
                }
            }
        }
        Err(err) => {
            if let bollard::errors::Error::DockerResponseServerError {
                status_code,
                message: _,
            } = &err
                && *status_code == 404
            {
                create_server_container(state.clone(), id, &server).await?;
            } else {
                return Err(err.into());
            }
        }
    }

    state.docker.start_container(&container_name, None).await?;

    let poll_state = state.clone();
    let server_uuid = id;

    // Create a cancellation token for this watcher
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    // Store the cancellation token in the watchers map
    state
        .watchers
        .write()
        .await
        .insert(server_uuid, cancel_token);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        let mut file_position: u64 = 0;

        loop {
            tokio::select! {
                _ = cancel_token_clone.cancelled() => {
                    // Cancellation token was triggered, exit the watcher loop
                    println!("Watcher for server {} cancelled", server_uuid);
                    break;
                }
                _ = interval.tick() => {
                    // Get the logs directory for this server
                    let logs_dir = match get_log_path(&server_uuid).await {
                        Ok(path) => path,
                        Err(err) => {
                            eprintln!("Failed to get log path: {:?}", err);
                            continue;
                        }
                    };

                    // Find the latest log file in the directory
                    let latest_log_file = match find_latest_log_file(&logs_dir).await {
                        Ok(Some(file)) => file,
                        Ok(None) => {
                            // No log file yet, continue polling
                            continue;
                        }
                        Err(err) => {
                            eprintln!("Failed to find latest log file: {:?}", err);
                            continue;
                        }
                    };

                    // Check the file for new players
                    match check_path(&latest_log_file, &poll_state, file_position).await {
                        Ok(new_pos) => {
                            file_position = new_pos;
                        }
                        Err(err) => {
                            eprintln!("Error checking log file: {:?}", err);
                        }
                    }
                }
            }
        }
    });

    pub async fn check_path(
        path: &PathBuf,
        state: &Arc<AppState>,
        pos: u64,
    ) -> Result<u64, anyhow::Error> {
        let file = File::open(path).await?;

        let mut buf = BufReader::new(file);

        buf.seek(SeekFrom::Start(pos)).await?;

        let mut new_players: HashMap<Uuid, String> = HashMap::new();
        let mut line = String::new();
        while let Ok(size) = buf.read_line(&mut line).await {
            if size == 0 {
                break;
            }
            let Some(matches) = LOG_UUID_PLAYER_REGEX.captures(&line) else {
                continue;
            };

            let Some(uuid) = matches
                .name("uuid")
                .and_then(|x| Uuid::parse_str(x.as_str()).ok())
            else {
                continue;
            };

            let Some(name) = matches
                .name("name")
                .and_then(|x| Some(x.as_str().to_string()))
            else {
                continue;
            };

            new_players.insert(uuid, name);
        }

        let pos = buf.stream_position().await?;

        let new_players = new_players.into_iter().map(|x| player::ActiveModel {
            uuid: Set(x.0),
            name: Set(x.1),
        });

        models::player::Entity::insert_many(new_players)
            .on_conflict(
                sea_query::OnConflict::column(player::Column::Uuid)
                    .update_column(player::Column::Uuid)
                    .to_owned(),
            )
            .exec(&state.db)
            .await?;

        Ok(pos)
    }

    let is_running = update_is_running(&state, server).await?;

    Ok(AppJson(SuccessResponse {
        success: is_running,
    }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/stop",
    params(IdParams),
    responses(
        (status = OK, description = "Server was stopped", body = SuccessResponse),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]

pub async fn stop_server(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let server = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    // Cancel the watcher for this server
    if let Some(cancel_token) = state.watchers.write().await.remove(&id) {
        cancel_token.cancel();
    }

    let str_uuid = id.to_string();
    let stop_response = state.docker.stop_container(&str_uuid, None).await;

    if let Err(err) = stop_response {
        if let bollard::errors::Error::DockerResponseServerError {
            status_code,
            message: _,
        } = &err
            && *status_code == 404
        {
            update_is_running_db(&state, server, false).await?;
            return Ok(AppJson(SuccessResponse { success: true }));
        } else {
            return Err(err.into());
        }
    }

    update_is_running(&state, server).await?;

    Ok(AppJson(SuccessResponse { success: true }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}/is-running",
    params(IdParams),
    responses(
        (status = OK, description = "Server running status", body = SuccessResponse),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]
pub async fn is_server_running(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let server = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    let is_running = update_is_running(&state, server).await?;

    Ok(AppJson(SuccessResponse {
        success: is_running,
    }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/{id}",
    params(IdParams),
    responses(
        (status = OK, description = "The server model", body = models::server::Model),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]
pub async fn get_server(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<models::server::Model>, ArsaError> {
    let server = models::server::Entity::find_by_id(id)
        .one(&state.db)
        .await?;
    server.map_or(Err(ArsaError::NotFound), |x| Ok(AppJson(x)))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "",
    responses(
        (status = OK, description = "List of all servers", body = Vec<models::server::Model>)
    )
)]
pub async fn get_servers(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<Vec<models::server::Model>>, ArsaError> {
    send_message(
        &state,
        &ServerStatusUpdates::Message {
            message: "Get Servers called".to_string(),
        },
    )?;
    Ok(AppJson(
        models::server::Entity::find().all(&state.db).await?,
    ))
}

#[derive(ToSchema, Serialize)]
pub struct UuidResponse {
    // Id
    uuid: Uuid,
}

#[utoipa::path(
    post,
    tag = "arsa",
    path = "/server",
    request_body(
        description = "Server configuration to create a new server",
        content = inline(models::server::Model),
        example = json!({
            "name": "My Arma Server",
            "isRunning": false,
            "branch": "stable",
            "config": {
                "bindAddress": "0.0.0.0",
                "bindPort": 2302,
                "publicAddress": "1.2.3.4",
                "publicPort": 2302,
                "a2s": {
                    "address": "0.0.0.0",
                    "port": 2303
                },
                "rcon": {
                    "address": "0.0.0.0",
                    "port": 2305,
                    "password": "admin123",
                    "maxClients": 5,
                    "permission": "admin",
                    "blacklist": [],
                    "whitelist": []
                },
                "game": {
                    "name": "Test Server",
                    "password": "password",
                    "passwordAdmin": "adminPassword",
                    "admins": ["admin1", "admin2"],
                    "scenarioId": "veh_rc_enoch",
                    "maxPlayers": 64,
                    "visible": true,
                    "crossPlatform": true,
                    "supportedPlatforms": ["PLATFORM_PC"],
                    "gameProperties": {
                        "serverMaxViewDistance": 10000,
                        "serverMinGrassDistance": 50,
                        "fastValidation": true,
                        "networkViewDistance": 2000,
                        "battlEye": true,
                        "disableThirdPerson": false,
                        "VONDisableUI": false,
                        "VONDisableDirectSpeechUI": false,
                        "VONCanTransmitCrossFaction": false,
                        "missionHeader": {}
                    },
                    "modsRequiredByDefault": false,
                    "mods": [
                        {
                            "modId": "mod1",
                            "name": "CBA_A3",
                            "version": "3.15.0",
                            "required": true
                        }
                    ]
                },
                "operating": {
                    "lobbyPlayerSynchronise": true,
                    "disableCrashReporter": false,
                    "disableNavmeshStreaming": [],
                    "disableServerShutdown": false,
                    "disableAI": false,
                    "playerSaveTime": 180,
                    "aiLimit": 5000,
                    "slotReservationTimeout": 60,
                    "joinQueue": {
                        "maxSize": 10
                    }
                }
            },
            "startupParameters": [
                {
                    "parameter": "port",
                    "tooltip": "Server port",
                    "enabled": true,
                    "type": "number",
                    "value": 2302
                }
            ]
        })
    ),
    responses(
        (status = OK, description = "Server was created successfully", body = UuidResponse),
        (status = BAD_REQUEST, description = "Invalid server configuration", body = ErrorResponse)
    )
)]
pub async fn post_server(
    State(state): State<Arc<AppState>>,
    AppJson(params): AppJson<models::server::Model>,
) -> Result<AppJson<UuidResponse>, ArsaError> {
    let uuid = models::server::ActiveModel {
        name: Set(params.name.to_owned()),
        is_running: Set(params.is_running.to_owned()),
        branch: Set(params.branch),
        uuid: Set(Uuid::new_v4()),
        config: Set(params.config),
        startup_parameters_wrapper: Set(params.startup_parameters_wrapper),
    }
    .insert(&state.db)
    .await?
    .uuid;

    create_dirs(uuid).await?;

    Ok(AppJson(UuidResponse { uuid }))
}

#[utoipa::path(
    put,
    path = "/server",
    tag = "arsa",
    request_body(
        description = "Server configuration to create a new server",
        content = inline(models::server::Model),
        example = json!({
            "name": "My Arma Server",
            "isRunning": false,
            "branch": "stable",
            "config": {
                "bindAddress": "0.0.0.0",
                "bindPort": 2302,
                "publicAddress": "1.2.3.4",
                "publicPort": 2302,
                "a2s": {
                    "address": "0.0.0.0",
                    "port": 2303
                },
                "rcon": {
                    "address": "0.0.0.0",
                    "port": 2305,
                    "password": "admin123",
                    "maxClients": 5,
                    "permission": "admin",
                    "blacklist": [],
                    "whitelist": []
                },
                "game": {
                    "name": "Test Server",
                    "password": "password",
                    "passwordAdmin": "adminPassword",
                    "admins": ["admin1", "admin2"],
                    "scenarioId": "veh_rc_enoch",
                    "maxPlayers": 64,
                    "visible": true,
                    "crossPlatform": true,
                    "supportedPlatforms": ["PLATFORM_PC"],
                    "gameProperties": {
                        "serverMaxViewDistance": 10000,
                        "serverMinGrassDistance": 50,
                        "fastValidation": true,
                        "networkViewDistance": 2000,
                        "battlEye": true,
                        "disableThirdPerson": false,
                        "VONDisableUI": false,
                        "VONDisableDirectSpeechUI": false,
                        "VONCanTransmitCrossFaction": false,
                        "missionHeader": {}
                    },
                    "modsRequiredByDefault": false,
                    "mods": [
                        {
                            "modId": "mod1",
                            "name": "CBA_A3",
                            "version": "3.15.0",
                            "required": true
                        }
                    ]
                },
                "operating": {
                    "lobbyPlayerSynchronise": true,
                    "disableCrashReporter": false,
                    "disableNavmeshStreaming": [],
                    "disableServerShutdown": false,
                    "disableAI": false,
                    "playerSaveTime": 180,
                    "aiLimit": 5000,
                    "slotReservationTimeout": 60,
                    "joinQueue": {
                        "maxSize": 10
                    }
                }
            },
            "startupParameters": [
                {
                    "parameter": "port",
                    "tooltip": "Server port",
                    "enabled": true,
                    "type": "number",
                    "value": 2302
                }
            ]
        })
    ),
    responses(
        (status = OK, description = "Server was created successfully", body = SuccessResponse),
        (status = BAD_REQUEST, description = "Invalid server configuration", body = ErrorResponse)
    )
)]
pub async fn put_server(
    State(state): State<Arc<AppState>>,
    AppJson(params): AppJson<models::server::Model>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let server = models::server::Entity::find_by_id(params.uuid)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    let mut server: models::server::ActiveModel = server.into();

    server.name = Set(params.name);
    server.is_running = Set(params.is_running);
    server.config = Set(params.config);
    server.branch = Set(params.branch);
    server.startup_parameters_wrapper = Set(params.startup_parameters_wrapper);

    server.update(&state.db).await?.uuid;

    create_dirs(params.uuid).await?;

    Ok(AppJson(SuccessResponse { success: true }))
}

#[utoipa::path(
    delete,
    tag = "arsa",
    path = "/{id}",
    params(IdParams),
    responses(
        (status = OK, description = "Server was deleted", body = SuccessResponse),
        (status = NOT_FOUND, description = "Server was not found", body = ErrorResponse)
    )
)]
pub async fn delete_server(
    State(state): State<Arc<AppState>>,
    Path(IdParams { id }): Path<IdParams>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let delete_result = models::server::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    if delete_result.rows_affected == 0 {
        return Err(ArsaError::NotFound);
    }

    Ok(AppJson(SuccessResponse { success: true }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/public-ip",
    responses((status = OK, description = "The public IPv4 for this server", body = IPv4Response))
)]
pub async fn get_public_ip(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<IPv4Response>, ArsaError> {
    Ok(AppJson(IPv4Response {
        ipv4: state.ip.to_string(),
    }))
}

fn get_image_branch_as_string(branch: &Branch) -> String {
    serde_json::to_string(branch)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn get_image_name(branch: &Branch) -> String {
    let base_name = "thewillard/arsa-test";

    format!("{}:{}", base_name, &get_image_branch_as_string(branch))
}

const VERSION_LABEL: &str = "de.grad.arsa.version";

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/image-version/{branch}",
    params(BranchParams),
    responses((status = OK, body = ImageVersionResponse))
)]
pub async fn get_image_version(
    State(state): State<Arc<AppState>>,
    Path(branch): Path<BranchParams>,
) -> Result<AppJson<ImageVersionResponse>, ArsaError> {
    let inspect_result = state
        .docker
        .inspect_image(&get_image_name(&branch.branch))
        .await;

    let image_inspect = match inspect_result {
        Ok(result) => result,
        Err(err) => {
            if let bollard::errors::Error::DockerResponseServerError {
                status_code,
                message: _,
            } = &err
                && *status_code == 404
            {
                return Err(ArsaError::NotFound);
            } else {
                return Err(err.into());
            }
        }
    };

    match image_inspect
        .config
        .and_then(|x| x.labels)
        .and_then(|x| x.get(VERSION_LABEL).cloned())
    {
        Some(version) => Ok(AppJson(ImageVersionResponse {
            version: version.to_owned(),
        })),
        None => Err(ArsaError::NotFound),
    }
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/pull-image/{branch}",
    params(BranchParams),
    responses((status = OK, body = SuccessResponse))
)]
pub async fn get_pull_image(
    State(state): State<Arc<AppState>>,
    Path(branch): Path<BranchParams>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    use bollard::query_parameters::ListImagesOptionsBuilder;
    let mut filters = HashMap::new();
    filters.insert(
        "reference".to_string(),
        vec!["thewillard/arsa-test".to_string()],
    );

    let _s = state.docker.list_images(Some(
        ListImagesOptionsBuilder::new().filters(&filters).build(),
    ));

    pull_image(
        &state,
        &serde_json::to_string(&branch.branch)
            .unwrap_or_default()
            .trim_matches('"'),
    )
    .await;

    Ok(AppJson(SuccessResponse { success: true }))
}

#[utoipa::path(
    get,
    tag = "arsa",
    path = "/status",
    responses((status = OK, body = crate::models::server::ArsStatus))
)]
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<crate::models::server::ArsStatus>, ArsaError> {
    send_message(
        &state,
        &ServerStatusUpdates::Message {
            message: "Status send".to_string(),
        },
    )?;

    Ok(AppJson(state.status.lock().await.clone()))
}

// Regex patterns
pub static LOG_DIR_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("^logs_([0-9]{4})-([0-9]{2})-([0-9]{2})_([0-9]{2})-([0-9]{2})-([0-9]{2})$").unwrap()
});

pub static LOG_UUID_PLAYER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("identityId=(?P<uuid>[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}) name=(?P<name>.*)").unwrap()
});

// Helper functions
pub fn send_message(
    state: &Arc<AppState>,
    status_update: &ServerStatusUpdates,
) -> Result<(), ArsaError> {
    let _ = state
        .channel
        .send(serde_json::to_string(&status_update)?)
        .expect("Couldn't send");
    Ok(())
}

pub async fn update_is_running(
    state: &Arc<AppState>,
    server: models::server::Model,
) -> Result<bool, ArsaError> {
    let is_running = is_server_container_running(state, server.uuid).await?;

    if server.is_running == is_running {
        return Ok(is_running);
    }

    let uuid = server.uuid;

    update_is_running_db(state, server, is_running).await?;

    send_message(
        state,
        &ServerStatusUpdates::IsRunningUpdate {
            uuid: uuid.to_string(),
            is_running,
        },
    )?;

    Ok(is_running)
}

async fn update_is_running_db(
    state: &Arc<AppState>,
    server: models::server::Model,
    is_running: bool,
) -> Result<(), ArsaError> {
    let mut server: models::server::ActiveModel = server.into();
    server.is_running = Set(is_running);
    server.update(&state.db).await?;
    Ok(())
}

pub async fn is_server_container_running(
    state: &Arc<AppState>,
    uuid: Uuid,
) -> Result<bool, ArsaError> {
    let str_uuid = uuid.to_string();
    let inspect_response = state
        .docker
        .inspect_container(
            &str_uuid,
            Some(InspectContainerOptionsBuilder::new().size(true).build()),
        )
        .await?;
    let is_running = inspect_response
        .state
        .and_then(|x| x.running)
        .unwrap_or_default();
    Ok(is_running)
}

pub async fn get_names_in_dir(path: &PathBuf) -> Result<Vec<String>, ArsaError> {
    let mut read_dir = fs::read_dir(path).await?;

    let mut names = vec![];
    while let Some(entry) = read_dir.next_entry().await? {
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    Ok(names)
}

pub async fn find_latest_log_file(logs_dir: &PathBuf) -> Result<Option<PathBuf>, ArsaError> {
    if !logs_dir.exists() {
        return Ok(None);
    }

    let mut read_dir = fs::read_dir(logs_dir).await?;
    let mut latest_file: Option<PathBuf> = None;
    let mut latest_time = std::time::SystemTime::UNIX_EPOCH;

    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();

        // Only consider directories matching the log directory pattern
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or_default();

        if !LOG_DIR_NAME_REGEX.is_match(&dir_name) {
            continue;
        }

        // Get the modification time to find the latest directory
        if let Ok(metadata) = fs::metadata(&path).await {
            if let Ok(modified) = metadata.modified() {
                if modified > latest_time {
                    latest_time = modified;

                    // Look for console.log in this directory
                    let console_log = path.join("console.log");
                    if console_log.exists() {
                        latest_file = Some(console_log);
                    }
                }
            }
        }
    }

    Ok(latest_file)
}

pub async fn create_server_container(
    state: Arc<AppState>,
    uuid: Uuid,
    server: &models::server::Model,
) -> Result<(), ArsaError> {
    let container_name = uuid.to_string();
    let network = "bridge".to_string();

    let config = &server.config;
    let bind_port = config.bind_port.to_string();
    let a2s_port = config.a2s.port.to_string();
    let rcon_port = config.rcon.port.to_string();

    create_dirs(uuid).await?;

    fs::write(
        get_config_json_path(&uuid).await?,
        serde_json::to_string(&config).unwrap(),
    )
    .await?;

    let mut port_bindings = HashMap::new();
    port_bindings.insert(
        format!("{}/udp", bind_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(bind_port),
        }]),
    );
    port_bindings.insert(
        format!("{}/udp", a2s_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(a2s_port),
        }]),
    );
    port_bindings.insert(
        format!("{}/udp", rcon_port),
        Some(vec![PortBinding {
            host_ip: None,
            host_port: Some(rcon_port),
        }]),
    );

    let local_profiles = get_profiles_path(&uuid).await?;
    let local_config = get_config_path(&uuid).await?;

    let mounts = vec![
        Mount {
            target: Some("/ars/arsa/config".to_string()),
            source: Some(local_config.into_os_string().into_string().unwrap()),
            typ: Some(MountTypeEnum::BIND),
            read_only: Some(true),
            ..Default::default()
        },
        Mount {
            target: Some("/ars/arsa/profiles".to_string()),
            source: Some(local_profiles.into_os_string().into_string().unwrap()),
            typ: Some(MountTypeEnum::BIND),
            ..Default::default()
        },
    ];

    let host_config = HostConfig {
        mounts: Some(mounts),
        port_bindings: Some(port_bindings),
        network_mode: Some("bridge".to_string()),
        ..Default::default()
    };

    let mut endpoints_config = HashMap::new();
    endpoints_config.insert(network, EndpointSettings::default());

    let s = NetworkingConfig {
        endpoints_config: Some(endpoints_config),
    };

    let options = CreateContainerOptionsBuilder::default()
        .name(&container_name)
        .build();

    let mut args = vec![
        "-config".to_string(),
        "/ars/arsa/config/config.json".to_string(),
        "-profile".to_string(),
        "/ars/arsa/profiles/".to_string(),
    ];

    for param in &server.startup_parameters_wrapper.startup_parameters {
        if !param.enabled {
            continue;
        }
        args.push(format!("-{}", param.parameter));
        if let Some(arg_value) = &param.value {
            args.push(arg_value.to_string());
        }
    }

    let config = ContainerCreateBody {
        image: Some("thewillard/arsa-test:1.6.0.121".to_string()),
        cmd: Some(args),
        exposed_ports: Some(vec!["17777/udp".to_string()]),
        hostname: Some(container_name.to_string()),
        host_config: Some(host_config),
        networking_config: Some(s),
        ..Default::default()
    };
    let _ = state.docker.create_container(Some(options), config).await?;

    Ok(())
}

async fn create_dirs(uuid: Uuid) -> Result<(), ArsaError> {
    fs::create_dir_all(get_config_path(&uuid).await?).await?;
    fs::create_dir_all(get_profiles_path(&uuid).await?).await?;
    Ok(())
}

pub async fn pull_image(state: &Arc<AppState>, branch: &str) {
    if branch.is_empty() {
        return;
    }

    let image_name = format!("thewillard/arsa-test:{branch}");

    dbg!(&image_name);

    let mut create = state.docker.create_image(
        Some(CreateImageOptions {
            from_image: Some(image_name.to_owned()),
            ..Default::default()
        }),
        None,
        None,
    );

    let _ = status_update(state, models::server::ArsStatus::Recreating).await;

    let state = state.clone();
    tokio::spawn(async move {
        while let Some(msg) = create.next().await {
            let msg = match msg {
                Ok(create_image_info) => ServerStatusUpdates::CreateImageProgress {
                    info: create_image_info,
                },
                Err(err) => ServerStatusUpdates::Error {
                    error: err.to_string(),
                },
            };
            let _ = send_message(&state, &msg);
        }
        let _ = status_update(&state, models::server::ArsStatus::Available).await;
    });
}

pub async fn status_update(
    state: &Arc<AppState>,
    status: models::server::ArsStatus,
) -> Result<(), ArsaError> {
    let mut status_lock = state.status.lock().await;
    *status_lock = status.clone();

    send_message(
        state,
        &ServerStatusUpdates::ArsStatusUpdate { ars_status: status },
    )?;

    Ok(())
}
