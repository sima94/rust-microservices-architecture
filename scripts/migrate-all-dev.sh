#!/usr/bin/env bash
set -euo pipefail

# Apply DB migrations for both services in dev environment.
#
# Optional env vars are forwarded to child scripts:
#   KUBECONFIG, K8S_NAMESPACE, DB_USER, MIGRATIONS_TABLE, ADOPT_EXISTING, DRY_RUN

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[migrate-all-dev] Running user-service migrations"
"${ROOT_DIR}/scripts/migrate-user-dev.sh"

echo "[migrate-all-dev] Running auth-service migrations"
"${ROOT_DIR}/scripts/migrate-auth-dev.sh"

echo "[migrate-all-dev] Done"
