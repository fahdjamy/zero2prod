use std::net::TcpListener;

// `tokio::test` is the testing equivalent of `tokio::main`.
// It also spares you from having to specify the `#[test]` attribute.
//
// You can inspect what code gets generated using
// `cargo expand --test health_check` (<- name of the test file)
#[tokio::test]
async fn health_check_works() {
    // Arrange
    let address = spawn_app();

    // we need to bring in request
    // to perform HTTP requests against our app
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(&format!("{}/health_check", &address))
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

fn spawn_app() -> String {
    // “Port 0 is special-cased at the OS level:
    //  trying to bind port 0 will trigger an OS scan for an
    //  available port which will then be bound to the application”
    let address = "127.0.0.1:0";

    let listener = TcpListener::bind(address).expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    let server = zero2prod::run(listener).expect("failed to bind address");

    let _ = tokio::spawn(server);

    // return app address to the caller
    format!("127.0.0.1:{}", port)
}
