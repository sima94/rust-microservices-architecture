#!/usr/bin/env bash
set -euo pipefail

# Deploy auth-service and user-service Helm releases for selected environment.
#
# Usage:
#   ./scripts/deploy-services.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG             Default: /tmp/oke-<env>-kubeconfig
#   K8S_NAMESPACE          Default: <env>
#   EXPOSE_PUBLIC          true|false (default: true for dev/staging, false for prod)
#   SECRETS_FILE           Default: .secrets/<env>.env
#   AUTH_DATABASE_URL      Required (env or secrets file)
#   AUTH_DATABASE_READ_URL Required (env or secrets file)
#   USER_DATABASE_URL      Required (env or secrets file)
#   USER_DATABASE_READ_URL Required (env or secrets file)
#   JWT_SHARED_SECRET      Required unless AUTH_JWT_SECRET and USER_JWT_SECRET are both set
#   OCI_LB_SUBNET_OCID     OCI public subnet OCID for LoadBalancer services
#   LB_WAIT_TIMEOUT_SEC    Wait timeout for external IP (default: 600)
#   WAIT_FOR_EXTERNAL_IP   true|false (default: true)
#   DOCKER_REGISTRY        Image registry prefix (e.g. ghcr.io/owner). When set,
#                          overrides microservice.image.repository via --set.
#   IMAGE_TAG              Image tag to deploy. When set, overrides
#                          microservice.image.tag via --set.
#   IMAGE_PULL_SECRET      Name of an existing kubernetes.io/dockerconfigjson
#                          secret in the target namespace used to pull images
#                          (e.g. ghcr-pull for private GHCR packages).
#   DRY_RUN                1 -> print commands only

ENVIRONMENT="${1:-dev}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
K8S_NAMESPACE="${K8S_NAMESPACE:-${ENVIRONMENT}}"
EXPOSE_PUBLIC="${EXPOSE_PUBLIC:-}"
SECRETS_FILE="${SECRETS_FILE:-${ROOT_DIR}/.secrets/${ENVIRONMENT}.env}"
OCI_LB_SUBNET_OCID="${OCI_LB_SUBNET_OCID:-}"
LB_WAIT_TIMEOUT_SEC="${LB_WAIT_TIMEOUT_SEC:-600}"
WAIT_FOR_EXTERNAL_IP="${WAIT_FOR_EXTERNAL_IP:-true}"
DOCKER_REGISTRY="${DOCKER_REGISTRY:-}"
IMAGE_TAG="${IMAGE_TAG:-}"
IMAGE_PULL_SECRET="${IMAGE_PULL_SECRET:-}"
DRY_RUN="${DRY_RUN:-0}"

AUTH_CHART_DIR="${ROOT_DIR}/auth-service/chart"
USER_CHART_DIR="${ROOT_DIR}/user-service/chart"
AUTH_DATABASE_URL="${AUTH_DATABASE_URL:-}"
AUTH_DATABASE_READ_URL="${AUTH_DATABASE_READ_URL:-}"
USER_DATABASE_URL="${USER_DATABASE_URL:-}"
USER_DATABASE_READ_URL="${USER_DATABASE_READ_URL:-}"
JWT_SHARED_SECRET="${JWT_SHARED_SECRET:-}"
AUTH_JWT_SECRET="${AUTH_JWT_SECRET:-}"
USER_JWT_SECRET="${USER_JWT_SECRET:-}"

if [[ -z "${EXPOSE_PUBLIC}" ]]; then
  if [[ "${ENVIRONMENT}" == "prod" ]]; then
    EXPOSE_PUBLIC="false"
  else
    EXPOSE_PUBLIC="true"
  fi
fi

require_cmd() {
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

  AUTH_DATABASE_URL="${AUTH_DATABASE_URL:-}"
  AUTH_DATABASE_READ_URL="${AUTH_DATABASE_READ_URL:-}"
  USER_DATABASE_URL="${USER_DATABASE_URL:-}"
  USER_DATABASE_READ_URL="${USER_DATABASE_READ_URL:-}"
  JWT_SHARED_SECRET="${JWT_SHARED_SECRET:-}"
  AUTH_JWT_SECRET="${AUTH_JWT_SECRET:-}"
  USER_JWT_SECRET="${USER_JWT_SECRET:-}"
}

validate_required_secrets() {
  local missing=()

  [[ -n "${AUTH_DATABASE_URL}" ]] || missing+=("AUTH_DATABASE_URL")
  [[ -n "${AUTH_DATABASE_READ_URL}" ]] || missing+=("AUTH_DATABASE_READ_URL")
  [[ -n "${USER_DATABASE_URL}" ]] || missing+=("USER_DATABASE_URL")
  [[ -n "${USER_DATABASE_READ_URL}" ]] || missing+=("USER_DATABASE_READ_URL")

  if [[ -z "${JWT_SHARED_SECRET}" && -z "${AUTH_JWT_SECRET}" ]]; then
    missing+=("JWT_SHARED_SECRET or AUTH_JWT_SECRET")
  fi
  if [[ -z "${JWT_SHARED_SECRET}" && -z "${USER_JWT_SECRET}" ]]; then
    missing+=("JWT_SHARED_SECRET or USER_JWT_SECRET")
  fi

  if [[ "${#missing[@]}" -gt 0 ]]; then
    echo "Missing required secrets for deploy-services:" >&2
    for item in "${missing[@]}"; do
      echo "  - ${item}" >&2
    done
    echo "Set env vars directly or provide SECRETS_FILE=${SECRETS_FILE}" >&2
    exit 1
  fi
}

yaml_escape() {
  printf "%s" "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g'
}

load_dev_subnet_from_tfstate() {
  if [[ -n "${OCI_LB_SUBNET_OCID}" ]]; then
    return 0
  fi
  if [[ "${ENVIRONMENT}" != "dev" ]]; then
    return 0
  fi

  local tfstate="${ROOT_DIR}/terraform/dev/terraform.tfstate"
  if [[ ! -f "${tfstate}" ]]; then
    return 0
  fi
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi

  OCI_LB_SUBNET_OCID="$(jq -r '
    .resources[]
    | select(.type == "oci_core_subnet" and .name == "public_subnet")
    | .instances[0].attributes.id // empty
  ' "${tfstate}" | head -n1)"
}

make_override_values_file() {
  local override_file
  override_file="$(mktemp)"

  {
    echo "microservice:"
    echo "  service:"
    if [[ "${EXPOSE_PUBLIC}" == "true" ]]; then
      echo "    type: LoadBalancer"
      if [[ -n "${OCI_LB_SUBNET_OCID}" ]]; then
        echo "    annotations:"
        echo "      service.beta.kubernetes.io/oci-load-balancer-subnet1: \"${OCI_LB_SUBNET_OCID}\""
      fi
    else
      echo "    type: ClusterIP"
    fi
  } > "${override_file}"

  echo "${override_file}"
}

make_auth_secrets_values_file() {
  local file="$1"
  local auth_db auth_read auth_jwt
  auth_db="$(yaml_escape "${AUTH_DATABASE_URL}")"
  auth_read="$(yaml_escape "${AUTH_DATABASE_READ_URL}")"
  auth_jwt="$(yaml_escape "${AUTH_JWT_SECRET:-${JWT_SHARED_SECRET}}")"

  cat > "${file}" <<EOF
microservice:
  secrets:
    DATABASE_URL: "${auth_db}"
    DATABASE_READ_URL: "${auth_read}"
    JWT_SECRET: "${auth_jwt}"
EOF
}

make_user_secrets_values_file() {
  local file="$1"
  local user_db user_read user_jwt
  user_db="$(yaml_escape "${USER_DATABASE_URL}")"
  user_read="$(yaml_escape "${USER_DATABASE_READ_URL}")"
  user_jwt="$(yaml_escape "${USER_JWT_SECRET:-${JWT_SHARED_SECRET}}")"

  cat > "${file}" <<EOF
microservice:
  secrets:
    DATABASE_URL: "${user_db}"
    DATABASE_READ_URL: "${user_read}"
    JWT_SECRET: "${user_jwt}"
EOF
}

deploy_release() {
  local release_name="$1"
  local chart_dir="$2"
  local override_file="$3"
  local secrets_file="$4"
  local image_repo="${5:-}"
  local image_tag="${6:-}"
  local image_pull_secret="${7:-}"
  local env_values_file="${chart_dir}/values-${ENVIRONMENT}.yaml"
  local -a values_args=("-f" "${chart_dir}/values.yaml")
  local -a set_args=()

  if [[ -f "${env_values_file}" ]]; then
    values_args+=("-f" "${env_values_file}")
  fi
  values_args+=("-f" "${override_file}")
  values_args+=("-f" "${secrets_file}")

  if [[ -n "${image_repo}" ]]; then
    set_args+=("--set" "microservice.image.repository=${image_repo}")
  fi
  if [[ -n "${image_tag}" ]]; then
    set_args+=("--set" "microservice.image.tag=${image_tag}")
  fi
  if [[ -n "${image_pull_secret}" ]]; then
    set_args+=("--set" "microservice.imagePullSecrets[0].name=${image_pull_secret}")
  fi

  if [[ "${DRY_RUN}" == "1" ]]; then
    echo "+ helm dependency update ${chart_dir}"
    echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} helm upgrade --install ${release_name} ${chart_dir} -n ${K8S_NAMESPACE} --create-namespace ${values_args[*]} ${set_args[*]:-}"
    return 0
  fi

  helm dependency update "${chart_dir}" >/dev/null
  SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" helm upgrade --install \
    "${release_name}" "${chart_dir}" \
    -n "${K8S_NAMESPACE}" \
    --create-namespace \
    "${values_args[@]}" \
    ${set_args[@]+"${set_args[@]}"}
}

wait_for_external_ip() {
  local service_name="$1"
  local deadline=$((SECONDS + LB_WAIT_TIMEOUT_SEC))
  local ip=""
  local host=""
  local describe_output=""

  while (( SECONDS < deadline )); do
    ip="$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" get svc "${service_name}" -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || true)"
    host="$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" get svc "${service_name}" -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null || true)"

    if [[ -n "${ip}" || -n "${host}" ]]; then
      echo "[deploy-services] ${service_name} external endpoint: ${ip:-${host}}"
      return 0
    fi

    describe_output="$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" describe svc "${service_name}" 2>/dev/null || true)"
    if echo "${describe_output}" | grep -q "Error Code: LimitExceeded"; then
      echo "[deploy-services] ${service_name} failed to get external endpoint: OCI LB limit exceeded (lb-100mbps-count)" >&2
      echo "[deploy-services] Request OCI limit increase or switch to shared Ingress (single LB)." >&2
      return 1
    fi

    sleep 10
  done

  echo "[deploy-services] Timed out waiting for external endpoint on ${service_name}" >&2
  return 1
}

case "${ENVIRONMENT}" in
  dev|staging|prod) ;;
  *)
    echo "Unsupported environment: ${ENVIRONMENT} (expected: dev|staging|prod)" >&2
    exit 1
    ;;
esac

require_cmd helm
require_cmd kubectl
load_secrets_file
validate_required_secrets

if [[ "${EXPOSE_PUBLIC}" == "true" ]]; then
  load_dev_subnet_from_tfstate
fi

OVERRIDE_VALUES_FILE="$(make_override_values_file)"
AUTH_SECRETS_VALUES_FILE="$(mktemp)"
USER_SECRETS_VALUES_FILE="$(mktemp)"
make_auth_secrets_values_file "${AUTH_SECRETS_VALUES_FILE}"
make_user_secrets_values_file "${USER_SECRETS_VALUES_FILE}"
trap 'rm -f "${OVERRIDE_VALUES_FILE}" "${AUTH_SECRETS_VALUES_FILE}" "${USER_SECRETS_VALUES_FILE}"' EXIT

echo "[deploy-services] env=${ENVIRONMENT} namespace=${K8S_NAMESPACE} expose_public=${EXPOSE_PUBLIC}"
if [[ "${EXPOSE_PUBLIC}" == "true" ]]; then
  if [[ -n "${OCI_LB_SUBNET_OCID}" ]]; then
    echo "[deploy-services] Using OCI LB subnet: ${OCI_LB_SUBNET_OCID}"
  else
    echo "[deploy-services] Warning: OCI_LB_SUBNET_OCID is not set (LB may stay pending on OKE)" >&2
  fi
fi

AUTH_IMAGE_REPO=""
USER_IMAGE_REPO=""
if [[ -n "${DOCKER_REGISTRY}" ]]; then
  AUTH_IMAGE_REPO="${DOCKER_REGISTRY}/auth-service"
  USER_IMAGE_REPO="${DOCKER_REGISTRY}/user-service"
fi

deploy_release "auth-service" "${AUTH_CHART_DIR}" "${OVERRIDE_VALUES_FILE}" "${AUTH_SECRETS_VALUES_FILE}" "${AUTH_IMAGE_REPO}" "${IMAGE_TAG}" "${IMAGE_PULL_SECRET}"
deploy_release "user-service" "${USER_CHART_DIR}" "${OVERRIDE_VALUES_FILE}" "${USER_SECRETS_VALUES_FILE}" "${USER_IMAGE_REPO}" "${IMAGE_TAG}" "${IMAGE_PULL_SECRET}"

if [[ "${DRY_RUN}" == "1" ]]; then
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl -n ${K8S_NAMESPACE} rollout status deployment/auth-service --timeout=180s"
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl -n ${K8S_NAMESPACE} rollout status deployment/user-service --timeout=180s"
  echo "+ SUPPRESS_LABEL_WARNING=True KUBECONFIG=${KUBECONFIG} kubectl -n ${K8S_NAMESPACE} get svc auth-service user-service -o wide"
  exit 0
fi

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" rollout status deployment/auth-service --timeout=180s
SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" rollout status deployment/user-service --timeout=180s
SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl -n "${K8S_NAMESPACE}" get svc auth-service user-service -o wide

if [[ "${EXPOSE_PUBLIC}" == "true" && "${WAIT_FOR_EXTERNAL_IP}" == "true" ]]; then
  wait_for_external_ip "auth-service"
  wait_for_external_ip "user-service"
fi

echo "[deploy-services] Done"
