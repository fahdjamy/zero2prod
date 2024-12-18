use crate::authentication;
use crate::authentication::{validate_credentials, AuthError, Credentials, UserId};
use crate::routes::dashboard::get_username;
use crate::utils::{e500, see_other};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    new_password: Secret<String>,
    current_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    pg_pool: web::Data<PgPool>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();

    let new_password_len = form.0.new_password.expose_secret().len();
    if !(12..=129).contains(&new_password_len) {
        FlashMessage::error("The password size must be between 12 and 129").send();
        return Ok(see_other("/admin/password"));
    }

    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("The new password doesn't match").send();
        return Ok(see_other("/admin/password"));
    };

    let username = get_username(*user_id, &pg_pool).await.map_err(e500)?;
    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };

    if let Err(err) = validate_credentials(credentials, &pg_pool).await {
        return match err {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect").send();
                Ok(see_other("/admin/password"))
            }
            AuthError::UnexpectedError(_) => Err(e500(err)),
        };
    };
    authentication::change_password(&pg_pool, *user_id, form.0.new_password)
        .await
        .map_err(e500)?;
    FlashMessage::error("Changed password successfully").send();
    Ok(see_other("/admin/password"))
}
