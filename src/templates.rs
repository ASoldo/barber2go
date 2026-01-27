use actix_web::HttpResponse;
use askama::Template;

pub fn render<T: Template>(template: T) -> HttpResponse {
    match template.render() {
        Ok(body) => HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(body),
        Err(err) => {
            log::error!("Template render error: {err}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
