//! src.routes.subscriptions "//!: The double exclamation mark indicates an inner documentation comment"
use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
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
pub async fn subscribe(
    form: web::Form<FormData>,
    // Retrieving a connection from the application state!
    db_pool: web::Data<PgPool>,
) -> HttpResponse {
    let event = String::from("creatingNewSubscriber");
    let req_id = Uuid::new_v4();
    log::info!(
        "requestId={}, event={}, name={}, email={}",
        event,
        req_id,
        form.name,
        form.email,
    );

    // `Result` has two variants: `Ok` and `Err`.
    // The first for successes, the second for failures.
    // We use a `match` statement to choose what to do based
    // on the outcome.
    // We will talk more about `Result` going forward!

    match sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
    )
    // We use `get_ref` to get an immutable reference to the `PgConnection`
    // wrapped by `web::Data`.
    // Using the pool as a drop-in replacement for PgConnection
    .execute(db_pool.get_ref())
    .await
    {
        Ok(_) => {
            log::info!(
                "requestId={}, event={}, message={}",
                req_id,
                event,
                String::from("Subscriber created")
            );
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            log::error!("requestId={}, failed to execute query: {:?}", req_id, err);
            HttpResponse::InternalServerError().finish()
        }
    }
}
