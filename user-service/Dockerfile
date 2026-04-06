# Stage 1: Generate recipe
FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef
WORKDIR /usr/src

# Stage 2: Cook dependencies (cached layer)
FROM chef AS planner
COPY shared/ /usr/src/shared/
COPY user-service/Cargo.toml /usr/src/app/Cargo.toml
COPY user-service/src /usr/src/app/src
WORKDIR /usr/src/app
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y cmake pkg-config && rm -rf /var/lib/apt/lists/*

COPY shared/ /usr/src/shared/
COPY --from=planner /usr/src/app/recipe.json /usr/src/app/recipe.json
WORKDIR /usr/src/app
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 3: Build application
COPY user-service/src ./src
COPY user-service/migrations ./migrations
RUN cargo build --release

# Stage 4: Minimal runtime
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y ca-certificates libpq5 curl && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

EXPOSE 8082

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8082/health || exit 1

COPY --from=builder /usr/src/app/target/release/user-service .

CMD ["./user-service"]
