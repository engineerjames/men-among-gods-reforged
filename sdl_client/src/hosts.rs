pub fn get_api_base_url() -> String {
    std::env::var("MAG_API_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| get_server_url() + ":5554")
}

#[allow(dead_code)]
pub fn get_gameserver_url() -> String {
    std::env::var("MAG_GAME_SERVER_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| get_server_url() + ":5555")
}

fn get_server_url() -> String {
    std::env::var("MAG_BASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("http://{}", get_server_ip()))
}

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
