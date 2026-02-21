/// Returns the base URL for the account/auth API.
///
/// Reads the `MAG_API_URL` environment variable. If unset or empty, falls
/// back to `<server_url>:5554`.
///
/// # Returns
/// * A `String` such as `"http://127.0.0.1:5554"`.
pub fn get_api_base_url() -> String {
    std::env::var("MAG_API_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| get_server_url() + ":5554")
}

/// Returns the URL for the game (TCP) server.
///
/// Reads the `MAG_GAME_SERVER_URL` environment variable. If unset or empty,
/// falls back to `<server_url>:5555`.
///
/// # Returns
/// * A `String` such as `"http://127.0.0.1:5555"`.
#[allow(dead_code)]
pub fn get_gameserver_url() -> String {
    std::env::var("MAG_GAME_SERVER_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| get_server_url() + ":5555")
}

/// Builds the server base URL from `MAG_BASE_URL`, falling back to
/// `http://<server_ip>` when the variable is unset.
///
/// # Returns
/// * A `String` like `"http://127.0.0.1"`.
fn get_server_url() -> String {
    std::env::var("MAG_BASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("http://{}", get_server_ip()))
}

/// Returns the raw server IP address or hostname.
///
/// Reads `MAG_SERVER_IP`. In debug builds the default is `127.0.0.1`;
/// in release builds it defaults to `menamonggods.ddns.net`.
///
/// # Returns
/// * A `String` containing the IP or hostname.
pub fn get_server_ip() -> String {
    std::env::var("MAG_SERVER_IP")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                "127.0.0.1".to_string()
            } else {
                "menamonggods.ddns.net".to_string()
            }
        })
}
