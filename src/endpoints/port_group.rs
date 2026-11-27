use axum::{
    Extension,
    extract::{Path, State},
};
use bollard::models::PortBinding;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    AppState,
    config::AppConfig,
    endpoints::auth::Claims,
    endpoints::settings::load_settings,
    models::{
        self,
        log::LogAction,
        port_group::{MAX_GAME_PORT, MIN_GAME_PORT, PortGroupRequest},
        responses::{
            ErrorResponse, PortConflict, PortConflictReason, PortConflictResponse, SuccessResponse,
        },
        server::ServerConfig,
    },
    shared::{AppJson, ArsaError, log_action},
};

/// Message for saving a server with custom ports while only port groups are allowed.
const PORT_GROUPS_ONLY_MESSAGE: &str =
    "Only port groups are allowed on this host. Pick a port group in the server's Basic settings.";

const MAX_NAME_LENGTH: usize = 50;

#[derive(IntoParams, Deserialize)]
pub struct PortGroupIdParams {
    /// Id of the port group
    #[param(example = 1)]
    pub id: i32,
}

/// The host ports a server binds: game, RCON and A2S.
pub fn server_ports(config: &ServerConfig) -> [u16; 3] {
    [config.bind_port, config.rcon.port, config.a2s.port]
}

/// The host ports the server's container binds, or `None` when there is no such container.
///
/// These can differ from the saved config: ports changed while a server runs only take effect
/// when it is started again.
async fn container_host_ports(state: &Arc<AppState>, uuid: Uuid) -> Option<Vec<u16>> {
    let inspect = state
        .docker
        .inspect_container(&uuid.to_string(), None)
        .await
        .ok()?;
    let ports = host_ports(inspect.host_config?.port_bindings?);
    (!ports.is_empty()).then_some(ports)
}

/// The host ports of a container's port bindings, sorted and without duplicates.
fn host_ports(port_bindings: HashMap<String, Option<Vec<PortBinding>>>) -> Vec<u16> {
    let mut ports: Vec<u16> = port_bindings
        .into_values()
        .flatten()
        .flatten()
        .filter_map(|binding| binding.host_port?.parse().ok())
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// Running servers other than `uuid`, with the host ports each one holds.
async fn running_servers_except(
    state: &Arc<AppState>,
    uuid: Uuid,
) -> Result<Vec<(models::server::Model, Vec<u16>)>, ArsaError> {
    let servers = models::server::Entity::find()
        .filter(models::server::Column::IsRunning.eq(true))
        .filter(models::server::Column::Uuid.ne(uuid))
        .all(&state.db)
        .await?;

    let mut running = Vec::with_capacity(servers.len());
    for server in servers {
        let ports = match container_host_ports(state, server.uuid).await {
            Some(ports) => ports,
            None => server_ports(&server.config).to_vec(),
        };
        running.push((server, ports));
    }
    Ok(running)
}

/// Port groups whose ports no running server (other than `uuid`) uses and that contain none of
/// the `taken` ports.
pub async fn free_port_groups(
    state: &Arc<AppState>,
    uuid: Uuid,
    taken: &[u16],
) -> Result<Vec<models::port_group::Model>, ArsaError> {
    let mut used: Vec<u16> = running_servers_except(state, uuid)
        .await?
        .into_iter()
        .flat_map(|(_, ports)| ports)
        .collect();
    used.extend_from_slice(taken);

    Ok(all_port_groups(state)
        .await?
        .into_iter()
        .filter(|group| group.ports().iter().all(|port| !used.contains(port)))
        .collect())
}

/// Running servers that share at least one port with `server`.
pub async fn find_port_conflicts(
    state: &Arc<AppState>,
    server: &models::server::Model,
) -> Result<Vec<PortConflict>, ArsaError> {
    let ports = server_ports(&server.config);

    Ok(running_servers_except(state, server.uuid)
        .await?
        .into_iter()
        .filter_map(|(other, other_ports)| {
            let mut shared: Vec<u16> = ports
                .iter()
                .copied()
                .filter(|port| other_ports.contains(port))
                .collect();
            shared.dedup();
            (!shared.is_empty()).then(|| PortConflict {
                uuid: other.uuid.to_string(),
                name: other.name,
                ports: shared,
            })
        })
        .collect())
}

/// Whether the game, RCON and A2S ports of the config are exactly those of a port group.
pub fn uses_port_group(config: &ServerConfig, groups: &[models::port_group::Model]) -> bool {
    groups
        .iter()
        .any(|group| server_ports(config) == group.ports())
}

/// False when only port groups are allowed and the config uses custom ports.
pub async fn ports_allowed(
    state: &Arc<AppState>,
    config: &ServerConfig,
) -> Result<bool, ArsaError> {
    if !load_settings(state).await?.port_groups_only {
        return Ok(true);
    }
    Ok(uses_port_group(config, &all_port_groups(state).await?))
}

/// Rejects saving a server with custom ports while only port groups are allowed.
pub async fn require_allowed_ports(
    state: &Arc<AppState>,
    config: &ServerConfig,
) -> Result<(), ArsaError> {
    if ports_allowed(state, config).await? {
        Ok(())
    } else {
        Err(ArsaError::BadRequestMessage(
            PORT_GROUPS_ONLY_MESSAGE.to_string(),
        ))
    }
}

/// Builds the 409 response for starting a server with custom ports while only port groups are allowed.
pub async fn not_a_port_group_error(
    state: &Arc<AppState>,
    server: &models::server::Model,
) -> ArsaError {
    match free_port_groups(state, server.uuid, &[]).await {
        Ok(free_port_groups) => ArsaError::PortConflict(PortConflictResponse {
            reason: PortConflictReason::NotAPortGroup,
            message: PORT_GROUPS_ONLY_MESSAGE.to_string(),
            ports: server_ports(&server.config).to_vec(),
            conflicts: vec![],
            free_port_groups,
        }),
        Err(err) => err,
    }
}

/// Builds the 409 response for a server whose ports are taken.
pub async fn port_conflict_error(
    state: &Arc<AppState>,
    server: &models::server::Model,
    conflicts: Vec<PortConflict>,
    ports: Vec<u16>,
) -> ArsaError {
    let message = if conflicts.is_empty() {
        format!(
            "Port {} is already in use on the host",
            ports
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "Ports already in use by {}",
            conflicts
                .iter()
                .map(|c| format!("\"{}\"", c.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    // Leave out groups with a taken port. When something outside ARSA holds the ports, no running
    // server uses them, so this is what keeps the server's own group from being offered.
    match free_port_groups(state, server.uuid, &ports).await {
        Ok(free_port_groups) => ArsaError::PortConflict(PortConflictResponse {
            reason: PortConflictReason::PortsInUse,
            message,
            ports,
            conflicts,
            free_port_groups,
        }),
        Err(err) => err,
    }
}

async fn all_port_groups(
    state: &Arc<AppState>,
) -> Result<Vec<models::port_group::Model>, ArsaError> {
    Ok(models::port_group::Entity::find()
        .order_by_asc(models::port_group::Column::GamePort)
        .all(&state.db)
        .await?)
}

fn require_admin(claims: &Claims) -> Result<(), ArsaError> {
    if claims.has_role(&AppConfig::get().admin_role) {
        Ok(())
    } else {
        Err(ArsaError::Unauthorized)
    }
}

/// Checks the request and returns the trimmed name.
async fn validate(
    state: &Arc<AppState>,
    request: &PortGroupRequest,
    own_id: Option<i32>,
) -> Result<String, ArsaError> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(ArsaError::BadRequestMessage(
            "The port group needs a name".to_string(),
        ));
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(ArsaError::BadRequestMessage(format!(
            "The name can have at most {MAX_NAME_LENGTH} characters"
        )));
    }
    if !(MIN_GAME_PORT..=MAX_GAME_PORT).contains(&request.game_port) {
        return Err(ArsaError::BadRequestMessage(format!(
            "The game port must be between {MIN_GAME_PORT} and {MAX_GAME_PORT}"
        )));
    }

    for other in all_port_groups(state).await? {
        if Some(other.id) == own_id {
            continue;
        }
        if other.name.eq_ignore_ascii_case(&name) {
            return Err(ArsaError::ConflictMessage(format!(
                "A port group named \"{}\" already exists",
                other.name
            )));
        }
        if other.overlaps(request.game_port) {
            return Err(ArsaError::ConflictMessage(format!(
                "Ports {}-{} overlap with port group \"{}\" ({}-{})",
                request.game_port,
                request.game_port + 2,
                other.name,
                other.game_port,
                other.a2s_port()
            )));
        }
    }

    Ok(name)
}

#[utoipa::path(
    get,
    tag = "port-group",
    path = "/port-groups",
    responses(
        (status = OK, description = "All port groups, ordered by game port", body = Vec<models::port_group::Model>)
    )
)]
pub async fn get_port_groups(
    State(state): State<Arc<AppState>>,
) -> Result<AppJson<Vec<models::port_group::Model>>, ArsaError> {
    Ok(AppJson(all_port_groups(&state).await?))
}

#[utoipa::path(
    post,
    tag = "port-group",
    path = "/port-groups",
    request_body = PortGroupRequest,
    responses(
        (status = OK, description = "The port group was created", body = models::port_group::Model),
        (status = BAD_REQUEST, description = "Invalid name or port", body = ErrorResponse),
        (status = CONFLICT, description = "The name is taken or the ports overlap another group", body = ErrorResponse),
        (status = UNAUTHORIZED, description = "Only admins can manage port groups", body = ErrorResponse)
    )
)]
pub async fn post_port_group(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    AppJson(request): AppJson<PortGroupRequest>,
) -> Result<AppJson<models::port_group::Model>, ArsaError> {
    require_admin(&claims)?;
    let name = validate(&state, &request, None).await?;

    let group = models::port_group::ActiveModel {
        name: Set(name),
        game_port: Set(request.game_port),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    Ok(AppJson(group))
}

#[utoipa::path(
    put,
    tag = "port-group",
    path = "/port-groups/{id}",
    params(PortGroupIdParams),
    request_body = PortGroupRequest,
    responses(
        (status = OK, description = "The port group was updated. Servers on the group move to its new ports; running ones use them once started again.", body = models::port_group::Model),
        (status = BAD_REQUEST, description = "Invalid name or port", body = ErrorResponse),
        (status = NOT_FOUND, description = "Port group was not found", body = ErrorResponse),
        (status = CONFLICT, description = "The name is taken or the ports overlap another group", body = ErrorResponse),
        (status = UNAUTHORIZED, description = "Only admins can manage port groups", body = ErrorResponse)
    )
)]
pub async fn put_port_group(
    State(state): State<Arc<AppState>>,
    Path(PortGroupIdParams { id }): Path<PortGroupIdParams>,
    Extension(claims): Extension<Claims>,
    AppJson(request): AppJson<PortGroupRequest>,
) -> Result<AppJson<models::port_group::Model>, ArsaError> {
    require_admin(&claims)?;

    let old = models::port_group::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;
    let name = validate(&state, &request, Some(id)).await?;

    let txn = state.db.begin().await?;

    let mut group: models::port_group::ActiveModel = old.clone().into();
    group.name = Set(name);
    group.game_port = Set(request.game_port);
    let updated = group.update(&txn).await?;
    let moved = move_group_servers(&txn, &old, &updated).await?;
    txn.commit().await?;

    for uuid in moved {
        let _ = log_action(
            &state,
            LogAction::ServerUpdated,
            Some(uuid),
            Some(claims.get_user()),
        )
        .await;
    }

    Ok(AppJson(updated))
}

/// Moves the servers on the `old` group's ports to the `new` group's ports and returns their ids.
///
/// A server belongs to a group by its ports, so without this a changed game port would leave the
/// group's servers on custom ports, which can't be used while only port groups are allowed.
async fn move_group_servers(
    db: &impl ConnectionTrait,
    old: &models::port_group::Model,
    new: &models::port_group::Model,
) -> Result<Vec<Uuid>, ArsaError> {
    let mut moved = Vec::new();
    if old.game_port == new.game_port {
        return Ok(moved);
    }
    for server in models::server::Entity::find().all(db).await? {
        if server_ports(&server.config) != old.ports() {
            continue;
        }
        let mut config = server.config.clone();
        move_to_port_group(&mut config, old, new);
        moved.push(server.uuid);
        let mut server: models::server::ActiveModel = server.into();
        server.config = Set(config);
        server.update(db).await?;
    }
    Ok(moved)
}

/// Moves a config on the `old` group's ports to the `new` group's ports. The public port follows
/// only when it was the game port, so a different port forwarded through NAT is kept.
fn move_to_port_group(
    config: &mut ServerConfig,
    old: &models::port_group::Model,
    new: &models::port_group::Model,
) {
    if config.public_port == old.game_port {
        config.public_port = new.game_port;
    }
    config.bind_port = new.game_port;
    config.rcon.port = new.rcon_port();
    config.a2s.port = new.a2s_port();
}

#[utoipa::path(
    delete,
    tag = "port-group",
    path = "/port-groups/{id}",
    params(PortGroupIdParams),
    responses(
        (status = OK, description = "The port group was deleted. Servers keep their ports.", body = SuccessResponse),
        (status = NOT_FOUND, description = "Port group was not found", body = ErrorResponse),
        (status = CONFLICT, description = "The last port group can't be deleted while only port groups are allowed", body = ErrorResponse),
        (status = UNAUTHORIZED, description = "Only admins can manage port groups", body = ErrorResponse)
    )
)]
pub async fn delete_port_group(
    State(state): State<Arc<AppState>>,
    Path(PortGroupIdParams { id }): Path<PortGroupIdParams>,
    Extension(claims): Extension<Claims>,
) -> Result<AppJson<SuccessResponse>, ArsaError> {
    require_admin(&claims)?;

    models::port_group::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(ArsaError::NotFound)?;

    if load_settings(&state).await?.port_groups_only && all_port_groups(&state).await?.len() == 1 {
        return Err(ArsaError::ConflictMessage(
            "Only port groups are allowed, so the last port group can't be deleted. Turn that setting off first.".to_string(),
        ));
    }

    let result = models::port_group::Entity::delete_by_id(id)
        .exec(&state.db)
        .await?;
    if result.rows_affected == 0 {
        return Err(ArsaError::NotFound);
    }

    Ok(AppJson(SuccessResponse { success: true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::server::ArsStatus;
    use sea_orm::Database;
    use std::collections::HashMap;
    use tokio::sync::{Mutex, RwLock, broadcast};

    async fn state() -> Arc<AppState> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.get_schema_registry(module_path!().split("::").next().unwrap())
            .sync(&db)
            .await
            .unwrap();
        Arc::new(AppState {
            ip: "127.0.0.1".parse().unwrap(),
            db,
            status: Mutex::new(ArsStatus::Available),
            channel: broadcast::channel(4).0,
            docker: bollard::Docker::connect_with_defaults().unwrap(),
            watchers: RwLock::new(HashMap::new()),
            oauth_states: RwLock::new(HashMap::new()),
            sessions: RwLock::new(HashMap::new()),
        })
    }

    async fn add_server(
        state: &Arc<AppState>,
        name: &str,
        game_port: u16,
        is_running: bool,
    ) -> models::server::Model {
        let seed: models::defaults::Model =
            serde_json::from_str(include_str!("defaults_seed.json")).unwrap();
        let mut config = seed.config;
        config.bind_port = game_port;
        config.public_port = game_port;
        config.rcon.port = game_port + 1;
        config.a2s.port = game_port + 2;
        models::server::ActiveModel {
            uuid: Set(Uuid::new_v4()),
            name: Set(name.to_string()),
            is_running: Set(is_running),
            branch: Set(seed.branch),
            player_count: Set(0),
            config: Set(config),
            startup_parameters_wrapper: Set(seed.startup_parameters_wrapper),
        }
        .insert(&state.db)
        .await
        .unwrap()
    }

    async fn add_group(
        state: &Arc<AppState>,
        name: &str,
        game_port: u16,
    ) -> models::port_group::Model {
        models::port_group::ActiveModel {
            name: Set(name.to_string()),
            game_port: Set(game_port),
            ..Default::default()
        }
        .insert(&state.db)
        .await
        .unwrap()
    }

    #[test]
    fn reads_host_ports_from_port_bindings() {
        let binding = |port: &str| PortBinding {
            host_ip: Some("0.0.0.0".to_string()),
            host_port: Some(port.to_string()),
        };
        let bindings = HashMap::from([
            (
                "2001/udp".to_string(),
                Some(vec![binding("2001"), binding("2001")]),
            ),
            ("2003/udp".to_string(), Some(vec![binding("2003")])),
            ("2002/udp".to_string(), Some(vec![binding("2002")])),
            ("9000/tcp".to_string(), None),
        ]);
        assert_eq!(host_ports(bindings), [2001, 2002, 2003]);
    }

    #[tokio::test]
    async fn free_groups_skip_taken_ports() {
        let state = state().await;
        add_group(&state, "A", 2001).await;
        add_group(&state, "B", 2004).await;
        let me = add_server(&state, "me", 2001, false).await;

        // Something outside ARSA holds A's RCON port: A must not be offered to switch to.
        let free: Vec<String> = free_port_groups(&state, me.uuid, &[2002])
            .await
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(free, ["B"]);

        let ArsaError::PortConflict(conflict) =
            port_conflict_error(&state, &me, vec![], vec![2002]).await
        else {
            panic!("expected a port conflict");
        };
        let free: Vec<String> = conflict
            .free_port_groups
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(free, ["B"]);
    }

    #[tokio::test]
    async fn changing_game_port_moves_the_group_servers() {
        let state = state().await;
        let old = add_group(&state, "A", 2001).await;
        let on_group = add_server(&state, "on A", 2001, true).await;
        let custom = add_server(&state, "custom", 2002, false).await;

        // A NAT setup advertising a different public port keeps it.
        let mut nat = add_server(&state, "nat", 2001, false).await;
        let mut config = nat.config.clone();
        config.public_port = 30001;
        let mut active: models::server::ActiveModel = nat.into();
        active.config = Set(config);
        nat = active.update(&state.db).await.unwrap();

        let new = models::port_group::Model {
            game_port: 3001,
            ..old.clone()
        };
        let mut moved = move_group_servers(&state.db, &old, &new).await.unwrap();
        moved.sort();
        let mut expected = vec![on_group.uuid, nat.uuid];
        expected.sort();
        assert_eq!(moved, expected);

        let find = |uuid| {
            let db = state.db.clone();
            async move {
                models::server::Entity::find_by_id(uuid)
                    .one(&db)
                    .await
                    .unwrap()
                    .unwrap()
                    .config
            }
        };
        let config = find(on_group.uuid).await;
        assert_eq!(server_ports(&config), [3001, 3002, 3003]);
        assert_eq!(config.public_port, 3001);
        assert_eq!(find(nat.uuid).await.public_port, 30001);
        assert_eq!(server_ports(&find(custom.uuid).await), [2002, 2003, 2004]);

        // Renaming only moves nothing.
        assert!(
            move_group_servers(&state.db, &new, &new)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn conflicts_only_with_running_servers_sharing_ports() {
        let state = state().await;
        add_server(&state, "running same", 2001, true).await;
        add_server(&state, "stopped same", 2001, false).await;
        add_server(&state, "running overlap", 2003, true).await;
        add_server(&state, "running elsewhere", 2010, true).await;
        let me = add_server(&state, "me", 2001, false).await;

        let mut conflicts = find_port_conflicts(&state, &me).await.unwrap();
        conflicts.sort_by(|a, b| a.name.cmp(&b.name));
        let summary: Vec<(String, Vec<u16>)> =
            conflicts.into_iter().map(|c| (c.name, c.ports)).collect();
        assert_eq!(
            summary,
            [
                ("running overlap".to_string(), vec![2003]),
                ("running same".to_string(), vec![2001, 2002, 2003]),
            ]
        );
    }

    #[tokio::test]
    async fn free_groups_skip_ports_of_running_servers() {
        let state = state().await;
        add_group(&state, "A", 2001).await;
        add_group(&state, "B", 2004).await;
        add_group(&state, "C", 2007).await;
        add_server(&state, "on A", 2001, true).await;
        add_server(&state, "stopped on B", 2004, false).await;
        // Custom ports touching C's RCON port.
        add_server(&state, "custom", 2008, true).await;
        let me = add_server(&state, "me", 2001, false).await;

        let free: Vec<String> = free_port_groups(&state, me.uuid, &[])
            .await
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(free, ["B"]);
    }

    async fn set_port_groups_only(state: &Arc<AppState>, on: bool) {
        models::settings::ActiveModel {
            id: Set(models::settings::SETTINGS_ID),
            port_groups_only: Set(on),
        }
        .insert(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn custom_ports_are_allowed_unless_port_groups_only() {
        let state = state().await;
        add_group(&state, "A", 2001).await;
        let custom = add_server(&state, "custom", 2010, false).await;
        let grouped = add_server(&state, "grouped", 2001, false).await;

        assert!(ports_allowed(&state, &custom.config).await.unwrap());

        set_port_groups_only(&state, true).await;
        assert!(!ports_allowed(&state, &custom.config).await.unwrap());
        assert!(ports_allowed(&state, &grouped.config).await.unwrap());
        assert!(matches!(
            require_allowed_ports(&state, &custom.config).await,
            Err(ArsaError::BadRequestMessage(_))
        ));

        // A2S off by one is not the group anymore.
        let mut almost = grouped.config.clone();
        almost.a2s.port += 1;
        assert!(!ports_allowed(&state, &almost).await.unwrap());

        let ArsaError::PortConflict(conflict) = not_a_port_group_error(&state, &custom).await
        else {
            panic!("expected a port conflict");
        };
        assert!(matches!(conflict.reason, PortConflictReason::NotAPortGroup));
        assert_eq!(conflict.free_port_groups.len(), 1);
    }

    #[tokio::test]
    async fn validate_rejects_overlaps_duplicates_and_bad_ports() {
        let state = state().await;
        add_group(&state, "A", 2001).await;

        let request = |name: &str, game_port| PortGroupRequest {
            name: name.to_string(),
            game_port,
        };
        assert!(matches!(
            validate(&state, &request("B", 2003), None).await,
            Err(ArsaError::ConflictMessage(_))
        ));
        assert!(matches!(
            validate(&state, &request(" a ", 3000), None).await,
            Err(ArsaError::ConflictMessage(_))
        ));
        assert!(matches!(
            validate(&state, &request("B", 80), None).await,
            Err(ArsaError::BadRequestMessage(_))
        ));
        assert!(matches!(
            validate(&state, &request("  ", 3000), None).await,
            Err(ArsaError::BadRequestMessage(_))
        ));
        assert_eq!(
            validate(&state, &request(" B ", 2004), None).await.unwrap(),
            "B"
        );
        // A group may move within its own range.
        let a = models::port_group::Entity::find()
            .one(&state.db)
            .await
            .unwrap()
            .unwrap();
        assert!(
            validate(&state, &request("A", 2002), Some(a.id))
                .await
                .is_ok()
        );
    }
}
