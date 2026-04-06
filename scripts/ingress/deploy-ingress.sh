#!/usr/bin/env bash
set -euo pipefail

# Full ingress deploy for selected environment:
# 1) optionally switch app services to ClusterIP
# 2) install/upgrade ingress-nginx controller
# 3) apply shared app ingress routes
#
# Usage:
#   ./scripts/ingress/deploy-ingress.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   SECRETS_FILE=.secrets/<env>.env
#   SET_SERVICES_CLUSTERIP=true|false (default: true)
#   WAIT_FOR_INGRESS_IP=true|false (default: true)
#   INGRESS_NAMESPACE=ingress-nginx
#   INGRESS_SERVICE_NAME=ingress-nginx-controller
#   LB_WAIT_TIMEOUT_SEC=600
#   DRY_RUN=0
#
# Additional env vars are forwarded to child scripts:
#   OCI_LB_SUBNET_OCID, VALUES_FILE, APP_NAMESPACE, INGRESS_NAME,
#   INGRESS_CLASS, INGRESS_HOST, TLS_SECRET_NAME, AUTH_SERVICE_NAME,
#   USER_SERVICE_NAME, AUTH_SERVICE_PORT, USER_SERVICE_PORT, SECRETS_FILE

ENVIRONMENT="${1:-dev}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
SET_SERVICES_CLUSTERIP="${SET_SERVICES_CLUSTERIP:-true}"
WAIT_FOR_INGRESS_IP="${WAIT_FOR_INGRESS_IP:-true}"
INGRESS_NAMESPACE="${INGRESS_NAMESPACE:-ingress-nginx}"
INGRESS_SERVICE_NAME="${INGRESS_SERVICE_NAME:-ingress-nginx-controller}"
LB_WAIT_TIMEOUT_SEC="${LB_WAIT_TIMEOUT_SEC:-600}"
DRY_RUN="${DRY_RUN:-0}"

wait_for_ingress_ip() {
  local deadline=$((SECONDS + LB_WAIT_TIMEOUT_SEC))
  local ip=""
  local host=""
  local describe_output=""

  while (( SECONDS < deadline )); do
    ip="$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${INGRESS_NAMESPACE}" get svc "${INGRESS_SERVICE_NAME}" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || true)"
    host="$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${INGRESS_NAMESPACE}" get svc "${INGRESS_SERVICE_NAME}" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null || true)"

    if [[ -n "${ip}" || -n "${host}" ]]; then
      echo "[ingress-deploy] ingress endpoint: ${ip:-${host}}"
      return 0
    fi

    describe_output="$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${INGRESS_NAMESPACE}" describe svc "${INGRESS_SERVICE_NAME}" 2>/dev/null || true)"
    if echo "${describe_output}" | grep -q "Error Code: LimitExceeded"; then
      echo "[ingress-deploy] failed to get ingress endpoint: OCI LB limit exceeded (lb-100mbps-count)" >&2
      return 1
    fi

    sleep 10
  done

  echo "[ingress-deploy] timed out waiting for ingress endpoint on ${INGRESS_SERVICE_NAME}" >&2
  return 1
}

if [[ ! "${ENVIRONMENT}" =~ ^(dev|staging|prod)$ ]]; then
  echo "Unsupported environment: ${ENVIRONMENT} (expected: dev|staging|prod)" >&2
  exit 1
fi

echo "[ingress-deploy] environment=${ENVIRONMENT}"
echo "[ingress-deploy] kubeconfig=${KUBECONFIG}"

if [[ "${SET_SERVICES_CLUSTERIP}" == "true" ]]; then
  echo "[ingress-deploy] Switching auth/user services to ClusterIP"
  EXPOSE_PUBLIC=false WAIT_FOR_EXTERNAL_IP=false KUBECONFIG="${KUBECONFIG}" DRY_RUN="${DRY_RUN}" \
    "${ROOT_DIR}/scripts/deploy-services.sh" "${ENVIRONMENT}"
fi

echo "[ingress-deploy] Installing ingress-nginx"
KUBECONFIG="${KUBECONFIG}" DRY_RUN="${DRY_RUN}" \
  "${ROOT_DIR}/scripts/ingress/install-ingress-nginx.sh" "${ENVIRONMENT}"

echo "[ingress-deploy] Applying app ingress routes"
KUBECONFIG="${KUBECONFIG}" DRY_RUN="${DRY_RUN}" \
  "${ROOT_DIR}/scripts/ingress/apply-app-ingress.sh" "${ENVIRONMENT}"

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl -n ${INGRESS_NAMESPACE} get svc ${INGRESS_SERVICE_NAME} -o wide"
  exit 0
fi

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${INGRESS_NAMESPACE}" get svc "${INGRESS_SERVICE_NAME}" -o wide

if [[ "${WAIT_FOR_INGRESS_IP}" == "true" ]]; then
  wait_for_ingress_ip
fi

echo "[ingress-deploy] Done"
