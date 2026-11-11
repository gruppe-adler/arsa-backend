# ARSA Backend

ARSA (Arma Reforger Server Admin) Backend is a Rust-based REST API and WebSocket server for managing Arma Reforger game servers. It provides administrative functionality for server lifecycle management, configuration, logging, and real-time monitoring.

## Features

- **Server Management**: Create, start, stop, and delete game servers
- **Container Integration**: Docker-based server deployment and orchestration
- **Real-time Updates**: WebSocket support for live server status and log streaming
- **Logging**: Access to server logs, crash reports, and player activity
- **Statistics**: Server performance metrics and size management
- **RESTful API**: OpenAPI/Swagger documentation included
- **Database**: SQLite-based persistence with SeaORM

## Prerequisites

- Rust 1.70+ (for local development)
- Docker and Docker daemon access
- SQLite 3 (included with SeaORM)
- Port 3000 available for API server

## Building

### Local Build

```bash
cargo build --release
```

The compiled binary will be available at `target/release/arsa-backend-rs`.

### Docker Build

```bash
docker build -t arsa-backend .
```

This uses a multi-stage build with cargo-chef for optimized caching.

## Running

### Local Development

```bash
cargo run
```

The server will start on `0.0.0.0:3000` and automatically:

- Create necessary directories for server data
- Initialize SQLite database at `./db/arsa.sqlite`
- Connect to the local Docker daemon

### Docker

Build

```bash
docker build -t arsa-backend .
```

Run

```bash
docker run --rm --name arsa-backend --env-file .env --group-add $(stat -c %g /var/run/docker.sock) -v arsa-db-volume:/app/db -v arsa-servers-volume:/app/ars/servers -v arsa-repo-volume:/app/ars/repo -v /var/run/docker.sock:/var/run/docker.sock -p 3000:3000 arsa-backend
```

Start / Stop

```bash
docker start arsa-backend
docker stop arsa-backend
```

The container needs access to the Docker daemon via socket mount to manage servers.

### Users and permissions

The backend and the Arma Reforger server containers run as the unprivileged user `1000:1000`. The backend creates the server configs, profiles and the addon repo, and the servers write their logs, saves and mods into them, so both must use the same user.

- `--group-add` gives the backend the group of the docker socket. The group id differs between hosts, so it isn't part of the image.
- `ARSA_ARS_USER` (default `1000:1000`) is the `uid:gid` the server containers run as. They also run with all capabilities dropped and `no-new-privileges`.
- With `ARSA_USE_VOLUME=false` (bind mounts), the server and repo directories on the host must be owned by that user.

Volumes created by an older, root-running backend contain files owned by root. Fix them once before starting the new version:

```bash
docker run --rm -v arsa-db-volume:/a -v arsa-servers-volume:/b -v arsa-repo-volume:/c alpine chown -R 1000:1000 /a /b /c
```

## API Documentation

Once running, access the interactive API documentation at:

```
http://localhost:3000/swagger-ui/
```

### Main Endpoints

- `GET /api/v2/servers` - List all servers
- `POST /api/v2/server` - Create a new server
- `GET /api/v2/server/{id}` - Get server details
- `POST /api/v2/server/{id}/start` - Start a server
- `POST /api/v2/server/{id}/stop` - Stop a server
- `DELETE /api/v2/server/{id}` - Delete a server
- `GET /api/v2/server/{id}/logs` - Get server logs
- `WS /ws` - WebSocket for real-time updates

## Architecture

- **Framework**: Axum web framework with Tower middleware
- **Database**: SeaORM + SQLite for data persistence
- **Container Runtime**: Bollard for Docker integration
- **Real-time**: tokio async runtime with broadcast channels
- **API Docs**: Utoipa for OpenAPI generation

## Development

### Project Structure

```
src/
├── main.rs           - Server setup and routing
├── endpoints/        - API endpoint handlers
├── models/           - Data models and schemas
└── shared/           - Shared utilities
```

### Running Tests

```bash
cargo test
```

### Environment

The server detects local IP automatically and requires:

- Docker daemon connection (via socket at `/var/run/docker.sock`)
- Write access to create data directories
- SQLite database directory permissions
