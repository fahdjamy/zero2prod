use crate::helpers::spawn_app;

#[tokio::test]
async fn an_error_flash_message_is_set_on_login_failure() {
    let app = spawn_app().await;

    // Act
    let login_body = serde_json::json!({
        "username": "username",
        "password": "password",
    });
    let response = app.post_login(&login_body).await;

    // Assert
    assert_is_redirect_to(response, "/login");
}

pub fn assert_is_redirect_to(response: reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
