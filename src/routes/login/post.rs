use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::error_chain_fmt;
use actix_web::error::InternalError;
use actix_web::http::header::LOCATION;
use actix_web::web::Form;
use actix_web::{web, HttpResponse};
use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, Secret};
use sha2::digest::KeyInit;
use sha2::Sha256;
use sqlx::PgPool;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

#[derive(thiserror::Error)]
pub enum LoginError {
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[tracing::instrument(
    skip(form, pool),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
pub async fn login(
    form: Form<FormData>,
    pool: web::Data<PgPool>,
    secret: web::Data<Secret<String>>,
) -> Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };
    tracing::Span::current().record("username", tracing::field::display(&credentials.username));
    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current().record("user_id", tracing::field::display(&user_id));
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish())
        }
        Err(error) => {
            let e = match error {
                AuthError::InvalidCredentials(_) => LoginError::AuthError(error.into()),
                AuthError::UnexpectedError(_) => LoginError::UnexpectedError(error.into()),
            };
            let query_string = format!("error={}", e);
            let hmac_tag = {
                let mut mac =
                    Hmac::<Sha256>::new_from_slice(secret.expose_secret().as_bytes()).unwrap();
                mac.update(query_string.as_bytes());
                // Generate the HMAC tag
                // Finalize and obtain the code: Generate the final HMAC tag using mac.finalize().
                // into_bytes() converts response to byte arrays.
                mac.finalize().into_bytes()
            };

            // {:x?}
            // Checks for LowerHex: If the value implements LowerHex, it uses that implementation to format the value.
            // If the value is a byte slice ([u8]), it iterates over the bytes and prints each byte in hexadecimal format with a space separator.
            let response = HttpResponse::SeeOther()
                .insert_header((
                    LOCATION,
                    format!("/login?{}&tag={:x?}", query_string, hmac_tag),
                ))
                .finish();
            Err(InternalError::from_response(e, response))
        }
    }
}
