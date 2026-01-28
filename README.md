# Barber2Go

A server-rendered appointment scheduling platform for mobile barbers. Built with **Actix-web** and **SQLite**, with a clean, responsive HTML/CSS/JS interface and a simple CMS powered by web components.

## Features
- Public booking flow with service selection and location details
- Barber dashboard with appointment claiming and status updates
- Super admin dashboard with activity log and team management
- CMS editor for live content blocks (stored in SQLite)
- PWA support with push notifications
- Live schedule updates via SSE (no manual refresh)
- Public status tracker page for clients (`/status/{id}`)
- Basic auth (HTTP Basic) for admin + barber routes

## Stack
- Rust 2024 + Actix-web (SSR)
- SQLite + SQLx migrations
- Askama templates
- Pure HTML, CSS, JS

## Getting started

### 1) Configure environment variables

```bash
export ADMIN_USER=admin
export ADMIN_PASSWORD=admin
export ADMIN_DISPLAY_NAME="Super Admin"
# Optional seed barber:
export SEED_BARBER=true
export BARBER_USER=barber1
export BARBER_PASSWORD=barbersecret
export BARBER_DISPLAY_NAME="Barber One"
```

> Change `ADMIN_PASSWORD` before deploying to production.

Optional:

```bash
export DATABASE_URL="sqlite://./data/barber2go.db"
export PORT=8080
export VAPID_SUBJECT="mailto:admin@barber2go.local"
export VAPID_PUBLIC_KEY="YOUR_VAPID_PUBLIC_KEY"
export VAPID_PRIVATE_KEY="YOUR_VAPID_PRIVATE_KEY"
```

### 2) Run the app

```bash
cargo run
```

Visit:
- Public site: `http://localhost:8080/`
- Admin dashboard: `http://localhost:8080/admin/dashboard`
- Barber dashboard: `http://localhost:8080/barber/dashboard`

> Admin + barber routes are protected with HTTP Basic auth. Your browser will prompt for credentials.

## CMS editing
Open `/admin/cms` to edit live content blocks. Changes persist to the `cms_blocks` table and immediately update the public pages.

## Realtime updates
- Admin + barber pages subscribe to `/events` (SSE) for live updates (no refresh).
- Clients can track updates on `/status/{id}` (SSE) or opt in to web push notifications.

## Push notifications (web push)
Push requires HTTPS + VAPID keys.

Generate VAPID keys (one-time) using Node:
```bash
npx web-push generate-vapid-keys
```

Set these env vars (locally or in Fly secrets):
```bash
export VAPID_PUBLIC_KEY="..."
export VAPID_PRIVATE_KEY="..."
export VAPID_SUBJECT="mailto:admin@barber2go.local"
```

## Maps + address autocomplete
The booking page uses OpenStreetMap (Nominatim) + Leaflet to suggest addresses and let users pin their location.

## Database
SQLite database lives at `data/barber2go.db` by default. SQLx migrations run automatically on startup.

## Fly.io deployment (recommended for SQLite)
This repo includes a `Dockerfile` and `fly.toml` configured for SQLite on a Fly volume. The volume
mount is defined as:

```toml
[mounts]
source = "data"
destination = "/data"
```

### 1) Create the volume (one-time)
```bash
flyctl volumes create data --app barber2go --region iad --size 1
```

### 2) Set secrets
```bash
flyctl secrets set \
  ADMIN_USER=admin \
  ADMIN_PASSWORD=admin \
  VAPID_PUBLIC_KEY="..." \
  VAPID_PRIVATE_KEY="..." \
  VAPID_SUBJECT="mailto:admin@barber2go.local" \
  DATABASE_URL="sqlite:///data/barber2go.db" \
  --app barber2go
```

### 3) Deploy (run from repo root so fly.toml is picked up)
```bash
flyctl deploy --app barber2go
```

### 4) SQLite requires a single machine
Run one machine (one volume). SQLite does not replicate across machines.
```bash
flyctl scale count 1 --app barber2go
```

### Useful checks
```bash
flyctl status --app barber2go
flyctl logs --app barber2go
flyctl volumes list --app barber2go
```

The database lives at `/data/barber2go.db` inside the mounted volume.

## Scripts
- `cargo run` — run locally
- `cargo build --release` — production build
