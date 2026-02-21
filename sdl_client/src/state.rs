use crate::{
    gfx_cache::GraphicsCache, network::NetworkRuntime, player_state::PlayerState,
    sfx_cache::SoundCache,
};

#[derive(Clone, Debug)]
pub struct ApiTokenState {
    pub base_url: String,
    pub username: Option<String>,
    pub token: Option<String>,
    pub login_target: Option<(u64, i32)>,
}

impl ApiTokenState {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            username: None,
            token: None,
            login_target: None,
        }
    }
}

pub struct AppState {
    pub gfx_cache: GraphicsCache,
    pub sfx_cache: SoundCache,
    pub api: ApiTokenState,
    pub network: Option<NetworkRuntime>,
    pub player_state: Option<PlayerState>,
}

impl AppState {
    pub fn new(gfx_cache: GraphicsCache, sfx_cache: SoundCache, api: ApiTokenState) -> Self {
        Self {
            gfx_cache,
            sfx_cache: sfx_cache,
            api,
            network: None,
            player_state: None,
        }
    }
}
