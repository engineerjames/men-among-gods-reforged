use bevy::audio::{AudioPlayer, PlaybackSettings, SpatialScale, Volume};
use bevy::prelude::*;

use crate::sfx_cache::SoundCache;

#[derive(Resource, Debug, Clone, Copy)]
pub struct SoundSettings {
    pub enabled: bool,
    /// Master volume multiplier in [0.0, 1.0].
    pub master_volume: f32,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            master_volume: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SoundEvent {
    Click,
    ServerPlaySound { nr: u32, vol: i32, pan: i32 },
}

#[derive(Resource, Default)]
pub struct SoundEventQueue {
    events: Vec<SoundEvent>,
}

impl SoundEventQueue {
    /// Enqueue a UI click sound.
    pub fn push_click(&mut self) {
        self.events.push(SoundEvent::Click);
    }

    /// Enqueue a server-driven sound by number and DirectSound params.
    pub fn push_server_play_sound(&mut self, nr: u32, vol: i32, pan: i32) {
        self.events
            .push(SoundEvent::ServerPlaySound { nr, vol, pan });
    }

    /// Drain queued events for playback.
    fn drain(&mut self) -> impl Iterator<Item = SoundEvent> + '_ {
        self.events.drain(..)
    }
}

/// Convert DirectSound volume units into Bevy `Volume`.
fn volume_linear_from_directsound(vol: i32) -> f32 {
    // Legacy client uses DirectSound volume values:
    // - range is typically [-10000..0], unit is 1/100 dB.
    // - example: -1000 means -10 dB.
    // Treat anything <= -10000 as effectively muted.
    if vol <= -10000 {
        return 0.0;
    }

    let db = (vol as f32) / 100.0;

    // Convert dB (amplitude) to linear gain. Clamp to keep it sane.
    let db = db.clamp(-100.0, 24.0);
    (10.0_f32).powf(db / 20.0).clamp(0.0, 4.0)
}

/// Convert DirectSound pan into a small 2D spatial x offset.
fn pan_to_x(pan: i32) -> f32 {
    // Legacy client uses DirectSound pan values (usually [-10000..10000]).
    // Map this into a small spatial x offset for simple stereo panning.
    (pan.clamp(-10000, 10000) as f32) / 2500.0
}

/// Spawn audio entities for queued sound events.
pub fn play_queued_sounds(
    mut commands: Commands,
    mut queue: ResMut<SoundEventQueue>,
    sfx: Res<SoundCache>,
    settings: Res<SoundSettings>,
) {
    for evt in queue.drain() {
        if !settings.enabled || settings.master_volume <= 0.0 {
            continue;
        }

        let (handle, vol, pan) = match evt {
            SoundEvent::Click => {
                let Some(h) = sfx.click() else {
                    continue;
                };
                (h.clone(), -1000, 0)
            }
            SoundEvent::ServerPlaySound { nr, vol, pan } => {
                let Some(h) = sfx.get_numbered(nr) else {
                    log::debug!("Skipping unknown server sound nr={nr}");
                    continue;
                };
                (h.clone(), vol, pan)
            }
        };

        let base = volume_linear_from_directsound(vol);
        let gain = (base * settings.master_volume).clamp(0.0, 4.0);

        let settings = PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(gain))
            .with_speed(1.0);
        let x = pan_to_x(pan);

        // Use spatial playback for left/right panning support.
        // Requires a single `SpatialListener` somewhere (we attach it to the camera).
        let settings = PlaybackSettings {
            spatial: true,
            spatial_scale: Some(SpatialScale::new_2d(1.0)),
            ..settings
        };

        commands.spawn((
            Name::new("SFX"),
            AudioPlayer::new(handle),
            settings,
            Transform::from_xyz(x, 0.0, 0.0),
            GlobalTransform::default(),
        ));
    }
}
