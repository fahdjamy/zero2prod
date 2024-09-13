//! src.routes.subscriptions "//!: The double exclamation mark indicates an inner documentation comment"
use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::domain::NewSubscriber;
use crate::email_client::EmailClient;

// this is a Library create because it doesn't contain a main function
#[derive(serde::Deserialize)]
pub struct FormData {
    pub name: String,
    pub email: String,
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
    skip(form, db_pool, email_client),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    // Retrieving a connection from the application state!
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
) -> HttpResponse {
    if !is_valid_name(&form.name) {
        return HttpResponse::BadRequest().finish();
    }
    // `web::Form` is a wrapper around `FormData` (web::Form is a struct tuple)
    // `form.0` gives us access to the underlying `FormData`
    let new_subscriber = match form.0.try_into() {
        Ok(name) => name,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if insert_subscriber(&db_pool, &new_subscriber).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    };

    // Email the new subscriber.
    let result = send_confirmation(&email_client, new_subscriber).await;
    if result.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
}

#[tracing::instrument(name = "Creating new subscriber", skip(email_client, new_subscriber))]
async fn send_confirmation(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
) -> Result<(), reqwest::Error> {
    let confirmation_link = "https://there-is-no-such-domain.com/subscriptions/confirm";

    // Send a confirmation email to the new subscriber.
    let subject = "Welcome!";
    let html_body = &format!(
        "Welcome to our newsletter!<br />\
                Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    let text_content = &format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email(new_subscriber.email, subject, html_body, text_content)
        .await
}

#[tracing::instrument(name = "Saving a new subscriber in DB", skip(new_subscriber, pool))]
pub async fn insert_subscriber(
    pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending')
            "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
    )
    .execute(pool)
    .await
    .map_err(|err| {
        tracing::error!("failed to execute query: {:?}", err);
        err
        // Using the `?` operator to return early
        // if the function failed, returning a sqlx::Error
    })?;
    Ok(())
}

/// Returns `true` if the input satisfies all our validation constraints
/// on subscriber names, `false` otherwise.
pub fn is_valid_name(s: &str) -> bool {
    // `.trim()` returns a view over the input `s` without trailing
    // whitespace-like characters.
    // `.is_empty` checks if the view contains any character.
    let is_empty_or_whitespace = s.trim().is_empty();

    // A grapheme is defined by the Unicode standard as a "user-perceived"
    // character: `å` is a single grapheme, but it is composed of two characters
    // (`a` and `̊`).
    //
    // `graphemes` returns an iterator over the graphemes in the input `s`.
    // `true` specifies that we want to use the extended grapheme definition set,
    // the recommended one.
    let is_too_long = s.graphemes(true).count() > 256;

    // Iterate over all characters in the input `s` to check if any of them matches
    // one of the characters in the forbidden array.
    let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
    let contains_forbidden_characters = s.chars().any(|g| forbidden_characters.contains(&g));

    // Return `false` if any of our conditions have been violated
    !(is_empty_or_whitespace || is_too_long || contains_forbidden_characters)
}
