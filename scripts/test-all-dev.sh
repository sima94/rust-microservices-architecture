#!/usr/bin/env bash
set -euo pipefail

# Run both user-service and auth-service tests against dev environment.
#
# Optional env vars:
#   RUN_FULL=0   # set to 1 to run full cargo test suites for both services
#   KUBECONFIG=/tmp/oke-dev-kubeconfig
#   K8S_NAMESPACE=dev
#   SECRETS_FILE=.secrets/dev.env

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_FULL="${RUN_FULL:-0}"

echo "[all-dev-tests] Running user-service tests"
RUN_FULL="${RUN_FULL}" "${ROOT_DIR}/scripts/test-user-dev.sh"

echo "[all-dev-tests] Running auth-service tests"
RUN_FULL="${RUN_FULL}" "${ROOT_DIR}/scripts/test-auth-dev.sh"

echo "[all-dev-tests] Done"
