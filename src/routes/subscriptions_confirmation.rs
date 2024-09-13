use actix_web::{web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct QueryParams {
    pub subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(_query_params))]
pub async fn confirm(_query_params: web::Query<QueryParams>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
