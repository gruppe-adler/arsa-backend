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
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/arsa-backend-rs /usr/local/bin/arsa-backend-rs
EXPOSE 3000
CMD ["/usr/local/bin/arsa-backend-rs"]
