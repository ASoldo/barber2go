use actix_web::{http::header, middleware::from_fn, web, HttpResponse, Result};
use actix_web_httpauth::middleware::HttpAuthentication;
use askama::Template;
use serde::Deserialize;

use crate::{
    auth::{admin_validator, hash_password, logout_guard, new_id, AuthUser},
    db::{fetch_appointment_event, log_activity},
    models::{
        ActivityRow, AppointmentRow, CmsBlockRow, ServiceOption, UserRow, ROLE_ADMIN,
        ROLE_BARBER, STATUS_ACCEPTED, STATUS_COMPLETED, STATUS_DECLINED, STATUS_PENDING,
    },
    push,
    state::{AppState, ServerEvent},
    templates::render,
};

#[derive(Clone, Debug)]
struct StatCard {
    label: String,
    value: i64,
}

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
    barber_id: String,
    barber_name: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Clone, Debug)]
struct ActivityView {
    message: String,
    created_at: String,
}

#[derive(Clone, Debug)]
struct BarberView {
    id: String,
    display_name: String,
    username: String,
    role: String,
    active: bool,
    selected: bool,
}

#[derive(Template)]
#[template(path = "admin_dashboard.html")]
struct AdminDashboardTemplate {
    admin_name: String,
    stats: Vec<StatCard>,
    upcoming: Vec<AppointmentView>,
    activities: Vec<ActivityView>,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "admin_appointments.html")]
struct AdminAppointmentsTemplate {
    appointments: Vec<AppointmentView>,
    status_filter: String,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "admin_appointment_detail.html")]
struct AdminAppointmentDetailTemplate {
    appointment: AppointmentView,
    barbers: Vec<BarberView>,
    statuses: Vec<StatusOption>,
    is_admin: bool,
}

#[derive(Clone, Debug)]
struct StatusOption {
    value: &'static str,
    selected: bool,
}

#[derive(Template)]
#[template(path = "admin_barbers.html")]
struct AdminBarbersTemplate {
    barbers: Vec<BarberView>,
    errors: Vec<String>,
    success: String,
    has_success: bool,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "admin_barber_stats.html")]
struct AdminBarberStatsTemplate {
    barber: BarberView,
    stats: Vec<StatCard>,
    recent: Vec<AppointmentView>,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "admin_cms.html")]
struct AdminCmsTemplate {
    blocks: Vec<CmsBlockRow>,
    is_admin: bool,
}

#[derive(Deserialize)]
struct AppointmentFilter {
    status: Option<String>,
}

#[derive(Deserialize)]
struct AppointmentUpdateForm {
    status: String,
    barber_id: Option<String>,
    scheduled_for: Option<String>,
}

#[derive(Deserialize)]
struct BarberCreateForm {
    username: String,
    display_name: String,
    password: String,
}

#[derive(Deserialize)]
struct CmsUpdatePayload {
    key: String,
    html: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin")
            .wrap(HttpAuthentication::basic(admin_validator))
            .wrap(from_fn(logout_guard))
            .service(web::resource("").route(web::get().to(index)))
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/dashboard").route(web::get().to(dashboard)))
            .service(web::resource("/appointments").route(web::get().to(list_appointments)))
            .service(
                web::resource("/appointments/{id}")
                    .route(web::get().to(appointment_detail))
                    .route(web::post().to(update_appointment)),
            )
            .service(web::resource("/barbers").route(web::get().to(list_barbers)).route(web::post().to(create_barber)))
            .service(web::resource("/barbers/{id}").route(web::get().to(barber_stats)))
            .service(web::resource("/cms").route(web::get().to(cms_editor)))
            .service(web::resource("/cms/save").route(web::post().to(save_cms))),
    );
}

async fn index() -> HttpResponse {
    HttpResponse::Found()
        .append_header((header::LOCATION, "/admin/dashboard"))
        .finish()
}

async fn dashboard(state: web::Data<AppState>, auth: web::ReqData<AuthUser>) -> Result<HttpResponse> {
    let total = count("SELECT COUNT(*) FROM appointments", &state).run().await;
    let pending = count(
        "SELECT COUNT(*) FROM appointments WHERE status = 'pending'",
        &state,
    )
    .run()
    .await;
    let accepted = count(
        "SELECT COUNT(*) FROM appointments WHERE status = 'accepted'",
        &state,
    )
    .run()
    .await;
    let completed = count(
        "SELECT COUNT(*) FROM appointments WHERE status = 'completed'",
        &state,
    )
    .run()
    .await;

    let stats = vec![
        StatCard {
            label: "Total appointments".to_string(),
            value: total,
        },
        StatCard {
            label: "Pending review".to_string(),
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

    let upcoming_rows = sqlx::query_as::<_, AppointmentRow>(
        r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                  a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                  a.latitude, a.longitude,
                  u.display_name as barber_name
           FROM appointments a
           LEFT JOIN users u ON a.barber_id = u.id
           ORDER BY a.scheduled_for DESC
           LIMIT 6"#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let upcoming = upcoming_rows.into_iter().map(to_view).collect();

    let activity_rows = sqlx::query_as::<_, ActivityRow>(
        "SELECT message, created_at FROM activities ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let activities = activity_rows
        .into_iter()
        .map(|row| ActivityView {
            message: row.message,
            created_at: row.created_at,
        })
        .collect();

    Ok(render(AdminDashboardTemplate {
        admin_name: auth.display_name.clone(),
        stats,
        upcoming,
        activities,
        is_admin: true,
    }))
}

async fn list_appointments(
    state: web::Data<AppState>,
    query: web::Query<AppointmentFilter>,
) -> Result<HttpResponse> {
    let status_filter = query.status.clone().unwrap_or_default();
    let rows = if status_filter.is_empty() {
        sqlx::query_as::<_, AppointmentRow>(
            r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                      a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                      a.latitude, a.longitude,
                      u.display_name as barber_name
               FROM appointments a
               LEFT JOIN users u ON a.barber_id = u.id
               ORDER BY a.requested_at DESC"#,
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, AppointmentRow>(
            r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                      a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                      a.latitude, a.longitude,
                      u.display_name as barber_name
               FROM appointments a
               LEFT JOIN users u ON a.barber_id = u.id
               WHERE a.status = ?
               ORDER BY a.requested_at DESC"#,
        )
        .bind(&status_filter)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let appointments = rows.into_iter().map(to_view).collect();

    Ok(render(AdminAppointmentsTemplate {
        appointments,
        status_filter,
        is_admin: true,
    }))
}

async fn appointment_detail(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let appointment_id = path.into_inner();
    let row = sqlx::query_as::<_, AppointmentRow>(
        r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                  a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                  a.latitude, a.longitude,
                  u.display_name as barber_name
           FROM appointments a
           LEFT JOIN users u ON a.barber_id = u.id
           WHERE a.id = ?
           LIMIT 1"#,
    )
    .bind(&appointment_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let appointment = match row {
        Some(row) => to_view(row),
        None => {
            return Ok(HttpResponse::NotFound().body("Appointment not found"));
        }
    };

    let mut barbers = fetch_barbers(&state).await.unwrap_or_default();
    for barber in &mut barbers {
        barber.selected = barber.id == appointment.barber_id;
    }
    let statuses = vec![
        StatusOption {
            value: STATUS_PENDING,
            selected: appointment.status == STATUS_PENDING,
        },
        StatusOption {
            value: STATUS_ACCEPTED,
            selected: appointment.status == STATUS_ACCEPTED,
        },
        StatusOption {
            value: STATUS_DECLINED,
            selected: appointment.status == STATUS_DECLINED,
        },
        StatusOption {
            value: STATUS_COMPLETED,
            selected: appointment.status == STATUS_COMPLETED,
        },
    ];

    Ok(render(AdminAppointmentDetailTemplate {
        appointment,
        barbers,
        statuses,
        is_admin: true,
    }))
}

async fn update_appointment(
    state: web::Data<AppState>,
    path: web::Path<String>,
    form: web::Form<AppointmentUpdateForm>,
    auth: web::ReqData<AuthUser>,
) -> Result<HttpResponse> {
    let appointment_id = path.into_inner();
    let form = form.into_inner();
    let status = form.status.clone();
    let barber_id = form.barber_id.as_ref().and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value.trim().to_string())
        }
    });

    if let Some(scheduled_for) = form.scheduled_for.as_ref() {
        if !scheduled_for.trim().is_empty() {
            sqlx::query(
                "UPDATE appointments SET status = ?, barber_id = ?, scheduled_for = ? WHERE id = ?",
            )
            .bind(&form.status)
            .bind(&barber_id)
            .bind(scheduled_for.trim())
            .bind(&appointment_id)
            .execute(&state.db)
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
        } else {
            sqlx::query("UPDATE appointments SET status = ?, barber_id = ? WHERE id = ?")
                .bind(&form.status)
                .bind(&barber_id)
                .bind(&appointment_id)
                .execute(&state.db)
                .await
                .map_err(actix_web::error::ErrorInternalServerError)?;
        }
    } else {
        sqlx::query("UPDATE appointments SET status = ?, barber_id = ? WHERE id = ?")
            .bind(&form.status)
            .bind(&barber_id)
            .bind(&appointment_id)
            .execute(&state.db)
            .await
            .map_err(actix_web::error::ErrorInternalServerError)?;
    }

    log_activity(
        &state.db,
        "appointment_updated",
        &format!("{} updated appointment {}.", auth.display_name, appointment_id),
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
        .append_header((header::LOCATION, format!("/admin/appointments/{appointment_id}")))
        .finish())
}

async fn list_barbers(state: web::Data<AppState>) -> Result<HttpResponse> {
    let barbers = fetch_barbers(&state).await.unwrap_or_default();
    Ok(render(AdminBarbersTemplate {
        barbers,
        errors: Vec::new(),
        success: String::new(),
        has_success: false,
        is_admin: true,
    }))
}

async fn create_barber(
    state: web::Data<AppState>,
    form: web::Form<BarberCreateForm>,
    auth: web::ReqData<AuthUser>,
) -> Result<HttpResponse> {
    let form = form.into_inner();
    let mut errors = Vec::new();
    if form.username.trim().is_empty() {
        errors.push("Username is required.".to_string());
    }
    if form.display_name.trim().is_empty() {
        errors.push("Display name is required.".to_string());
    }
    if form.password.trim().len() < 6 {
        errors.push("Password must be at least 6 characters.".to_string());
    }

    if !errors.is_empty() {
        let barbers = fetch_barbers(&state).await.unwrap_or_default();
        return Ok(render(AdminBarbersTemplate {
            barbers,
            errors,
            success: String::new(),
            has_success: false,
            is_admin: true,
        }));
    }

    let password_hash = hash_password(&form.password)
        .map_err(|_| actix_web::error::ErrorInternalServerError("hash failure"))?;
    let now = chrono::Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"INSERT INTO users (id, username, display_name, role, password_hash, active, created_at)
           VALUES (?, ?, ?, ?, ?, 1, ?)"#,
    )
    .bind(new_id())
    .bind(form.username.trim())
    .bind(form.display_name.trim())
    .bind(ROLE_BARBER)
    .bind(password_hash)
    .bind(now)
    .execute(&state.db)
    .await;

    if let Err(err) = result {
        let barbers = fetch_barbers(&state).await.unwrap_or_default();
        return Ok(render(AdminBarbersTemplate {
            barbers,
            errors: vec![format!("Failed to create barber: {err}")],
            success: String::new(),
            has_success: false,
            is_admin: true,
        }));
    }

    log_activity(
        &state.db,
        "barber_created",
        &format!("{} created a new barber profile.", auth.display_name),
        Some(&auth.id),
        None,
    )
    .await;

    let barbers = fetch_barbers(&state).await.unwrap_or_default();
    Ok(render(AdminBarbersTemplate {
        barbers,
        errors: Vec::new(),
        success: "Barber created successfully.".to_string(),
        has_success: true,
        is_admin: true,
    }))
}

async fn barber_stats(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let barber_id = path.into_inner();
    let barber = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, display_name, role, password_hash, active, created_at FROM users WHERE id = ?",
    )
    .bind(&barber_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let barber = match barber {
        Some(user) => BarberView {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            role: user.role,
            active: user.active == 1,
            selected: false,
        },
        None => return Ok(HttpResponse::NotFound().body("Barber not found")),
    };

    let total = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ?",
        &state,
    )
    .run_with_param(&barber.id)
    .await;
    let pending = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ? AND status = 'pending'",
        &state,
    )
    .run_with_param(&barber.id)
    .await;
    let accepted = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ? AND status = 'accepted'",
        &state,
    )
    .run_with_param(&barber.id)
    .await;
    let completed = count(
        "SELECT COUNT(*) FROM appointments WHERE barber_id = ? AND status = 'completed'",
        &state,
    )
    .run_with_param(&barber.id)
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
                  u.display_name as barber_name
           FROM appointments a
           LEFT JOIN users u ON a.barber_id = u.id
           WHERE a.barber_id = ?
           ORDER BY a.requested_at DESC
           LIMIT 8"#,
    )
    .bind(&barber.id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let recent = rows.into_iter().map(to_view).collect();

    Ok(render(AdminBarberStatsTemplate {
        barber,
        stats,
        recent,
        is_admin: true,
    }))
}

async fn cms_editor(state: web::Data<AppState>) -> Result<HttpResponse> {
    let blocks = sqlx::query_as::<_, CmsBlockRow>(
        "SELECT key, title, html FROM cms_blocks ORDER BY key",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(render(AdminCmsTemplate { blocks, is_admin: true }))
}

async fn save_cms(
    state: web::Data<AppState>,
    payload: web::Json<CmsUpdatePayload>,
    auth: web::ReqData<AuthUser>,
) -> Result<HttpResponse> {
    let payload = payload.into_inner();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO cms_blocks (key, title, html, updated_at, updated_by)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT(key) DO UPDATE SET html = excluded.html, updated_at = excluded.updated_at, updated_by = excluded.updated_by"#,
    )
    .bind(&payload.key)
    .bind(&payload.key)
    .bind(&payload.html)
    .bind(&now)
    .bind(&auth.id)
    .execute(&state.db)
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    log_activity(
        &state.db,
        "cms_updated",
        &format!("{} updated CMS block {}.", auth.display_name, payload.key),
        Some(&auth.id),
        None,
    )
    .await;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "ok": true })))
}

async fn fetch_barbers(state: &web::Data<AppState>) -> Result<Vec<BarberView>, sqlx::Error> {
    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, display_name, role, password_hash, active, created_at FROM users WHERE role IN (?, ?) ORDER BY display_name",
    )
    .bind(ROLE_BARBER)
    .bind(ROLE_ADMIN)
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|user| BarberView {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            role: user.role,
            active: user.active == 1,
            selected: false,
        })
        .collect())
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
        barber_id: row.barber_id.unwrap_or_default(),
        barber_name: row.barber_name.unwrap_or_else(|| "Unassigned".to_string()),
        latitude: row.latitude,
        longitude: row.longitude,
    }
}

fn count(query: &str, state: &web::Data<AppState>) -> CountQuery {
    CountQuery {
        query: query.to_string(),
        state: state.clone(),
    }
}

struct CountQuery {
    query: String,
    state: web::Data<AppState>,
}

impl CountQuery {
    async fn run(self) -> i64 {
        sqlx::query_scalar::<_, i64>(&self.query)
            .fetch_one(&self.state.db)
            .await
            .unwrap_or(0)
    }

    async fn run_with_param(self, param: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(&self.query)
            .bind(param)
            .fetch_one(&self.state.db)
            .await
            .unwrap_or(0)
    }
}

#[allow(dead_code)]
fn service_catalog() -> Vec<ServiceOption> {
    vec![
        ServiceOption {
            name: "Signature Cut",
            duration: "45 min",
            description: "Precision cut, styling, and lineup.",
            selected: false,
        },
        ServiceOption {
            name: "Fade & Line-Up",
            duration: "35 min",
            description: "Skin fade with sharp finishing touches.",
            selected: false,
        },
        ServiceOption {
            name: "Beard Sculpt",
            duration: "25 min",
            description: "Shape, trim, and conditioning for the beard.",
            selected: false,
        },
        ServiceOption {
            name: "Full Grooming",
            duration: "60 min",
            description: "Cut, beard, and grooming refresh.",
            selected: false,
        },
    ]
}
