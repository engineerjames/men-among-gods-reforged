use jsonwebtoken::{DecodingKey, TokenData, Validation};
use lazy_static::lazy_static;
use log::error;
use mag_core::types::JwtClaims;
use regex::Regex;

/// Verifies the provided JWT token using the supplied signing secret.
///
/// The secret is provided by the caller (typically cached on `ApiState`) so this
/// function does not perform any process-environment lookups.
///
/// # Arguments
/// * `token` - The JWT token to verify.
/// * `secret` - HMAC signing secret (raw bytes) for HS256 validation.
///
/// # Returns
/// * `Ok(TokenData<JwtClaims>)` if the token is valid, containing the decoded claims.
/// * `Err(String)` with a sanitized error message if the token is invalid.
pub fn verify_token(token: &str, secret: &[u8]) -> Result<TokenData<JwtClaims>, String> {
    let token_data = match jsonwebtoken::decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    ) {
        Ok(data) => data,
        Err(err) => {
            error!("JWT decode failed: {}", err);
            return Err("Unauthorized".to_owned());
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
pub fn get_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|s| s.trim());

    let token = token?.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        return None;
    }

    Some(token.to_owned())
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

/// Validates that a password reset code is exactly 6 ASCII digits.
///
/// # Arguments
/// * `code` - The reset code to validate.
///
/// # Returns
/// * `true` if the code is valid, `false` otherwise.
pub(crate) fn is_valid_reset_code(code: &str) -> bool {
    code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
}

pub(crate) const MIN_CHARACTER_NAME_LEN: usize = 4;
pub(crate) const MAX_CHARACTER_NAME_LEN: usize = 15;
pub(crate) const MAX_DESCRIPTION_LEN: usize = 200;

/// Normalizes and validates a player-visible character name.
///
/// The legacy `CmdSetUser` flow accepted ASCII letters only, title-cased the
/// first letter, rejected `Self`, and effectively capped names at 15 bytes via
/// the bad-name check.
///
/// # Arguments
/// * `name` - Raw user-provided character name.
///
/// # Returns
/// * `Ok(String)` with the canonical display name.
/// * `Err(String)` when the name violates legacy-compatible format rules.
pub(crate) fn normalize_character_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let len = trimmed.len();

    if len < MIN_CHARACTER_NAME_LEN {
        return Err(format!(
            "Character name must be at least {} characters",
            MIN_CHARACTER_NAME_LEN
        ));
    }

    if len > MAX_CHARACTER_NAME_LEN {
        return Err(format!(
            "Character name must be at most {} characters",
            MAX_CHARACTER_NAME_LEN
        ));
    }

    if !trimmed.as_bytes().iter().all(u8::is_ascii_alphabetic) {
        return Err("Character name must contain ASCII letters only".to_owned());
    }

    let mut normalized = trimmed.to_ascii_lowercase();
    if let Some(first) = normalized.get_mut(0..1) {
        first.make_ascii_uppercase();
    }

    if normalized == "Self" {
        return Err("Character name is reserved".to_owned());
    }

    Ok(normalized)
}

/// Validates a normalized character name against banned substring patterns.
///
/// # Arguments
/// * `name` - Canonical character name from [`normalize_character_name`].
/// * `bad_names` - Banned name patterns loaded from KeyDB.
///
/// # Returns
/// * `Ok(())` when no pattern matches.
/// * `Err(String)` when the name contains a banned pattern.
pub(crate) fn validate_character_name_bad_patterns(
    name: &str,
    bad_names: &[String],
) -> Result<(), String> {
    let name_lc = name.to_ascii_lowercase();
    for pattern in bad_names {
        let pattern = pattern
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();

        if pattern.len() >= 3 && name_lc.contains(&pattern) {
            return Err("Character name is not allowed".to_owned());
        }
    }

    Ok(())
}

pub(crate) fn default_character_description(name: &str) -> String {
    let name = name.trim();
    // Keep it printable ASCII (server will sanitize to printable ASCII anyway).
    // Must contain the player's name and be > 12 characters.
    format!(
        "{} is a new adventurer. {} looks somewhat nondescript.",
        name, name
    )
}

pub(crate) fn validate_character_description(name: &str, description: &str) -> Result<(), String> {
    let name = name.trim();
    let description = description.trim();

    if name.is_empty() {
        return Err("Character name is required".to_owned());
    }

    if description.len() < 10 {
        return Err("Description must be at least 10 characters".to_owned());
    }

    if description.len() > MAX_DESCRIPTION_LEN {
        return Err(format!(
            "Description must be at most {} characters",
            MAX_DESCRIPTION_LEN
        ));
    }

    if !description
        .as_bytes()
        .iter()
        .copied()
        .all(|b| (32..=126).contains(&b))
    {
        return Err("Description must be ASCII-only (printable characters)".to_owned());
    }

    if description.contains('"') {
        return Err("Description must not contain double quotes".to_owned());
    }

    if !description.contains(name) {
        return Err("Description must contain the character name".to_owned());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        default_character_description, get_token_from_headers, is_valid_email_regex,
        is_valid_password, is_valid_reset_code, is_valid_username, normalize_character_name,
        validate_character_description, validate_character_name_bad_patterns,
    };
    use jsonwebtoken::{EncodingKey, Header};
    use mag_core::types::JwtClaims;

    use super::verify_token;

    #[test]
    fn token_from_headers_extracts_bearer() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer abc.def.ghi".parse().unwrap(),
        );

        let token = get_token_from_headers(&headers);
        assert_eq!(Some("abc.def.ghi".to_owned()), token);
    }

    #[test]
    fn token_from_headers_does_not_accept_raw_token() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "raw.token".parse().unwrap(),
        );

        let token = get_token_from_headers(&headers);
        assert_eq!(None, token);
    }

    #[test]
    fn token_from_headers_missing_returns_none() {
        let headers = axum::http::HeaderMap::new();
        let token = get_token_from_headers(&headers);
        assert_eq!(None, token);
    }

    #[test]
    fn verify_token_accepts_valid_jwt() {
        let claims = JwtClaims {
            sub: "tester".to_owned(),
            exp: 1_999_999_999,
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"test-secret"),
        )
        .expect("token encode");

        let result = verify_token(&token, b"test-secret");
        assert!(result.is_ok(), "expected valid token");
    }

    #[test]
    fn verify_token_rejects_invalid_jwt() {
        let result = verify_token("not-a-jwt", b"test-secret");
        assert!(result.is_err(), "expected invalid token");
    }

    #[test]
    fn verify_token_rejects_wrong_secret() {
        let claims = JwtClaims {
            sub: "tester".to_owned(),
            exp: 1_999_999_999,
        };
        let token = jsonwebtoken::encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"correct-secret"),
        )
        .expect("token encode");

        let result = verify_token(&token, b"wrong-secret");
        assert!(result.is_err(), "expected wrong-secret failure");
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

    #[test]
    fn character_name_normalization_title_cases_ascii_letters() {
        assert_eq!(normalize_character_name("  aLiCe  ").unwrap(), "Alice");
    }

    #[test]
    fn character_name_validation_rejects_too_short_or_long() {
        assert!(normalize_character_name("Ada").is_err());
        assert!(normalize_character_name("abcdefghijklmnop").is_err());
    }

    #[test]
    fn character_name_validation_rejects_non_letters() {
        assert!(normalize_character_name("Alice1").is_err());
        assert!(normalize_character_name("Alice_Bob").is_err());
    }

    #[test]
    fn character_name_validation_rejects_self() {
        assert!(normalize_character_name("self").is_err());
        assert!(normalize_character_name("Self").is_err());
    }

    #[test]
    fn character_name_bad_patterns_rejects_substring_matches() {
        let bad_names = vec!["bad".to_owned(), " no pe ".to_owned()];
        assert!(validate_character_name_bad_patterns("Baddie", &bad_names).is_err());
        assert!(validate_character_name_bad_patterns("Noper", &bad_names).is_err());
        assert!(validate_character_name_bad_patterns("Alice", &bad_names).is_ok());
    }

    #[test]
    fn description_validation_accepts_valid() {
        let name = "TestHero";
        let desc = "TestHero is a brave warrior.";
        assert!(validate_character_description(name, desc).is_ok());
    }

    #[test]
    fn description_validation_rejects_non_ascii() {
        let name = "TestHero";
        let desc = "TestHero is brave ☃";
        assert!(validate_character_description(name, desc).is_err());
    }

    #[test]
    fn description_validation_rejects_missing_name() {
        let name = "TestHero";
        let desc = "A brave warrior who travels.";
        assert!(validate_character_description(name, desc).is_err());
    }

    #[test]
    fn description_validation_rejects_double_quotes() {
        let name = "TestHero";
        let desc = "TestHero says \"hello\" to everyone.";
        assert!(validate_character_description(name, desc).is_err());
    }

    #[test]
    fn default_description_is_valid() {
        let name = "TestHero";
        let desc = default_character_description(name);
        assert!(validate_character_description(name, &desc).is_ok());
    }

    #[test]
    fn reset_code_accepts_six_digits() {
        assert!(is_valid_reset_code("000000"));
        assert!(is_valid_reset_code("123456"));
        assert!(is_valid_reset_code("999999"));
    }

    #[test]
    fn reset_code_rejects_invalid() {
        assert!(!is_valid_reset_code(""));
        assert!(!is_valid_reset_code("12345"));
        assert!(!is_valid_reset_code("1234567"));
        assert!(!is_valid_reset_code("abcdef"));
        assert!(!is_valid_reset_code("12345a"));
        assert!(!is_valid_reset_code("12 345"));
    }
}
