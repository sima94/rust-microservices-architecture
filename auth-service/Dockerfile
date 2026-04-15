# Stage 1: cargo-chef base
FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src/app

# Stage 2: planner — compute recipe from full workspace manifests + sources
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml shared/Cargo.toml
COPY auth-service/Cargo.toml auth-service/Cargo.toml
COPY user-service/Cargo.toml user-service/Cargo.toml
COPY postman/generator/Cargo.toml postman/generator/Cargo.toml
COPY shared/src shared/src
COPY auth-service/src auth-service/src
COPY user-service/src user-service/src
COPY postman/generator/src postman/generator/src
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: builder — install native deps, cook deps, build binary
FROM chef AS builder
RUN apt-get update && apt-get install -y cmake pkg-config libssl-dev libsasl2-dev libcurl4-openssl-dev zlib1g-dev && rm -rf /var/lib/apt/lists/*
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin auth-service
COPY Cargo.toml Cargo.lock ./
COPY shared shared
COPY auth-service auth-service
COPY user-service user-service
COPY postman/generator postman/generator
RUN cargo build --release --bin auth-service

# Stage 4: minimal runtime
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y ca-certificates libpq5 curl && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/local/bin
EXPOSE 8081
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8081/health || exit 1
COPY --from=builder /usr/src/app/target/release/auth-service .
CMD ["./auth-service"]
