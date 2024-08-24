//! src.routes.subscriptions "//!: The double exclamation mark indicates an inner documentation comment"
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
pub async fn subscribe(
    form: web::Form<FormData>,
    // Retrieving a connection from the application state!
    db_pool: web::Data<PgPool>,
) -> HttpResponse {
    let request_id = Uuid::new_v4();
    let event = String::from("creatingNewSubscriber");

    // Spans, like logs, have an associated level
    // `info_span` creates a span at the info-level
    //
    // You can enter (and exit) a span multiple times. Closing, instead, is final:
    // it happens when the span itself is dropped.
    // This comes pretty handy when you have a unit of work that can be paused and then resumed -
    // e.g. an asynchronous task!
    let request_span = tracing::info_span!(
        "",
        %request_id,
        %event,
        // Notice that we prefixed all of them with a % symbol:
        // we are telling tracing to use their Display implementation for logging purposes
        subscriber_name = %form.name,
        subscriber_email = %form.email
    );

    let _request_span_guard = request_span.enter();

    // `Result` has two variants: `Ok` and `Err`.
    // The first for successes, the second for failures.

    // We do not call `.enter` on query_span!
    // `.instrument` takes care of it at the right moments
    // in the query future lifetime
    let query_span = tracing::info_span!(
        "query=Creating user in DB ",
        %event,
    );

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
    .instrument(query_span)
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => {
            tracing::error!(
                "requestId={}, failed to execute query: {:?}",
                request_id,
                err
            );
            HttpResponse::InternalServerError().finish()
        }
    }

    // `_request_span_guard` is dropped at the end of `subscribe`
    // That's when we "exit" the span
}
