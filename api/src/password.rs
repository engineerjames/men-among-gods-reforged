//! Server-side password hashing for stored account credentials.
//!
//! Background: clients pre-hash the user's password with Argon2 using a
//! deterministic salt derived from the username and submit the resulting PHC
//! string to `/login` and `/accounts`. Treating that pre-hashed value as the
//! credential of record is dangerous — if the KeyDB store leaks, the stored
//! PHC strings can be replayed directly against `/login` without any cracking
//! work.
//!
//! This module applies a second, server-side Argon2 hash on top of the
//! client-provided PHC string with a fresh random salt before persisting it.
//! Verification re-applies the server-side Argon2 against the stored salt and
//! parameters using the constant-time comparison that `argon2::PasswordHash`
//! performs internally.
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

/// Wraps the client-supplied PHC string in a fresh server-side Argon2 envelope.
///
/// # Arguments
///
/// * `client_phc` - The PHC string already produced by the client (also a
///   valid Argon2 PHC string in practice).
///
/// # Returns
///
/// * `Ok(String)` containing the server-side Argon2 PHC envelope suitable for
///   storage.
/// * `Err(String)` if the hash could not be generated.
pub(crate) fn hash_for_storage(client_phc: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(client_phc.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| format!("Failed to hash password: {err}"))
}

/// Verifies `client_phc` against the stored server-side Argon2 envelope.
///
/// # Arguments
///
/// * `stored` - The PHC string currently stored in KeyDB.
/// * `client_phc` - The PHC string submitted by the client.
///
/// # Returns
///
/// * `true` if the stored envelope verifies against the submitted credential.
/// * `false` if the stored value is unparseable or the password does not match.
pub(crate) fn verify(stored: &str, client_phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(client_phc.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CLIENT_PHC: &str = "$argon2id$v=19$m=19456,t=2,p=1$bWFnOnRlc3RlcmFhYWFhYQ$3jBjqfMTYHzn2vJ5p7L+oZjF8Z6FZk7+wB2k1z0c3iA";

    #[test]
    fn hash_for_storage_produces_verifiable_envelope() {
        let stored = hash_for_storage(SAMPLE_CLIENT_PHC).expect("hash succeeds");
        // Server-side envelope must NOT equal the client-supplied PHC.
        assert_ne!(stored, SAMPLE_CLIENT_PHC);
        assert!(verify(&stored, SAMPLE_CLIENT_PHC));
    }

    #[test]
    fn verify_rejects_wrong_client_phc() {
        let stored = hash_for_storage(SAMPLE_CLIENT_PHC).expect("hash succeeds");
        let other = "$argon2id$v=19$m=19456,t=2,p=1$c29tZW90aGVyc2FsdHRlc3Q$0000000000000000000000000000000000000000000";
        assert!(!verify(&stored, other));
    }

    #[test]
    fn verify_rejects_raw_client_phc_as_stored_value() {
        // A raw client PHC string sitting in the store must not be accepted
        // just because it equals the submitted value.
        let stored = SAMPLE_CLIENT_PHC.to_owned();
        assert!(!verify(&stored, SAMPLE_CLIENT_PHC));
    }

    #[test]
    fn verify_rejects_unparseable_stored_value() {
        let stored = "not-a-phc-string".to_owned();
        assert!(!verify(&stored, SAMPLE_CLIENT_PHC));
    }

    #[test]
    fn two_hash_for_storage_calls_produce_distinct_salts() {
        let a = hash_for_storage(SAMPLE_CLIENT_PHC).expect("hash a");
        let b = hash_for_storage(SAMPLE_CLIENT_PHC).expect("hash b");
        assert_ne!(a, b);
    }
}
