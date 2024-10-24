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
    // Step 1; Try to log in
    assert_is_redirect_to(response, "/login");

    // Step 2; Follow the redirect and check that error is set
    let html_page = app.get_login_html().await;
    assert!(html_page.contains(r#"<p><i>Authentication failed</i></p>"#));

    let html_page = app.get_login_html().await;
    // Step 3; Reload log in page and error shouldn't be there anymore
    assert!(!html_page.contains(r#"<p><i>Authentication failed</i></p>"#));
}

#[tokio::test]
pub async fn redirect_to_admin_after_login_success() {
    let app = spawn_app().await;

    let login_body = serde_json::json!({
        "username": &app.test_user.username,
        "password": &app.test_user.password,
    });

    let response = app.post_login(&login_body).await;
    assert_is_redirect_to(response, "/admin/dashboard");

    // follow redirect url
    let html_page = app.get_admin_dashboard_html().await;
    assert!(html_page.contains(&format!("Welcome {}", &app.test_user.username)));
}

pub fn assert_is_redirect_to(response: reqwest::Response, location: &str) {
    assert_eq!(response.status().as_u16(), 303);
    assert_eq!(response.headers().get("Location").unwrap(), location);
}
