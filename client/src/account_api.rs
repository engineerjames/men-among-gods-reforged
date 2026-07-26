use std::time::Duration;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mag_core::traits::{Class, Sex};
use reqwest::StatusCode;

use crate::cert_trust;

pub use mag_core::types::api::CharacterSummary;
use mag_core::types::api::{
    CharacterErrorResponse, CreateAccountRequest, CreateAccountResponse, CreateCharacterRequest,
    CreateGameLoginTicketRequest, CreateGameLoginTicketResponse, GetCharactersResponse,
    LoginRequest, LoginResponse, NetworkTestProbeRequest, NetworkTestProbeResponse,
    NetworkTestSummary, NetworkTestSummaryRequest, NetworkTestSummaryResponse,
    ResetPasswordConfirm, ResetPasswordConfirmResponse, ResetPasswordRequest,
    ResetPasswordRequestResponse, UploadClientLogRequest, UploadClientLogResponse,
};

/// Hashes a password into Argon2 PHC format using a deterministic salt.
///
/// # Arguments
/// * `username` - Account username (used to derive the salt).
/// * `password` - Raw password input.
///
/// # Returns
/// * `Ok(hash)` containing the PHC string.
/// * `Err(String)` when hashing fails.
fn hash_password(username: &str, password: &str) -> Result<String, String> {
    let username_lc = username.trim().to_lowercase();
    let salt_seed = format!("mag:{}", username_lc);
    let salt = SaltString::encode_b64(salt_seed.as_bytes())
        .map_err(|err| format!("Failed to encode password salt: {err}"))?;
    let password_hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| format!("Failed to hash password: {err}"))?
        .to_string();
    Ok(password_hash)
}

/// Logs in to the account API and returns a JWT on success.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `username` - Account username.
/// * `password` - Raw password input.
///
/// # Returns
/// * `Ok(token)` containing the JWT.
/// * `Err(String)` when the request or authentication fails.
pub fn login(base_url: &str, username: &str, password: &str) -> Result<String, String> {
    log::info!(
        "Logging in using API at {} with username '{}'",
        base_url,
        username
    );
    let password_hash = hash_password(username, password)?;
    let client = cert_trust::build_reqwest_client()?;

    let url = format!("{}/login", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .json(&LoginRequest {
            username: username.to_owned(),
            password: password_hash,
        })
        .send()
        .map_err(|err| format!("Login request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        let body: LoginResponse = resp
            .json()
            .map_err(|err| format!("Failed to parse login response: {err}"))?;
        let token = body
            .token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "Login failed: empty token".to_owned())?;
        return Ok(token);
    }

    let body = resp.json::<LoginResponse>().ok();
    Err(login_failure_message(
        status,
        body.as_ref().and_then(|body| body.error.as_deref()),
    ))
}

fn login_failure_message(status: StatusCode, error: Option<&str>) -> String {
    if let Some(error) = error.map(str::trim).filter(|error| !error.is_empty()) {
        return error.to_owned();
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid password format",
        StatusCode::UNAUTHORIZED => "Invalid username or password",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Login failed",
    };
    format!("{message} ({})", status.as_u16())
}

/// Creates a new account via the account API.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `email` - Account email address.
/// * `username` - Desired username.
/// * `password` - Raw password input.
///
/// # Returns
/// * `Ok(message)` on success.
/// * `Err(String)` when validation or the request fails.
pub fn create_account(
    base_url: &str,
    email: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
    log::info!(
        "Creating account using API at {} with username '{}' and email '{}'",
        base_url,
        username,
        email
    );
    let password_hash = hash_password(username, password)?;
    let client = cert_trust::build_reqwest_client()?;

    let url = format!("{}/accounts", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .json(&CreateAccountRequest {
            email: email.to_owned(),
            username: username.to_owned(),
            password: password_hash,
        })
        .send()
        .map_err(|err| format!("Account creation request failed: {err}"))?;

    let status = resp.status();
    let body: CreateAccountResponse = resp
        .json()
        .map_err(|err| format!("Failed to parse account creation response: {err}"))?;

    if status.is_success() && body.id.is_some() {
        return Ok(format!("Account created for {}", body.username));
    }

    let fallback = match status {
        StatusCode::BAD_REQUEST => "Invalid account details",
        StatusCode::CONFLICT => "Account already exists",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Account creation failed",
    };

    Err(body.error.unwrap_or_else(|| fallback.to_owned()))
}

/// Creates a new character via the account API.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `token` - JWT bearer token.
/// * `name` - Character name.
/// * `description` - Character description.
/// * `sex` - Character sex.
/// * `class` - Character class.
///
/// # Returns
/// * `Ok(CharacterSummary)` on success.
/// * `Err(String)` when validation or the request fails.
pub fn create_character(
    base_url: &str,
    token: &str,
    name: &str,
    description: Option<&str>,
    sex: Sex,
    class: Class,
) -> Result<CharacterSummary, String> {
    log::info!(
        "Creating character using API at {} with name '{}', sex {:?}, and class {:?}'",
        base_url,
        name,
        sex,
        class
    );

    let client = cert_trust::build_reqwest_client()?;

    let url = format!("{}/characters", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .bearer_auth(token)
        .json(&CreateCharacterRequest {
            name: name.to_owned(),
            description: description.map(|value| value.to_owned()),
            sex,
            class,
        })
        .send()
        .map_err(|err| format!("Character creation request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        let body: CharacterSummary = resp
            .json()
            .map_err(|err| format!("Failed to parse character creation response: {err}"))?;
        return Ok(body);
    }

    // The API reports the exact validation failure (name/description rules, class
    // restrictions, character limit) in the error envelope. Prefer it over the
    // generic status-code text so players know what to fix.
    let api_error = resp
        .json::<CharacterErrorResponse>()
        .ok()
        .map(|body| body.error)
        .map(|error| error.trim().to_owned())
        .filter(|error| !error.is_empty());

    if let Some(error) = api_error {
        return Err(error);
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid character details",
        StatusCode::UNPROCESSABLE_ENTITY => "Character name is already taken",
        StatusCode::CONFLICT => "You have too many characters",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Character creation failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Retrieves all characters for the authenticated account.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `token` - JWT bearer token.
///
/// # Returns
/// * `Ok(Vec<CharacterSummary>)` on success.
/// * `Err(String)` when the request fails.
pub fn get_characters(base_url: &str, token: &str) -> Result<Vec<CharacterSummary>, String> {
    log::info!("Retrieving characters using API at {}", base_url,);

    let client = cert_trust::build_reqwest_client()?;

    let url = format!("{}/characters", base_url.trim_end_matches('/'));
    // The API is configured with a global rate limit (currently ~10 req/sec).
    // The client often performs back-to-back requests (e.g. login -> get characters),
    // so retry a few times on 429 with a short backoff to smooth UX.
    let mut last_status = None;
    for attempt in 0..=2u32 {
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .map_err(|err| format!("Get characters request failed: {err}"))?;

        let status = resp.status();
        last_status = Some(status);

        if status.is_success() {
            let body: GetCharactersResponse = resp
                .json()
                .map_err(|err| format!("Failed to parse characters response: {err}"))?;
            return Ok(body.characters);
        }

        if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
            std::thread::sleep(Duration::from_millis(150));
            continue;
        }

        break;
    }

    let status = last_status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let message = match status {
        StatusCode::TOO_MANY_REQUESTS => "Rate limited",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Get characters failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Deletes a character by id for the authenticated account.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `token` - JWT bearer token.
/// * `character_id` - Character id to delete.
///
/// # Returns
/// * `Ok(())` on success.
/// * `Err(String)` when the request fails.
pub fn delete_character(base_url: &str, token: &str, character_id: u64) -> Result<(), String> {
    let client = cert_trust::build_reqwest_client()?;

    let url = format!(
        "{}/characters/{}",
        base_url.trim_end_matches('/'),
        character_id
    );
    let resp = client
        .delete(url)
        .bearer_auth(token)
        .send()
        .map_err(|err| format!("Delete character request failed: {err}"))?;

    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Invalid character delete request",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Delete character failed",
    };

    Err(format!("{message} ({})", status.as_u16()))
}

/// Creates a short-lived, one-time login ticket for the game server.
///
/// The returned ticket is meant to be sent over the TCP login handshake using `CL_API_LOGIN`.
///
/// # Arguments
///
/// * `base_url` - API base URL used by this function.
/// * `token` - Authentication token used by this function.
/// * `character_id` - Character id used by this function.
///
/// # Returns
///
/// * `Ok` when `create_game_login_ticket` succeeds, or `Err` with failure details.
pub fn create_game_login_ticket(
    base_url: &str,
    token: &str,
    character_id: u64,
) -> Result<u64, String> {
    log::info!(
        "Creating game login ticket using API at {} for character id {}",
        base_url,
        character_id
    );
    let client = cert_trust::build_reqwest_client()?;

    let url = format!("{}/game/login_ticket", base_url.trim_end_matches('/'));
    let resp = client
        .post(url)
        .bearer_auth(token)
        .json(&CreateGameLoginTicketRequest {
            character_id,
            client_version: mag_core::constants::VERSION,
        })
        .send()
        .map_err(|err| format!("Create login ticket request failed: {err}"))?;

    let status = resp.status();

    if status.is_success() {
        let body: CreateGameLoginTicketResponse = resp
            .json()
            .map_err(|err| format!("Failed to parse create ticket response: {err}"))?;
        if let Some(ticket) = body.ticket {
            return Ok(ticket);
        }
        return Err("Ticket creation failed: empty ticket".to_owned());
    }

    let fallback = match status {
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::BAD_REQUEST => "Invalid request or unsupported client version",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Ticket creation failed",
    };

    // Try to extract a descriptive error from the response body; tolerate
    // non-JSON bodies (e.g. plain-text 401 from an intermediate proxy).
    let api_error = resp
        .json::<CreateGameLoginTicketResponse>()
        .ok()
        .and_then(|b| b.error);
    Err(api_error.unwrap_or_else(|| format!("{} ({})", fallback, status.as_u16())))
}

/// Uploads a gzip-compressed client log to the diagnostics API endpoint.
///
/// # Arguments
///
/// * `base_url` - API base URL used by this function.
/// * `character_id` - Character id associated with the uploaded log.
/// * `compressed_log` - Gzip-compressed log bytes.
///
/// # Returns
///
/// * `Ok(saved_file_name)` when upload succeeds.
/// * `Err(String)` when the request fails.
pub fn upload_client_log(
    base_url: &str,
    token: &str,
    character_id: u64,
    compressed_log: &[u8],
) -> Result<String, String> {
    if compressed_log.is_empty() {
        return Err("Diagnostics upload failed: compressed payload is empty".to_owned());
    }

    let client = cert_trust::build_reqwest_client()?;
    let url = format!("{}/diag/client-log", base_url.trim_end_matches('/'));
    let compressed_log_b64 = STANDARD.encode(compressed_log);

    let mut last_status = None;
    let mut last_error: Option<String> = None;
    for attempt in 0..=2u32 {
        let resp = client
            .post(&url)
            .bearer_auth(token)
            .json(&UploadClientLogRequest {
                character_id,
                compressed_log_b64: compressed_log_b64.clone(),
            })
            .send()
            .map_err(|err| format!("Diagnostics upload request failed: {err}"))?;

        let status = resp.status();
        last_status = Some(status);

        let body = resp.json::<UploadClientLogResponse>().ok();
        if status.is_success() {
            if let Some(saved_file) = body
                .as_ref()
                .and_then(|value| value.saved_file.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Ok(saved_file.to_owned());
            }
            return Err("Diagnostics upload failed: missing saved_file in response".to_owned());
        }

        last_error = body
            .as_ref()
            .and_then(|value| value.error.as_deref())
            .map(ToOwned::to_owned);

        if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
            std::thread::sleep(Duration::from_millis(1100));
            continue;
        }

        break;
    }

    let status = last_status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if let Some(error) = last_error
        .map(|e| e.trim().to_owned())
        .filter(|e| !e.is_empty())
    {
        return Err(error);
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Diagnostics upload rejected",
        StatusCode::PAYLOAD_TOO_LARGE => "Diagnostics upload payload too large",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::TOO_MANY_REQUESTS => "Diagnostics upload rate limited",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Diagnostics upload failed",
    };
    Err(format!("{message} ({})", status.as_u16()))
}

/// Sends one diagnostics network-test probe request and returns round-trip timing.
///
/// # Arguments
///
/// * `client` - Pre-built HTTP client reused across the entire test run.
/// * `base_url` - API base URL used by this function.
/// * `token` - JWT bearer token for the authenticated session.
/// * `character_id` - Character id associated with the test run.
/// * `run_id` - Client-generated run correlation id.
/// * `sample_index` - Zero-based sample index in the current run.
/// * `client_payload` - Probe payload bytes approximating one client command packet.
/// * `requested_server_payload_bytes` - Response payload size requested from the server.
///
/// # Returns
///
/// * `Ok((server_unix_ms, response_payload_bytes))` when probe succeeds.
/// * `Err(String)` when request or server validation fails.
#[allow(clippy::too_many_arguments)]
pub fn run_network_test_probe(
    client: &reqwest::blocking::Client,
    base_url: &str,
    token: &str,
    character_id: u64,
    run_id: &str,
    sample_index: u32,
    client_payload: &[u8],
    requested_server_payload_bytes: u16,
) -> Result<(u64, Vec<u8>), String> {
    let url = format!("{}/diag/network-test/probe", base_url.trim_end_matches('/'));

    let mut last_status = None;
    let mut last_error: Option<String> = None;
    for attempt in 0..=2u32 {
        let resp = client
            .post(&url)
            .bearer_auth(token)
            .json(&NetworkTestProbeRequest {
                character_id,
                run_id: run_id.to_owned(),
                sample_index,
                client_payload_b64: STANDARD.encode(client_payload),
                requested_server_payload_bytes,
            })
            .send()
            .map_err(|err| format!("Network test probe request failed: {err}"))?;

        let status = resp.status();
        last_status = Some(status);
        let body = resp.json::<NetworkTestProbeResponse>().ok();

        if status.is_success() {
            if let Some(server_unix_ms) = body.as_ref().and_then(|value| value.server_unix_ms) {
                let payload_b64 = body
                    .as_ref()
                    .and_then(|value| value.server_payload_b64.as_deref())
                    .ok_or_else(|| {
                        "Network test probe failed: missing server_payload_b64 in response"
                            .to_owned()
                    })?;
                let response_payload = STANDARD.decode(payload_b64.as_bytes()).map_err(|err| {
                    format!("Network test probe failed decoding server payload: {err}")
                })?;
                if response_payload.len() != usize::from(requested_server_payload_bytes) {
                    return Err(format!(
                        "Network test probe failed: expected {} response bytes, got {}",
                        requested_server_payload_bytes,
                        response_payload.len()
                    ));
                }
                return Ok((server_unix_ms, response_payload));
            }
            return Err("Network test probe failed: missing server_unix_ms in response".to_owned());
        }

        last_error = body
            .as_ref()
            .and_then(|value| value.error.as_deref())
            .map(ToOwned::to_owned);

        if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
            std::thread::sleep(Duration::from_millis(1100));
            continue;
        }

        break;
    }

    let status = last_status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if let Some(error) = last_error
        .map(|e| e.trim().to_owned())
        .filter(|e| !e.is_empty())
    {
        return Err(error);
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Network test probe rejected",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::TOO_MANY_REQUESTS => "Network test probe rate limited",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Network test probe failed",
    };
    Err(format!("{message} ({})", status.as_u16()))
}

/// Sends final diagnostics network-test summary metrics to the API.
///
/// # Arguments
///
/// * `client` - Pre-built HTTP client reused across the entire test run.
/// * `base_url` - API base URL used by this function.
/// * `token` - JWT bearer token for the authenticated session.
/// * `character_id` - Character id associated with the test run.
/// * `run_id` - Client-generated run correlation id.
/// * `summary` - Aggregated network-test metrics.
///
/// # Returns
///
/// * `Ok(())` when submission succeeds.
/// * `Err(String)` when request or server validation fails.
pub fn submit_network_test_summary(
    client: &reqwest::blocking::Client,
    base_url: &str,
    token: &str,
    character_id: u64,
    run_id: &str,
    summary: NetworkTestSummary,
) -> Result<(), String> {
    let url = format!(
        "{}/diag/network-test/summary",
        base_url.trim_end_matches('/')
    );

    let mut last_status = None;
    let mut last_error: Option<String> = None;
    for attempt in 0..=2u32 {
        let resp = client
            .post(&url)
            .bearer_auth(token)
            .json(&NetworkTestSummaryRequest {
                character_id,
                run_id: run_id.to_owned(),
                summary: summary.clone(),
            })
            .send()
            .map_err(|err| format!("Network test summary request failed: {err}"))?;

        let status = resp.status();
        last_status = Some(status);
        let body = resp.json::<NetworkTestSummaryResponse>().ok();

        if status.is_success() {
            if body.as_ref().map(|value| value.accepted).unwrap_or(false) {
                return Ok(());
            }
            return Err("Network test summary failed: API did not accept payload".to_owned());
        }

        last_error = body
            .as_ref()
            .and_then(|value| value.error.as_deref())
            .map(ToOwned::to_owned);

        if status == StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
            std::thread::sleep(Duration::from_millis(1100));
            continue;
        }

        break;
    }

    let status = last_status.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if let Some(error) = last_error
        .map(|e| e.trim().to_owned())
        .filter(|e| !e.is_empty())
    {
        return Err(error);
    }

    let message = match status {
        StatusCode::BAD_REQUEST => "Network test summary rejected",
        StatusCode::UNAUTHORIZED => "Unauthorized",
        StatusCode::TOO_MANY_REQUESTS => "Network test summary rate limited",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Network test summary failed",
    };
    Err(format!("{message} ({})", status.as_u16()))
}

/// Requests a password reset code to be sent to the account's email.
///
/// The API always returns a generic success message regardless of whether
/// the account or email matched, to avoid information leakage.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `username` - Account username.
/// * `email` - Email address associated with the account.
///
/// # Returns
/// * `Ok(message)` - Generic success message from the API.
/// * `Err(String)` when the request fails.
pub fn request_password_reset(
    base_url: &str,
    username: &str,
    email: &str,
) -> Result<String, String> {
    log::info!(
        "Requesting password reset using API at {} for username '{}'",
        base_url,
        username,
    );
    let client = cert_trust::build_reqwest_client()?;

    let url = format!(
        "{}/accounts/reset-password/request",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&ResetPasswordRequest {
            username: username.to_owned(),
            email: email.to_owned(),
        })
        .send()
        .map_err(|err| format!("Password reset request failed: {err}"))?;

    let status = resp.status();
    let body: ResetPasswordRequestResponse = resp
        .json()
        .map_err(|err| format!("Failed to parse reset request response: {err}"))?;

    if status.is_success() {
        return Ok(body.message);
    }

    let fallback = match status {
        StatusCode::TOO_MANY_REQUESTS => "Too many reset attempts, please try again later",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Password reset request failed",
    };

    Err(fallback.to_owned())
}

/// Confirms a password reset by submitting the 6-digit code and new password.
///
/// The new password is hashed client-side (Argon2, deterministic salt from
/// username) before being sent, matching the account creation flow.
///
/// # Arguments
/// * `base_url` - API base URL.
/// * `username` - Account username.
/// * `code` - 6-digit reset code received via email.
/// * `new_password` - New raw password input.
///
/// # Returns
/// * `Ok(message)` on success.
/// * `Err(String)` when validation or the request fails.
pub fn confirm_password_reset(
    base_url: &str,
    username: &str,
    code: &str,
    new_password: &str,
) -> Result<String, String> {
    log::info!(
        "Confirming password reset using API at {} for username '{}'",
        base_url,
        username,
    );
    let password_hash = hash_password(username, new_password)?;
    let client = cert_trust::build_reqwest_client()?;

    let url = format!(
        "{}/accounts/reset-password/confirm",
        base_url.trim_end_matches('/')
    );
    let resp = client
        .post(url)
        .json(&ResetPasswordConfirm {
            username: username.to_owned(),
            code: code.to_owned(),
            new_password: password_hash,
        })
        .send()
        .map_err(|err| format!("Password reset confirm request failed: {err}"))?;

    let status = resp.status();
    let body: ResetPasswordConfirmResponse = resp
        .json()
        .map_err(|err| format!("Failed to parse reset confirm response: {err}"))?;

    if status.is_success() {
        return Ok(body.message);
    }

    let fallback = match status {
        StatusCode::BAD_REQUEST => "Invalid request",
        StatusCode::UNAUTHORIZED => "Invalid or expired reset code",
        StatusCode::TOO_MANY_REQUESTS => "Too many attempts, please try again later",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error",
        _ => "Password reset failed",
    };

    Err(fallback.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_failure_message_uses_api_error() {
        assert_eq!(
            login_failure_message(StatusCode::FORBIDDEN, Some("Account banned")),
            "Account banned"
        );
    }

    #[test]
    fn login_failure_message_falls_back_to_status() {
        assert_eq!(
            login_failure_message(StatusCode::FORBIDDEN, None),
            "Login failed (403)"
        );
    }
}
