# In-cluster test runner.
# Builds test binaries and runs them inside the K8s cluster
# with full access to all services (DB, Redis, Kafka).
#
# Build: docker build -t test-runner -f test-runner/Dockerfile .
# Usage: deployed as K8s Job via scripts/test-in-cluster.sh

FROM rustlang/rust:nightly AS builder

RUN apt-get update && apt-get install -y cmake pkg-config && rm -rf /var/lib/apt/lists/*

# Copy shared crate
COPY shared/ /usr/src/shared/

# Build user-service test binary
WORKDIR /usr/src/user-service
COPY user-service/Cargo.toml ./
COPY user-service/src ./src
COPY user-service/tests ./tests
COPY user-service/migrations ./migrations
RUN cargo test --test api_tests --no-run 2>&1 | tail -1
RUN cp $(find /usr/src/user-service/target/debug/deps -name "api_tests-*" -type f -executable | head -1) /usr/src/user-test-bin

# Build auth-service test binary
WORKDIR /usr/src/auth-service
COPY auth-service/Cargo.toml ./
COPY auth-service/src ./src
COPY auth-service/tests ./tests
COPY auth-service/migrations ./migrations
RUN cargo test --test api_tests --no-run 2>&1 | tail -1
RUN cp $(find /usr/src/auth-service/target/debug/deps -name "api_tests-*" -type f -executable | head -1) /usr/src/auth-test-bin

# Minimal runtime with test binaries
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y ca-certificates libpq5 && rm -rf /var/lib/apt/lists/*

WORKDIR /tests

COPY --from=builder /usr/src/user-test-bin ./user-api-tests
COPY --from=builder /usr/src/auth-test-bin ./auth-api-tests
COPY test-runner/entrypoint.sh ./entrypoint.sh
RUN chmod +x entrypoint.sh user-api-tests auth-api-tests

ENTRYPOINT ["./entrypoint.sh"]
