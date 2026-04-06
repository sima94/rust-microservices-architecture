# docker/

Infrastructure configs for PostgreSQL HA (Patroni), load balancing (HAProxy), and connection pooling (PgBouncer).

## Architecture Overview

```
Client → PgBouncer → HAProxy → Patroni (3-node PostgreSQL cluster) ← etcd (consensus)
```

Each service has its own isolated database cluster:

| Component | Auth cluster | User cluster |
|-----------|-------------|-------------|
| Patroni nodes | patroni-auth-1/2/3 | patroni-user-1/2/3 |
| HAProxy write | haproxy-auth-write | haproxy-user-write |
| HAProxy read | haproxy-auth-read | haproxy-user-read |
| PgBouncer write | pgbouncer-auth-write (port 5433) | pgbouncer-user-write (port 5432) |
| PgBouncer read | pgbouncer-auth-read (port 5433) | pgbouncer-user-read (port 5432) |
| Database | `auth_db` | `rust_db` |
| DB user | `rust_user` | `rust_user` |

## Patroni (HA PostgreSQL)

### How it works
- 3 PostgreSQL nodes per cluster, managed by Patroni
- etcd provides distributed consensus for leader election
- One node is **master** (read-write), others are **replicas** (read-only, streaming replication)
- Automatic failover: if master dies, a replica is promoted within ~30 seconds

### Key settings (from entrypoint.sh)
- **TTL**: 30s — if leader doesn't renew within 30s, failover starts
- **Loop wait**: 10s — Patroni checks cluster state every 10s
- **Max replication lag**: 1MB — replica won't be promoted if too far behind
- **WAL**: `wal_level=replica`, 5 max senders, 5 replication slots, hot_standby=on

### Patroni REST API (port 8008)
Used by HAProxy for health checks:
- `GET /master` → 200 if node is current master
- `GET /replica` → 200 if node is a healthy replica
- `GET /health` → general node health
- `GET /cluster` → full cluster state (all members, roles, lag)

### Post-init scripts
- `post-init-user.sh` → creates `rust_user` + `rust_db`
- `post-init-auth.sh` → creates `rust_user` + `auth_db`
- Selected via `POST_INIT_SCRIPT` env var in docker-compose.yml
- Runs only on initial cluster bootstrap (not on restarts)

### Credentials (dev only)
- Superuser: `postgres` / `postgres`
- Replication: `replicator` / `replicator_pass`
- App user: `rust_user` / `rust_pass` (set via PgBouncer AUTH_TYPE=trust)

## HAProxy (Read/Write Splitting)

### Write configs (`*-write.cfg`)
- Routes to **master only** (checks `GET /master` on port 8008)
- `on-marked-down shutdown-sessions` — kills connections immediately if master goes down (prevents stale writes)
- No load balancing — only one master at a time

### Read configs (`*-read.cfg`)
- Routes to **replicas** via roundrobin (checks `GET /replica` on port 8008)
- All 3 nodes are backends — master can also serve reads if it responds to `/replica`
- Simple roundrobin, no session affinity

### Health check timing
- `inter 3s` — check every 3 seconds
- `fall 3` — mark down after 3 consecutive failures (~9s)
- `rise 2` — mark up after 2 consecutive successes (~6s)

## PgBouncer (Connection Pooling)

Configured in docker-compose.yml (not in docker/ dir):
- `POOL_MODE: session` — one DB connection per client session
- `MAX_CLIENT_CONN: 200` — max client connections
- `DEFAULT_POOL_SIZE: 20` — connections to PostgreSQL per pool
- `AUTH_TYPE: trust` — no password verification (dev mode)

## Common Operations

### Check cluster health
```bash
# Patroni cluster state (shows master/replica roles)
docker exec rust-microservices-architecture-patroni-user-1-1 \
  curl -s http://localhost:8008/cluster | python3 -m json.tool

# Same for auth cluster
docker exec rust-microservices-architecture-patroni-auth-1-1 \
  curl -s http://localhost:8008/cluster | python3 -m json.tool
```

### Simulate failover
```bash
# Stop current master (Patroni auto-promotes a replica)
docker compose stop patroni-user-1

# Watch failover happen (~30s)
docker compose logs -f patroni-user-2 patroni-user-3

# Restart old master (rejoins as replica)
docker compose start patroni-user-1
```

### Reset database (fresh start)
```bash
docker compose down
docker volume rm rust-microservices-architecture_patroni-user-1-data \
  rust-microservices-architecture_patroni-user-2-data \
  rust-microservices-architecture_patroni-user-3-data
docker compose up -d
```

## File Reference

| File | Purpose |
|------|---------|
| `patroni/Dockerfile` | PostgreSQL 15 + Patroni + etcd3 client |
| `patroni/entrypoint.sh` | Generates patroni.yml dynamically from env vars |
| `patroni/post-init-user.sh` | Creates rust_user + rust_db |
| `patroni/post-init-auth.sh` | Creates rust_user + auth_db |
| `haproxy/auth-write.cfg` | Routes writes to auth master |
| `haproxy/auth-read.cfg` | Routes reads to auth replicas (roundrobin) |
| `haproxy/user-write.cfg` | Routes writes to user master |
| `haproxy/user-read.cfg` | Routes reads to user replicas (roundrobin) |
| `postgres/init-master.sh` | Legacy — creates replication user (Patroni handles this now) |
| `postgres/replica-entrypoint.sh` | Legacy — pg_basebackup replica setup (Patroni handles this now) |
