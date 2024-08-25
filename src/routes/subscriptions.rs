//! src.routes.subscriptions "//!: The double exclamation mark indicates an inner documentation comment"
use crate::telemetry::init_subscriber;
use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use tracing::Instrument;
use uuid::Uuid;

// this is a Library create because it doesn't contain a main function
#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

/// Extract form data using serde.
/// This handler get called only if content type is *x-www-form-urlencoded*
/// and content of the request could be deserialized to a `FormData` struct

// #[tracing::instrument] creates a span at the beginning of the function invocation and
// automatically attaches all arguments passed to the function to the context of the span -
// in our case, form and db_pool. Often function arguments won’t be displayable on log records
// (e.g.pool) or like to specify more explicitly what should/how
// they should be captured (e.g. naming each field of form) -
// we explicitly tell tracing to ignore them using the skip directive.
#[tracing::instrument(
    name = "Creating new subscriber",
    skip(form, db_pool),
    fields(
        request_id = %Uuid::new_v4(),
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    // Retrieving a connection from the application state!
    db_pool: web::Data<PgPool>,
) -> HttpResponse {
    match insert_subscriber(&db_pool, &form).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(name = "Saving a new subscriber in DB", skip(form, pool))]
pub async fn insert_subscriber(pool: &PgPool, form: &FormData) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at)
    VALUES ($1, $2, $3, $4)
            "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    .execute(pool)
    .await
    .map_err(|err| {
        tracing::error!("requestId={}, failed to execute query: {:?}", err);
        err
        // Using the `?` operator to return early
        // if the function failed, returning a sqlx::Error
    })?;
    Ok(())
}
