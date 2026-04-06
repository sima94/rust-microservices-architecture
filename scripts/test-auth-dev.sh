#!/usr/bin/env bash
set -euo pipefail

# Run auth-service tests against Kubernetes dev environment via kubectl port-forward.
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-dev-kubeconfig
#   K8S_NAMESPACE=dev
#   SECRETS_FILE=.secrets/dev.env
#   AUTH_DB_SERVICE=auth-db-pgbouncer-write
#   REDIS_SERVICE=redis
#   AUTH_LOCAL_DB_PORT=25432
#   LOCAL_REDIS_PORT=16379
#   DB_USER=<required>
#   DB_PASSWORD=<required>
#   AUTH_DB_NAME=auth_db
#   JWT_SECRET=<required>
#   RUN_FULL=0   # set to 1 to run full cargo test after integration tests
#   NO_PORT_FORWARD=0  # set to 1 if port-forwards are already active

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_DIR="${ROOT_DIR}/auth-service"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-dev-kubeconfig}"
K8S_NAMESPACE="${K8S_NAMESPACE:-dev}"
SECRETS_FILE="${SECRETS_FILE:-${ROOT_DIR}/.secrets/dev.env}"
AUTH_DB_SERVICE="${AUTH_DB_SERVICE:-auth-db-pgbouncer-write}"
REDIS_SERVICE="${REDIS_SERVICE:-redis}"

AUTH_LOCAL_DB_PORT="${AUTH_LOCAL_DB_PORT:-25432}"
LOCAL_REDIS_PORT="${LOCAL_REDIS_PORT:-16379}"

DB_USER="${DB_USER:-}"
DB_PASSWORD="${DB_PASSWORD:-}"
AUTH_DB_NAME="${AUTH_DB_NAME:-auth_db}"
JWT_SECRET="${JWT_SECRET:-}"

RUN_FULL="${RUN_FULL:-0}"
NO_PORT_FORWARD="${NO_PORT_FORWARD:-0}"

PF_PIDS=()

cleanup() {
  for pid in "${PF_PIDS[@]:-}"; do
    if kill -0 "${pid}" >/dev/null 2>&1; then
      kill "${pid}" >/dev/null 2>&1 || true
      wait "${pid}" 2>/dev/null || true
    fi
  done
}
trap cleanup EXIT

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

load_secrets_file() {
  if [[ ! -f "${SECRETS_FILE}" ]]; then
    return 0
  fi

  set -a
  # shellcheck disable=SC1090
  source "${SECRETS_FILE}"
  set +a

  DB_USER="${DB_USER:-}"
  DB_PASSWORD="${DB_PASSWORD:-}"
  JWT_SECRET="${JWT_SECRET:-}"
}

require_env() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "Missing required env var: ${name}" >&2
    echo "Set it directly or provide SECRETS_FILE=${SECRETS_FILE}" >&2
    exit 1
  fi
}

wait_for_port() {
  local port="$1"
  local name="$2"

  for _ in {1..30}; do
    if (echo >"/dev/tcp/127.0.0.1/${port}") >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  echo "Port-forward for ${name} on port ${port} did not become ready in time." >&2
  return 1
}

start_port_forward() {
  local service="$1"
  local local_port="$2"
  local remote_port="$3"
  local log_file="/tmp/pf-${service}-${local_port}.log"

  KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" \
    port-forward "svc/${service}" "${local_port}:${remote_port}" >"${log_file}" 2>&1 &
  local pid=$!
  PF_PIDS+=("${pid}")

  wait_for_port "${local_port}" "${service}"
}

require_bin kubectl
require_bin cargo
load_secrets_file
require_env DB_USER
require_env DB_PASSWORD
require_env JWT_SECRET

if [[ "${NO_PORT_FORWARD}" != "1" ]]; then
  echo "[auth-dev-tests] Starting port-forward for ${AUTH_DB_SERVICE} (${AUTH_LOCAL_DB_PORT}->5432)"
  start_port_forward "${AUTH_DB_SERVICE}" "${AUTH_LOCAL_DB_PORT}" "5432"

  echo "[auth-dev-tests] Starting port-forward for ${REDIS_SERVICE} (${LOCAL_REDIS_PORT}->6379)"
  start_port_forward "${REDIS_SERVICE}" "${LOCAL_REDIS_PORT}" "6379"
fi

echo "[auth-dev-tests] Running auth integration tests"
(
  cd "${SERVICE_DIR}"
  DATABASE_URL="postgres://${DB_USER}:${DB_PASSWORD}@127.0.0.1:${AUTH_LOCAL_DB_PORT}/${AUTH_DB_NAME}?sslmode=disable" \
  REDIS_URL="redis://127.0.0.1:${LOCAL_REDIS_PORT}" \
  JWT_SECRET="${JWT_SECRET}" \
  cargo test --test api_tests -- --nocapture
)

if [[ "${RUN_FULL}" == "1" ]]; then
  echo "[auth-dev-tests] Running full auth test suite"
  (
    cd "${SERVICE_DIR}"
    DATABASE_URL="postgres://${DB_USER}:${DB_PASSWORD}@127.0.0.1:${AUTH_LOCAL_DB_PORT}/${AUTH_DB_NAME}?sslmode=disable" \
    REDIS_URL="redis://127.0.0.1:${LOCAL_REDIS_PORT}" \
    JWT_SECRET="${JWT_SECRET}" \
    cargo test -- --nocapture
  )
fi

echo "[auth-dev-tests] Done"
