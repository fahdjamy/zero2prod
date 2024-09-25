use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::routes::error_chain_fmt;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use anyhow::Context;
use sqlx::PgPool;

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[derive(thiserror::Error)]
pub enum PublishError {
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
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

pub async fn publish_newsletter(
    body: actix_web::web::Json<BodyData>,
    pool: actix_web::web::Data<PgPool>,
    email_client: actix_web::web::Data<EmailClient>,
) -> Result<HttpResponse, PublishError> {
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
