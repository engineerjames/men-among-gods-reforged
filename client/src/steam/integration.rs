#[cfg(feature = "steam-integration")]
use steamworks::Client;

#[cfg(feature = "steam-integration")]
use crate::steam::constants::MEN_AMONG_GODS_APP_ID;

#[cfg(feature = "steam-integration")]
pub type Steam = SteamClient;
#[cfg(not(feature = "steam-integration"))]
pub type Steam = NoSteam;

pub enum PresenceKey {
    StatusMenu,
    SelectingCharacter,
    StatusPlaying,
    StatusArea(&'static str),
}

impl Into<&str> for PresenceKey {
    fn into(self) -> &'static str {
        match self {
            PresenceKey::StatusMenu => "#Status_Menu",
            PresenceKey::SelectingCharacter => "#Selecting_Character",
            PresenceKey::StatusPlaying => "#Status_Playing",
            PresenceKey::StatusArea(_) => "#Status_Area",
        }
    }
}

/// Trait that specifies the API expected of the SteamWorks Client
/// object. This allows us to use compile-time type dispatching to
/// keep the number of configuration checks to a minimum.
pub trait SteamApi {
    fn init() -> Self;
    fn is_active(&self) -> bool;
    fn set_presence(&self, _token: &str, _name: Option<&str>);
    fn run_callbacks(&self);
}

pub struct NoSteam;
impl SteamApi for NoSteam {
    fn init() -> Self {
        NoSteam {}
    }

    fn is_active(&self) -> bool {
        false
    }

    fn set_presence(&self, _token: &str, _name: Option<&str>) {}

    fn run_callbacks(&self) {}
}

#[cfg(feature = "steam-integration")]
pub struct SteamClient {
    client: Option<Client>,
}

#[cfg(feature = "steam-integration")]
impl SteamApi for SteamClient {
    fn init() -> Self {
        let client = steamworks::Client::init_app(MEN_AMONG_GODS_APP_ID)
            .inspect_err(|e| log::info!("Steam unavailable: {e}"))
            .ok();

        if client.is_some() {
            log::info!("Steam available, and rich presence enabled!");
        }

        Self { client }
    }

    fn is_active(&self) -> bool {
        self.client.is_some()
    }

    fn set_presence(&self, token: &str, name: Option<&str>) {
        if let Some(ref client) = self.client {
            let f = client.friends();
            f.set_rich_presence("name", name);
            f.set_rich_presence("steam_display", Some(token));
        } else {
            log::warn!("Unable to set presence, Steam client not connected.");
        }
    }

    fn run_callbacks(&self) {
        if let Some(ref client) = self.client {
            client.run_callbacks();
        } else {
            log::warn!("Unable to run callbacks, Steam client not connected.");
        }
    }
}

pub fn init() -> Steam {
    Steam::init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_steam_is_never_active() {
        assert!(!NoSteam::init().is_active());
    }

    #[test]
    fn no_steam_calls_are_noops() {
        let steam = NoSteam::init();
        steam.set_presence("#Status_Playing", Some("Piltur"));
        steam.set_presence("#Status_Menu", None);
        steam.run_callbacks();
        assert!(!steam.is_active());
    }

    #[cfg(not(feature = "steam-integration"))]
    #[test]
    fn init_without_feature_is_inactive() {
        assert!(!init().is_active());
    }

    #[cfg(feature = "steam-integration")]
    mod steam_client {
        use super::*;

        fn disconnected() -> SteamClient {
            SteamClient { client: None }
        }

        #[test]
        fn disconnected_client_is_inactive() {
            assert!(!disconnected().is_active());
        }

        #[test]
        fn disconnected_client_calls_are_noops() {
            let steam = disconnected();
            steam.set_presence("#Status_Playing", Some("Piltur"));
            steam.set_presence("#Status_Menu", None);
            steam.run_callbacks();
            assert!(!steam.is_active());
        }

        #[test]
        #[ignore = "requires a running Steam client"]
        fn init_connects_when_steam_running() {
            let steam = init();
            assert!(steam.is_active());
            steam.set_presence("#Status_Playing", Some("Test"));
            steam.run_callbacks();
        }
    }
}
