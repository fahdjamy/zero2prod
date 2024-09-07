use reqwest::Client;
use secrecy::{ExposeSecret, Secret};

use crate::domain::SubscriberEmail;

#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    to: &'a str,
    from: &'a str,
    subject: &'a str,
    html_body: &'a str,
    text_body: &'a str,
}

#[derive(Clone)]
pub struct EmailClient {
    base_url: String,
    http_client: Client,
    sender: SubscriberEmail,
    authorization_token: Secret<String>,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: Secret<String>,
        time_out: std::time::Duration,
    ) -> Self {
        let client = Client::builder().timeout(time_out).build().unwrap();
        Self {
            sender,
            base_url,
            authorization_token,
            http_client: client,
        }
    }

    /// Create a test instance of `EmailClient`.
    pub fn email_client(
        base_url: String,
        sender: SubscriberEmail,
        auth_token: Secret<String>,
    ) -> EmailClient {
        let time_out = std::time::Duration::from_millis(200);
        EmailClient::new(base_url, sender, auth_token, time_out)
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/email", self.base_url);

        let request_body = SendEmailRequest {
            subject,
            to: recipient.as_ref(),
            html_body: html_content,
            text_body: text_content,
            from: self.sender.as_ref(),
        };

        let response = self
            .http_client
            .post(&url)
            .header(
                "X-Postmark-Server-Token",
                self.authorization_token.expose_secret(),
            )
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(_) => Ok(()),
            Err(err) => {
                eprintln!("Error sending email: {}", err); // Print error to stderr
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use claims::assert_ok;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::Secret;
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            // Try parsing the body as a json value
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                // Check that all the mandatory fields are populated
                // without inspecting the field values
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("HtmlBody").is_some()
                    && body.get("TextBody").is_some()
            } else {
                // If parsing failed, do not match the request
                false
            }
        }
    }

    #[tokio::test]
    async fn send_email_sends_expected_request_to_base_url() {
        // Arrange
        // wiremock::MockServer is a full-blown HTTP server.
        // MockServer::start asks the operating system for a random available port and spins up the
        // server on a background thread, ready to listen for incoming requests.
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let base_url = mock_server.uri();
        let email_client = EmailClient::email_client(base_url, sender, Secret::new(Faker.fake()));

        let subscriber_email = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let subject: String = Sentence(1..2).fake();
        let content: String = Paragraph(1..10).fake();

        // Out of the box, wiremock::MockServer returns 404 Not Found to all incoming requests.
        // We instruct the mock server to behave differently by mounting a Mock.
        Mock::given(header_exists("X-Postmark-Server-Token"))
            .and(header("Content-Type", "application/json"))
            .and(path("/email"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            // We expect the mock to be called at least once.
            // If that does not happen, the `MockServer` will panic on shutdown,
            // causing the whole test to fail.
            .expect(1)
            .mount(&mock_server)
            .await;

        let response = email_client
            .send_email(subscriber_email, &subject, &content, &content)
            .await;

        // Assert
        assert_ok!(response)
    }
}
