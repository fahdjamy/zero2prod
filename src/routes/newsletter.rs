use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use actix_web::body::BoxBody;
use actix_web::http::header::{HeaderMap, HeaderValue};
use actix_web::http::{header, StatusCode};
use actix_web::{HttpRequest, HttpResponse, ResponseError};
use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use base64::Engine;
use secrecy::ExposeSecret;
use secrecy::Secret;
use sqlx::PgPool;

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error("Authentication Failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    // anyhow::Error provides the capability to enrich an error with additional context out of the box.
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError {
    // `status_code` is invoked by the default `error_response`
    // implementation. We are providing a bespoke `error_response` implementation
    // therefore there is no need to maintain a `status_code` implementation anymore.

    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    // actix_web::http::header provides a collection of constants
                    // for the names of several well-known/standard HTTP headers
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

pub struct Credentials {
    username: String,
    password: Secret<String>,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(body, pool, email_client, request),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn publish_newsletter(
    request: HttpRequest,
    pool: actix_web::web::Data<PgPool>,
    body: actix_web::web::Json<BodyData>,
    email_client: actix_web::web::Data<EmailClient>,
) -> Result<HttpResponse, PublishError> {
    let basic_credentials = basic_auth(request.headers())
        // Bubble up the error, performing the necessary conversion
        .map_err(|e| PublishError::AuthError(e))?;

    tracing::Span::current().record(
        "username",
        &tracing::field::display(&basic_credentials.username),
    );

    let user_id = validate_credentials(basic_credentials, &pool).await?;

    tracing::Span::current().record("user_id", &tracing::field::display(user_id.to_string()));

    let subscribers = get_confirmed_subscriber(&pool).await?;

    for subscriber in subscribers {
        match subscriber {
            Ok(sub) => {
                email_client
                    .send_email(
                        &sub.email,
                        &body.title,
                        &body.content.html,
                        &body.content.text,
                    )
                    .await
                    // .with_context is lazy unlike .context.
                    // It takes a closure as argument and the closure is only called in case of an error
                    // If the context you are adding is static - e.g.context("Oh no!") - they are equivalent.
                    // format! allocates memory on the heap to store its output string. Using context,
                    // we would be allocating that string every time we send an email out.
                    .with_context(|| format!("Failed to send newsletter to {}", sub.email))?
            }
            Err(err) => {
                tracing::warn!(
                    // We record the error chain as a structured field
                    // on the log record.
                    // ? before a value within a formatting macro, it instructs the macro to use the
                    // Debug formatting trait for that value
                    // https://stackoverflow.com/questions/74008676/question-mark-operator-before-variables-what-do-they-do-in-rust
                    error.cause_chain = ?err,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}

// An adapter between the storage layer and the domain layer
#[tracing::instrument(name = "Get confirmed subscriber", skip(pool))]
async fn get_confirmed_subscriber(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    // sqlx::query_as! maps the retrieved rows to the type specified as its first argument,
    // ConfirmedSubscriber, saving us a bunch of boilerplate.

    // We only need `Row` to map the data coming out of this query.
    // Nesting its definition inside the function itself is a simple way
    // to clearly communicate this coupling (and to ensure it doesn't
    // get used elsewhere by mistake).
    struct Row {
        email: String,
    }
    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Map into the domain type
    let confirmed_subscribers = rows
        .into_iter()
        // filter_map is a handy combinator - it returns a new iterator containing only the items
        // for which a closure returned a Some variant.
        .map(|r| match SubscriberEmail::parse(r.email) {
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
        })
        .collect();

    Ok(confirmed_subscribers)
}

fn basic_auth(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // The header value, if present, must be a valid UTF8 string
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_segment)
        .context("Failed to base64-decode 'Basic' credentials.")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    // Split into two segments, using ':' as delimiter
    let mut credentials = decoded_credentials.splitn(2, ':');

    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A username must be provided in 'Basic' auth."))?
        .to_string();

    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("A password must be provided in 'Basic' auth."))?
        .to_string();

    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

pub fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    // https://medium.com/coderhack-com/coderhack-cryptography-libraries-and-uses-in-rust-31957242299f

    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)?
    .to_string();
    Ok(Secret::new(password_hash))
}

#[tracing::instrument(
    name = "Validate credentials",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), PublishError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC string format.")?;

    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(PublishError::AuthError)
}

async fn validate_credentials(
    credentials: Credentials,
    pg_pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let row: Option<_> = sqlx::query!(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
        credentials.username
    )
    .fetch_optional(pg_pool)
    .await
    .context("Failed to retrieve user credentials")
    .map_err(PublishError::UnexpectedError)?;

    let (user_id, expected_password_hash) = get_stored_credentials(&credentials.username, &pg_pool)
        .await
        .map_err(PublishError::UnexpectedError)?
        .ok_or_else(|| PublishError::AuthError(anyhow::anyhow!("Unknown Username")))?;

    let expected_password_hash = PasswordHash::new(&expected_password_hash.expose_secret())
        .context("Failed to parse hash into PHC string format")
        .map_err(PublishError::UnexpectedError)?;

    tracing::info_span!("Verify Password Hash")
        .in_scope(|| {
            Argon2::default().verify_password(
                credentials.password.expose_secret().as_bytes(),
                &expected_password_hash,
            )
        })
        .context("Invalid password")
        .map_err(PublishError::AuthError)?;

    Ok(user_id)
}

#[tracing::instrument(name = "Get stored credentials", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT user_id, password_hash
        FROM users
        WHERE username = $1
        "#,
        username,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to retrieve stored credentials.")?
    .map(|row| (row.user_id, Secret::new(row.password_hash)));
    Ok(row)
}
