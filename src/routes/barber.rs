use actix_web::{http::header, middleware::from_fn, web, HttpResponse, Result};
use actix_web_httpauth::middleware::HttpAuthentication;
use askama::Template;
use serde::Deserialize;

use crate::{
    auth::{barber_validator, logout_guard, AuthUser},
    db::{fetch_appointment_event, log_activity},
    models::{
        AppointmentRow, STATUS_ACCEPTED, STATUS_COMPLETED, STATUS_DECLINED, STATUS_PENDING,
    },
    push,
    state::{AppState, ServerEvent},
    templates::render,
};

#[derive(Clone, Debug)]
struct AppointmentView {
    id: String,
    client_name: String,
    client_phone: String,
    client_email: String,
    has_email: bool,
    address: String,
    service: String,
    notes: String,
    has_notes: bool,
    scheduled_for: String,
    status: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Clone, Debug)]
struct StatCard {
    label: String,
    value: i64,
}

#[derive(Template)]
#[template(path = "barber_dashboard.html")]
struct BarberDashboardTemplate {
    barber_name: String,
    stats: Vec<StatCard>,
    upcoming: Vec<AppointmentView>,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "barber_appointments.html")]
struct BarberAppointmentsTemplate {
    appointments: Vec<AppointmentView>,
    barber_id: String,
    is_admin: bool,
}

#[derive(Deserialize)]
struct AppointmentStatusForm {
    status: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/barber")
            .wrap(HttpAuthentication::basic(barber_validator))
            .wrap(from_fn(logout_guard))
            .service(web::resource("").route(web::get().to(index)))
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/dashboard").route(web::get().to(dashboard)))
            .service(web::resource("/appointments").route(web::get().to(list_appointments)))
            .service(
                web::resource("/appointments/{id}/status")
                    .route(web::post().to(update_status)),
            ),
    );
}

async fn index() -> HttpResponse {
    HttpResponse::Found()
        .append_header((header::LOCATION, "/barber/dashboard"))
        .finish()
}

async fn dashboard(state: web::Data<AppState>, auth: web::ReqData<AuthUser>) -> Result<HttpResponse> {
    let total = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ?",
        &state,
        &auth.id,
    )
    .await;
    let pending = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ? AND status = 'pending'",
        &state,
        &auth.id,
    )
    .await;
    let accepted = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ? AND status = 'accepted'",
        &state,
        &auth.id,
    )
    .await;
    let completed = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ? AND status = 'completed'",
        &state,
        &auth.id,
    )
    .await;

    let stats = vec![
        StatCard {
            label: "Total appointments".to_string(),
            value: total,
        },
        StatCard {
            label: "Pending".to_string(),
            value: pending,
        },
        StatCard {
            label: "Accepted".to_string(),
            value: accepted,
        },
        StatCard {
            label: "Completed".to_string(),
            value: completed,
        },
    ];

    let rows = sqlx::query_as::<_, AppointmentRow>(
        r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                  a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                  a.latitude, a.longitude,
                  NULL as barber_name
           FROM appointments a
           WHERE a.barber_id = ?
           ORDER BY a.scheduled_for DESC
           LIMIT 8"#,
    )
    .bind(&auth.id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let upcoming = rows.into_iter().map(to_view).collect();

    Ok(render(BarberDashboardTemplate {
        barber_name: auth.display_name.clone(),
        stats,
        upcoming,
        is_admin: false,
    }))
}

async fn list_appointments(state: web::Data<AppState>, auth: web::ReqData<AuthUser>) -> Result<HttpResponse> {
    let rows = sqlx::query_as::<_, AppointmentRow>(
        r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                  a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                  a.latitude, a.longitude,
                  NULL as barber_name
           FROM appointments a
           WHERE a.barber_id = ? OR (a.barber_id IS NULL AND a.status = ?)
           ORDER BY a.requested_at DESC"#,
    )
    .bind(&auth.id)
    .bind(STATUS_PENDING)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let appointments = rows.into_iter().map(to_view).collect();

    Ok(render(BarberAppointmentsTemplate {
        appointments,
        barber_id: auth.id.clone(),
        is_admin: false,
    }))
}

async fn update_status(
    state: web::Data<AppState>,
    auth: web::ReqData<AuthUser>,
    path: web::Path<String>,
    form: web::Form<AppointmentStatusForm>,
) -> Result<HttpResponse> {
    let appointment_id = path.into_inner();
    let form = form.into_inner();
    let status = form.status;
    let allowed = [STATUS_ACCEPTED, STATUS_DECLINED, STATUS_COMPLETED, STATUS_PENDING];
    if !allowed.contains(&status.as_str()) {
        return Ok(HttpResponse::BadRequest().body("Invalid status"));
    }

    let current = sqlx::query_as::<_, (Option<String>, String)>(
        "SELECT barber_id, status FROM appointments WHERE id = ?",
    )
    .bind(&appointment_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let (barber_id, _current_status) = match current {
        Some(row) => row,
        None => return Ok(HttpResponse::NotFound().body("Appointment not found")),
    };

    let can_edit = barber_id.as_deref().is_none() || barber_id.as_deref() == Some(&auth.id);
    if !can_edit {
        return Ok(HttpResponse::Forbidden().body("Not allowed"));
    }

    let assigned = if status == STATUS_ACCEPTED {
        Some(auth.id.clone())
    } else {
        barber_id
    };

    sqlx::query("UPDATE appointments SET status = ?, barber_id = ? WHERE id = ?")
        .bind(&status)
        .bind(assigned)
        .bind(&appointment_id)
        .execute(&state.db)
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    log_activity(
        &state.db,
        "barber_status_update",
        &format!("{} updated appointment {} to {}.", auth.display_name, appointment_id, status),
        Some(&auth.id),
        Some(&appointment_id),
    )
    .await;

    let status_url = format!("/status/{appointment_id}");
    push::notify_appointment(
        &state,
        &appointment_id,
        "Appointment updated",
        &format!("Status changed to {}.", status),
        Some(status_url.as_str()),
    )
    .await;

    if let Some(row) = fetch_appointment_event(&state.db, &appointment_id).await {
        let _ = state
            .events
            .send(ServerEvent::from_row("appointment_updated", row));
    }

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/barber/appointments"))
        .finish())
}

fn to_view(row: AppointmentRow) -> AppointmentView {
    let notes = row.notes.unwrap_or_default();
    let client_email = row.client_email.unwrap_or_default();
    AppointmentView {
        id: row.id,
        client_name: row.client_name,
        client_phone: row.client_phone,
        client_email: client_email.clone(),
        has_email: !client_email.trim().is_empty(),
        address: row.address,
        service: row.service,
        notes: notes.clone(),
        has_notes: !notes.trim().is_empty(),
        scheduled_for: row.scheduled_for,
        status: row.status,
        latitude: row.latitude,
        longitude: row.longitude,
    }
}

async fn count(query: &str, state: &web::Data<AppState>, param: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(query)
        .bind(param)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0)
}
