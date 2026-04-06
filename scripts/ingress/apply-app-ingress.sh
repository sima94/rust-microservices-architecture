#!/usr/bin/env bash
set -euo pipefail

# Apply shared Ingress routes for auth-service and user-service.
#
# Usage:
#   ./scripts/ingress/apply-app-ingress.sh [dev|staging|prod]
#
# Optional env vars:
#   KUBECONFIG=/tmp/oke-<env>-kubeconfig
#   APP_NAMESPACE=<env>
#   INGRESS_NAME=microservices
#   INGRESS_CLASS=nginx
#   INGRESS_HOST=<optional-hostname>
#   TLS_SECRET_NAME=<optional-tls-secret, requires INGRESS_HOST>
#   AUTH_SERVICE_NAME=auth-service
#   USER_SERVICE_NAME=user-service
#   AUTH_SERVICE_PORT=8081
#   USER_SERVICE_PORT=8082
#   DRY_RUN=0

ENVIRONMENT="${1:-dev}"

KUBECONFIG="${KUBECONFIG:-/tmp/oke-${ENVIRONMENT}-kubeconfig}"
APP_NAMESPACE="${APP_NAMESPACE:-${ENVIRONMENT}}"
INGRESS_NAME="${INGRESS_NAME:-microservices}"
INGRESS_CLASS="${INGRESS_CLASS:-nginx}"
INGRESS_HOST="${INGRESS_HOST:-}"
TLS_SECRET_NAME="${TLS_SECRET_NAME:-}"
AUTH_SERVICE_NAME="${AUTH_SERVICE_NAME:-auth-service}"
USER_SERVICE_NAME="${USER_SERVICE_NAME:-user-service}"
AUTH_SERVICE_PORT="${AUTH_SERVICE_PORT:-8081}"
USER_SERVICE_PORT="${USER_SERVICE_PORT:-8082}"
DRY_RUN="${DRY_RUN:-0}"

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

render_paths() {
  cat <<EOF
        paths:
          - path: /api/v1/auth
            pathType: Prefix
            backend:
              service:
                name: ${AUTH_SERVICE_NAME}
                port:
                  number: ${AUTH_SERVICE_PORT}
          - path: /api/v1/oauth
            pathType: Prefix
            backend:
              service:
                name: ${AUTH_SERVICE_NAME}
                port:
                  number: ${AUTH_SERVICE_PORT}
          - path: /api/v1/clients
            pathType: Prefix
            backend:
              service:
                name: ${AUTH_SERVICE_NAME}
                port:
                  number: ${AUTH_SERVICE_PORT}
          - path: /api/v1/users
            pathType: Prefix
            backend:
              service:
                name: ${USER_SERVICE_NAME}
                port:
                  number: ${USER_SERVICE_PORT}
EOF
}

render_manifest() {
  cat <<EOF
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: ${INGRESS_NAME}
  namespace: ${APP_NAMESPACE}
  labels:
    app.kubernetes.io/part-of: rust-microservices
  annotations:
    nginx.ingress.kubernetes.io/proxy-read-timeout: "60"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "60"
spec:
  ingressClassName: ${INGRESS_CLASS}
EOF

  if [[ -n "${INGRESS_HOST}" && -n "${TLS_SECRET_NAME}" ]]; then
    cat <<EOF
  tls:
    - hosts:
        - ${INGRESS_HOST}
      secretName: ${TLS_SECRET_NAME}
EOF
  fi

  if [[ -n "${INGRESS_HOST}" ]]; then
    cat <<EOF
  rules:
    - host: ${INGRESS_HOST}
      http:
EOF
    render_paths
  else
    cat <<EOF
  rules:
    - http:
EOF
    render_paths
  fi
}

if [[ ! "${ENVIRONMENT}" =~ ^(dev|staging|prod)$ ]]; then
  echo "Unsupported environment: ${ENVIRONMENT} (expected: dev|staging|prod)" >&2
  exit 1
fi

if [[ -n "${TLS_SECRET_NAME}" && -z "${INGRESS_HOST}" ]]; then
  echo "TLS_SECRET_NAME requires INGRESS_HOST to be set." >&2
  exit 1
fi

require_bin kubectl

echo "[app-ingress] environment=${ENVIRONMENT}"
echo "[app-ingress] kubeconfig=${KUBECONFIG}"
echo "[app-ingress] namespace=${APP_NAMESPACE}"
echo "[app-ingress] ingress_name=${INGRESS_NAME}"
echo "[app-ingress] ingress_class=${INGRESS_CLASS}"
if [[ -n "${INGRESS_HOST}" ]]; then
  echo "[app-ingress] ingress_host=${INGRESS_HOST}"
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  render_manifest
  exit 0
fi

render_manifest | SUPPRESS_LABEL_WARNING=True KUBECONFIG="${KUBECONFIG}" kubectl apply -f -

echo "[app-ingress] Done"
