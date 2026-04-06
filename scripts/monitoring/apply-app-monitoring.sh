#!/usr/bin/env bash
set -euo pipefail

# Apply ServiceMonitor + PrometheusRule for app services.
#
# Usage:
#   ./scripts/monitoring/apply-app-monitoring.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   APP_NAMESPACE=<env>
#   MONITORING_NAMESPACE=monitoring
#   USER_SERVICE_NAME=user-service
#   AUTH_SERVICE_NAME=auth-service
#   SCRAPE_INTERVAL=15s
#   SCRAPE_TIMEOUT=10s
#   TEMPLATE_FILE=.../app-observability.yaml.tpl
#   DRY_RUN=0

ENVIRONMENT="${1:-dev}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT_DIR="${ROOT_DIR}/scripts/monitoring"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
APP_NAMESPACE="${APP_NAMESPACE:-${ENVIRONMENT}}"
MONITORING_NAMESPACE="${MONITORING_NAMESPACE:-monitoring}"
USER_SERVICE_NAME="${USER_SERVICE_NAME:-user-service}"
AUTH_SERVICE_NAME="${AUTH_SERVICE_NAME:-auth-service}"
SCRAPE_INTERVAL="${SCRAPE_INTERVAL:-15s}"
SCRAPE_TIMEOUT="${SCRAPE_TIMEOUT:-10s}"
TEMPLATE_FILE="${TEMPLATE_FILE:-${SCRIPT_DIR}/templates/app-observability.yaml.tpl}"
DRY_RUN="${DRY_RUN:-0}"

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

escape_sed() {
  printf "%s" "$1" | sed -e 's/[\/&]/\\&/g'
}

render_template() {
  local app_ns user_svc auth_svc mon_ns scrape_interval scrape_timeout
  app_ns="$(escape_sed "${APP_NAMESPACE}")"
  user_svc="$(escape_sed "${USER_SERVICE_NAME}")"
  auth_svc="$(escape_sed "${AUTH_SERVICE_NAME}")"
  mon_ns="$(escape_sed "${MONITORING_NAMESPACE}")"
  scrape_interval="$(escape_sed "${SCRAPE_INTERVAL}")"
  scrape_timeout="$(escape_sed "${SCRAPE_TIMEOUT}")"

  sed \
    -e "s|__APP_NAMESPACE__|${app_ns}|g" \
    -e "s|__MONITORING_NAMESPACE__|${mon_ns}|g" \
    -e "s|__USER_SERVICE_NAME__|${user_svc}|g" \
    -e "s|__AUTH_SERVICE_NAME__|${auth_svc}|g" \
    -e "s|__SCRAPE_INTERVAL__|${scrape_interval}|g" \
    -e "s|__SCRAPE_TIMEOUT__|${scrape_timeout}|g" \
    "${TEMPLATE_FILE}"
}

require_bin kubectl

if [[ ! "${ENVIRONMENT}" =~ ^(dev|staging|prod)$ ]]; then
  echo "Unsupported environment: ${ENVIRONMENT} (expected: dev|staging|prod)" >&2
  exit 1
fi

if [[ ! -f "${TEMPLATE_FILE}" ]]; then
  echo "Template file not found: ${TEMPLATE_FILE}" >&2
  exit 1
fi

if [[ "${DRY_RUN}" != "1" ]]; then
  if ! SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl get crd servicemonitors.monitoring.coreos.com >/dev/null 2>&1; then
    echo "ServiceMonitor CRD is missing. Install kube-prometheus-stack first." >&2
    exit 1
  fi
fi

echo "[app-monitoring] environment=${ENVIRONMENT}"
echo "[app-monitoring] kubeconfig=${KUBECONFIG}"
echo "[app-monitoring] app_namespace=${APP_NAMESPACE}"
echo "[app-monitoring] monitoring_namespace=${MONITORING_NAMESPACE}"
echo "[app-monitoring] user_service=${USER_SERVICE_NAME}"
echo "[app-monitoring] auth_service=${AUTH_SERVICE_NAME}"

if [[ "${DRY_RUN}" == "1" ]]; then
  render_template
  exit 0
fi

render_template | SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl apply -f -

echo "[app-monitoring] Done"
