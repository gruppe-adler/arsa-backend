use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
    time::Duration,
};

use axum::{
    Extension,
    extract::{Path, State},
    http::StatusCode,
};
use bollard::{
    container::LogOutput,
    query_parameters::{
        CreateContainerOptionsBuilder, CreateImageOptions, ListContainersOptionsBuilder,
        LogsOptions, RemoveContainerOptionsBuilder, RemoveImageOptionsBuilder,
        StopContainerOptionsBuilder,
    },
    secret::{ContainerCreateBody, CreateImageInfo, HostConfig},
};
use futures::StreamExt;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, TransactionTrait,
    sea_query::OnConflict,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use crate::{
    AppState,
    config::AppConfig,
    endpoints::{
        auth::Claims,
        server::{
            Branch, BranchParams, LOG_SCENARIOS_COUNT, LOG_SCENARIOS_MISSION,
            ListScenariosResponse, ScenarioEntry, get_image_branch_as_string, get_image_name,
            send_message,
        },
    },
    models::{
        self,
        log::LogAction,
        responses::{ErrorResponse, ImageVersionResponse, ServerStatusUpdates, SuccessResponse},
        server::ArsStatus,
    },
    shared::{AppJson, ArsaError, harden_ars_host_config, log_action},
};

pub async fn pull_image(state: &Arc<AppState>, branch: &Branch, actor: String) {
    let image_name = get_image_name(branch);

    println!("Pulling: {}", image_name);

    let mut create = state.docker.create_image(
        Some(CreateImageOptions {
            from_image: Some(image_name.to_owned()),
            ..Default::default()
        }),
        None,
        None,
    );

    if let Err(err) = status_update(state, models::server::ArsStatus::Recreating).await {
        println!("{:?}", err);
    }

    let _ = log_action(state, LogAction::ImagePullStarted, None, Some(actor)).await;

    let pull_id = Uuid::new_v4();

    let state = state.clone();
    tokio::spawn(async move {
        while let Some(msg) = create.next().await {
            if let Err(err) = send_pull_message(&state, &pull_id, msg).await {
                println!("{:?}", err);
            }
        }

        if let Err(err) = send_message(
            &state,
            &ServerStatusUpdates::CreateImageFinished {
                pull_id: pull_id.to_string(),
            },
        ) {
            println!("{:?}", err);
        }

        if let Err(err) = status_update(&state, models::server::ArsStatus::Available).await {
            println!("{:?}", err);
        }

        if let Err(err) = models::pull_log::Entity::delete_many()
            .filter(models::pull_log::Column::PullId.eq(pull_id))
            .exec(&state.db)
            .await
        {
            println!("{:?}", err);
        }
    });
}

pub async fn send_pull_message(
    state: &Arc<AppState>,
    pull_id: &Uuid,
    msg: Result<CreateImageInfo, bollard::errors::Error>,
) -> Result<(), ArsaError> {
    let pull_log = match msg {
        Ok(info) => {
            let mut id = info.id.unwrap_or_default();
            if id.is_empty() {
                id = Uuid::new_v4().to_string();
            }
            models::pull_log::Model {
                id,
                pull_id: *pull_id,
                error_detail_code: info
                    .error_detail
                    .as_ref()
                    .and_then(|x| x.code)
                    .unwrap_or_default(),
                error_detail_message: info
                    .error_detail
                    .and_then(|x| x.message)
                    .unwrap_or_default(),
                status: info.status.unwrap_or_default(),
                progress_detail_current: info
                    .progress_detail
                    .as_ref()
                    .and_then(|x| x.current)
                    .unwrap_or_default(),
                progress_detail_total: info
                    .progress_detail
                    .and_then(|x| x.total)
                    .unwrap_or_default(),
            }
        }
        Err(err) => models::pull_log::Model {
            id: Uuid::new_v4().to_string(),
            pull_id: *pull_id,
            error_detail_message: err.to_string(),
            ..Default::default()
        },
    };

    let _ = models::pull_log::Entity::insert(pull_log.clone().into_active_model())
        .on_conflict(
            OnConflict::column(models::pull_log::Column::Id)
                .update_columns([
                    models::pull_log::Column::PullId,
                    models::pull_log::Column::Status,
                    models::pull_log::Column::ProgressDetailCurrent,
                    models::pull_log::Column::ProgressDetailTotal,
                    models::pull_log::Column::ErrorDetailCode,
                    models::pull_log::Column::ErrorDetailMessage,
                ])
                .to_owned(),
        )
        .exec(&state.db)
        .await?;

    send_message(
        state,
        &ServerStatusUpdates::CreateImageProgress { info: pull_log },
    )?;

    Ok(())
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

#[utoipa::path(
    get,
    tag = "image",
    path = "/pull/logs",
    responses(
        (status = OK, description = "Pull logs", body = Vec<models::pull_log::Model>),
    )
)]
pub async fn get_pull_logs(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<AppJson<Vec<models::pull_log::Model>>, ArsaError> {
    if !claims.has_role(&AppConfig::get().admin_role) {
        return Err(ArsaError::Unauthorized);
    }

    let logs = models::pull_log::Entity::find().all(&state.db).await?;

    Ok(AppJson(logs))
}

const VERSION_LABEL: &str = "de.grad.arsa.version";

#[utoipa::path(
    get,
    tag = "image",
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
                return Ok(AppJson(ImageVersionResponse {
                    version: "n/a".to_owned(),
                }));
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
        None => Ok(AppJson(ImageVersionResponse {
            version: "n/a".to_owned(),
        })),
    }
}

#[utoipa::path(
    put,
    tag = "image",
    path = "/scenarios/{branch}",
    params(BranchParams),
    responses(
        (status = ACCEPTED, description = "Scenario update started. The result is sent over the websocket as a `scenariosUpdateFinished` message.", body = SuccessResponse),
        (status = NOT_FOUND, description = "Image not found", body = ErrorResponse),
        (status = CONFLICT, description = "An update for this branch is already running", body = ErrorResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to inspect image", body = ErrorResponse),
    )
)]
pub async fn update_scenarios_from_branch(
    State(state): State<Arc<AppState>>,
    Path(branch): Path<BranchParams>,
    Extension(claims): Extension<Claims>,
) -> Result<(StatusCode, AppJson<SuccessResponse>), ArsaError> {
    if !claims.has_role(&AppConfig::get().admin_role) {
        return Err(ArsaError::Unauthorized);
    }

    let branch = branch.branch;
    let image_name = get_image_name(&branch);
    if let Err(err) = state.docker.inspect_image(&image_name).await {
        if let bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        } = &err
        {
            return Err(ArsaError::NotFound);
        }
        return Err(err.into());
    }

    let guard = ScenarioUpdateGuard::acquire(branch).ok_or(ArsaError::Conflict)?;

    /*
        Listing takes ~16s, so it runs detached instead of inside the request.
        Browsers don't keep a silent request open that long: creating the container adds a
        network interface, and Firefox drops connections that stay quiet for 5s after a network
        change. That cancelled the handler halfway through, leaving the container behind and
        the list unchanged.
    */
    tokio::spawn(async move {
        let _guard = guard;
        let message = match update_scenarios(&state, branch, image_name).await {
            Ok(count) => ServerStatusUpdates::ScenariosUpdateFinished {
                branch,
                success: true,
                count,
                error: None,
            },
            Err(err) => {
                println!("Scenario update failed: {:?}", err);
                ServerStatusUpdates::ScenariosUpdateFinished {
                    branch,
                    success: false,
                    count: 0,
                    error: Some(match err {
                        ArsaError::UnknownError(message) => message,
                        other => format!("{:?}", other),
                    }),
                }
            }
        };
        if let Err(err) = send_message(&state, &message) {
            println!("{:?}", err);
        }
    });

    Ok((StatusCode::ACCEPTED, AppJson(SuccessResponse { success: true })))
}

/// Branches with a scenario update in progress, so two updates of one branch can't race.
static SCENARIO_UPDATES_RUNNING: LazyLock<std::sync::Mutex<Vec<Branch>>> =
    LazyLock::new(Default::default);

struct ScenarioUpdateGuard(Branch);

impl ScenarioUpdateGuard {
    fn acquire(branch: Branch) -> Option<Self> {
        let mut running = SCENARIO_UPDATES_RUNNING
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        if running.contains(&branch) {
            return None;
        }
        running.push(branch);
        Some(Self(branch))
    }
}

impl Drop for ScenarioUpdateGuard {
    fn drop(&mut self) {
        SCENARIO_UPDATES_RUNNING
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .retain(|branch| *branch != self.0);
    }
}

/// Lists the image's scenarios and replaces the stored list for `branch`.
/// Returns the number of scenarios stored.
async fn update_scenarios(
    state: &Arc<AppState>,
    branch: Branch,
    image_name: String,
) -> Result<u32, ArsaError> {
    let container_name = format!(
        "list-scenarios-{}-{}",
        get_image_branch_as_string(&branch),
        Uuid::new_v4()
    );

    let create_options = CreateContainerOptionsBuilder::default()
        .name(&container_name)
        .build();

    let config = ContainerCreateBody {
        image: Some(image_name),
        // Without -profile the server uses $HOME, which isn't writable for ARSA_ARS_USER in older images.
        cmd: Some(vec![
            "-listScenarios".to_string(),
            "-profile".to_string(),
            "/tmp/profile".to_string(),
        ]),
        user: Some(AppConfig::get().ars_user.clone()),
        host_config: Some(harden_ars_host_config(HostConfig {
            network_mode: Some("bridge".to_string()),
            ..Default::default()
        })),
        ..Default::default()
    };

    state
        .docker
        .create_container(Some(create_options), config)
        .await?;

    let listed = list_scenarios(state, &container_name).await;

    let _ = state
        .docker
        .remove_container(
            &container_name,
            Some(RemoveContainerOptionsBuilder::new().force(true).build()),
        )
        .await;

    let (entry_count, scenarios) = listed?;

    // Don't replace the stored list with a partial one.
    if scenarios.is_empty() || (scenarios.len() as u32) < entry_count {
        return Err(ArsaError::UnknownError(format!(
            "Only parsed {} of {} scenarios from the server output, keeping the existing list",
            scenarios.len(),
            entry_count
        )));
    }

    let count = scenarios.len() as u32;
    let scenarios = scenarios
        .into_iter()
        .map(|(path, name)| models::scenarios::ActiveModel {
            branch: Set(branch),
            path: Set(path),
            name: Set(name),
        });

    let txn = state.db.begin().await?;
    models::scenarios::Entity::delete_many()
        .filter(models::scenarios::Column::Branch.eq(branch))
        .exec(&txn)
        .await?;
    models::scenarios::Entity::insert_many(scenarios)
        .exec(&txn)
        .await?;
    txn.commit().await?;

    Ok(count)
}

/// Starts the container and parses its `-listScenarios` output.
/// Returns the announced scenario count and the parsed `(path, name)` pairs.
async fn list_scenarios(
    state: &Arc<AppState>,
    container_name: &str,
) -> Result<(u32, Vec<(String, String)>), ArsaError> {
    state.docker.start_container(container_name, None).await?;

    let mut logs = state.docker.logs(
        container_name,
        Some(LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            timestamps: false,
            tail: "all".to_string(),
            ..Default::default()
        }),
    );

    /*
        Without a TTY the server block-buffers stdout (8 KB), so the scenario list stalls
        partway and the rest only appears when the process exits.
        So we gracefully stop the server with a 'SIGINT' after 15s, which flushes the remaining scenarios to the log.
    */
    let stop_state = state.clone();
    let stop_container_name = container_name.to_string();
    let stopper = tokio::spawn(async move {
        sleep(Duration::from_secs(15)).await;
        let _ = stop_state
            .docker
            .stop_container(
                &stop_container_name,
                Some(StopContainerOptionsBuilder::new().signal("SIGINT").build()),
            )
            .await;
    });

    let mut entry_count = 0_u32;
    let mut scenarios: Vec<(String, String)> = Vec::new();

    let _ = timeout(Duration::from_secs(30), async {
        while let Some(chunk) = logs.next().await {
            let Ok(LogOutput::StdOut { message }) = chunk else {
                continue;
            };
            let line = String::from_utf8_lossy(&message);

            if entry_count == 0
                && let Some(count) = LOG_SCENARIOS_COUNT
                    .captures(&line)
                    .and_then(|matches| matches.name("MissionCount"))
                    .and_then(|count| count.as_str().parse::<u32>().ok())
            {
                entry_count = count;
            }

            if let Some(matches) = LOG_SCENARIOS_MISSION.captures(&line)
                && let (Some(path), Some(name)) = (matches.name("Path"), matches.name("Name"))
                && !scenarios.iter().any(|(known, _)| known == path.as_str())
            {
                scenarios.push((path.as_str().to_string(), name.as_str().to_string()));
            }

            if entry_count > 0 && scenarios.len() as u32 >= entry_count {
                break;
            }
        }
    })
    .await;

    stopper.abort();

    Ok((entry_count, scenarios))
}

#[utoipa::path(
    get,
    tag = "image",
    path = "/scenarios/{branch}",
    params(BranchParams),
    responses(
        (status = OK, description = "Scenarios list returned successfully", body = ListScenariosResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to get scenarios", body = ErrorResponse),
    )
)]
pub async fn get_scenarios_from_branch(
    State(state): State<Arc<AppState>>,
    Path(branch): Path<BranchParams>,
) -> Result<AppJson<ListScenariosResponse>, ArsaError> {
    let scenarios = models::scenarios::Entity::find()
        .filter(models::scenarios::Column::Branch.eq(branch.branch))
        .all(&state.db)
        .await?
        .into_iter()
        .map(|x| ScenarioEntry {
            name: x.name,
            path: x.path,
        })
        .collect();

    Ok(AppJson(ListScenariosResponse {
        branch: branch.branch,
        scenarios,
    }))
}

#[utoipa::path(
    get,
    tag = "image",
    path = "/pull/image/{branch}",
    params(BranchParams),
    responses((status = OK, body = SuccessResponse))
)]
pub async fn get_pull_image(
    State(state): State<Arc<AppState>>,
    Path(branch): Path<BranchParams>,
    Extension(claims): Extension<Claims>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    if !claims.has_role(&AppConfig::get().admin_role) {
        return Err(ArsaError::Unauthorized);
    }

    if *(state.status.lock().await) == ArsStatus::Recreating {
        return Err(ArsaError::BadRequest);
    }

    let image_name = get_image_name(&branch.branch);

    let inspect_result = state
        .docker
        .inspect_image(&get_image_name(&branch.branch))
        .await;

    if let Ok(inspect_result) = inspect_result {
        let mut filters: HashMap<String, Vec<String>> = HashMap::new();
        filters.insert("ancestor".to_string(), vec![image_name.clone()]);

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
            let container_name = container
                .names
                .unwrap_or_default()
                .first()
                .cloned()
                .unwrap_or_default();
            let container_name = &container_name.trim_matches('/');
            state.docker.stop_container(container_name, None).await?;

            state
                .docker
                .remove_container(
                    container_name,
                    Some(RemoveContainerOptionsBuilder::new().force(true).build()),
                )
                .await?;
        }

        if let Some(image_id) = inspect_result.id {
            state
                .docker
                .remove_image(
                    &image_id,
                    Some(RemoveImageOptionsBuilder::new().force(true).build()),
                    None,
                )
                .await?;
        }
    }

    pull_image(&state, &branch.branch, claims.get_user()).await;

    Ok(AppJson(SuccessResponse { success: true }))
}

#[utoipa::path(
    get,
    tag = "image",
    path = "/status",
    responses((status = OK, body = crate::models::server::ArsStatus))
)]
pub async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<crate::models::server::ArsStatus>, ArsaError> {
    Ok(AppJson(state.status.lock().await.clone()))
}
