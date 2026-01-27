use actix_web::{dev::ServiceRequest, error::ErrorUnauthorized, web, Error, HttpMessage};
use actix_web_httpauth::extractors::basic::BasicAuth;
use argon2::{
    password_hash::{self, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use uuid::Uuid;

use crate::{models::ROLE_ADMIN, models::UserRow, state::AppState};

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

    let user = sqlx::query_as::<_, UserRow>(
        r#"SELECT id, username, display_name, role, password_hash, active, created_at
           FROM users
           WHERE username = ? AND active = 1
           LIMIT 1"#,
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| ErrorUnauthorized("Unauthorized"))?;

    let user = match user {
        Some(user) => user,
        None => return Err(ErrorUnauthorized("Unauthorized")),
    };

    if !verify_password(password, &user.password_hash) {
        return Err(ErrorUnauthorized("Unauthorized"));
    }

    Ok(AuthUser {
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

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
