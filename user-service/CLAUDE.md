# user-service

User CRUD microservice. Manages user profiles with Redis caching and read/write database splitting.

> **TESTING IS MANDATORY.** Before writing any implementation, check "Required Tests" below.
> After every change: `cargo test`. All tests must pass. See root CLAUDE.md for full TDD rules.

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /api/v1/users | Bearer (scope: read) | List all users |
| GET | /api/v1/users/{id} | Bearer (scope: read) | Get user by ID |
| POST | /api/v1/users | Bearer (scope: write) | Create user |
| PUT | /api/v1/users/{id} | Bearer (scope: write) | Update user |
| DELETE | /api/v1/users/{id} | Bearer (scope: write) | Delete user |
| GET | /health | None | Health check |

## Database

- **Host**: PostgreSQL via Patroni cluster (port 5432 through PgBouncer)
- **Tables**: `users` (id, name, email, created_at, updated_at)
- **Migrations**: `migrations/` directory

## Kafka

- **Consumes**: `user.events` topic (group: `user-service-group`)
- **Handles**: `user.registered` events from auth-service → auto-creates user profile

## Caching

- Cache TTL: 300 seconds (5 minutes)
- Keys: `user:{id}`, `users:list`
- Invalidation: on create, update, delete operations

## Testing

```bash
cargo test                          # All tests
cargo test --test api_tests         # Integration tests only
cargo test --lib                    # Unit tests only
cargo test test_create_user         # Specific test
```

## Required Tests

### GET /api/v1/users
- [ ] With valid token (scope: read) → 200 + [User]
- [ ] Without token → 401
- [ ] Token without "read" scope → 403

### GET /api/v1/users/{id}
- [ ] Existing user → 200 + User
- [ ] Nonexistent id → 404
- [ ] Without token → 401

### POST /api/v1/users
- [ ] Valid name + email → 201 + User
- [ ] Duplicate email → 409 Conflict
- [ ] Missing name → 400
- [ ] Without token → 401
- [ ] Token without "write" scope → 403

### PUT /api/v1/users/{id}
- [ ] Valid update → 200 + Updated User
- [ ] Nonexistent id → 404
- [ ] Without token → 401

### DELETE /api/v1/users/{id}
- [ ] Existing user → 204 No Content
- [ ] Nonexistent id → 404
- [ ] Without token → 401

### GET /health
- [ ] Returns 200 + {"status": "healthy", "service": "user-service", ...}
