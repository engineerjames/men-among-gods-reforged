use crate::{gfx_cache::GraphicsCache, sfx_cache::SoundCache};

#[derive(Clone, Debug)]
pub struct ApiTokenState {
    pub base_url: String,
    pub username: Option<String>,
    pub token: Option<String>,
}

impl ApiTokenState {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            username: None,
            token: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty())
    }
}

pub struct AppState {
    pub gfx_cache: GraphicsCache,
    pub _sfx_cache: SoundCache,
    pub api: ApiTokenState,
}

impl AppState {
    pub fn new(gfx_cache: GraphicsCache, sfx_cache: SoundCache, api: ApiTokenState) -> Self {
        Self {
            gfx_cache,
            _sfx_cache: sfx_cache,
            api,
        }
    }
}

pub fn default_api_base_url() -> String {
    std::env::var("MAG_API_BASE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                "http://127.0.0.1:5554".to_string()
            } else {
                "http://menamonggods.ddns.net:5554".to_string()
            }
        })
}
