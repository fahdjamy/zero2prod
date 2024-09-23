//! src.routes.subscriptions "//!: The double exclamation mark indicates an inner documentation comment"

use actix_web::http::StatusCode;
use actix_web::{web, HttpResponse, ResponseError};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::fmt::Formatter;
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;

pub enum SubscribeError {
    PoolError(sqlx::Error),
    ValidationError(String),
    SendEmailError(reqwest::Error),
    StoreTokenError(StoreTokenError),
    InsertSubscriberError(sqlx::Error),
    TransactionCommitError(sqlx::Error),
}

impl From<reqwest::Error> for SubscribeError {
    fn from(e: reqwest::Error) -> Self {
        Self::SendEmailError(e)
    }
}

impl From<StoreTokenError> for SubscribeError {
    fn from(e: StoreTokenError) -> Self {
        Self::StoreTokenError(e)
    }
}

impl From<String> for SubscribeError {
    fn from(e: String) -> Self {
        Self::ValidationError(e)
    }
}

impl std::fmt::Display for SubscribeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscribeError::ValidationError(err) => write!(f, "{}", err),
            SubscribeError::PoolError(_) => write!(f, "Failed to acquire database connection pool"),
            SubscribeError::InsertSubscriberError(_) => {
                write!(f, "Failed to save the new subscriber")
            }
            SubscribeError::TransactionCommitError(_) => {
                write!(
                    f,
                    "Failed to commit all changes and save new subscriber information plus token"
                )
            }
            SubscribeError::SendEmailError(_) => write!(f, "Failed to sent confirmation error"),
            SubscribeError::StoreTokenError(_) => write!(
                f,
                "Failed to save confirmation token for then new subscriber"
            ),
        }
    }
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for SubscribeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // &str does not implement `Error` - we consider it the root cause
            SubscribeError::ValidationError(_) => None,
            SubscribeError::PoolError(e) => Some(e),
            SubscribeError::SendEmailError(e) => Some(e),
            SubscribeError::InsertSubscriberError(e) => Some(e),
            SubscribeError::TransactionCommitError(e) => Some(e),
            SubscribeError::StoreTokenError(e) => Some(e),
        }
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::PoolError(_)
            | SubscribeError::SendEmailError(_)
            | SubscribeError::InsertSubscriberError(_)
            | SubscribeError::TransactionCommitError(_)
            | SubscribeError::StoreTokenError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Database error encountered while storing a subscription token"
        )
    }
}

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::error::Error for StoreTokenError {
    // source is useful when writing code that needs to handle a variety of errors: it provides
    // a structured way to navigate the error chain without having to know anything about
    // the specific error type you are working with.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // The compiler transparently casts `&sqlx::Error` into a `&dyn Error`
        Some(&self.0)
    }
}

// this is a Library create because it doesn't contain a main function
#[derive(serde::Deserialize)]
pub struct FormData {
    pub name: String,
    pub email: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self { email, name })
    }
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
) -> Result<HttpResponse, SubscribeError> {
    // `web::Form` is a wrapper around `FormData` (web::Form is a struct tuple)
    // `form.0` gives us access to the underlying `FormData`
    let new_subscriber = form.0.try_into()?;

    let mut transaction = db_pool.begin().await.map_err(SubscribeError::PoolError)?;

    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .map_err(SubscribeError::InsertSubscriberError)?;

    // generate a confirmation token to be used to make user from pending to confirmed
    let subscription_token = generate_subscription_token();

    // store the user's generated subscription token
    // The `?` operator transparently invokes the `Into` trait
    // on our behalf - we don't need an explicit `map_err` anymore.
    store_user_subscription_token(&mut transaction, subscriber_id, &subscription_token).await?;

    transaction
        .commit()
        .await
        .map_err(SubscribeError::TransactionCommitError)?;

    // Email the new subscriber.
    send_confirmation(
        &base_url.0,
        &email_client,
        new_subscriber,
        &subscription_token,
    )
    .await?;
    Ok(HttpResponse::Ok().finish())
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

#[tracing::instrument(name = "Creating new subscriber", skip(transaction, subscriber_id))]
async fn store_user_subscription_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );

    transaction.execute(query).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        // Wrapping the underlying error with our custom StoreTokenError error.
        return StoreTokenError(e);
    })?;
    Ok(())
}

#[tracing::instrument(
    name = "Saving a new subscriber in DB",
    skip(new_subscriber, transaction)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
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

fn error_chain_fmt(err: &impl std::error::Error, format: &mut Formatter<'_>) -> std::fmt::Result {
    write!(format, "{}\n", err)?;

    let mut current = err.source();

    // Iterates over the whole chain of errors that led to the failure we are trying to print.
    while let Some(cause) = current {
        writeln!(format, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
