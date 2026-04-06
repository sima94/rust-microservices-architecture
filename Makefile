.PHONY: up down build test fmt clippy check docker-build migrate-dev deploy-dev clean \
       smoke-dev test-dev test-dev-full test-in-cluster

# ── Development ──────────────────────────────────────────────────

up:
	docker compose up -d

down:
	docker compose down

clean:
	cargo clean
	docker compose down -v

# ── Build & Check ────────────────────────────────────────────────

build:
	cargo build --workspace

test:
	cargo test --workspace

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace -- -D warnings

check: fmt-check clippy test

# ── Docker ───────────────────────────────────────────────────────

docker-build:
	docker build -t auth-service:latest -f auth-service/Dockerfile .
	docker build -t user-service:latest -f user-service/Dockerfile .

# ── Testing against Dev Cluster ──────────────────────────────────

smoke-dev:
	./scripts/smoke-test.sh dev

test-dev:
	./scripts/test-dev.sh integration

test-dev-full:
	./scripts/test-dev.sh full

test-in-cluster:
	./scripts/test-in-cluster.sh dev

# ── Database Migrations ──────────────────────────────────────────

migrate-dev:
	./scripts/migrate-all-dev.sh

# ── Deployment ───────────────────────────────────────────────────

deploy-dev:
	./scripts/deploy-services.sh dev

deploy-staging:
	./scripts/deploy-services.sh staging

deploy-prod:
	./scripts/deploy-services.sh prod

# ── Monitoring ───────────────────────────────────────────────────

monitoring-dev:
	./scripts/monitoring/deploy-monitoring.sh dev

# ── Postman ──────────────────────────────────────────────────────

postman:
	cd postman/generator && cargo run

postman-offline:
	cd postman/generator && cargo run -- --offline
