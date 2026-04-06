#!/usr/bin/env bash
set -euo pipefail

# Full monitoring deploy for selected environment:
# 1) install/upgrade kube-prometheus-stack
# 2) apply app ServiceMonitor + PrometheusRule
#
# Usage:
#   ./scripts/monitoring/deploy-monitoring.sh [dev|staging|prod]
#
# Optional env vars are forwarded to child scripts:
#   KUBECONFIG, MONITORING_NAMESPACE, MONITORING_RELEASE,
#   APP_NAMESPACE, USER_SERVICE_NAME, AUTH_SERVICE_NAME,
#   SCRAPE_INTERVAL, SCRAPE_TIMEOUT, VALUES_FILE, DRY_RUN

ENVIRONMENT="${1:-dev}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "[monitoring-deploy] Installing monitoring stack (${ENVIRONMENT})"
"${ROOT_DIR}/scripts/monitoring/install-kube-prometheus-stack.sh" "${ENVIRONMENT}"

echo "[monitoring-deploy] Applying app monitoring objects (${ENVIRONMENT})"
"${ROOT_DIR}/scripts/monitoring/apply-app-monitoring.sh" "${ENVIRONMENT}"

echo "[monitoring-deploy] Done"
