#!/usr/bin/env bash
set -euo pipefail

# Run integration tests as a K8s Job inside the cluster.
# No port-forwards needed — tests have direct access to DB, Redis, Kafka.
#
# Usage:
#   ./scripts/test-in-cluster.sh [dev|staging]
#   ./scripts/test-in-cluster.sh dev --service user    # only user-service tests
#   ./scripts/test-in-cluster.sh dev --service auth    # only auth-service tests
#   ./scripts/test-in-cluster.sh dev --build           # rebuild test image first
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   K8S_NAMESPACE=<env>
#   TEST_IMAGE=<registry>/test-runner:<tag>
#   SECRETS_FILE=.secrets/<env>.env

ENVIRONMENT="${1:-dev}"
shift || true

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
K8S_NAMESPACE="${K8S_NAMESPACE:-${ENVIRONMENT}}"
SECRETS_FILE="${SECRETS_FILE:-${ROOT_DIR}/.secrets/${ENVIRONMENT}.env}"
TEST_SERVICE="all"
DO_BUILD=0
REGISTRY="${DOCKER_REGISTRY:-${DOCKER_REGISTRY:-your-registry.example.com/namespace}}"
TEST_IMAGE="${TEST_IMAGE:-${REGISTRY}/test-runner:latest}"
JOB_NAME="integration-test-$(date +%s)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --service) TEST_SERVICE="$2"; shift 2 ;;
    --build)   DO_BUILD=1; shift ;;
    *)         echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

# Load secrets for DB URLs and JWT
load_secrets() {
  if [[ -f "${SECRETS_FILE}" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "${SECRETS_FILE}"
    set +a
  fi
}

load_secrets

# Resolve DB URLs from secrets or existing cluster configmaps
USER_DATABASE_URL="${USER_DATABASE_URL:-postgres://${DB_USER:-rust_user}:${DB_PASSWORD:-rust_pass}@user-db-pgbouncer-write:5432/${USER_DB_NAME:-rust_db}?sslmode=disable}"
AUTH_DATABASE_URL="${AUTH_DATABASE_URL:-postgres://${DB_USER:-rust_user}:${DB_PASSWORD:-rust_pass}@auth-db-pgbouncer-write:5432/${AUTH_DB_NAME:-auth_db}?sslmode=disable}"
REDIS_URL="${REDIS_URL:-redis://redis:6379}"
JWT_SECRET="${JWT_SECRET:-${JWT_SHARED_SECRET:-your-dev-secret-change-in-production}}"
KAFKA_BROKER="${KAFKA_BROKER:-kafka:29092}"

# ── Build (optional) ──────────────────────────────────────────

if [[ "${DO_BUILD}" == "1" ]]; then
  echo "[test-in-cluster] Building test-runner image..."
  docker build -t "${TEST_IMAGE}" -f "${ROOT_DIR}/test-runner/Dockerfile" "${ROOT_DIR}"
  echo "[test-in-cluster] Pushing ${TEST_IMAGE}..."
  docker push "${TEST_IMAGE}"
fi

# ── Create K8s Job ─────────────────────────────────────────────

echo "[test-in-cluster] Creating Job/${JOB_NAME} in ns/${K8S_NAMESPACE}"
echo "[test-in-cluster] Image: ${TEST_IMAGE}"
echo "[test-in-cluster] Service: ${TEST_SERVICE}"

# Pick correct DATABASE_URL based on which service we're testing
if [[ "${TEST_SERVICE}" == "user" ]]; then
  DATABASE_URL="${USER_DATABASE_URL}"
elif [[ "${TEST_SERVICE}" == "auth" ]]; then
  DATABASE_URL="${AUTH_DATABASE_URL}"
else
  # For "all", user-service tests run first, then auth — each needs its own DB.
  # The entrypoint handles switching; we pass both as separate env vars.
  DATABASE_URL="${USER_DATABASE_URL}"
fi

SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl apply -n "${K8S_NAMESPACE}" -f - <<EOF
apiVersion: batch/v1
kind: Job
metadata:
  name: ${JOB_NAME}
  labels:
    app: integration-test
spec:
  backoffLimit: 0
  ttlSecondsAfterFinished: 600
  template:
    metadata:
      labels:
        app: integration-test
    spec:
      restartPolicy: Never
      containers:
        - name: tests
          image: ${TEST_IMAGE}
          env:
            - name: TEST_SERVICE
              value: "${TEST_SERVICE}"
            - name: DATABASE_URL
              value: "${DATABASE_URL}"
            - name: DATABASE_READ_URL
              value: "${DATABASE_URL}"
            - name: USER_DATABASE_URL
              value: "${USER_DATABASE_URL}"
            - name: AUTH_DATABASE_URL
              value: "${AUTH_DATABASE_URL}"
            - name: REDIS_URL
              value: "${REDIS_URL}"
            - name: JWT_SECRET
              value: "${JWT_SECRET}"
            - name: KAFKA_BROKER
              value: "${KAFKA_BROKER}"
          resources:
            requests:
              cpu: "250m"
              memory: "256Mi"
            limits:
              cpu: "1"
              memory: "512Mi"
EOF

# ── Wait for completion ────────────────────────────────────────

echo "[test-in-cluster] Waiting for Job to complete..."

if ! SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
  kubectl wait -n "${K8S_NAMESPACE}" \
  --for=condition=complete --timeout=300s \
  "job/${JOB_NAME}" 2>/dev/null; then

  # Check if it failed
  FAILED=$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
    kubectl get job -n "${K8S_NAMESPACE}" "${JOB_NAME}" \
    -o jsonpath='{.status.failed}' 2>/dev/null || echo "0")

  if [[ "${FAILED}" != "0" && "${FAILED}" != "" ]]; then
    echo "[test-in-cluster] ✗ Tests FAILED"
    echo ""
    SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
      kubectl logs -n "${K8S_NAMESPACE}" "job/${JOB_NAME}" --tail=50
    exit 1
  fi

  echo "[test-in-cluster] Timed out waiting. Fetching logs..."
  SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
    kubectl logs -n "${K8S_NAMESPACE}" "job/${JOB_NAME}" --tail=100
  exit 1
fi

# ── Show results ───────────────────────────────────────────────

echo ""
SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
  kubectl logs -n "${K8S_NAMESPACE}" "job/${JOB_NAME}"

echo ""
echo "[test-in-cluster] ✓ Tests PASSED"
