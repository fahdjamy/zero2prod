use crate::helpers::spawn_app;
use crate::login::assert_is_redirect_to;

#[tokio::test]
async fn must_be_logged_in_to_access_admin_dashboard() {
    let app = spawn_app().await;

    let response = app.get_admin_dashboard().await;

    // Assert
    assert_is_redirect_to(response, "/login");
}
