use crate::{
    gfx_cache::GraphicsCache, network::NetworkRuntime, player_state::PlayerState,
    preferences::DisplayMode, sfx_cache::SoundCache, ui::panning_background::PanningBackground,
};

/// A display-related change requested by a scene, to be applied by the main
/// loop which owns the SDL2 window and renderer.
#[derive(Clone, Debug)]
pub enum DisplayCommand {
    SetDisplayMode(DisplayMode),
    SetPixelPerfectScaling(bool),
    SetVSync(bool),
}

/// Holds the data needed to connect a character to the game server after
/// obtaining a login ticket from the API.
#[derive(Clone, Debug)]
pub struct GameLoginTarget {
    pub ticket: u64,
    pub race: i32,
    pub character_id: u64,
    pub character_name: String,
}

/// Tracks the current API authentication state, including the base URL,
/// logged-in username, JWT token, and an optional pending game-login target.
#[derive(Clone, Debug)]
pub struct ApiTokenState {
    pub base_url: String,
    pub username: Option<String>,
    pub token: Option<String>,
    pub login_target: Option<GameLoginTarget>,
}

impl ApiTokenState {
    /// Creates a new `ApiTokenState` with the given API base URL and no
    /// active session.
    ///
    /// # Arguments
    /// * `base_url` - The root URL of the account/auth API (e.g. `http://127.0.0.1:5554`).
    ///
    /// # Returns
    /// * A new `ApiTokenState` with all session fields set to `None`.
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            username: None,
            token: None,
            login_target: None,
        }
    }
}

/// Central application state threaded through every scene.
///
/// Owns the graphics cache, sound cache, API auth state, and the optional
/// network runtime and player state that exist only while connected to the
/// game server.
///
/// The lifetime `'tc` ties the graphics cache (and its GPU textures) to the
/// [`TextureCreator`](sdl2::render::TextureCreator) that lives in `main()`.
pub struct AppState<'tc> {
    pub gfx_cache: GraphicsCache<'tc>,
    pub sfx_cache: SoundCache,
    pub api: ApiTokenState,
    pub network: Option<NetworkRuntime>,
    pub player_state: Option<PlayerState>,
    /// Master volume multiplier (0.0 = muted, 1.0 = full). Set by the escape menu slider.
    pub master_volume: f32,
    pub music_enabled: bool,
    /// Current display mode (windowed, fullscreen, borderless).
    pub display_mode: DisplayMode,
    /// Whether pixel-perfect (integer-only) scaling is active.
    pub pixel_perfect_scaling: bool,
    /// Whether VSync is enabled.
    pub vsync_enabled: bool,
    /// Pending display change to be applied by the main loop.
    pub display_command: Option<DisplayCommand>,
    /// Shared panning background used by all pre-game scenes.
    pub panning_background: PanningBackground,
    /// Username carried between the request-reset and enter-reset-code scenes.
    pub reset_username: Option<String>,
}

impl<'tc> AppState<'tc> {
    /// Creates a new `AppState` with the given caches and API state.
    ///
    /// Network and player state start as `None`; they are set when the client
    /// connects to the game server.
    ///
    /// # Arguments
    /// * `gfx_cache` - Pre-loaded sprite / texture cache.
    /// * `sfx_cache` - Pre-loaded sound effect and music cache.
    /// * `api` - Initialized API token state.
    ///
    /// # Returns
    /// * A new `AppState` ready for use in the scene manager.
    pub fn new(
        gfx_cache: GraphicsCache<'tc>,
        sfx_cache: SoundCache,
        api: ApiTokenState,
        panning_background: PanningBackground,
    ) -> Self {
        Self {
            gfx_cache,
            sfx_cache: sfx_cache,
            api,
            network: None,
            player_state: None,
            master_volume: 1.0,
            music_enabled: true,
            display_mode: DisplayMode::default(),
            pixel_perfect_scaling: false,
            vsync_enabled: false,
            display_command: None,
            panning_background,
            reset_username: None,
        }
    }
}
