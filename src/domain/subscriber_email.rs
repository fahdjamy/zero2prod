use validator::Validate;

#[derive(Debug, Validate)]
pub struct SubscriberEmail {
    #[validate(email)]
    mail: String,
}

impl SubscriberEmail {
    pub fn parse(s: String) -> Result<SubscriberEmail, String> {
        let email = SubscriberEmail { mail: s };
        if let Err(e) = email.validate() {
            return Err(format!("Invalid email: {}", e));
        }
        Ok(email)
    }
}

impl AsRef<str> for SubscriberEmail {
    fn as_ref(&self) -> &str {
        &self.mail
    }
}

#[cfg(test)]
mod tests {
    use claims::assert_err;

    use super::SubscriberEmail;

    #[test]
    fn empty_string_is_rejected() {
        let email = "".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }

    #[test]
    fn email_missing_at_symbol_is_rejected() {
        let email = "wrong_mail.com".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }

    #[test]
    fn email_missing_subject_is_rejected() {
        let email = "@domain.com".to_string();
        assert_err!(SubscriberEmail::parse(email));
    }
}
