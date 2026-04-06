#!/usr/bin/env bash
set -euo pipefail

# Unified test runner for dev environment.
# Picks the right strategy based on what you need.
#
# Usage:
#   ./scripts/test-dev.sh smoke              # Quick — HTTP tests only (~5s)
#   ./scripts/test-dev.sh integration        # Port-forward + cargo test (~60s)
#   ./scripts/test-dev.sh in-cluster         # K8s Job, no port-forward (~90s)
#   ./scripts/test-dev.sh full               # All of the above
#
# Optional env vars:
#   ENVIRONMENT=dev
#   TEST_SERVICE=all|user|auth

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEVEL="${1:-smoke}"
ENVIRONMENT="${ENVIRONMENT:-dev}"
TEST_SERVICE="${TEST_SERVICE:-all}"
FAILED=0

run_step() {
  local name="$1"
  shift
  echo ""
  echo "╔═══════════════════════════════════════════════════════╗"
  echo "║  ${name}"
  echo "╚═══════════════════════════════════════════════════════╝"
  echo ""
  if "$@"; then
    echo "  → ${name}: PASSED ✓"
  else
    echo "  → ${name}: FAILED ✗"
    FAILED=$((FAILED + 1))
  fi
}

case "${LEVEL}" in
  smoke)
    run_step "Smoke Tests" "${ROOT_DIR}/scripts/smoke-test.sh" "${ENVIRONMENT}"
    ;;

  integration)
    if [[ "${TEST_SERVICE}" == "all" || "${TEST_SERVICE}" == "user" ]]; then
      run_step "User Service Integration" RUN_FULL=1 "${ROOT_DIR}/scripts/test-user-dev.sh"
    fi
    if [[ "${TEST_SERVICE}" == "all" || "${TEST_SERVICE}" == "auth" ]]; then
      run_step "Auth Service Integration" RUN_FULL=1 "${ROOT_DIR}/scripts/test-auth-dev.sh"
    fi
    ;;

  in-cluster)
    run_step "In-Cluster Tests" "${ROOT_DIR}/scripts/test-in-cluster.sh" "${ENVIRONMENT}" --service "${TEST_SERVICE}"
    ;;

  full)
    run_step "Smoke Tests"              "${ROOT_DIR}/scripts/smoke-test.sh" "${ENVIRONMENT}"
    run_step "User Service Integration" RUN_FULL=1 "${ROOT_DIR}/scripts/test-user-dev.sh"
    run_step "Auth Service Integration" RUN_FULL=1 "${ROOT_DIR}/scripts/test-auth-dev.sh"
    ;;

  *)
    echo "Usage: $0 [smoke|integration|in-cluster|full]" >&2
    exit 1
    ;;
esac

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [[ "${FAILED}" -eq 0 ]]; then
  echo "ALL TEST LEVELS PASSED ✓"
else
  echo "${FAILED} LEVEL(S) FAILED ✗"
  exit 1
fi
