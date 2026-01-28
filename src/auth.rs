use actix_web::{
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    error::ErrorUnauthorized,
    http::header,
    middleware::Next,
    web, Error, HttpMessage, HttpRequest, HttpResponse,
};
use actix_web::cookie::{Cookie, SameSite, time::Duration};
use actix_web_httpauth::extractors::basic::BasicAuth;
use argon2::{
    password_hash::{self, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use uuid::Uuid;

use crate::{models::{ROLE_ADMIN, ROLE_BARBER}, models::UserRow, state::AppState};

pub const AUTH_REALM: &str = "Barber2Go";
const LOGOUT_COOKIE: &str = "b2g_logged_out";

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub display_name: String,
    pub role: String,
}

pub fn hash_password(password: &str) -> Result<String, password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    let parsed_hash = PasswordHash::new(password_hash);
    match parsed_hash {
        Ok(hash) => Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok(),
        Err(_) => false,
    }
}

async fn authenticate(req: &ServiceRequest, credentials: &BasicAuth) -> Result<AuthUser, Error> {
    let state = req
        .app_data::<web::Data<AppState>>()
        .ok_or_else(|| ErrorUnauthorized("Unauthorized"))?;
    let username = credentials.user_id();
    let password = credentials.password().unwrap_or_default();
    authenticate_credentials(&state, username, password)
        .await
        .ok_or_else(|| ErrorUnauthorized("Unauthorized"))
}

pub async fn authenticate_credentials(
    state: &AppState,
    username: &str,
    password: &str,
) -> Option<AuthUser> {

    let user = sqlx::query_as::<_, UserRow>(
        r#"SELECT id, username, display_name, role, password_hash, active, created_at
           FROM users
           WHERE username = ? AND active = 1
           LIMIT 1"#,
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| ErrorUnauthorized("Unauthorized"))
    .ok()?;

    let user = match user {
        Some(user) => user,
        None => return None,
    };

    if !verify_password(password, &user.password_hash) {
        return None;
    }

    Some(AuthUser {
        id: user.id,
        display_name: user.display_name,
        role: user.role,
    })
}

pub async fn basic_validator(
    req: ServiceRequest,
    credentials: BasicAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    match authenticate(&req, &credentials).await {
        Ok(user) => {
            req.extensions_mut().insert(user);
            Ok(req)
        }
        Err(err) => Err((err, req)),
    }
}

pub async fn admin_validator(
    req: ServiceRequest,
    credentials: BasicAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    match authenticate(&req, &credentials).await {
        Ok(user) => {
            if user.role != ROLE_ADMIN {
                return Err((ErrorUnauthorized("Admin access required"), req));
            }
            req.extensions_mut().insert(user);
            Ok(req)
        }
        Err(err) => Err((err, req)),
    }
}

pub async fn barber_validator(
    req: ServiceRequest,
    credentials: BasicAuth,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    match authenticate(&req, &credentials).await {
        Ok(user) => {
            if user.role != ROLE_BARBER {
                return Err((ErrorUnauthorized("Barber access required"), req));
            }
            req.extensions_mut().insert(user);
            Ok(req)
        }
        Err(err) => Err((err, req)),
    }
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn logout_cookie(req: &HttpRequest) -> Cookie<'static> {
    let mut builder = Cookie::build(LOGOUT_COOKIE, "1")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::days(365));
    if req.connection_info().scheme() == "https" {
        builder = builder.secure(true);
    }
    builder.finish()
}

pub fn clear_logout_cookie(req: &HttpRequest) -> Cookie<'static> {
    let mut builder = Cookie::build(LOGOUT_COOKIE, "")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(Duration::seconds(0));
    if req.connection_info().scheme() == "https" {
        builder = builder.secure(true);
    }
    builder.finish()
}

pub fn is_logged_out(req: &HttpRequest) -> bool {
    req.cookie(LOGOUT_COOKIE).is_some()
}

pub async fn logout_guard<B>(
    req: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<BoxBody>, Error>
where
    B: actix_web::body::MessageBody + 'static,
{
    if is_logged_out(req.request()) {
        let path = req.path();
        let login_target = if path.starts_with("/barber") {
            "/barber/dashboard"
        } else {
            "/admin/dashboard"
        };
        let login_url = format!("/login?next={}", login_target);
        let body = format!(
            r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Logged out</title>
    <style>
      body {{
        font-family: "Source Sans 3", system-ui, -apple-system, sans-serif;
        background: #f6efe6;
        color: #2d2723;
        padding: 48px 20px;
      }}
      .card {{
        max-width: 520px;
        margin: 0 auto;
        background: #ffffff;
        border-radius: 20px;
        padding: 32px;
        box-shadow: 0 18px 40px rgba(68, 52, 42, 0.12);
      }}
      a {{
        color: #c66a2d;
        text-decoration: none;
        font-weight: 600;
      }}
    </style>
  </head>
  <body>
    <div class="card">
      <h1>You're logged out</h1>
      <p>Your session has been closed.</p>
      <p><a href="{login_url}">Log in again</a> or <a href="/">return to the public site</a>.</p>
    </div>
  </body>
</html>"#
        );
        let response = HttpResponse::Unauthorized()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .content_type("text/html; charset=utf-8")
            .body(body);
        return Ok(req.into_response(response));
    }

    let res = next.call(req).await?;
    Ok(res.map_into_boxed_body())
}
