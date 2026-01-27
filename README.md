# Barber2Go

A server-rendered appointment scheduling platform for mobile barbers. Built with **Actix-web** and **SQLite**, with a clean, responsive HTML/CSS/JS interface and a simple CMS powered by web components.

## Features
- Public booking flow with service selection and location details
- Barber dashboard with appointment claiming and status updates
- Super admin dashboard with activity log and team management
- CMS editor for live content blocks (stored in SQLite)
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

Optional:

```bash
export DATABASE_URL="sqlite://./data/barber2go.db"
export PORT=8080
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

## Database
SQLite database lives at `data/barber2go.db` by default. SQLx migrations run automatically on startup.

## Vercel deployment note
Vercel’s Rust runtime is designed for **serverless functions** under the `api/` directory rather than long‑running servers. This project is a full Actix server, so the recommended production setup is:

1) Deploy the Actix server to a Rust-friendly host (Fly.io, Render, Railway, VPS, etc.).
2) Use Vercel as a reverse proxy/front door with rewrites pointing to your backend origin.

If you want a pure Vercel deployment, you’ll need to port the handlers to Vercel’s Rust `vercel_runtime` function model.

## Scripts
- `cargo run` — run locally
- `cargo build --release` — production build
