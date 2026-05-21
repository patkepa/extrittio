# Hetzner Deployment Notes

This note is intentionally local and should not be committed.

## Server

- SSH alias: `hetzner`
- Public IP: `46.225.138.164`
- Hostname: `extrittio-main`
- Public URL: `https://extrittio.kepa.pm`
- DNS/CDN: Cloudflare

## Current Live Deployment

- Live release deployment path: `/opt/extrittio-deploy`
- Compose file: `/opt/extrittio-deploy/docker-compose.release.yml`
- Compose project: `extrittio`
- Current image: `ghcr.io/extrittio/extrittio:sha-0c8a20b`
- Nginx only accepts `Host: extrittio.kepa.pm` for the public Extrittio site.
- Direct IP or unknown host access is rejected by Nginx with status `444`.
- Swagger/OpenAPI docs are disabled for the public production deployment.
- Admin credentials are stored on the server in `/opt/extrittio/deploy-credentials.txt`.

## Legacy Source-Built Deployment

- Path: `/opt/extrittio`
- Compose file: `/opt/extrittio/docker-compose.public.yml`
- This has been replaced by the GHCR image deployment.

## GHCR Access

The container package is private. The server is logged into GHCR as `patkepa` and needs a token with `read:packages` scope to pull new images.

```bash
docker login ghcr.io
```

## Manual Update Commands

Deploy a specific image tag:

```bash
ssh hetzner
cd /opt/extrittio-deploy
./deploy-release.sh sha-0c8a20b
```

Deploy the latest `main` image after CI publishes it:

```bash
ssh hetzner
cd /opt/extrittio-deploy
./deploy-release.sh main
```
