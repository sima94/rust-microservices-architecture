#!/usr/bin/env bash
set -euo pipefail

# Remove app monitoring objects and optionally uninstall kube-prometheus-stack.
#
# Usage:
#   ./scripts/monitoring/uninstall-monitoring.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   APP_NAMESPACE=<env>
#   MONITORING_NAMESPACE=monitoring
#   MONITORING_RELEASE=monitoring
#   REMOVE_STACK=0   # set to 1 to uninstall helm release
#   DRY_RUN=0

ENVIRONMENT="${1:-dev}"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
APP_NAMESPACE="${APP_NAMESPACE:-${ENVIRONMENT}}"
MONITORING_NAMESPACE="${MONITORING_NAMESPACE:-monitoring}"
MONITORING_RELEASE="${MONITORING_RELEASE:-monitoring}"
REMOVE_STACK="${REMOVE_STACK:-0}"
DRY_RUN="${DRY_RUN:-0}"

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

run() {
  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "+ $*"
  else
    "$@"
  fi
}

require_bin kubectl
require_bin helm

echo "[monitoring-uninstall] environment=${ENVIRONMENT}"
echo "[monitoring-uninstall] app_namespace=${APP_NAMESPACE}"
echo "[monitoring-uninstall] monitoring_namespace=${MONITORING_NAMESPACE}"

run /bin/zsh -lc "SUPPRESS_LABEL_WARNING=True KUBECONFIG='${KUBECONFIG}' kubectl -n '${APP_NAMESPACE}' delete servicemonitor '${APP_NAMESPACE}-user-service' '${APP_NAMESPACE}-auth-service' --ignore-not-found"
run /bin/zsh -lc "SUPPRESS_LABEL_WARNING=True KUBECONFIG='${KUBECONFIG}' kubectl -n '${APP_NAMESPACE}' delete prometheusrule '${APP_NAMESPACE}-microservices-alerts' --ignore-not-found"

if [[ "${REMOVE_STACK}" == "1" ]]; then
  run /bin/zsh -lc "SUPPRESS_LABEL_WARNING=True KUBECONFIG='${KUBECONFIG}' helm -n '${MONITORING_NAMESPACE}' uninstall '${MONITORING_RELEASE}'"
fi

echo "[monitoring-uninstall] Done"
