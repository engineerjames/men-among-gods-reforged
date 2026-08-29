//! Per-player weather dispatch and area-driven tick driver.
//!
//! Weather is purely a client-visible effect. The server keeps the *active*
//! weather kind / intensity / tint cached on each [`crate::types::server_player::ServerPlayer`]
//! so it can avoid retransmitting unchanged state every tick. State is
//! transient and never persisted to KeyDB.
//!
//! See [`docs/server/DESIGN.md`](../../../docs/server/DESIGN.md) for the
//! protocol overview and [`core::weather`] for the wire-format constants.

use core::server_commands::ServerCommandType;
use core::weather::{WeatherFlags, WeatherKind};
use core::weather_areas::{self, AREA_WEATHER_PROFILES};

use crate::game_state::GameState;
use crate::helpers::random_mod;
use crate::network_manager::xsend;

/// Approximately one second at 36 TPS — how often the per-player dispatcher
/// re-evaluates each player's location against the current area weather state.
const WEATHER_TICK_PERIOD: u32 = 36;

/// How often (in ticks) an idle area is re-evaluated for a new weather
/// trigger, and how long an area waits after clearing before its next
/// trigger roll (~5 minutes at 36 TPS).
const AREA_EVAL_PERIOD_TICKS: u32 = 36 * 60 * 5;

/// Currently active weather for one area, rolled from a
/// [`core::weather_areas::WeatherCandidate`] by [`area_weather_system_tick`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ActiveAreaWeather {
    /// Active weather kind for this area.
    pub kind: WeatherKind,
    /// Particle/effect intensity (0..=255).
    pub intensity: u8,
    /// RGBA tint; `[0;4]` means "use the kind's client-default tint".
    pub tint: [u8; 4],
    /// Wire-protocol flags forwarded to the client.
    pub flags: u8,
    /// Tick at which this weather expires.
    pub expire_tick: u32,
}

/// Runtime weather state for one area, parallel to
/// [`core::weather_areas::AREA_WEATHER_PROFILES`]. Transient — never
/// persisted to KeyDB.
#[derive(Clone, Copy, Debug, Default)]
pub struct AreaWeatherRuntime {
    /// Currently active weather for this area, or `None` if clear.
    pub active: Option<ActiveAreaWeather>,
    /// Tick at which this area becomes eligible for its next trigger roll.
    pub next_eval_tick: u32,
}

/// Build the 10-byte `SV_WEATHER` packet body and send it to a single player.
///
/// Updates the player's cached weather fields so subsequent ticks can decide
/// whether anything has actually changed.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `player_id` - Index into `gs.players`.
/// * `kind` - Discriminant byte of [`WeatherKind`].
/// * `intensity` - 0..=255.
/// * `duration_ticks` - 0 = persistent until replaced.
/// * `tint` - RGBA; alpha 0 means "use the kind's client-default tint".
/// * `flags` - Wire flags; the `WEATHER_FLAG_OVERRIDE` bit marks an admin
///   override that the area driver will respect.
pub fn send_weather(
    gs: &mut GameState,
    player_id: usize,
    kind: u8,
    intensity: u8,
    duration_ticks: u16,
    tint: [u8; 4],
    flags: u8,
) {
    if player_id >= gs.players.len() {
        return;
    }

    let mut buf = [0u8; 10];
    buf[0] = ServerCommandType::SetWeather as u8;
    buf[1] = kind;
    buf[2] = intensity;
    buf[3..5].copy_from_slice(&duration_ticks.to_le_bytes());
    buf[5] = tint[0];
    buf[6] = tint[1];
    buf[7] = tint[2];
    buf[8] = tint[3];
    buf[9] = flags;

    let expire_tick = if duration_ticks == 0 {
        0
    } else {
        (gs.globals.ticker as u32).wrapping_add(u32::from(duration_ticks))
    };

    {
        let p = &mut gs.players[player_id];
        p.weather_kind = kind;
        p.weather_intensity = intensity;
        p.weather_expire_tick = expire_tick;
        p.weather_tint = tint;
        p.weather_flags = flags;
    }

    xsend(gs, player_id, &buf, 10);
}

/// Convenience wrapper that clears any active weather on a player.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `player_id` - Index into `gs.players`.
pub fn clear_weather(gs: &mut GameState, player_id: usize) {
    send_weather(gs, player_id, WeatherKind::None as u8, 0, 0, [0; 4], 0);
}

/// Area-driven weather system tick.
///
/// Called once per server tick. For each configured [`AREA_WEATHER_PROFILES`]
/// entry: clears the area's active weather once it expires, and — once the
/// area is idle and due for re-evaluation — rolls `trigger_chance_per_eval`
/// (out of 1000) for a chance to start a new weighted-random candidate with
/// a random duration in `[min_duration_ticks, max_duration_ticks]`.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
pub fn area_weather_system_tick(gs: &mut GameState) {
    area_weather_system_tick_with_rng(gs, random_mod);
}

/// Implementation of [`area_weather_system_tick`] with the RNG injected as a
/// closure, so trigger/duration decisions can be driven deterministically in
/// tests instead of seeding a global RNG.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `rng` - Given an exclusive upper bound, returns a value in `[0, bound)`.
fn area_weather_system_tick_with_rng(gs: &mut GameState, mut rng: impl FnMut(u32) -> u32) {
    let ticker = gs.globals.ticker as u32;
    for (idx, profile) in AREA_WEATHER_PROFILES.iter().enumerate() {
        let Some(runtime) = gs.area_weather.get_mut(idx) else {
            continue;
        };

        if let Some(active) = runtime.active {
            if ticker >= active.expire_tick {
                runtime.active = None;
                runtime.next_eval_tick = ticker + AREA_EVAL_PERIOD_TICKS;
            }
            continue;
        }

        if ticker < runtime.next_eval_tick {
            continue;
        }
        runtime.next_eval_tick = ticker + AREA_EVAL_PERIOD_TICKS;

        if rng(1000) >= profile.trigger_chance_per_eval {
            continue;
        }

        let total = weather_areas::total_weight(profile);
        if total == 0 {
            continue;
        }
        let candidate = weather_areas::pick_candidate(profile, rng(total));
        // +1 so the inclusive max duration is reachable via rng's exclusive upper bound.
        let span = profile
            .max_duration_ticks
            .saturating_sub(profile.min_duration_ticks)
            + 1;
        let duration = profile.min_duration_ticks + rng(span);

        let mut flags = candidate.flags;
        if profile.ignore_indoor_fade {
            flags |= WeatherFlags::IgnoreIndoorFade;
        }

        runtime.active = Some(ActiveAreaWeather {
            kind: candidate.kind,
            intensity: candidate.intensity,
            tint: candidate.tint.unwrap_or([0u8; 4]),
            flags: flags.bits(),
            expire_tick: ticker + duration,
        });
        log::info!(
            "area weather triggered: {} -> {:?} for {} ticks",
            profile.label,
            candidate.kind,
            duration
        );
    }
}

/// Per-player weather tick.
///
/// Called once per server tick from the main loop. Throttles itself to roughly
/// one update per second per player. Behavior:
///
/// 1. If the player has an admin override (`WEATHER_FLAG_OVERRIDE`) and it
///    has expired, clear it (so the area system can take over again).
/// 2. Otherwise, look up the player's area and dispatch whatever weather is
///    currently active there (populated by [`area_weather_system_tick`]), or
///    clear back to [`WeatherKind::None`] if the area has no active weather
///    or the player isn't in a configured area. Logs a chat line to the
///    player on kind transitions (area-driven only; admin overrides don't
///    get an automatic message).
///
/// Skips players that aren't connected and in a normal play state.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `nr` - Player index.
pub fn weather_tick(gs: &mut GameState, nr: usize) {
    if nr == 0 || nr >= gs.players.len() {
        return;
    }
    if gs.players[nr].sock.is_none() {
        return;
    }
    if gs.players[nr].state != core::constants::ST_NORMAL {
        return;
    }
    let cn = gs.players[nr].usnr;
    if cn == 0 {
        return;
    }

    // Throttle: each player gets re-evaluated ~once per second, with the
    // player index used as a phase offset so updates spread across ticks.
    let ticker = gs.globals.ticker as u32;
    let phase = (nr as u32) % WEATHER_TICK_PERIOD;
    if !ticker
        .wrapping_sub(phase)
        .is_multiple_of(WEATHER_TICK_PERIOD)
    {
        return;
    }

    let flags_before = gs.players[nr].weather_flags;
    let expire = gs.players[nr].weather_expire_tick;
    let is_override = (flags_before & WeatherFlags::Override.bits()) != 0;

    // Expire any timed weather (override or area-driven).
    if expire != 0 && ticker >= expire {
        clear_weather(gs, nr);
        // Fall through to immediately apply the area state this tick.
    } else if is_override {
        // Active override — leave it alone until it expires.
        return;
    }

    let x = i32::from(gs.characters[cn].x);
    let y = i32::from(gs.characters[cn].y);
    let active = weather_areas::area_weather_profile_index_for(x, y)
        .and_then(|idx| gs.area_weather.get(idx))
        .and_then(|runtime| runtime.active);

    let (target_kind, intensity, tint, flags) = match active {
        Some(a) => (a.kind, a.intensity, a.tint, a.flags),
        None => (WeatherKind::None, 0u8, [0u8; 4], 0u8),
    };

    let prev_kind = WeatherKind::from(gs.players[nr].weather_kind);
    let cur_intensity = gs.players[nr].weather_intensity;
    let cur_tint = gs.players[nr].weather_tint;
    let cur_flags = gs.players[nr].weather_flags;

    if prev_kind == target_kind
        && cur_intensity == intensity
        && cur_tint == tint
        && cur_flags == flags
    {
        return;
    }
    log::info!(
        "weather_tick: player {} ({}) changing from {:?} to {:?}",
        nr,
        cn,
        prev_kind,
        target_kind
    );
    send_weather(gs, nr, target_kind as u8, intensity, 0, tint, flags);

    if target_kind != WeatherKind::None && target_kind != prev_kind {
        if let Some(msg) = core::weather::weather_start_message(target_kind) {
            gs.do_character_log(cn, core::types::FontColor::Blue, msg);
        }
    } else if target_kind == WeatherKind::None
        && prev_kind != WeatherKind::None
        && let Some(msg) = core::weather::weather_stop_message(prev_kind)
    {
        gs.do_character_log(cn, core::types::FontColor::Blue, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use crate::tls::GameStream;
    use std::net::{TcpListener, TcpStream};

    fn attach_test_socket(gs: &mut GameState, nr: usize) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");
        let client = TcpStream::connect(addr).expect("connect client");
        let (server, _) = listener.accept().expect("accept client");
        drop(client);
        gs.players[nr].sock = Some(GameStream::Plain(server));
    }

    #[test]
    fn send_weather_packs_expected_bytes_and_caches_state() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.globals.ticker = 1;

            send_weather(gs, nr, 4, 200, 0x10, [220, 60, 30, 90], 0b1000_0001);

            let tbuf = &gs.players[nr].tbuf[..10];
            assert_eq!(tbuf[0], ServerCommandType::SetWeather as u8);
            assert_eq!(tbuf[1], 4);
            assert_eq!(tbuf[2], 200);
            assert_eq!(&tbuf[3..5], &0x0010u16.to_le_bytes());
            assert_eq!(&tbuf[5..9], &[220, 60, 30, 90]);
            assert_eq!(tbuf[9], 0b1000_0001);
            assert_eq!(gs.players[nr].tptr, 10);

            let p = &gs.players[nr];
            assert_eq!(p.weather_kind, 4);
            assert_eq!(p.weather_intensity, 200);
            assert_eq!(p.weather_tint, [220, 60, 30, 90]);
            assert_eq!(p.weather_flags, 0b1000_0001);
            assert_eq!(p.weather_expire_tick, 1u32.wrapping_add(0x10));
        });
    }

    #[test]
    fn clear_weather_sends_none_kind() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            gs.players[nr].weather_kind = 4;
            gs.players[nr].weather_flags = WeatherFlags::Override.bits();

            clear_weather(gs, nr);
            let tbuf = &gs.players[nr].tbuf[..10];
            assert_eq!(tbuf[1], WeatherKind::None as u8);
            assert_eq!(gs.players[nr].weather_kind, 0);
            assert_eq!(gs.players[nr].weather_flags, 0);
        });
    }

    #[test]
    fn weather_tick_skips_when_override_active_and_unexpired() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            // Place character inside Strange Forest (would otherwise trigger fireflies).
            let cn = gs.players[nr].usnr;
            gs.characters[cn].x = 550;
            gs.characters[cn].y = 320;

            // Pre-set an unexpired override (Fire).
            gs.globals.ticker = 100;
            gs.players[nr].weather_kind = WeatherKind::Fire as u8;
            gs.players[nr].weather_intensity = 200;
            gs.players[nr].weather_flags = WeatherFlags::Override.bits();
            gs.players[nr].weather_expire_tick = 10_000;
            gs.players[nr].tptr = 0;

            // Force the throttle window for this player.
            let phase = (nr as u32) % WEATHER_TICK_PERIOD;
            gs.globals.ticker = phase as i32;

            weather_tick(gs, nr);
            assert_eq!(gs.players[nr].tptr, 0, "override should not be replaced");
            assert_eq!(gs.players[nr].weather_kind, WeatherKind::Fire as u8);
        });
    }

    #[test]
    fn weather_tick_applies_active_area_weather_when_no_override() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            let cn = gs.players[nr].usnr;
            gs.characters[cn].x = 550; // Strange Forest
            gs.characters[cn].y = 320;

            let idx = weather_areas::area_weather_profile_index_for(550, 320)
                .expect("Strange Forest profile");
            gs.area_weather[idx].active = Some(ActiveAreaWeather {
                kind: WeatherKind::Fireflies,
                intensity: 64,
                tint: [0; 4],
                flags: 0,
                expire_tick: 100_000,
            });

            // Force throttle window.
            let phase = (nr as u32) % WEATHER_TICK_PERIOD;
            gs.globals.ticker = phase as i32;
            gs.players[nr].tptr = 0;

            weather_tick(gs, nr);
            assert_eq!(
                gs.players[nr].weather_kind,
                WeatherKind::Fireflies as u8,
                "should apply the area's currently active weather"
            );
            assert_eq!(gs.players[nr].weather_intensity, 64);
            assert!(gs.players[nr].tptr >= 10);
        });
    }

    #[test]
    fn weather_tick_stays_clear_when_area_has_no_active_weather() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            let cn = gs.players[nr].usnr;
            gs.characters[cn].x = 550; // Strange Forest, but no active roll set.
            gs.characters[cn].y = 320;

            let phase = (nr as u32) % WEATHER_TICK_PERIOD;
            gs.globals.ticker = phase as i32;
            gs.players[nr].tptr = 0;

            weather_tick(gs, nr);
            assert_eq!(gs.players[nr].weather_kind, WeatherKind::None as u8);
            assert_eq!(gs.players[nr].tptr, 0, "no packet when nothing changed");
        });
    }

    /// Reconstructs the concatenated text of all `SV_LOG*` packets appended
    /// to `tbuf` starting at `start` (each is 16 bytes: 1-byte header + up to
    /// 15 payload bytes, zero-padded).
    fn reconstruct_logged_text(tbuf: &[u8], start: usize, tptr: usize) -> String {
        let mut text = String::new();
        let mut offset = start;
        while offset + 16 <= tptr {
            let payload = &tbuf[offset + 1..offset + 16];
            let end = payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(payload.len());
            text.push_str(&String::from_utf8_lossy(&payload[..end]));
            offset += 16;
        }
        text
    }

    #[test]
    fn weather_tick_logs_start_and_stop_messages_on_transition() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            let cn = gs.players[nr].usnr;
            gs.characters[cn].x = 550; // Strange Forest
            gs.characters[cn].y = 320;

            let idx = weather_areas::area_weather_profile_index_for(550, 320)
                .expect("Strange Forest profile");
            gs.area_weather[idx].active = Some(ActiveAreaWeather {
                kind: WeatherKind::Fireflies,
                intensity: 64,
                tint: [0; 4],
                flags: 0,
                expire_tick: 100_000,
            });

            let phase = (nr as u32) % WEATHER_TICK_PERIOD;
            gs.globals.ticker = phase as i32;
            gs.players[nr].tptr = 0;

            weather_tick(gs, nr);
            let start_msg = core::weather::weather_start_message(WeatherKind::Fireflies).unwrap();
            let tptr = gs.players[nr].tptr;
            let logged = reconstruct_logged_text(&gs.players[nr].tbuf, 10, tptr);
            assert!(
                logged.contains(start_msg),
                "expected start message, got: {logged:?}"
            );

            // Now clear the area's active weather and re-run on the next throttle window.
            gs.area_weather[idx].active = None;
            gs.players[nr].tptr = 0;
            gs.globals.ticker += WEATHER_TICK_PERIOD as i32;

            weather_tick(gs, nr);
            let stop_msg = core::weather::weather_stop_message(WeatherKind::Fireflies).unwrap();
            let tptr = gs.players[nr].tptr;
            let logged = reconstruct_logged_text(&gs.players[nr].tbuf, 10, tptr);
            assert!(
                logged.contains(stop_msg),
                "expected stop message, got: {logged:?}"
            );
        });
    }

    #[test]
    fn area_weather_system_tick_triggers_and_expires_deterministically() {
        with_test_gs(|gs| {
            gs.globals.ticker = 0;
            // Roll 0 always satisfies "trigger_roll < trigger_chance" (chance > 0)
            // and always picks the first candidate / minimum duration.
            area_weather_system_tick_with_rng(gs, |_bound| 0);

            let strange_forest_idx = weather_areas::area_weather_profile_index_for(550, 320)
                .expect("Strange Forest profile");
            let active = gs.area_weather[strange_forest_idx]
                .active
                .expect("should have triggered with roll 0");
            assert_eq!(active.kind, WeatherKind::Fireflies);
            assert_eq!(
                active.expire_tick,
                AREA_WEATHER_PROFILES[strange_forest_idx].min_duration_ticks
            );

            // Advance past expiry; the expiry-clear branch always short-circuits
            // before any re-trigger roll for that same area/tick.
            gs.globals.ticker = active.expire_tick as i32;
            area_weather_system_tick_with_rng(gs, |_bound| 0);
            assert!(
                gs.area_weather[strange_forest_idx].active.is_none(),
                "expired weather should clear"
            );
        });
    }

    #[test]
    fn area_weather_system_tick_sets_ignore_indoor_fade_for_pentagram_quest() {
        with_test_gs(|gs| {
            gs.globals.ticker = 0;
            area_weather_system_tick_with_rng(gs, |_bound| 0);

            let idx = weather_areas::area_weather_profile_index_for(300, 400)
                .expect("Pentagram Quest profile");
            let active = gs.area_weather[idx]
                .active
                .expect("should have triggered with roll 0");
            assert_eq!(active.kind, WeatherKind::Fire);
            assert_ne!(
                active.flags & WeatherFlags::IgnoreIndoorFade.bits(),
                0,
                "Pentagram Quest is flagged indoors but should ignore the fade"
            );
            assert_ne!(
                active.flags & WeatherFlags::Additive.bits(),
                0,
                "the Fire candidate's own Additive flag should be preserved"
            );
        });
    }

    #[test]
    fn area_weather_system_tick_never_triggers_below_chance_threshold() {
        with_test_gs(|gs| {
            gs.globals.ticker = 0;
            // A roll always >= any trigger_chance_per_eval (all configured
            // profiles use chances well under 1000) should never trigger.
            area_weather_system_tick_with_rng(gs, |_bound| 1000);
            assert!(
                gs.area_weather.iter().all(|r| r.active.is_none()),
                "no area should have triggered"
            );
        });
    }

    #[test]
    fn weather_tick_clears_expired_override() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            let cn = gs.players[nr].usnr;
            // Place character outside any area so the area lookup yields None.
            gs.characters[cn].x = 1;
            gs.characters[cn].y = 1;

            // Override that has already expired.
            gs.players[nr].weather_kind = WeatherKind::Fire as u8;
            gs.players[nr].weather_intensity = 200;
            gs.players[nr].weather_flags = WeatherFlags::Override.bits();
            gs.players[nr].weather_expire_tick = 50;

            // Force throttle window with current ticker past expiration.
            let phase = (nr as u32) % WEATHER_TICK_PERIOD;
            // Pick a ticker >= expire and aligned to phase.
            let ticker = ((100u32 / WEATHER_TICK_PERIOD) + 1) * WEATHER_TICK_PERIOD + phase;
            gs.globals.ticker = ticker as i32;
            gs.players[nr].tptr = 0;

            weather_tick(gs, nr);

            assert_eq!(gs.players[nr].weather_flags, 0, "override bit cleared");
            assert_eq!(gs.players[nr].weather_kind, WeatherKind::None as u8);
        });
    }
}
