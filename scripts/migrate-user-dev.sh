#!/usr/bin/env bash
set -euo pipefail

# Apply user-service DB migrations in Kubernetes dev environment.
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-dev-kubeconfig
#   K8S_NAMESPACE=dev
#   USER_DB_POD=user-db-0
#   DB_USER=rust_user
#   USER_DB_NAME=rust_db
#   MIGRATIONS_TABLE=schema_migrations
#   ADOPT_EXISTING=1   # if migration-created table already exists, mark migration as applied
#   DRY_RUN=0          # set to 1 to print planned actions without executing

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIGRATION_FILE="${ROOT_DIR}/user-service/migrations/users/up.sql"
MIGRATION_ID="user-service/migrations/users/up.sql"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-dev-kubeconfig}"
K8S_NAMESPACE="${K8S_NAMESPACE:-dev}"
USER_DB_POD="${USER_DB_POD:-user-db-0}"
DB_USER="${DB_USER:-rust_user}"
USER_DB_NAME="${USER_DB_NAME:-rust_db}"
MIGRATIONS_TABLE="${MIGRATIONS_TABLE:-schema_migrations}"
ADOPT_EXISTING="${ADOPT_EXISTING:-1}"
DRY_RUN="${DRY_RUN:-0}"

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

sql_escape() {
  printf "%s" "$1" | sed "s/'/''/g"
}

extract_first_table() {
  local file="$1"
  local table
  table="$(grep -Eio 'CREATE[[:space:]]+TABLE[[:space:]]+\"?[a-zA-Z_][a-zA-Z0-9_]*\"?' "$file" | head -n1 | awk '{print $3}' | tr -d '"')"
  if [[ "$table" =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
    printf "%s" "$table"
  fi
}

kubectl_psql() {
  local sql="$1"
  SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" exec "${USER_DB_POD}" -- \
    psql -v ON_ERROR_STOP=1 -U "${DB_USER}" -d "${USER_DB_NAME}" -tAc "${sql}"
}

apply_sql_file() {
  local file="$1"
  SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" exec -i "${USER_DB_POD}" -- \
    psql -v ON_ERROR_STOP=1 -U "${DB_USER}" -d "${USER_DB_NAME}" -f - < "${file}"
}

require_bin kubectl

if [[ ! -f "${MIGRATION_FILE}" ]]; then
  echo "Migration file not found: ${MIGRATION_FILE}" >&2
  exit 1
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "[migrate-user-dev] DRY_RUN=1"
  echo "[migrate-user-dev] target: pod=${USER_DB_POD} db=${USER_DB_NAME} namespace=${K8S_NAMESPACE}"
  echo "[migrate-user-dev] would ensure table '${MIGRATIONS_TABLE}' exists"
  echo "[migrate-user-dev] would apply: ${MIGRATION_ID}"
  exit 0
fi

echo "[migrate-user-dev] Ensuring migration history table exists"
kubectl_psql "CREATE TABLE IF NOT EXISTS ${MIGRATIONS_TABLE} (id TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW());" >/dev/null

escaped_id="$(sql_escape "${MIGRATION_ID}")"
applied="$(kubectl_psql "SELECT 1 FROM ${MIGRATIONS_TABLE} WHERE id='${escaped_id}' LIMIT 1;")"

if [[ "${applied}" == "1" ]]; then
  echo "[migrate-user-dev] Already applied: ${MIGRATION_ID}"
  exit 0
fi

table_name="$(extract_first_table "${MIGRATION_FILE}")"
if [[ -n "${table_name}" ]]; then
  table_exists="$(kubectl_psql "SELECT to_regclass('public.${table_name}') IS NOT NULL;")"
  if [[ "${table_exists}" == "t" && "${ADOPT_EXISTING}" == "1" ]]; then
    echo "[migrate-user-dev] Table '${table_name}' already exists, adopting existing migration state"
    kubectl_psql "INSERT INTO ${MIGRATIONS_TABLE}(id) VALUES ('${escaped_id}') ON CONFLICT DO NOTHING;" >/dev/null
    echo "[migrate-user-dev] Adopted: ${MIGRATION_ID}"
    exit 0
  fi
fi

echo "[migrate-user-dev] Applying: ${MIGRATION_ID}"
apply_sql_file "${MIGRATION_FILE}"
kubectl_psql "INSERT INTO ${MIGRATIONS_TABLE}(id) VALUES ('${escaped_id}') ON CONFLICT DO NOTHING;" >/dev/null

echo "[migrate-user-dev] Done"
