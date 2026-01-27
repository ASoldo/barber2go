mod auth;
mod db;
mod filters;
mod models;
mod routes;
mod state;
mod templates;

use actix_files::Files;
use actix_web::{middleware, web, App, HttpServer};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::env;
use std::str::FromStr;

use crate::state::AppState;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if let Err(err) = run().await {
        eprintln!("Startup error: {err}");
        std::process::exit(1);
    }
    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://./data/barber2go.db".to_string());
    db::ensure_sqlite_dir(&db_url)?;

    let connect_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await?;

    db::run_migrations(&pool).await?;
    db::seed_defaults(&pool).await?;

    let state = AppState { db: pool.clone() };

    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);

    let address = format!("0.0.0.0:{port}");
    log::info!("Starting Barber2Go on http://{address}");

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(middleware::Logger::default())
            .service(Files::new("/static", "./static").prefer_utf8(true))
            .configure(routes::public::configure)
            .configure(routes::admin::configure)
            .configure(routes::barber::configure)
    })
    .bind(address)?
    .run()
    .await?;

    Ok(())
}
