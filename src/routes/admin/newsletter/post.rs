use crate::authentication::UserId;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::idempotency::{get_saved_response, save_response, IdempotencyKey};
use crate::utils::{e400, e500, see_other};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

#[tracing::instrument(
    name = "Publish a newsletter issue",
    skip(form, pool, email_client, user_id)
)]
pub async fn publish_newsletter(
    pool: web::Data<PgPool>,
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
    email_client: web::Data<EmailClient>,
) -> Result<HttpResponse, actix_web::Error> {
    // Consume the ReqData and return its wrapped value.
    let user_id = user_id.into_inner();
    // Destructure the form to avoid upsetting the borrow-checker
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    // return early if we have a save newsletter response
    if let Some(saved_response) = get_saved_response(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        FlashMessage::info("The newsletter issue has been published successfully.").send();
        return Ok(saved_response);
    }

    let confirmed_subs = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in confirmed_subs {
        match subscriber {
            Ok(sub) => email_client
                .send_email(&sub.email, &title, &html_content, &text_content)
                .await
                .with_context(|| format!("Failed to send newsletter issue to {}", sub.email))
                .map_err(e500)?,
            Err(err) => {
                tracing::warn!(
                    error.cause_chain = ?err,
                    error.message = %err,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }
    FlashMessage::info("The newsletter issue has been published!").send();
    let response = see_other("/admin/newsletters");
    let response = save_response(&pool, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

async fn get_confirmed_subscribers(
    pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| match SubscriberEmail::parse(r.email) {
        Ok(email) => Ok(ConfirmedSubscriber { email }),
        Err(err) => Err(anyhow::anyhow!(err)),
    })
    .collect();
    Ok(confirmed_subscribers)
}
