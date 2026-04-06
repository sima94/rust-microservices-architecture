#!/usr/bin/env bash
set -euo pipefail

# Apply auth-service DB migrations in Kubernetes dev environment.
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-dev-kubeconfig
#   K8S_NAMESPACE=dev
#   AUTH_DB_POD=auth-db-0
#   DB_USER=rust_user
#   AUTH_DB_NAME=auth_db
#   MIGRATIONS_TABLE=schema_migrations
#   ADOPT_EXISTING=1   # if migration-created table already exists, mark migration as applied
#   DRY_RUN=0          # set to 1 to print planned actions without executing

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIGRATIONS_DIR="${ROOT_DIR}/auth-service/migrations"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-dev-kubeconfig}"
K8S_NAMESPACE="${K8S_NAMESPACE:-dev}"
AUTH_DB_POD="${AUTH_DB_POD:-auth-db-0}"
DB_USER="${DB_USER:-rust_user}"
AUTH_DB_NAME="${AUTH_DB_NAME:-auth_db}"
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
  SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" exec "${AUTH_DB_POD}" -- \
    psql -v ON_ERROR_STOP=1 -U "${DB_USER}" -d "${AUTH_DB_NAME}" -tAc "${sql}"
}

apply_sql_file() {
  local file="$1"
  SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" exec -i "${AUTH_DB_POD}" -- \
    psql -v ON_ERROR_STOP=1 -U "${DB_USER}" -d "${AUTH_DB_NAME}" -f - < "${file}"
}

require_bin kubectl

if [[ ! -d "${MIGRATIONS_DIR}" ]]; then
  echo "Migrations directory not found: ${MIGRATIONS_DIR}" >&2
  exit 1
fi

MIGRATION_FILES=()
while IFS= read -r file; do
  MIGRATION_FILES+=("${file}")
done < <(find "${MIGRATIONS_DIR}" -mindepth 2 -maxdepth 2 -name up.sql | sort)
if [[ "${#MIGRATION_FILES[@]}" -eq 0 ]]; then
  echo "No auth migrations found in ${MIGRATIONS_DIR}" >&2
  exit 1
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "[migrate-auth-dev] DRY_RUN=1"
  echo "[migrate-auth-dev] target: pod=${AUTH_DB_POD} db=${AUTH_DB_NAME} namespace=${K8S_NAMESPACE}"
  echo "[migrate-auth-dev] would ensure table '${MIGRATIONS_TABLE}' exists"
  for file in "${MIGRATION_FILES[@]}"; do
    rel="${file#${ROOT_DIR}/}"
    echo "[migrate-auth-dev] would apply: ${rel}"
  done
  exit 0
fi

echo "[migrate-auth-dev] Ensuring migration history table exists"
kubectl_psql "CREATE TABLE IF NOT EXISTS ${MIGRATIONS_TABLE} (id TEXT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW());" >/dev/null

for file in "${MIGRATION_FILES[@]}"; do
  rel="${file#${ROOT_DIR}/}"
  escaped_id="$(sql_escape "${rel}")"
  applied="$(kubectl_psql "SELECT 1 FROM ${MIGRATIONS_TABLE} WHERE id='${escaped_id}' LIMIT 1;")"

  if [[ "${applied}" == "1" ]]; then
    echo "[migrate-auth-dev] Already applied: ${rel}"
    continue
  fi

  table_name="$(extract_first_table "${file}")"
  if [[ -n "${table_name}" ]]; then
    table_exists="$(kubectl_psql "SELECT to_regclass('public.${table_name}') IS NOT NULL;")"
    if [[ "${table_exists}" == "t" && "${ADOPT_EXISTING}" == "1" ]]; then
      echo "[migrate-auth-dev] Table '${table_name}' already exists, adopting: ${rel}"
      kubectl_psql "INSERT INTO ${MIGRATIONS_TABLE}(id) VALUES ('${escaped_id}') ON CONFLICT DO NOTHING;" >/dev/null
      continue
    fi
  fi

  echo "[migrate-auth-dev] Applying: ${rel}"
  apply_sql_file "${file}"
  kubectl_psql "INSERT INTO ${MIGRATIONS_TABLE}(id) VALUES ('${escaped_id}') ON CONFLICT DO NOTHING;" >/dev/null
done

echo "[migrate-auth-dev] Done"
