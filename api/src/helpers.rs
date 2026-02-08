use lazy_static::lazy_static;
use regex::Regex;
use std::env;

use crate::types;

use jsonwebtoken::{DecodingKey, TokenData, Validation};
use log::error;

pub async fn verify_token(token: &str) -> Result<TokenData<types::JwtClaims>, String> {
    let secret = match env::var("API_JWT_SECRET") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("JWT secret missing for get_characters");
            return Err("Internal server error".to_string());
        }
    };

    let token_data = match jsonwebtoken::decode::<types::JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => data,
        Err(err) => {
            error!("JWT decode failed: {}", err);
            return Err("Unauthorized".to_string());
        }
    };

    Ok(token_data)
}

pub async fn get_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.trim().to_string());

    let token = match token {
        Some(value) => value
            .strip_prefix("Bearer ")
            .unwrap_or(&value)
            .trim()
            .to_string(),
        _ => {
            return None;
        }
    };

    Some(token)
}

pub(crate) fn is_valid_email_regex(email: &str) -> bool {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    }
    RE.is_match(email)
}

pub(crate) fn is_valid_username(username: &str) -> bool {
    let len = username.chars().count();
    (3..=20).contains(&len)
}

pub(crate) fn is_valid_password(password: &str) -> bool {
    lazy_static! {
        static ref ARGON2_RE: Regex = Regex::new(
            r"^\$(argon2(id|i|d))\$[A-Za-z0-9+/=\-_,.]+\$[A-Za-z0-9+/=\-_,.]+\$[A-Za-z0-9+/=\-_,.]+\$[A-Za-z0-9+/=\-_,.]+$"
        )
        .unwrap();
    }

    ARGON2_RE.is_match(password)
}

#[cfg(test)]
mod tests {
    use super::{is_valid_email_regex, is_valid_password, is_valid_username};

    #[test]
    fn email_validation_accepts_common_addresses() {
        let samples = [
            "user@example.com",
            "user.name+tag@example.co.uk",
            "user_name@example.io",
            "user-name@example.net",
            "u123@example.org",
        ];

        for email in samples {
            assert!(is_valid_email_regex(email), "expected valid: {email}");
        }
    }

    #[test]
    fn email_validation_rejects_invalid_addresses() {
        let samples = [
            "",
            "plainaddress",
            "@example.com",
            "user@",
            "user@example",
            "user@example.",
            "user@@example.com",
            "user name@example.com",
            "user@exa mple.com",
        ];

        for email in samples {
            assert!(!is_valid_email_regex(email), "expected invalid: {email}");
        }
    }

    #[test]
    fn username_validation_enforces_length_bounds() {
        assert!(!is_valid_username("ab"));
        assert!(is_valid_username("abc"));
        assert!(is_valid_username("valid_username"));
        assert!(is_valid_username("a".repeat(20).as_str()));
        assert!(!is_valid_username("a".repeat(21).as_str()));
    }

    #[test]
    fn password_validation_accepts_phc_hashes() {
        let samples = [
            "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$ZmFrZWhhc2g",
            "$argon2i$v=19$m=4096,t=3,p=1$c2FsdA$ZGF0YQ",
            "$argon2d$v=19$m=1024,t=2,p=2$c2FsdA$ZGF0YQ",
        ];

        for password in samples {
            assert!(is_valid_password(password), "expected valid: {password}");
        }
    }

    #[test]
    fn password_validation_rejects_plaintext_and_malformed() {
        let samples = [
            "plaintext-password",
            "short",
            "$argon2id$v=19$m=65536,t=3,p=4$onlysalt",
            "$argon2id$v=19$m=65536,t=3,p=4$ZmFrZXNhbHQ$",
            "$bcrypt$10$invalidformat",
            "$pbkdf2-sha256$missing$fields",
            "$scrypt$ln=15,r=8,p=1$c2FsdA$ZGF0YQ",
        ];

        for password in samples {
            assert!(!is_valid_password(password), "expected invalid: {password}");
        }
    }
}
