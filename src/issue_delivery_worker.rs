use crate::configuration::Settings;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::startup::get_connection_pool;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::time::Duration;
use tracing::field::display;
use tracing::Span;
use uuid::Uuid;

pub async fn run_worker_until_stopped(configuration: Settings) -> Result<(), anyhow::Error> {
    let connection_pool = get_connection_pool(&configuration.database);

    let email_client = configuration.email_client.client();
    worker_loop(connection_pool, email_client).await
}

type PgTransaction = Transaction<'static, Postgres>;

async fn worker_loop(pg_pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pg_pool, &email_client).await {
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(_) => {
                // if we experience a transient failure18, we need to sleep for a while to
                // improve our future chances of success. This could be further refined by
                // introducing an exponential backoff with jitter.
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(ExecutionOutcome::TaskCompleted) => {}
        }
    }
}

pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}

#[tracing::instrument(
    skip_all,
    fields(
        newsletter_issue_id=tracing::field::Empty,
        subscriber_email=tracing::field::Empty
    ),
    err
)]
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    let task = deque_task(pool).await?;
    if task.is_none() {
        return Ok(ExecutionOutcome::EmptyQueue);
    }
    if let Some((transaction, issue_id, email)) = task {
        Span::current()
            .record("newsletter_issue_id", &display(issue_id))
            .record("subscriber_email", &display(&email));
        match SubscriberEmail::parse(email.clone()) {
            Ok(email) => {
                let issue = get_issue(&pool, issue_id).await?;
                if let Err(e) = email_client
                    .send_email(
                        &email,
                        &issue.title,
                        &issue.html_content,
                        &issue.text_content,
                    )
                    .await
                {
                    tracing::error!(
                        error.cause_chain = ?e,
                        error.message = %e,
                        "Failed to deliver issue to a confirmed subscriber. Skipping.",
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    error.cause_chain = ?e,
                    error.message = %e,
                    "Skipping a confirmed subscriber. Their stored details are invalid.",
                )
            }
        }
        delete_task(transaction, issue_id, &email).await?;
    }
    Ok(ExecutionOutcome::TaskCompleted)
}

#[tracing::instrument(skip_all)]
async fn deque_task(pool: &PgPool) -> Result<Option<(PgTransaction, Uuid, String)>, anyhow::Error> {
    let mut tx: PgTransaction = pool.begin().await?;
    let row = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email
        FROM issue_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(r) = row {
        Ok(Some((tx, r.newsletter_issue_id, r.subscriber_email)))
    } else {
        Ok(None)
    }
}

#[tracing::instrument(skip_all)]
async fn delete_task(
    mut transaction: PgTransaction,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE 
            newsletter_issue_id = $1 AND
            subscriber_email = $2 
        "#,
        issue_id,
        email
    );
    transaction.execute(query).await?;
    transaction.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content
        FROM newsletter_issues
        WHERE
            newsletter_issue_id = $1"#,
        issue_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}
