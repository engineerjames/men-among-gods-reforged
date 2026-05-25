//! Platform-specific utilities (file manager, OS detection, etc.).

use std::path::Path;

use crate::preferences::{DisplayMode, Settings};

// ---------------------------------------------------------------------------
// Platform detection
// ---------------------------------------------------------------------------

/// The hardware / OS platform the client is running on.
///
/// Used by [`PlatformProfile`] to select appropriate first-run defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// Valve Steam Deck handheld PC.
    SteamDeck,
    /// Generic Linux desktop.
    Linux,
    /// macOS desktop.
    MacOS,
    /// Windows desktop.
    Windows,
}

/// Carries the detected platform and provides platform-appropriate default
/// [`Settings`] values.
///
/// Detection happens once at startup via [`PlatformProfile::detect`].  The
/// result is stored on [`crate::state::AppState`] and consulted during the
/// first-run settings bootstrap.
#[derive(Clone, Copy, Debug)]
pub struct PlatformProfile {
    /// The platform detected at startup.
    pub platform: Platform,
}

impl PlatformProfile {
    /// Detects the current platform and returns the corresponding
    /// `PlatformProfile`.
    ///
    /// Detection strategy (in priority order):
    ///
    /// 1. On non-Linux targets the platform is determined at compile time.
    /// 2. On Linux: if the `SteamDeck` environment variable equals `"1"` (set
    ///    by the SteamOS runtime session), the platform is [`Platform::SteamDeck`].
    /// 3. On Linux: if `/sys/class/dmi/id/product_name` contains `"Steam Deck"`,
    ///    the platform is [`Platform::SteamDeck`] (covers running from a TTY or
    ///    outside the Steam runtime).
    /// 4. Otherwise the platform is [`Platform::Linux`].
    ///
    /// # Returns
    ///
    /// * A `PlatformProfile` whose `platform` field reflects the detected hardware.
    pub fn detect() -> Self {
        let platform = Self::detect_platform();
        log::info!("Detected platform: {platform:?}");
        Self { platform }
    }

    #[cfg(target_os = "macos")]
    fn detect_platform() -> Platform {
        Platform::MacOS
    }

    #[cfg(target_os = "windows")]
    fn detect_platform() -> Platform {
        Platform::Windows
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn detect_platform() -> Platform {
        // Check the SteamOS runtime environment variable first — it is always
        // set to "1" when running inside the Steam Deck session.
        if std::env::var("SteamDeck").as_deref() == Ok("1") {
            return Platform::SteamDeck;
        }

        // Fallback: read the DMI product name from sysfs.  This covers running
        // from a desktop terminal outside the Steam runtime.
        if let Ok(name) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
            if name.contains("Steam Deck") {
                return Platform::SteamDeck;
            }
        }

        Platform::Linux
    }

    /// Returns `true` if the detected platform is a Steam Deck.
    ///
    /// # Returns
    ///
    /// * `true` for [`Platform::SteamDeck`], `false` for all other platforms.
    pub fn is_steam_deck(&self) -> bool {
        self.platform == Platform::SteamDeck
    }

    /// Applies platform-specific default settings to `settings`.
    ///
    /// This should be called **only on the first run** (i.e. when no profile
    /// file exists yet) so that platform defaults are never applied on top of
    /// a user's deliberate configuration changes.
    ///
    /// # Arguments
    ///
    /// * `settings` - The [`Settings`] to modify in-place.
    pub fn apply_first_run_defaults(&self, settings: &mut Settings) {
        match self.platform {
            Platform::SteamDeck => {
                // The Steam Deck has a 1280×800 display.  Fullscreen with
                // continuous letterboxing (pixel_perfect_scaling = false) is
                // the correct fit — integer 2× would produce 1920×1080 which
                // exceeds the panel resolution.
                settings.display_mode = DisplayMode::Fullscreen;
                settings.vsync_enabled = true;
                settings.pixel_perfect_scaling = false;
                // A sensible non-zero starting volume.
                settings.master_volume = 0.5;
            }
            Platform::Linux | Platform::MacOS | Platform::Windows => {
                // Desktop defaults are already correct via Settings::default().
            }
        }
    }
}

/// Open [directory] in the native file manager.
///
/// Uses `open` on macOS, `xdg-open` on Linux, and `explorer` on Windows.
/// Failures are logged but silently ignored so call-sites never panic.
///
/// # Arguments
///
/// * `directory` - The directory path to reveal in the file manager.
pub fn open_directory_in_file_manager(directory: &Path) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };

    match std::process::Command::new(program).arg(directory).spawn() {
        Ok(_) => log::info!("Opened directory: {}", directory.display()),
        Err(e) => log::warn!(
            "Failed to open directory {} with {}: {}",
            directory.display(),
            program,
            e,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_deck_defaults_applied() {
        let profile = PlatformProfile {
            platform: Platform::SteamDeck,
        };
        let mut settings = Settings::default();
        profile.apply_first_run_defaults(&mut settings);

        assert_eq!(settings.display_mode, DisplayMode::Fullscreen);
        assert!(settings.vsync_enabled);
        assert!(!settings.pixel_perfect_scaling);
        assert!((settings.master_volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn linux_defaults_are_noop() {
        let profile = PlatformProfile {
            platform: Platform::Linux,
        };
        let before = Settings::default();
        let mut after = Settings::default();
        profile.apply_first_run_defaults(&mut after);

        assert_eq!(before.display_mode, after.display_mode);
        assert_eq!(before.vsync_enabled, after.vsync_enabled);
        assert_eq!(before.pixel_perfect_scaling, after.pixel_perfect_scaling);
        assert!((before.master_volume - after.master_volume).abs() < f32::EPSILON);
    }

    #[test]
    fn is_steam_deck_matches_platform() {
        assert!(
            PlatformProfile {
                platform: Platform::SteamDeck
            }
            .is_steam_deck()
        );
        assert!(
            !PlatformProfile {
                platform: Platform::Linux
            }
            .is_steam_deck()
        );
        assert!(
            !PlatformProfile {
                platform: Platform::MacOS
            }
            .is_steam_deck()
        );
        assert!(
            !PlatformProfile {
                platform: Platform::Windows
            }
            .is_steam_deck()
        );
    }
}
