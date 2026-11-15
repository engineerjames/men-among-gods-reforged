#[cfg(feature = "steam-integration")]
use steamworks::Client;

#[cfg(feature = "steam-integration")]
pub type Steam = SteamClient;
#[cfg(not(feature = "steam-integration"))]
pub type Steam = NoSteam;

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
        let client = steamworks::Client::init()
            .inspect_err(|e| log::info!("Steam unavailable: {e}"))
            .ok();

        log::info!("Steam available, and rich presence enabled!");
        Self { client }
    }

    fn is_active(&self) -> bool {
        true
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
