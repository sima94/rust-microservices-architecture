#!/usr/bin/env bash
set -euo pipefail

# Install/upgrade kube-prometheus-stack for selected environment.
#
# Usage:
#   ./scripts/monitoring/install-kube-prometheus-stack.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   MONITORING_NAMESPACE=monitoring
#   MONITORING_RELEASE=monitoring
#   VALUES_FILE=<custom-values.yaml>
#   DRY_RUN=0

ENVIRONMENT="${1:-dev}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT_DIR="${ROOT_DIR}/scripts/monitoring"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
MONITORING_NAMESPACE="${MONITORING_NAMESPACE:-monitoring}"
MONITORING_RELEASE="${MONITORING_RELEASE:-monitoring}"
VALUES_FILE="${VALUES_FILE:-${SCRIPT_DIR}/values/${ENVIRONMENT}.yaml}"
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

if [[ ! "${ENVIRONMENT}" =~ ^(dev|staging|prod)$ ]]; then
  echo "Unsupported environment: ${ENVIRONMENT} (expected: dev|staging|prod)" >&2
  exit 1
fi

if [[ ! -f "${VALUES_FILE}" ]]; then
  echo "Values file not found: ${VALUES_FILE}" >&2
  exit 1
fi

require_bin kubectl
require_bin helm

echo "[monitoring-install] environment=${ENVIRONMENT}"
echo "[monitoring-install] kubeconfig=${KUBECONFIG}"
echo "[monitoring-install] namespace=${MONITORING_NAMESPACE}"
echo "[monitoring-install] release=${MONITORING_RELEASE}"
echo "[monitoring-install] values=${VALUES_FILE}"

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl create namespace ${MONITORING_NAMESPACE} --dry-run=client -o yaml | SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl apply -f -"
  echo "+ helm repo add prometheus-community https://prometheus-community.github.io/helm-charts"
  echo "+ helm repo update"
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} helm upgrade --install ${MONITORING_RELEASE} prometheus-community/kube-prometheus-stack -n ${MONITORING_NAMESPACE} -f ${VALUES_FILE}"
  exit 0
fi

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl create namespace "${MONITORING_NAMESPACE}" --dry-run=client -o yaml \
  | SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl apply -f -

helm repo add prometheus-community https://prometheus-community.github.io/helm-charts >/dev/null 2>&1 || true
helm repo update >/dev/null

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" helm upgrade --install \
  "${MONITORING_RELEASE}" prometheus-community/kube-prometheus-stack \
  -n "${MONITORING_NAMESPACE}" \
  -f "${VALUES_FILE}"

echo "[monitoring-install] Done"
