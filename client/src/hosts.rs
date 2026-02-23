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

/// Extracts the hostname from an API base URL.
///
/// # Arguments
/// * `base_url` - API base URL such as `http://127.0.0.1:5554`.
///
/// # Returns
/// * `Some(host)` when parsing succeeds.
/// * `None` when the URL cannot be parsed or has no host.
pub fn get_host_from_api_base_url(base_url: &str) -> Option<String> {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
}

/// Builds the server base URL from `MAG_BASE_URL`, falling back to
/// `https://<server_ip>` when the variable is unset.
///
/// # Returns
/// * A `String` like `"https://127.0.0.1"`.
fn get_server_url() -> String {
    std::env::var("MAG_BASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("https://{}", get_server_ip()))
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
