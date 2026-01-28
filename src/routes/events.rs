use actix_files::NamedFile;
use actix_web::{http::header, middleware::from_fn, web, HttpResponse, Result};
use actix_web_httpauth::middleware::HttpAuthentication;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::{
    auth::{basic_validator, logout_guard},
    state::{AppState, ServerEvent},
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/events")
            .wrap(HttpAuthentication::basic(basic_validator))
            .wrap(from_fn(logout_guard))
            .route(web::get().to(stream_events)),
    )
    .service(
        web::resource("/status/{id}/events").route(web::get().to(stream_status_events)),
    )
    .service(web::resource("/sw.js").route(web::get().to(service_worker)));
}

async fn service_worker() -> Result<NamedFile> {
    Ok(NamedFile::open("./static/sw.js")?)
}

async fn stream_events(state: web::Data<AppState>) -> HttpResponse {
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => Some(Ok::<web::Bytes, actix_web::Error>(event_to_bytes(&event))),
        Err(_) => None,
    });

    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .streaming(stream)
}

fn event_to_bytes(event: &ServerEvent) -> web::Bytes {
    let payload = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    web::Bytes::from(format!("event: update\ndata: {}\n\n", payload))
}

#[derive(serde::Serialize)]
struct PublicStatusEvent {
    appointment_id: Option<String>,
    status: Option<String>,
    service: Option<String>,
    scheduled_for: Option<String>,
    barber_name: Option<String>,
}

async fn stream_status_events(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> HttpResponse {
    let appointment_id = path.into_inner();
    let rx = state.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |result| {
        let event = match result {
            Ok(event) => event,
            Err(_) => return None,
        };
        if event.appointment_id.as_deref() != Some(&appointment_id) {
            return None;
        }
        let public = PublicStatusEvent {
            appointment_id: event.appointment_id,
            status: event.status,
            service: event.service,
            scheduled_for: event.scheduled_for,
            barber_name: event.barber_name,
        };
        Some(Ok::<web::Bytes, actix_web::Error>(public_event_to_bytes(&public)))
    });

    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/event-stream"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .streaming(stream)
}

fn public_event_to_bytes(event: &PublicStatusEvent) -> web::Bytes {
    let payload = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    web::Bytes::from(format!("event: update\ndata: {}\n\n", payload))
}
