# rust-microservices-architecture

Rust microservices platform using Actix-web 4, SQLx (PostgreSQL), Redis, and Kafka.
Each service follows a Repository → Service → Controller layered pattern.
Services are independently deployable with their own database (per-service Patroni cluster).

## Repository Structure

```
/auth-service          - OAuth2/PKCE authentication service (port 8081)
/user-service          - User CRUD service (port 8082)
/shared                - Shared library crate (db, cache, errors, middleware, test utils)
/docker                - Patroni, HAProxy, PostgreSQL infrastructure configs
/helm/charts           - Reusable Helm chart for Kubernetes deployment
/nginx                 - Reverse proxy configuration (port 8080)
/docker-compose.yml    - Local development environment
```

## Development Setup

```bash
# Start all infrastructure (Patroni clusters, Redis, Kafka, nginx)
docker compose up -d

# Run a specific service locally
cd user-service && cargo run
cd auth-service && cargo run

# Run tests (requires PostgreSQL and Redis running via docker compose)
cd user-service && cargo test
cd auth-service && cargo test
```

## Docker Compose Infrastructure

docker-compose.yml runs 22 containers. Key relationships:

```
nginx (8080)
  ├── auth-service (8081) → pgbouncer-auth-write/read → haproxy-auth-write/read → patroni-auth-1/2/3
  └── user-service (8082) → pgbouncer-user-write/read → haproxy-user-write/read → patroni-user-1/2/3
                                                                                          ↑
redis (6379)                                                                          etcd (2379)
kafka (9092) ← zookeeper (2181)
```

- **Each service has its own Patroni cluster** (3 PostgreSQL nodes with auto-failover)
- **Patroni scope** determines cluster identity: `user-cluster` vs `auth-cluster`
- **POST_INIT_SCRIPT** env var selects which DB gets created (rust_db vs auth_db)
- **Services panic on startup** if Patroni isn't ready yet — just `docker compose restart auth-service user-service`
- **Volume names** include project prefix: `rust-microservices-architecture_patroni-user-1-data`
- See `docker/AGENTS.md` for Patroni, HAProxy, and PgBouncer details

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| DATABASE_URL | Yes | - | Write pool connection string |
| DATABASE_READ_URL | No | DATABASE_URL | Read pool connection string |
| REDIS_URL | No | redis://127.0.0.1:6379 | Redis connection string |
| JWT_SECRET | Yes | - | Shared JWT signing secret |
| KAFKA_BROKER | No | localhost:9092 | Kafka bootstrap server |
| DB_POOL_MAX_SIZE | No | 10 | Max DB connections per pool |

## Code Conventions

### Architecture Pattern

Repository → Service → Controller (3 layers, never skip)

- **Repository**: Direct SQLx queries, receives `&PgPool`, returns `Result<T, sqlx::Error>`
- **Service**: Business logic, receives `web::Data<T>`, calls repository, returns `Result<T, ServiceError>`
- **Controller**: HTTP handlers, calls service, returns `Result<HttpResponse, ServiceError>`

### Repository Pattern (IMPORTANT - follow this exactly)

Repositories are zero-sized structs with static async methods:
```rust
pub struct XRepository;
impl XRepository {
    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<X, sqlx::Error> { ... }
    pub async fn create(pool: &PgPool, new: NewX) -> Result<X, sqlx::Error> { ... }
}
```
Do NOT use trait abstractions. Do NOT use &self methods. Keep static.

### Service Pattern

```rust
pub struct XService;
impl XService {
    pub async fn create(pools: web::Data<DbPools>, redis: web::Data<RedisPool>, input: NewX) -> Result<X, ServiceError> { ... }
}
```

### Read/Write Splitting

- ALL reads go through `pools.read` (replica)
- ALL writes go through `pools.write` (master)
- After writes, invalidate relevant cache keys

### Caching Pattern

- Check Redis cache first → on miss, query DB → store result in cache
- Cache invalidation on every write operation
- Cache keys follow: `"entity:{id}"` or `"entity:list"` format

### Error Handling

Use `shared::errors::ServiceError` enum. Common variants:
- `NotFound` → 404
- `InternalError(String)` → 500
- `Unauthorized(String)` → 401
- `Forbidden(String)` → 403
- `Conflict(String)` → 409
- `BadRequest(String)` → 400
- `InvalidClient(String)` → 401 (OAuth specific)
- `InvalidGrant(String)` → 400 (OAuth specific)
- `InvalidRequest(String)` → 400 (OAuth specific)

### Naming Conventions

- Files: `snake_case` (user_repository.rs, user_service.rs, user_controller.rs)
- Structs: `PascalCase` (UserRepository, UserService)
- Functions: `snake_case` (find_by_id, create_user)
- Models: Entity (`User`), NewEntity (`NewUser`), UpdateEntity (`UpdateUser`)
- Controller routes: `init_routes(cfg: &mut web::ServiceConfig)`

### Module Structure (every service)

```
src/
  main.rs             - Server setup, OpenAPI, routes
  lib.rs              - Module declarations
  config.rs           - AppConfig from env vars (if applicable)
  models/
    mod.rs            - Re-exports
    entity.rs         - Struct definitions with #[derive(ToSchema)]
    dto.rs            - Request/Response DTOs (if many)
  repositories/
    mod.rs
    entity_repository.rs
  services/
    mod.rs
    entity_service.rs
  controllers/
    mod.rs
    health_controller.rs  - Uses shared::health
    v1/
      mod.rs
      entity_controller.rs
  middleware/          - Service-specific middleware (if any)
  kafka.rs            - Kafka producer/consumer (if applicable)
```

### OpenAPI / Swagger

Every service includes utoipa + utoipa-swagger-ui:
- All handler functions must be `pub`
- All request/response models derive `ToSchema`
- Swagger UI available at `/swagger-ui/` per service
- OpenAPI JSON at `/api-docs/openapi.json`

## TESTING IS MANDATORY — NO EXCEPTIONS

**Nothing is "done" until tests pass. This is the #1 rule of this project.**

An agent that writes code without tests, or skips running tests, is doing it wrong.
Every PR, every feature, every bug fix, every refactor — tests are not optional, they are the definition of done.

### Rule 1: Tests FIRST, implementation SECOND (TDD)
Every new endpoint, feature, or bug fix MUST have a test written BEFORE implementation.
The workflow is always: write failing test → write code to pass it → verify → next test.

### Rule 2: User defines WHAT is tested, agent writes HOW
The agent does NOT decide which test cases to cover — this is defined in the "Required Tests"
section for each service's AGENTS.md. The agent implements those tests and code that passes them.

### Rule 3: Every endpoint MUST have at minimum these test cases:
1. Happy path (successful request — correct status + response body)
2. Validation (bad input → 400 Bad Request)
3. Authorization (no token → 401, wrong scope → 403)
4. Not found (nonexistent resource → 404)
5. Conflict (duplicate → 409, where relevant)

### Rule 4: TDD workflow order
1. Write failing integration test for the endpoint
2. Implement: models → repository → service → controller
3. `cargo test` after EACH layer — do not batch
4. Test passes = feature complete
5. NEVER modify a test to make it pass — modify the implementation

### Rule 5: Existing tests are never deleted
When the agent changes code, ALL existing tests MUST still pass.
If a test fails after a change, it's a bug in the change — not in the test.

### Rule 6: Run ALL tests before declaring work complete
After finishing any task, run the FULL test suite for every affected service:
```bash
cd user-service && cargo test
cd auth-service && cargo test
```
If ANY test fails, the task is NOT done. Fix it before moving on.

### Rule 7: No untested code enters the codebase
- New function? → needs a test
- Bug fix? → needs a regression test that reproduces the bug first
- Refactor? → existing tests must still pass, add tests if behavior changes
- Config change? → verify with integration test or manual health check

### Test Template for new endpoints

When adding a new endpoint, MUST cover:

**For GET (read):**
- [ ] Happy path with valid token → 200
- [ ] Nonexistent resource → 404
- [ ] No token → 401
- [ ] Wrong scope → 403

**For POST (create):**
- [ ] Valid input → 201
- [ ] Missing required field → 400
- [ ] Duplicate (unique constraint) → 409
- [ ] No token → 401
- [ ] Wrong scope → 403

**For PUT (update):**
- [ ] Valid update → 200
- [ ] Nonexistent resource → 404
- [ ] Invalid input → 400
- [ ] No token → 401

**For DELETE:**
- [ ] Existing resource → 204
- [ ] Nonexistent resource → 404
- [ ] No token → 401

## Testing Guidelines

### Integration Test Pattern

```rust
use shared::test_utils::{TEST_JWT_SECRET, create_test_token, get_test_pools};

macro_rules! setup_app {
    () => {{
        dotenvy::dotenv().ok();
        let pools = shared::test_utils::get_test_pools().await;
        let jwt_secret = TEST_JWT_SECRET.to_string();
        test::init_service(
            App::new()
                .app_data(web::Data::new(pools))
                .app_data(web::Data::new(jwt_secret))
                .service(web::scope("/api/v1").configure(routes))
        ).await
    }};
}
```

### Repository Test Pattern

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn get_test_pool() -> PgPool {
        shared::test_utils::get_test_pool().await
    }

    #[tokio::test]
    async fn test_create_and_find() {
        let pool = get_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        // ... test operations against &mut *tx ...
        tx.rollback().await.unwrap();
    }
}
```

### Running Tests

```bash
# All tests for a service (requires PostgreSQL running)
cd user-service && cargo test

# Specific test
cd user-service && cargo test test_create_user

# Only unit tests (no DB required)
cd auth-service && cargo test --lib
```

## Adding a New Service

1. Copy `service-template/` (or existing service) as starting point
2. Update `Cargo.toml` (package name, add `shared = { path = "../shared" }`)
3. Pick next port (8083, 8084, ...)
4. Create database and write SQL migrations in `migrations/`
5. Add Patroni cluster + HAProxy + PgBouncer to `docker-compose.yml`
6. Add nginx proxy rules in `nginx/nginx.conf`
7. Write Required Tests in the service's `AGENTS.md`
8. Implement using TDD workflow (tests first!)
9. Add Swagger/OpenAPI documentation

## Postman Collection

**When ANY API endpoint changes, regenerate the Postman collection.**

### How to regenerate

```bash
# With services running (fetches live OpenAPI specs):
cd postman/generator && cargo run

# Without services (uses cached specs from postman/openapi/):
cd postman/generator && cargo run -- --offline
```

### Files

| File | Maintained | Purpose |
|------|-----------|---------|
| `postman/overlay.json` | Manually | Variable chains, folder structure, PKCE scripts, error cases |
| `postman/generator/` | Manually | Rust crate that merges OpenAPI + overlay → Postman JSON |
| `postman/openapi/*.json` | Auto-cached | OpenAPI specs saved on each generation run |
| `postman/Rust_Microservices.postman_collection.json` | **Generated** | Output — do NOT edit by hand |

### When to update overlay.json

- New endpoint → add to appropriate folder in `overlay.json`, then regenerate
- Removed endpoint → remove from `overlay.json`, then regenerate
- Changed URL/method/body → regeneration picks it up from OpenAPI automatically
- New variable chain (saving response field) → add `save` entry in `overlay.json`
- New auth flow or PKCE logic → update `scripts` section in `overlay.json`

### overlay.json key fields per request

- `operation` + `service` — links to OpenAPI operationId (method, path, content_type auto-detected)
- `body` — request body with `{{variable}}` references
- `save` — `{"var_name": "response.field"}` — auto-generates test script to save variable
- `save` with `?` suffix — `{"token": "refresh_token?"}` — optional field, wrapped in `if` check
- `prerequest` — name of script from `scripts` section (e.g., `"pkce"`)
- `accept_status` — `[201, 409]` — multi-status handling (success or already exists)
- `expect_status` — expected HTTP status for assertions
- `custom: true` — hand-defined request (not from OpenAPI), e.g., health checks, error cases

## Common Pitfalls

- Forgetting to add new handlers to the `OpenApi` paths list in `main.rs`
- Forgetting to invalidate cache after write operations
- Using `pools.write` for reads (always use `pools.read` for queries)
- Not wrapping repository test data in transactions (causes test pollution)
- Missing `pub` on handler functions (required by utoipa)
- Dockerfiles use `rustlang/rust:nightly` — ensure `edition = "2024"` compatibility
- Kafka consumer/producer setup requires Kafka running (tests gracefully skip Kafka)
- Auth service uses port 5433 for PostgreSQL, user service uses 5432
- Always import from `shared::` crate, never duplicate db/cache/errors/middleware code
