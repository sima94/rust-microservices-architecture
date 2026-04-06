#!/usr/bin/env bash
set -euo pipefail

# E2E smoke tests against a live environment.
# Tests actual HTTP endpoints — no compilation, no port-forwards, fast feedback.
#
# Usage:
#   ./scripts/smoke-test.sh [dev|staging|prod]
#   ./scripts/smoke-test.sh --url http://89.168.100.28
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   K8S_NAMESPACE=<env>
#   BASE_URL=<override ingress URL>

ENVIRONMENT="${1:-dev}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
K8S_NAMESPACE="${K8S_NAMESPACE:-${ENVIRONMENT}}"
BASE_URL="${BASE_URL:-}"

PASS=0
FAIL=0
TOTAL=0

# ── Helpers ─────────────────────────────────────────────────────

red()   { printf "\033[0;31m%s\033[0m" "$1"; }
green() { printf "\033[0;32m%s\033[0m" "$1"; }
bold()  { printf "\033[1m%s\033[0m" "$1"; }

assert_status() {
  local description="$1"
  local expected="$2"
  local method="$3"
  local url="$4"
  shift 4
  local extra_args=("$@")

  TOTAL=$((TOTAL + 1))

  local status
  status=$(curl -s -o /dev/null -w "%{http_code}" -X "${method}" "${url}" ${extra_args[@]+"${extra_args[@]}"} 2>/dev/null || echo "000")

  if [[ "${status}" == "${expected}" ]]; then
    echo "  $(green "✓") ${description} (${status})"
    PASS=$((PASS + 1))
  else
    echo "  $(red "✗") ${description} — expected ${expected}, got ${status}"
    FAIL=$((FAIL + 1))
  fi
}

assert_json_field() {
  local description="$1"
  local method="$2"
  local url="$3"
  local field="$4"
  shift 4
  local extra_args=("$@")

  TOTAL=$((TOTAL + 1))

  local body
  body=$(curl -s -X "${method}" "${url}" ${extra_args[@]+"${extra_args[@]}"} 2>/dev/null || echo "{}")

  if echo "${body}" | python3 -c "import sys,json; d=json.load(sys.stdin); assert '${field}' in d" 2>/dev/null; then
    echo "  $(green "✓") ${description} (has '${field}')"
    PASS=$((PASS + 1))
  else
    echo "  $(red "✗") ${description} — missing '${field}' in response"
    FAIL=$((FAIL + 1))
  fi
}

# ── Resolve base URL ────────────────────────────────────────────

if [[ "${ENVIRONMENT}" == --url ]]; then
  BASE_URL="${2:-}"
  if [[ -z "${BASE_URL}" ]]; then
    echo "Usage: $0 --url <base-url>" >&2
    exit 1
  fi
fi

if [[ -z "${BASE_URL}" ]]; then
  if ! command -v kubectl >/dev/null 2>&1; then
    echo "kubectl not found and BASE_URL not set" >&2
    exit 1
  fi

  INGRESS_IP=$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
    kubectl get ingress -n "${K8S_NAMESPACE}" microservices \
    -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null || true)

  INGRESS_HOST=$(SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" \
    kubectl get ingress -n "${K8S_NAMESPACE}" microservices \
    -o jsonpath='{.status.loadBalancer.ingress[0].hostname}' 2>/dev/null || true)

  BASE_URL="http://${INGRESS_IP:-${INGRESS_HOST}}"

  if [[ "${BASE_URL}" == "http://" ]]; then
    echo "Could not resolve ingress URL for env=${ENVIRONMENT}" >&2
    exit 1
  fi
fi

echo ""
echo "$(bold "Smoke Tests") — ${BASE_URL} (env: ${ENVIRONMENT})"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# ── Auth Service ────────────────────────────────────────────────

echo ""
echo "$(bold "Auth Service")"

assert_status \
  "POST /auth/register with short password → 400" \
  400 POST "${BASE_URL}/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d '{"email":"smoke-short@test.com","password":"abc"}'

SMOKE_EMAIL="smoke-$(date +%s)@test.com"
assert_status \
  "POST /auth/register happy path → 201" \
  201 POST "${BASE_URL}/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"${SMOKE_EMAIL}\",\"password\":\"password123\"}"

assert_status \
  "POST /auth/register duplicate → 409" \
  409 POST "${BASE_URL}/api/v1/auth/register" \
  -H "Content-Type: application/json" \
  -d "{\"email\":\"${SMOKE_EMAIL}\",\"password\":\"password123\"}"

assert_status \
  "POST /clients/register happy path → 201" \
  201 POST "${BASE_URL}/api/v1/clients/register" \
  -H "Content-Type: application/json" \
  -d '{"client_name":"smoke-client","redirect_uri":"https://example.com/callback","scopes":"read write"}'

assert_status \
  "POST /oauth/token wrong client → 401" \
  401 POST "${BASE_URL}/api/v1/oauth/token" \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d 'grant_type=client_credentials&client_id=nonexistent&client_secret=wrong'

# ── User Service ────────────────────────────────────────────────

echo ""
echo "$(bold "User Service")"

assert_status \
  "GET /users without token → 401" \
  401 GET "${BASE_URL}/api/v1/users"

assert_status \
  "GET /users/999999 without token → 401" \
  401 GET "${BASE_URL}/api/v1/users/999999"

assert_status \
  "POST /users without token → 401" \
  401 POST "${BASE_URL}/api/v1/users" \
  -H "Content-Type: application/json" \
  -d '{"name":"smoke","email":"smoke@test.com"}'

# ── Summary ─────────────────────────────────────────────────────

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [[ "${FAIL}" -eq 0 ]]; then
  echo "$(green "ALL ${TOTAL} TESTS PASSED") ✓"
else
  echo "$(red "${FAIL}/${TOTAL} TESTS FAILED") ✗"
fi
echo ""

exit "${FAIL}"
