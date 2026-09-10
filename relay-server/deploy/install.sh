#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="${REPO_DIR:-/opt/localmind-relay}"
PUBLIC_IP="${PUBLIC_IP:-39.107.53.230}"
COMPOSE_FILE="${REPO_DIR}/relay-server/docker-compose.yml"
CADDYFILE="/etc/caddy/Caddyfile"
ROOT_CA_SOURCE="/var/lib/caddy/.local/share/caddy/pki/authorities/local/root.crt"
ROOT_CA_TARGET="${REPO_DIR}/relay-server/certs/localmind-relay-ca.crt"

if [[ ! -f "${COMPOSE_FILE}" ]]; then
  echo "missing ${COMPOSE_FILE}" >&2
  exit 1
fi

install -d -m 0755 "${REPO_DIR}/relay-server/certs"
install -m 0644 "${REPO_DIR}/relay-server/deploy/Caddyfile" "${CADDYFILE}"
sed -i "s/39\.107\.53\.230/${PUBLIC_IP}/g" "${CADDYFILE}"

cd "${REPO_DIR}"
docker compose -f "${COMPOSE_FILE}" up -d --build

systemctl restart caddy
caddy validate --config "${CADDYFILE}"

for attempt in $(seq 1 30); do
  if curl -fsS "http://127.0.0.1:8080/health" >/dev/null; then
    break
  fi
  if [[ "${attempt}" -eq 30 ]]; then
    echo "relay health check failed" >&2
    docker compose -f "${COMPOSE_FILE}" logs --tail=100 relay >&2 || true
    exit 1
  fi
  sleep 2
done

if [[ -f "${ROOT_CA_SOURCE}" ]]; then
  install -m 0644 "${ROOT_CA_SOURCE}" "${ROOT_CA_TARGET}"
else
  echo "Caddy root CA not found at ${ROOT_CA_SOURCE}" >&2
  exit 1
fi

echo "Relay deployed: https://${PUBLIC_IP}"
echo "Public CA certificate: ${ROOT_CA_TARGET}"
