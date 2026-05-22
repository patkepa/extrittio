#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <version> [deploy-dir]" >&2
  exit 2
fi

version="$1"
deploy_dir="${2:-/opt/extrittio-deploy}"
env_file="$deploy_dir/.env"
compose_file="$deploy_dir/docker-compose.release.yml"

if [[ ! -f "$env_file" ]]; then
  echo "missing $env_file" >&2
  exit 1
fi

if [[ ! -f "$compose_file" ]]; then
  echo "missing $compose_file" >&2
  exit 1
fi

if grep -q '^EXTRITTIO_VERSION=' "$env_file"; then
  tmp_file="$(mktemp)"
  sed "s/^EXTRITTIO_VERSION=.*/EXTRITTIO_VERSION=$version/" "$env_file" > "$tmp_file"
  cat "$tmp_file" > "$env_file"
  rm -f "$tmp_file"
else
  printf '\nEXTRITTIO_VERSION=%s\n' "$version" >> "$env_file"
fi

cd "$deploy_dir"
docker compose --env-file "$env_file" -f "$compose_file" pull
docker compose --env-file "$env_file" -f "$compose_file" up -d
docker compose --env-file "$env_file" -f "$compose_file" ps

domain="$(sed -n 's/^EXTRITTIO_DOMAIN=//p' "$env_file" | tail -1)"
health_token="$(sed -n 's/^EXTRITTIO_HEALTH_TOKEN=//p' "$env_file" | tail -1)"
if [[ -n "$domain" ]]; then
  curl -fsS -H "Host: $domain" http://127.0.0.1/health >/dev/null
  if [[ -n "$health_token" ]]; then
    curl -fsS -H "Host: $domain" -H "X-Extrittio-Health-Token: $health_token" http://127.0.0.1/ready >/dev/null
  else
    curl -fsS -H "Host: $domain" http://127.0.0.1/ready >/dev/null
  fi
fi

echo "deployed Extrittio $version"
