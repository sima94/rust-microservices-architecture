# auth-service

OAuth2/PKCE authentication microservice. Handles user registration, client management, and token issuance.

> **TESTING IS MANDATORY.** Before writing any implementation, check "Required Tests" below.
> After every change: `cargo test`. All tests must pass. See root CLAUDE.md for full TDD rules.

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | /api/v1/auth/register | None | Register new user |
| POST | /api/v1/clients/register | None | Register OAuth client |
| POST | /api/v1/oauth/authorize | None | Authorization code grant (PKCE) |
| POST | /api/v1/oauth/token | None | Token exchange |
| POST | /api/v1/oauth/revoke | None | Token revocation |
| GET | /health | None | Health check |

## Database

- **Host**: PostgreSQL via Patroni cluster (port 5433 through PgBouncer)
- **Tables**: `auth_users`, `oauth_clients`, `authorization_codes`, `refresh_tokens`
- **Migrations**: `migrations/` directory (4 migration pairs)

## Kafka

- **Produces**: `user.events` topic
- **Events**: `user.registered` (published after successful registration)

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| ACCESS_TOKEN_TTL_SECS | 300 | Access token lifetime in seconds |
| REFRESH_TOKEN_TTL_DAYS | 7 | Refresh token lifetime in days |

## Caching

- Cache TTL: 600 seconds (10 minutes) — clients change rarely
- Keys: `oauth_client:{client_id}`

## Testing

```bash
cargo test                          # All tests
cargo test --test api_tests         # Integration tests only
cargo test --lib                    # Unit tests only
```

## Required Tests

### POST /api/v1/auth/register
- [ ] Valid email + password (8+ chars) → 201 + {id, email}
- [ ] Duplicate email → 409 Conflict
- [ ] Password < 8 characters → 400
- [ ] Missing email field → 400
- [ ] Invalid email format (no @) → 400

### POST /api/v1/clients/register
- [ ] Valid client_name + redirect_uri + scopes → 201 + {client_id, client_secret}
- [ ] Missing client_name → 400
- [ ] Invalid redirect_uri → 400

### POST /api/v1/oauth/authorize
- [ ] Valid PKCE flow (code_challenge + credentials) → 200 + {code, state}
- [ ] Wrong password → 401
- [ ] Nonexistent client_id → 400
- [ ] Missing code_challenge → 400

### POST /api/v1/oauth/token
- [ ] grant_type=authorization_code + valid code + code_verifier → 200 + tokens
- [ ] grant_type=client_credentials + valid client → 200 + access_token
- [ ] grant_type=refresh_token + valid refresh → 200 + new tokens
- [ ] Wrong client_secret → 401
- [ ] Expired authorization code → 400
- [ ] Wrong code_verifier (PKCE) → 400
- [ ] Already used authorization code → 400
- [ ] Invalid refresh token → 400

### POST /api/v1/oauth/revoke
- [ ] Valid refresh token → 200
- [ ] Nonexistent token → 200 (idempotent, must not fail)
- [ ] Wrong client_secret → 401

### GET /health
- [ ] Returns 200 + {"status": "healthy", "service": "auth-service", ...}
