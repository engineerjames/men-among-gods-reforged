use lazy_static::lazy_static;
use regex::Regex;
use std::env;

use crate::types;

use jsonwebtoken::{DecodingKey, TokenData, Validation};
use log::error;

/// Verifies the provided JWT token using the secret key from the environment variable `API_JWT_SECRET`.
///
/// # Arguments
/// * `token` - The JWT token to verify.
///
/// # Returns
/// * `Ok(TokenData<types::JwtClaims>)` if the token is valid, containing the decoded claims.
/// * `Err(String)` if the token is invalid or if the secret key is missing.
pub async fn verify_token(token: &str) -> Result<TokenData<types::JwtClaims>, String> {
    let secret = match env::var("API_JWT_SECRET") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            error!("JWT secret missing for verify_token");
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

/// Retrieves the JWT token from the `Authorization` header in the provided headers.
///
/// # Arguments
/// * `headers` - The HTTP headers from which to extract the token.
///
/// # Returns
/// * `Some(String)` containing the token if found, `None` otherwise.
pub async fn get_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.trim());

    let token = token?.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        return None;
    }

    Some(token.to_string())
}

/// Validates email format using a regex pattern. This is a basic check and may not cover
/// all valid email formats, but it should be sufficient for common use cases.
///
/// # Arguments
/// * `email` - The email address to validate.
///
/// # Returns
/// * `true` if the email is valid, `false` otherwise.
pub(crate) fn is_valid_email_regex(email: &str) -> bool {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    }
    RE.is_match(email)
}

/// Validates username based on length constraints (between 3 and 20 characters).
/// This is a simple check and can be expanded to include additional rules such as
/// allowed characters, but it serves as a basic validation for username length.
///
/// # Arguments
/// * `username` - The username to validate.
///
/// # Returns
/// * `true` if the username is valid, `false` otherwise.
pub(crate) fn is_valid_username(username: &str) -> bool {
    let len = username.chars().count();
    (3..=20).contains(&len)
}

/// Validates password format to ensure it is a valid Argon2 hash in PHC string format.
/// This is a basic check to ensure that the password is not stored in plaintext and follows
/// the expected format for Argon2 hashes. It does not verify the strength of the password or
/// the parameters used in the hash.
///
/// # Arguments
/// * `password` - The password hash to validate.
///
/// # Returns
/// * `true` if the password hash is valid, `false` otherwise.
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
    use super::{
        get_token_from_headers, is_valid_email_regex, is_valid_password, is_valid_username,
    };
    use crate::types;
    use jsonwebtoken::{EncodingKey, Header};
    use std::sync::Mutex;

    use super::verify_token;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env_var(key: &str, value: Option<&str>) -> Option<String> {
        let previous = std::env::var(key).ok();
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        previous
    }

    fn restore_env_var(key: &str, previous: Option<String>) {
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn token_from_headers_extracts_bearer() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                "Bearer abc.def.ghi".parse().unwrap(),
            );

            let token = get_token_from_headers(&headers).await;
            assert_eq!(Some("abc.def.ghi".to_string()), token);
        });
    }

    #[test]
    fn token_from_headers_accepts_raw_token() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert(
                axum::http::header::AUTHORIZATION,
                "raw.token".parse().unwrap(),
            );

            let token = get_token_from_headers(&headers).await;
            assert_eq!(Some("raw.token".to_string()), token);
        });
    }

    #[test]
    fn token_from_headers_missing_returns_none() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let headers = axum::http::HeaderMap::new();
            let token = get_token_from_headers(&headers).await;
            assert_eq!(None, token);
        });
    }

    #[test]
    fn verify_token_accepts_valid_jwt() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let _lock = ENV_LOCK.lock().unwrap();
            let previous = set_env_var("API_JWT_SECRET", Some("test-secret"));

            let claims = types::JwtClaims {
                sub: "tester".to_string(),
                exp: 1_999_999_999,
            };
            let token = jsonwebtoken::encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(b"test-secret"),
            )
            .expect("token encode");

            let result = verify_token(&token).await;
            restore_env_var("API_JWT_SECRET", previous);
            assert!(result.is_ok(), "expected valid token");
        });
    }

    #[test]
    fn verify_token_rejects_invalid_jwt() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let _lock = ENV_LOCK.lock().unwrap();
            let previous = set_env_var("API_JWT_SECRET", Some("test-secret"));

            let result = verify_token("not-a-jwt").await;
            restore_env_var("API_JWT_SECRET", previous);
            assert!(result.is_err(), "expected invalid token");
        });
    }

    #[test]
    fn verify_token_requires_secret() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let _lock = ENV_LOCK.lock().unwrap();
            let previous = set_env_var("API_JWT_SECRET", None);

            let result = verify_token("abc.def.ghi").await;
            restore_env_var("API_JWT_SECRET", previous);
            assert!(result.is_err(), "expected missing secret failure");
        });
    }

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
