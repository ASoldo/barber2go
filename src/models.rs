use serde::Serialize;

pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_BARBER: &str = "barber";

pub const STATUS_PENDING: &str = "pending";
pub const STATUS_ACCEPTED: &str = "accepted";
pub const STATUS_DECLINED: &str = "declined";
pub const STATUS_COMPLETED: &str = "completed";

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: String,
    pub active: i64,
    pub created_at: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppointmentRow {
    pub id: String,
    pub client_name: String,
    pub client_phone: String,
    pub client_email: Option<String>,
    pub address: String,
    pub service: String,
    pub notes: Option<String>,
    pub requested_at: String,
    pub scheduled_for: String,
    pub status: String,
    pub barber_id: Option<String>,
    pub barber_name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ActivityRow {
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CmsBlockRow {
    pub key: String,
    pub title: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceOption {
    pub name: &'static str,
    pub duration: &'static str,
    pub description: &'static str,
    pub selected: bool,
}
