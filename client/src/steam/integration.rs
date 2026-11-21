#[cfg(feature = "steam-integration")]
use steamworks::Client;

#[cfg(feature = "steam-integration")]
use crate::steam::constants::MEN_AMONG_GODS_APP_ID;

#[cfg(feature = "steam-integration")]
pub type Steam = SteamClient;
#[cfg(not(feature = "steam-integration"))]
pub type Steam = NoSteam;

/// Rich presence states, mapped to tokens in `rich_presence.txt`.
pub enum Presence {
    /// In the main menu.
    StatusMenu,
    /// Selecting a character to play as.
    SelectingCharacter,
    /// Generic playing status, not tied to any specific area.
    StatusPlaying,
    /// Playing in the named area (fills the `%area%` substitution).
    StatusArea(&'static str),
    /// Clears all rich presence keys.
    Unset,
}

impl From<&Presence> for &'static str {
    /// Returns the `steam_display` localization token for `presence`.
    fn from(presence: &Presence) -> Self {
        match presence {
            Presence::StatusMenu => "#Status_Menu",
            Presence::SelectingCharacter => "#Selecting_Character",
            Presence::StatusPlaying => "#Status_Playing",
            Presence::StatusArea(_) => "#Status_Area",
            Presence::Unset => "",
        }
    }
}

/// Trait that specifies the API expected of the SteamWorks Client
/// object. This allows us to use compile-time type dispatching to
/// keep the number of configuration checks to a minimum.
pub trait SteamApi {
    fn init() -> Self;
    fn is_active(&self) -> bool;
    fn set_presence(&self, presence: Presence);
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

    fn set_presence(&self, _presence: Presence) {}

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

    fn set_presence(&self, presence: Presence) {
        if let Some(ref client) = self.client {
            let f = client.friends();
            match presence {
                Presence::Unset => f.clear_rich_presence(),
                _ => {
                    let area = match presence {
                        Presence::StatusArea(area) => Some(area),
                        _ => None,
                    };
                    f.set_rich_presence("area", area);
                    f.set_rich_presence("steam_display", Some((&presence).into()));
                }
            }
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
    fn presence_maps_to_localization_tokens() {
        let token = |p: Presence| -> &'static str { (&p).into() };
        assert_eq!(token(Presence::StatusMenu), "#Status_Menu");
        assert_eq!(token(Presence::SelectingCharacter), "#Selecting_Character");
        assert_eq!(token(Presence::StatusPlaying), "#Status_Playing");
        assert_eq!(token(Presence::StatusArea("Aston")), "#Status_Area");
    }

    #[test]
    fn no_steam_calls_are_noops() {
        let steam = NoSteam::init();
        steam.set_presence(Presence::StatusArea("Aston"));
        steam.set_presence(Presence::StatusMenu);
        steam.set_presence(Presence::Unset);
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
            steam.set_presence(Presence::StatusArea("Aston"));
            steam.set_presence(Presence::StatusMenu);
            steam.set_presence(Presence::Unset);
            steam.run_callbacks();
            assert!(!steam.is_active());
        }

        #[test]
        #[ignore = "requires a running Steam client"]
        fn init_connects_when_steam_running() {
            let steam = init();
            assert!(steam.is_active());
            steam.set_presence(Presence::StatusPlaying);
            steam.run_callbacks();
        }
    }
}
