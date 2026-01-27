use actix_web::{web, HttpResponse, Result};
use askama::Template;
use serde::Deserialize;

use crate::{
    auth::new_id,
    db::log_activity,
    models::{AppointmentRow, CmsBlockRow, ServiceOption, STATUS_PENDING},
    state::AppState,
    templates::render,
};

#[derive(Clone, Debug)]
struct BarberSummary {
    id: String,
    display_name: String,
    initials: String,
    selected: bool,
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    hero_html: String,
    about_html: String,
    services_html: String,
    services: Vec<ServiceOption>,
    barbers: Vec<BarberSummary>,
}

#[derive(Clone, Debug, Default)]
struct BookingView {
    client_name: String,
    client_phone: String,
    client_email: String,
    address: String,
    scheduled_for: String,
    notes: String,
}

#[derive(Template)]
#[template(path = "book.html")]
struct BookingTemplate {
    services: Vec<ServiceOption>,
    barbers: Vec<BarberSummary>,
    form: BookingView,
    errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "book_success.html")]
struct BookingSuccessTemplate {
    appointment_id: String,
}

#[derive(Template)]
#[template(path = "barbers.html")]
struct BarbersTemplate {
    barbers: Vec<BarberSummary>,
}

#[derive(Deserialize)]
struct BookingForm {
    client_name: String,
    client_phone: String,
    client_email: Option<String>,
    address: String,
    service: String,
    scheduled_for: String,
    notes: Option<String>,
    preferred_barber_id: Option<String>,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/").route(web::get().to(home)))
        .service(web::resource("/book").route(web::get().to(show_booking)).route(web::post().to(create_booking)))
        .service(web::resource("/barbers").route(web::get().to(list_barbers)))
        .service(web::resource("/health").route(web::get().to(health)));
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

async fn home(state: web::Data<AppState>) -> Result<HttpResponse> {
    let hero_html = cms_block_html(&state, "home_hero").await;
    let about_html = cms_block_html(&state, "home_about").await;
    let services_html = cms_block_html(&state, "home_services").await;
    let services = service_catalog();
    let barbers = fetch_barbers(&state).await.unwrap_or_default();

    Ok(render(HomeTemplate {
        hero_html,
        about_html,
        services_html,
        services,
        barbers,
    }))
}

async fn show_booking(state: web::Data<AppState>) -> Result<HttpResponse> {
    let services = service_catalog();
    let barbers = fetch_barbers(&state).await.unwrap_or_default();

    Ok(render(BookingTemplate {
        services,
        barbers,
        form: BookingView::default(),
        errors: Vec::new(),
    }))
}

async fn create_booking(
    state: web::Data<AppState>,
    form: web::Form<BookingForm>,
) -> Result<HttpResponse> {
    let form = form.into_inner();
    let mut errors = Vec::new();
    if form.client_name.trim().is_empty() {
        errors.push("Full name is required.".to_string());
    }
    if form.client_phone.trim().is_empty() {
        errors.push("Phone number is required.".to_string());
    }
    if form.address.trim().is_empty() {
        errors.push("Service address is required.".to_string());
    }
    if form.service.trim().is_empty() {
        errors.push("Please select a service.".to_string());
    }
    if form.scheduled_for.trim().is_empty() {
        errors.push("Please pick a date and time.".to_string());
    }

    if !errors.is_empty() {
        let mut services = service_catalog();
        for service in &mut services {
            service.selected = form.service == service.name;
        }
        let mut barbers = fetch_barbers(&state).await.unwrap_or_default();
        let selected_id = form.preferred_barber_id.as_deref();
        for barber in &mut barbers {
            barber.selected = selected_id == Some(barber.id.as_str());
        }
        return Ok(render(BookingTemplate {
            services,
            barbers,
            form: BookingView {
                client_name: form.client_name,
                client_phone: form.client_phone,
                client_email: form.client_email.unwrap_or_default(),
                address: form.address,
                scheduled_for: form.scheduled_for,
                notes: form.notes.unwrap_or_default(),
            },
            errors,
        }));
    }

    let appointment_id = new_id();
    let now = chrono::Utc::now().to_rfc3339();
    let preferred_barber = form
        .preferred_barber_id
        .unwrap_or_default()
        .trim()
        .to_string();
    let barber_id = if preferred_barber.is_empty() {
        None
    } else {
        Some(preferred_barber)
    };

    sqlx::query(
        r#"INSERT INTO appointments
           (id, client_name, client_phone, client_email, address, service, notes, requested_at, scheduled_for, status, barber_id)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&appointment_id)
    .bind(&form.client_name)
    .bind(&form.client_phone)
    .bind(form.client_email)
    .bind(&form.address)
    .bind(&form.service)
    .bind(form.notes)
    .bind(now)
    .bind(&form.scheduled_for)
    .bind(STATUS_PENDING)
    .bind(barber_id)
    .execute(&state.db)
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    log_activity(
        &state.db,
        "appointment_created",
        &format!("New appointment requested for {}.", form.client_name),
        None,
        Some(&appointment_id),
    )
    .await;

    Ok(render(BookingSuccessTemplate { appointment_id }))
}

async fn list_barbers(state: web::Data<AppState>) -> Result<HttpResponse> {
    let barbers = fetch_barbers(&state).await.unwrap_or_default();
    Ok(render(BarbersTemplate { barbers }))
}

async fn fetch_barbers(state: &web::Data<AppState>) -> Result<Vec<BarberSummary>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, display_name FROM users WHERE role = 'barber' AND active = 1 ORDER BY display_name",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, display_name)| {
            let initials = display_name
                .split_whitespace()
                .filter_map(|part| part.chars().next())
                .take(2)
                .collect::<String>();
            BarberSummary {
                id,
                display_name,
                initials: initials.to_uppercase(),
                selected: false,
            }
        })
        .collect())
}

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

async fn cms_block_html(state: &web::Data<AppState>, key: &str) -> String {
    let row = sqlx::query_as::<_, CmsBlockRow>(
        "SELECT key, title, html FROM cms_blocks WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(&state.db)
    .await;

    row.ok()
        .flatten()
        .map(|block| block.html)
        .unwrap_or_default()
}

#[allow(dead_code)]
async fn fetch_latest_appointments(state: &web::Data<AppState>) -> Result<Vec<AppointmentRow>, sqlx::Error> {
    sqlx::query_as::<_, AppointmentRow>(
        r#"SELECT a.id, a.client_name, a.client_phone, a.client_email, a.address, a.service,
                  a.notes, a.requested_at, a.scheduled_for, a.status, a.barber_id,
                  u.display_name as barber_name
           FROM appointments a
           LEFT JOIN users u ON a.barber_id = u.id
           ORDER BY a.requested_at DESC
           LIMIT 5"#,
    )
    .fetch_all(&state.db)
    .await
}
