use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::extract::{Path, State};
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
    ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, sea_query::OnConflict,
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use crate::{
    AppState,
    endpoints::server::{
        Branch, BranchParams, LOG_SCENARIOS_COUNT, LOG_SCENARIOS_MISSION, ListScenariosResponse,
        ScenarioEntry, get_image_branch_as_string, get_image_name, send_message,
    },
    models::{
        self,
        log::LogAction,
        responses::{ErrorResponse, ImageVersionResponse, ServerStatusUpdates, SuccessResponse},
        server::ArsStatus,
    },
    shared::{AppJson, ArsaError, log_action},
};

pub async fn pull_image(state: &Arc<AppState>, branch: &Branch) {
    let image_name = get_image_name(branch);
    dbg!(&image_name);

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

    let _ = log_action(state, LogAction::ImagePullStarted, None).await;

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
            .filter(models::pull_log::Column::PullId.contains(pull_id))
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
) -> Result<AppJson<Vec<models::pull_log::Model>>, ArsaError> {
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
    put,
    tag = "image",
    path = "/scenarios/{branch}",
    params(BranchParams),
    responses(
        (status = OK, description = "Scenarios list updated", body = SuccessResponse),
        (status = NOT_FOUND, description = "Image not found", body = ErrorResponse),
        (status = INTERNAL_SERVER_ERROR, description = "Failed to start image or parse scenarios", body = ErrorResponse),
    )
)]
pub async fn update_scenarios_from_branch(
    State(state): State<Arc<AppState>>,
    Path(branch): Path<BranchParams>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    let image_name = get_image_name(&branch.branch);
    let inspect_result = state.docker.inspect_image(&image_name).await;
    let _ = match inspect_result {
        Ok(value) => value,
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

    let container_name = format!(
        "list-scenarios-{}-{}",
        get_image_branch_as_string(&branch.branch),
        Uuid::new_v4()
    );

    let create_options = CreateContainerOptionsBuilder::default()
        .name(&container_name)
        .build();

    let config = ContainerCreateBody {
        image: Some(image_name.clone()),
        cmd: Some(vec!["-listScenarios".to_string()]),
        host_config: Some(HostConfig {
            network_mode: Some("bridge".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };

    state
        .docker
        .create_container(Some(create_options), config)
        .await?;
    state.docker.start_container(&container_name, None).await?;

    let mut logs = state.docker.logs(
        &container_name,
        Some(LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            timestamps: false,
            tail: "all".to_string(),
            ..Default::default()
        }),
    );

    let mut entry_count = 0_u32;
    let mut scenarios = Vec::new();

    /*
        When you start the server without a valid config.json,
        the server throws an error while printing out the scenarios and the logs get 'stuck' (?).
        So we gracefully stop the server with a 'SIGINT' after 15s which then prints the remaining scenarios to the log.
    */

    let state_clone = state.clone();
    let container_name_clone = container_name.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(15)).await;
        let _ = state_clone
            .docker
            .stop_container(
                &container_name_clone,
                Some(StopContainerOptionsBuilder::new().signal("SIGINT").build()),
            )
            .await;
    });

    let _ = timeout(Duration::from_secs(30), async {
        loop {
            tokio::task::yield_now().await;
            if entry_count > 0 && entry_count == scenarios.len() as u32 {
                break;
            }

            if let Some(chunk) = logs.next().await
                && let Ok(log_chunk) = chunk
                && let LogOutput::StdOut { message } = log_chunk
            {
                let line = String::from_utf8_lossy(&message);
                if entry_count == 0
                    && let Some(matches) = LOG_SCENARIOS_COUNT.captures(&line)
                    && let Some(count) = matches
                        .name("MissionCount")
                        .and_then(|x| x.as_str().parse::<u32>().ok())
                {
                    entry_count = count;
                }

                if let Some(matches) = LOG_SCENARIOS_MISSION.captures(&line)
                    && let Some(path) = matches.name("Path")
                    && let Some(name) = matches.name("Name")
                {
                    scenarios.push((path.as_str().to_string(), name.as_str().to_string()));
                }
            }
        }
    })
    .await;

    let _ = state
        .docker
        .remove_container(
            &container_name,
            Some(RemoveContainerOptionsBuilder::new().force(true).build()),
        )
        .await;

    let _ = models::scenarios::Entity::delete_many()
        .filter(models::scenarios::Column::Branch.eq(branch.branch))
        .exec(&state.db)
        .await?;

    let scenarios = scenarios.iter().map(|x| models::scenarios::ActiveModel {
        branch: Set(branch.branch),
        path: Set(x.0.clone()),
        name: Set(x.1.clone()),
    });

    models::scenarios::Entity::insert_many(scenarios)
        .exec(&state.db)
        .await?;

    Ok(AppJson(SuccessResponse { success: true }))
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
) -> Result<AppJson<SuccessResponse>, ArsaError> {
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

    pull_image(&state, &branch.branch).await;

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
