# Rust Microservices Architecture

Production-ready microservices platform built with Rust, featuring high-availability PostgreSQL, read/write splitting, Redis caching, and event-driven communication via Kafka.

## Architecture

```
                         ┌─────────────┐
                         │   nginx     │
                         │   :8080     │
                         └──────┬──────┘
                        ┌───────┴───────┐
                        │               │
                ┌───────▼──────┐ ┌──────▼───────┐
                │ auth-service │ │ user-service  │
                │    :8081     │ │    :8082      │
                └───────┬──────┘ └──────┬───────┘
                        │               │
         ┌──────────────┼───────────────┼──────────────┐
         │              │               │              │
    ┌────▼────┐   ┌─────▼─────┐   ┌────▼────┐   ┌────▼────┐
    │  Redis  │   │   Kafka   │   │ PgBouncer│   │PgBouncer│
    │  :6379  │   │   :9092   │   │auth write│   │usr write│
    └─────────┘   └───────────┘   │  :5433   │   │  :5432  │
                                  └────┬─────┘   └────┬────┘
                                       │              │
                                  ┌────▼─────┐   ┌────▼────┐
                                  │ HAProxy  │   │ HAProxy │
                                  │auth-write│   │usr-write│
                                  └────┬─────┘   └────┬────┘
                                       │              │
                              ┌────────┼────────┐  ┌──┼────────┐
                              │        │        │  │  │        │
                           ┌──▼──┐  ┌──▼──┐  ┌─▼┐┌▼──▼┐ ┌────▼┐
                           │pg-a1│  │pg-a2│  │a3││u1  │ │u2 u3│
                           │master│ │repl │  │r ││mstr│ │repl │
                           └─────┘  └─────┘  └──┘└────┘ └─────┘
                                    Patroni + etcd consensus
```

## Tech Stack

| Component | Technology | Purpose |
|-----------|-----------|---------|
| Services | Rust + Actix-web 4 | HTTP API servers |
| Database | PostgreSQL 15 | Primary data store |
| HA | Patroni + etcd | Automatic failover (3-node clusters) |
| Load Balancing | HAProxy | Read/write splitting |
| Connection Pool | PgBouncer | Connection pooling |
| Cache | Redis 7 | Response caching with TTL |
| Events | Kafka + Zookeeper | Async inter-service communication |
| Reverse Proxy | nginx | API gateway, routing |
| API Docs | utoipa + Swagger UI | OpenAPI 3.0 documentation |

## Services

### auth-service (port 8081)

OAuth2/PKCE authentication service handling user registration, client management, and token issuance.

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/api/v1/auth/register` | POST | None | Register new user |
| `/api/v1/clients/register` | POST | None | Register OAuth client |
| `/api/v1/oauth/authorize` | POST | None | Authorization code grant (PKCE) |
| `/api/v1/oauth/token` | POST | None | Token exchange |
| `/api/v1/oauth/revoke` | POST | None | Token revocation |
| `/health` | GET | None | Health check |

**Database:** `auth_db` via Patroni auth-cluster (port 5433)
**Kafka:** Produces `user.registered` events to `user.events` topic

### user-service (port 8082)

User CRUD service with Redis caching and read/write database splitting.

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/api/v1/users` | GET | Bearer (scope: read) | List all users |
| `/api/v1/users/{id}` | GET | Bearer (scope: read) | Get user by ID |
| `/api/v1/users` | POST | Bearer (scope: write) | Create user |
| `/api/v1/users/{id}` | PUT | Bearer (scope: write) | Update user |
| `/api/v1/users/{id}` | DELETE | Bearer (scope: write) | Delete user |
| `/health` | GET | None | Health check |

**Database:** `rust_db` via Patroni user-cluster (port 5432)
**Kafka:** Consumes `user.registered` events, auto-creates user profiles

### shared (library crate)

Common code used by both services: database pools, Redis cache, error types, JWT middleware, test utilities.

## Quick Start

### Prerequisites

- Docker Desktop
- Rust nightly (`rustup default nightly`)
- cmake (`brew install cmake`)

### 1. Start infrastructure

```bash
docker compose up -d
```

This starts 22 containers: 2x Patroni clusters (6 PostgreSQL nodes), 4x HAProxy, 4x PgBouncer, Redis, Kafka, Zookeeper, etcd, nginx.

### 2. Wait for Patroni clusters to bootstrap (~30 seconds)

```bash
# Check cluster health
docker exec rust-microservices-architecture-patroni-user-1-1 \
  curl -s http://localhost:8008/cluster | python3 -m json.tool

docker exec rust-microservices-architecture-patroni-auth-1-1 \
  curl -s http://localhost:8008/cluster | python3 -m json.tool
```

### 3. Run database migrations

```bash
# User service
PGPASSWORD=rust_pass psql -h localhost -p 5432 -U rust_user -d rust_db \
  -f user-service/migrations/users/up.sql

# Auth service (all 4 migrations)
for dir in auth-service/migrations/*/; do
  PGPASSWORD=rust_pass psql -h localhost -p 5433 -U rust_user -d auth_db \
    -f "${dir}up.sql"
done
```

For Kubernetes dev environment (OKE), use the automated scripts:

```bash
# Both services
./scripts/migrate-all-dev.sh

# User only
./scripts/migrate-user-dev.sh

# Auth only
./scripts/migrate-auth-dev.sh

# Preview only (no changes)
DRY_RUN=1 ./scripts/migrate-all-dev.sh
```

## Secrets Standard (Dev/Staging/Prod)

Do not keep real credentials in scripts or committed values files.

Use a local secrets file (not committed):

```bash
mkdir -p .secrets
cp .secrets/dev.env.example .secrets/dev.env
cp .secrets/staging.env.example .secrets/staging.env
cp .secrets/prod.env.example .secrets/prod.env
# edit each .secrets/<env>.env with real values
```

All deploy/test scripts can load this file via:

```bash
SECRETS_FILE=.secrets/dev.env <script>
```

## Service Deploy (Helm + Public Exposure)

For OKE deploys of `auth-service` and `user-service`, use:

```bash
# Default behavior:
# - dev/staging => public LoadBalancer services
# - prod        => internal ClusterIP services
SECRETS_FILE=.secrets/dev.env ./scripts/deploy-services.sh dev
SECRETS_FILE=.secrets/staging.env ./scripts/deploy-services.sh staging
SECRETS_FILE=.secrets/prod.env ./scripts/deploy-services.sh prod
```

Useful options:

```bash
# Force public exposure in any environment
SECRETS_FILE=.secrets/staging.env EXPOSE_PUBLIC=true ./scripts/deploy-services.sh staging

# Force internal-only services
SECRETS_FILE=.secrets/dev.env EXPOSE_PUBLIC=false ./scripts/deploy-services.sh dev

# Explicit OCI LB subnet (recommended on OKE)
SECRETS_FILE=.secrets/dev.env OCI_LB_SUBNET_OCID=ocid1.subnet... ./scripts/deploy-services.sh dev

# Skip waiting for external IP assignment (useful in CI)
SECRETS_FILE=.secrets/dev.env WAIT_FOR_EXTERNAL_IP=false ./scripts/deploy-services.sh dev

# Preview commands only
SECRETS_FILE=.secrets/dev.env DRY_RUN=1 ./scripts/deploy-services.sh dev
```

Pipeline best practice: run this in the **deploy** stage (not build stage), because public exposure is an environment/runtime concern.
If OCI returns `LimitExceeded (lb-100mbps-count)`, either increase LB quota or switch to a shared Ingress (single public LB for multiple services).

## Shared Ingress (One Public IP/Domain)

Scripts for a single public entrypoint (Ingress NGINX) are under `scripts/ingress/`:

```bash
# Full flow (recommended):
# 1) switch auth/user services to ClusterIP
# 2) install ingress-nginx (LoadBalancer)
# 3) apply shared routes (/api/v1/auth|oauth|clients|users)
SECRETS_FILE=.secrets/dev.env ./scripts/ingress/deploy-ingress.sh dev

# Install controller only
./scripts/ingress/install-ingress-nginx.sh dev

# Apply/refresh app ingress routes only
./scripts/ingress/apply-app-ingress.sh dev

# Optional host + TLS
INGRESS_HOST=api.dev.example.com TLS_SECRET_NAME=api-dev-tls \
  ./scripts/ingress/apply-app-ingress.sh dev
```

## Monitoring (Prometheus + Grafana)

Reusable scripts are provided for `dev`, `staging`, and `prod`.

```bash
# Full deploy: install kube-prometheus-stack + apply ServiceMonitor/alerts
./scripts/monitoring/deploy-monitoring.sh dev

# Staging / prod
./scripts/monitoring/deploy-monitoring.sh staging
./scripts/monitoring/deploy-monitoring.sh prod

# Preview rendered objects and commands only
DRY_RUN=1 ./scripts/monitoring/deploy-monitoring.sh dev

# Remove app monitoring objects (and optionally stack)
./scripts/monitoring/uninstall-monitoring.sh dev
REMOVE_STACK=1 ./scripts/monitoring/uninstall-monitoring.sh dev
```

Default kubeconfig path is `/tmp/oke-<env>-kubeconfig`, but can be overridden via `KUBECONFIG`.

### 4. Run services locally (for development)

```bash
cd auth-service && cargo run
cd user-service && cargo run
```

Or use the Docker-built services (already running via compose):
```bash
curl -s http://localhost:8080/health/auth
curl -s http://localhost:8080/health/user
```

### 5. Access Swagger UI

- Auth service: http://localhost:8081/swagger-ui/
- User service: http://localhost:8082/swagger-ui/
- Prometheus metrics:
  - Auth service: `GET /metrics` (port 8081)
  - User service: `GET /metrics` (port 8082)

## Running Tests

Tests require Docker infrastructure to be running.

```bash
# All tests for a service
cd user-service && cargo test
cd auth-service && cargo test

# Only unit tests (faster, no integration tests)
cd user-service && cargo test --lib
cd auth-service && cargo test --lib

# Specific test
cd user-service && cargo test test_create_user
```

If tests fail with `PoolTimedOut`, ensure Docker containers are running and Patroni clusters have bootstrapped.
If tests fail with `relation "X" does not exist`, run the migrations (step 3 above).

For dev-cluster integration scripts, load secrets explicitly:

```bash
SECRETS_FILE=.secrets/dev.env ./scripts/test-all-dev.sh
SECRETS_FILE=.secrets/dev.env ./scripts/test-user-dev.sh
SECRETS_FILE=.secrets/dev.env ./scripts/test-auth-dev.sh
```

## Infrastructure Details

### Database Architecture (per service)

```
App → PgBouncer (connection pool) → HAProxy (routing) → Patroni (HA PostgreSQL)
```

Each service has its own isolated database cluster:

| Component | Auth cluster | User cluster |
|-----------|-------------|-------------|
| Database | `auth_db` | `rust_db` |
| DB user | `rust_user` / `rust_pass` | `rust_user` / `rust_pass` |
| PgBouncer write | localhost:5433 | localhost:5432 |
| PgBouncer read | localhost:5435 | localhost:5434 |
| Patroni nodes | patroni-auth-1/2/3 | patroni-user-1/2/3 |

### Read/Write Splitting

- **Writes** go through `HAProxy write` which routes to the Patroni **master** only
- **Reads** go through `HAProxy read` which load-balances across **replicas** (roundrobin)
- HAProxy health checks Patroni REST API (`GET /master` and `GET /replica` on port 8008)
- If master dies, Patroni auto-promotes a replica within ~30 seconds

### Port Map

| Port | Service |
|------|---------|
| 8080 | nginx (API gateway) |
| 8081 | auth-service |
| 8082 | user-service |
| 5432 | PgBouncer user-write |
| 5433 | PgBouncer auth-write |
| 5434 | PgBouncer user-read |
| 5435 | PgBouncer auth-read |
| 6379 | Redis |
| 9092 | Kafka |
| 2181 | Zookeeper |
| 2379 | etcd |

### Common Operations

```bash
# Restart services after Patroni crash
docker compose restart auth-service user-service

# Simulate failover
docker compose stop patroni-user-1   # master dies
# Wait 30s, replica auto-promotes
docker compose start patroni-user-1  # rejoins as replica

# Reset database (fresh start)
docker compose down
docker volume rm rust-microservices-architecture_patroni-user-{1,2,3}-data
docker volume rm rust-microservices-architecture_patroni-auth-{1,2,3}-data
docker compose up -d
# Then re-run migrations

# View Patroni cluster state
docker exec rust-microservices-architecture-patroni-user-1-1 \
  curl -s http://localhost:8008/cluster | python3 -m json.tool
```

## Project Structure

```
rust-microservices-architecture/
├── auth-service/           # OAuth2/PKCE authentication service
│   ├── src/
│   │   ├── controllers/    # HTTP handlers (v1/)
│   │   ├── models/         # Data structures
│   │   ├── repositories/   # Database queries (static methods)
│   │   ├── services/       # Business logic
│   │   └── main.rs         # Server setup, routes, OpenAPI
│   ├── migrations/         # SQL migrations (4 tables)
│   ├── tests/              # Integration tests
│   └── .env                # DATABASE_URL, JWT_SECRET, KAFKA_BROKER
│
├── user-service/           # User CRUD service
│   ├── src/                # Same layered structure
│   ├── migrations/         # SQL migrations (1 table)
│   ├── tests/              # Integration tests
│   └── .env
│
├── shared/                 # Shared library crate
│   └── src/
│       ├── db.rs           # DbPools, init_pools()
│       ├── cache.rs        # RedisPool, get/set/invalidate
│       ├── errors.rs       # ServiceError enum
│       ├── health.rs       # Generic health check handler
│       ├── middleware/      # JWT auth, scope checks
│       └── test_utils.rs   # Test helpers
│
├── docker/
│   ├── patroni/            # HA PostgreSQL (Dockerfile, entrypoint, init scripts)
│   └── haproxy/            # Read/write load balancer configs
│
├── helm/charts/            # Kubernetes Helm chart
├── nginx/nginx.conf        # Reverse proxy config
└── docker-compose.yml      # Full local development environment (22 containers)
```

## Code Conventions

- **Architecture:** Repository -> Service -> Controller (3 layers, never skip)
- **Repositories:** Zero-sized structs with static async methods, receive `&PgPool`
- **Read/Write split:** All reads through `pools.read`, all writes through `pools.write`
- **Caching:** Redis check -> DB query -> cache result. Invalidate on every write.
- **Error handling:** `shared::errors::ServiceError` enum maps to HTTP status codes
- **Testing:** TDD mandatory. Tests first, implementation second. See `CLAUDE.md` for full rules.
