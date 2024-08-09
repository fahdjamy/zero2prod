use actix_web::HttpResponse;

pub async fn check_health() -> HttpResponse {
    "checking health.....".to_string();
    HttpResponse::Ok().finish()
}
