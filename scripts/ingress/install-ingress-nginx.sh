#!/usr/bin/env bash
set -euo pipefail

# Install/upgrade ingress-nginx controller for selected environment.
#
# Usage:
#   ./scripts/ingress/install-ingress-nginx.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   INGRESS_NAMESPACE=ingress-nginx
#   INGRESS_RELEASE=ingress-nginx
#   VALUES_FILE=<custom-values.yaml>
#   OCI_LB_SUBNET_OCID=<oci-subnet-ocid>
#   DRY_RUN=0

ENVIRONMENT="${1:-dev}"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT_DIR="${ROOT_DIR}/scripts/ingress"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
INGRESS_NAMESPACE="${INGRESS_NAMESPACE:-ingress-nginx}"
INGRESS_RELEASE="${INGRESS_RELEASE:-ingress-nginx}"
VALUES_FILE="${VALUES_FILE:-${SCRIPT_DIR}/values/${ENVIRONMENT}.yaml}"
OCI_LB_SUBNET_OCID="${OCI_LB_SUBNET_OCID:-}"
DRY_RUN="${DRY_RUN:-0}"

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

load_dev_subnet_from_tfstate() {
  if [[ -n "${OCI_LB_SUBNET_OCID}" ]]; then
    return 0
  fi
  if [[ "${ENVIRONMENT}" != "dev" ]]; then
    return 0
  fi
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi

  local tfstate="${ROOT_DIR}/terraform/dev/terraform.tfstate"
  if [[ ! -f "${tfstate}" ]]; then
    return 0
  fi

  OCI_LB_SUBNET_OCID="$(jq -r '
    .resources[]
    | select(.type == "oci_core_subnet" and .name == "public_subnet")
    | .instances[0].attributes.id // empty
  ' "${tfstate}" | head -n1)"
}

build_effective_values_file() {
  local effective_values_file
  effective_values_file="$(mktemp)"
  cp "${VALUES_FILE}" "${effective_values_file}"

  if [[ -n "${OCI_LB_SUBNET_OCID}" ]]; then
    cat >> "${effective_values_file}" <<EOF
controller:
  service:
    annotations:
      service.beta.kubernetes.io/oci-load-balancer-subnet1: "${OCI_LB_SUBNET_OCID}"
EOF
  fi

  echo "${effective_values_file}"
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

load_dev_subnet_from_tfstate
EFFECTIVE_VALUES_FILE="$(build_effective_values_file)"
trap 'rm -f "${EFFECTIVE_VALUES_FILE}"' EXIT

echo "[ingress-install] environment=${ENVIRONMENT}"
echo "[ingress-install] kubeconfig=${KUBECONFIG}"
echo "[ingress-install] namespace=${INGRESS_NAMESPACE}"
echo "[ingress-install] release=${INGRESS_RELEASE}"
echo "[ingress-install] values=${VALUES_FILE}"
if [[ -n "${OCI_LB_SUBNET_OCID}" ]]; then
  echo "[ingress-install] oci_lb_subnet_ocid=${OCI_LB_SUBNET_OCID}"
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl create namespace ${INGRESS_NAMESPACE} --dry-run=client -o yaml | SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl apply -f -"
  echo "+ helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx"
  echo "+ helm repo update"
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} helm upgrade --install ${INGRESS_RELEASE} ingress-nginx/ingress-nginx -n ${INGRESS_NAMESPACE} -f ${EFFECTIVE_VALUES_FILE}"
  exit 0
fi

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl create namespace "${INGRESS_NAMESPACE}" --dry-run=client -o yaml \
  | SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl apply -f -

helm repo add ingress-nginx https://kubernetes.github.io/ingress-nginx >/dev/null 2>&1 || true
helm repo update >/dev/null

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" helm upgrade --install \
  "${INGRESS_RELEASE}" ingress-nginx/ingress-nginx \
  -n "${INGRESS_NAMESPACE}" \
  -f "${EFFECTIVE_VALUES_FILE}"

echo "[ingress-install] Done"
