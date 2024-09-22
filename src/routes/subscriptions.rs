//! src.routes.subscriptions "//!: The double exclamation mark indicates an inner documentation comment"
use actix_web::{web, HttpResponse};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{Error, Executor, PgPool, Postgres, Transaction};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::domain::NewSubscriber;
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;

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
    skip(form, db_pool, email_client, base_url),
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
    base_url: web::Data<ApplicationBaseUrl>,
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

    let mut transaction = match db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscriber_id = match insert_subscriber(&mut transaction, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let confirmation_token = generate_subscription_token();

    if store_user_subscription_token(&mut transaction, subscriber_id, &confirmation_token)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    if transaction.commit().await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    // Email the new subscriber.
    let result = send_confirmation(
        &base_url.0,
        &email_client,
        new_subscriber,
        &confirmation_token,
    )
    .await;
    if result.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
}

#[tracing::instrument(
    name = "Creating new subscriber",
    skip(email_client, new_subscriber, base_url)
)]
async fn send_confirmation(
    base_url: &str,
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    confirmation_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, confirmation_token
    );

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

#[tracing::instrument(name = "Creating new subscriber", skip(db_pool, subscriber_id))]
async fn store_user_subscription_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), Error> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );

    transaction.execute(query).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        return e;
    })?;
    Ok(())
}

#[tracing::instrument(name = "Saving a new subscriber in DB", skip(new_subscriber, pool))]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
    INSERT INTO subscriptions (id, email, name, subscribed_at, status)
    VALUES ($1, $2, $3, $4, 'pending')
            "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
    );

    transaction.execute(query).await.map_err(|err| {
        tracing::error!("failed to execute query: {:?}", err);
        return err;
        // Using the `?` operator to return early
        // if the function failed, returning a sqlx::Error
    })?;
    Ok(subscriber_id)
}

/// Generate a random 25-characters-long case-sensitive subscription token.
fn generate_subscription_token() -> String {
    // Retrieve the lazily-initialized thread-local random number generator, seeded by the system.
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        // Using 25 characters we get roughly ~10^45 possible tokens
        .take(25)
        .collect()
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
