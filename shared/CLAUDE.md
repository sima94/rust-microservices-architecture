# shared

Shared library crate containing common code used by all microservices.

## What this crate provides

| Module | Description |
|--------|-------------|
| `db` | Database pool types (`DbPools`, `DbPool`), initialization (`init_pools()`, `init_pool()`) |
| `cache` | Redis pool type (`RedisPool`), initialization, generic cache functions |
| `errors` | Unified `ServiceError` enum with `ResponseError` implementation |
| `health` | Generic health check handler (parameterized by service name) |
| `middleware::jwt_auth` | `Claims`, `AuthenticatedUser`, `FromRequest` implementation |
| `middleware::scopes` | `require_scope()` authorization helper |
| `test_utils` | Test helpers: `get_test_pool()`, `create_test_token()`, constants |

## Usage in services

```rust
// Cargo.toml
shared = { path = "../shared" }

// In code
use shared::db::{DbPools, init_pools};
use shared::cache::{RedisPool, init_redis, get_cached, set_cached, invalidate};
use shared::errors::ServiceError;
use shared::middleware::jwt_auth::AuthenticatedUser;
use shared::middleware::scopes::require_scope;
```

## Adding a new module

1. Create `src/new_module.rs`
2. Add `pub mod new_module;` to `src/lib.rs`
3. Add any new dependencies to `Cargo.toml`
4. Update this CLAUDE.md with the module description

## Important rules

- This crate should ONLY contain code that is used by 2+ services
- Service-specific logic stays in the service (e.g., cache key functions)
- Keep dependencies minimal — every dep here is pulled into all services
- Test utilities go in `test_utils.rs`, not scattered across modules
