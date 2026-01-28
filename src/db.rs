use std::{env, fs, path::Path};

use chrono::Utc;
use sqlx::SqlitePool;

use crate::{
    auth::{hash_password, new_id},
    models::{AppointmentRow, ROLE_ADMIN, ROLE_BARBER},
};

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

pub fn ensure_sqlite_dir(db_url: &str) -> std::io::Result<()> {
    let path = if let Some(path) = db_url.strip_prefix("sqlite://") {
        Some(path)
    } else if let Some(path) = db_url.strip_prefix("sqlite:") {
        Some(path)
    } else {
        None
    };

    let Some(path) = path else {
        return Ok(());
    };

    let path = path.split('?').next().unwrap_or(path);
    if path == ":memory:" || path.is_empty() {
        return Ok(());
    }

    let path = path.strip_prefix("file:").unwrap_or(path);
    let db_path = Path::new(path);
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub async fn seed_defaults(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    seed_admin(pool).await?;
    seed_cms(pool).await?;
    Ok(())
}

pub async fn log_activity(
    pool: &SqlitePool,
    kind: &str,
    message: &str,
    user_id: Option<&str>,
    appointment_id: Option<&str>,
) {
    let _ = sqlx::query(
        r#"INSERT INTO activities (id, kind, message, created_at, user_id, appointment_id)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(new_id())
    .bind(kind)
    .bind(message)
    .bind(Utc::now().to_rfc3339())
    .bind(user_id)
    .bind(appointment_id)
    .execute(pool)
    .await;
}

pub async fn fetch_appointment_event(
    pool: &SqlitePool,
    appointment_id: &str,
) -> Option<AppointmentRow> {
    sqlx::query_as::<_, AppointmentRow>(
        r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                  a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                  a.latitude, a.longitude,
                  u.display_name as barber_name
           FROM appointments a
           LEFT JOIN users u ON a.barber_id = u.id
           WHERE a.id = ?
           LIMIT 1"#,
    )
    .bind(appointment_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
}

async fn seed_admin(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM users WHERE role = ? LIMIT 1",
    )
    .bind(ROLE_ADMIN)
    .fetch_optional(pool)
    .await?;

    if existing.is_some() {
        return Ok(());
    }

    let username = env::var("ADMIN_USER").unwrap_or_else(|_| "admin".to_string());
    let password = env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin".to_string());
    let display_name = env::var("ADMIN_DISPLAY_NAME").unwrap_or_else(|_| "Super Admin".to_string());

    if password == "admin" {
        log::warn!("ADMIN_PASSWORD not set. Using default password 'admin'. Set ADMIN_PASSWORD in production.");
    }

    let password_hash = hash_password(&password).map_err(|_| sqlx::Error::Protocol("password hash failed".into()))?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT INTO users (id, username, display_name, role, password_hash, active, created_at)
           VALUES (?, ?, ?, ?, ?, 1, ?)"#,
    )
    .bind(new_id())
    .bind(username)
    .bind(display_name)
    .bind(ROLE_ADMIN)
    .bind(password_hash)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

async fn seed_cms(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let blocks = vec![
        (
            "home_hero",
            "Home Hero",
            r#"<h1>Fresh cuts, booked fast.</h1>
<p>Barber2Go brings licensed barbers to your doorstep. Book in minutes, get a professional cut at home.</p>"#,
        ),
        (
            "home_about",
            "Home About",
            r#"<h2>Mobile-first barbering</h2>
<p>We partner with vetted barbers who travel to you—home, hotel, or office. Clear schedules, real-time updates, no hassle.</p>"#,
        ),
        (
            "home_services",
            "Home Services Intro",
            r#"<h2>Services built for busy days</h2>
<p>From quick clean-ups to full grooming, choose a service and a time that works for you.</p>"#,
        ),
    ];

    for (key, title, html) in blocks {
        let exists = sqlx::query_as::<_, (String,)>("SELECT key FROM cms_blocks WHERE key = ? LIMIT 1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
        if exists.is_some() {
            continue;
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO cms_blocks (key, title, html, updated_at, updated_by)
               VALUES (?, ?, ?, ?, NULL)"#,
        )
        .bind(key)
        .bind(title)
        .bind(html)
        .bind(now)
        .execute(pool)
        .await?;
    }

    let barber_seed = env::var("SEED_BARBER").unwrap_or_else(|_| "false".to_string());
    if barber_seed == "true" {
        let exists = sqlx::query_as::<_, (String,)>("SELECT id FROM users WHERE role = ? LIMIT 1")
            .bind(ROLE_BARBER)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            let username = env::var("BARBER_USER").unwrap_or_else(|_| "barber1".to_string());
            let password = env::var("BARBER_PASSWORD").unwrap_or_else(|_| "change-me".to_string());
            let display_name = env::var("BARBER_DISPLAY_NAME").unwrap_or_else(|_| "Barber One".to_string());
            if password == "change-me" {
                log::warn!("BARBER_PASSWORD not set. Using default password 'change-me'. Set BARBER_PASSWORD in production.");
            }
            let password_hash = hash_password(&password)
                .map_err(|_| sqlx::Error::Protocol("password hash failed".into()))?;
            let now = Utc::now().to_rfc3339();
            sqlx::query(
                r#"INSERT INTO users (id, username, display_name, role, password_hash, active, created_at)
                   VALUES (?, ?, ?, ?, ?, 1, ?)"#,
            )
            .bind(new_id())
            .bind(username)
            .bind(display_name)
            .bind(ROLE_BARBER)
            .bind(password_hash)
            .bind(now)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}
