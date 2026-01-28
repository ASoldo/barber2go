use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::models::AppointmentRow;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub events: broadcast::Sender<ServerEvent>,
    pub push: PushConfig,
}

#[derive(Clone, Debug)]
pub struct PushConfig {
    pub public_key: String,
    pub private_key: String,
    pub subject: String,
}

impl PushConfig {
    pub fn enabled(&self) -> bool {
        !(self.public_key.trim().is_empty() || self.private_key.trim().is_empty())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ServerEvent {
    pub kind: String,
    pub appointment_id: Option<String>,
    pub status: Option<String>,
    pub client_name: Option<String>,
    pub client_phone: Option<String>,
    pub client_email: Option<String>,
    pub address: Option<String>,
    pub service: Option<String>,
    pub notes: Option<String>,
    pub scheduled_for: Option<String>,
    pub barber_name: Option<String>,
    pub barber_id: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl ServerEvent {
    pub fn from_row(kind: &str, row: AppointmentRow) -> Self {
        Self {
            kind: kind.to_string(),
            appointment_id: Some(row.id),
            status: Some(row.status),
            client_name: Some(row.client_name),
            client_phone: Some(row.client_phone),
            client_email: row.client_email,
            address: Some(row.address),
            service: Some(row.service),
            notes: row.notes,
            scheduled_for: Some(row.scheduled_for),
            barber_name: row.barber_name,
            barber_id: row.barber_id,
            latitude: row.latitude,
            longitude: row.longitude,
        }
    }
}
