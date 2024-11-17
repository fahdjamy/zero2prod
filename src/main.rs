use std::fmt::{Debug, Display};
use tokio::task::JoinError;
use zero2prod::configuration::get_configuration;
use zero2prod::issue_delivery_worker::run_worker_until_stopped;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

// this is a binary crate because it contains a main function
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    // Panic if we can't read configuration
    let configuration = get_configuration().expect("Failed to read configuration.");
    let application = Application::build(configuration.clone()).await?;
    let application_task = tokio::spawn(application.run_until_stopped());
    let worker_task = tokio::spawn(run_worker_until_stopped(configuration));

    // tokio::select! returns as soon as one of the two tasks completes or errors out
    // There's a pitfall to be mindful of when using tokio::select! - all selected Futures are
    // polled as a single task. This has consequences, as tokio’s documentation highlights:
    //
    // “By running all async expressions on the current task, the expressions are able to run
    // concurrently but not in parallel. This means all expressions are run on the same thread and
    // if one branch blocks the thread, all other expressions will be unable to continue.
    // If parallelism is required, spawn each async expression using tokio::spawn and pass the join
    // handle to select!.”

    tokio::select! {
        outcome = application_task => report_exit("API", outcome),
        outcome = worker_task =>  report_exit("Background worker", outcome),
    };

    Ok(())
}

fn report_exit(task_name: &str, outcome: Result<Result<(), impl Debug + Display>, JoinError>) {
    match outcome {
        Ok(Ok(())) => {
            tracing::info!("{} has exited", task_name)
        }
        Ok(Err(e)) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{} failed",
                task_name
            )
        }
        Err(e) => {
            tracing::error!(
                error.cause_chain = ?e,
                error.message = %e,
                "{}' task failed to complete",
                task_name
            )
        }
    }
}
