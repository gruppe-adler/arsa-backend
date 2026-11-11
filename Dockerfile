# Planner stage: analyze dependencies
FROM rust:latest AS planner
WORKDIR /app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Cacher stage: compile dependencies
FROM rust:latest AS cacher
WORKDIR /app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Builder stage: compile application
FROM rust:latest AS builder
WORKDIR /app
COPY . .
COPY --from=cacher /app/target target
RUN cargo build --release

# Runtime stage: minimal image
FROM debian:trixie-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/arsa-backend /usr/local/bin/arsa-backend

# Run as 1000, the same user as the Arma Reforger server containers (ARSA_ARS_USER), so everything
# the backend creates in the server and repo directories is writable by the servers.
# The directories are created here so new volumes mounted on them are owned by 1000.
# Access to the docker socket is granted at runtime with --group-add (the socket's group id).
RUN mkdir -p /app/db /app/ars/servers /app/ars/repo && chown -R 1000:1000 /app
USER 1000:1000

EXPOSE 3000
ENV ARSA_USE_VOLUME=true
CMD ["/usr/local/bin/arsa-backend"]

# Declared after the chown, so new volumes are created owned by 1000. /app/ars itself isn't a volume:
# servers/ and repo/ are mounted separately, so it only created an extra anonymous volume.
VOLUME [ "/app/db", "/app/ars/servers", "/app/ars/repo" ]