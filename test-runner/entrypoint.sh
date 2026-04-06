#!/usr/bin/env bash
set -euo pipefail

# In-cluster test runner entrypoint.
# Runs pre-compiled test binaries with access to cluster services.
# Env vars (DATABASE_URL, REDIS_URL, JWT_SECRET, etc.) are injected by K8s Job spec.

SERVICE="${TEST_SERVICE:-all}"
FAILED=0

run_suite() {
  local name="$1"
  local binary="$2"
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "Running: ${name}"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

  if "./${binary}" --nocapture; then
    echo "✓ ${name}: PASSED"
  else
    echo "✗ ${name}: FAILED"
    FAILED=$((FAILED + 1))
  fi
}

case "${SERVICE}" in
  user)
    run_suite "user-service integration tests" "user-api-tests"
    ;;
  auth)
    run_suite "auth-service integration tests" "auth-api-tests"
    ;;
  all)
    run_suite "user-service integration tests" "user-api-tests"
    run_suite "auth-service integration tests" "auth-api-tests"
    ;;
  *)
    echo "Unknown TEST_SERVICE: ${SERVICE} (expected: user|auth|all)" >&2
    exit 1
    ;;
esac

echo ""
if [[ "${FAILED}" -eq 0 ]]; then
  echo "ALL SUITES PASSED ✓"
else
  echo "${FAILED} SUITE(S) FAILED ✗"
  exit 1
fi
