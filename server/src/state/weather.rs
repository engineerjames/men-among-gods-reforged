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
use core::weather::{WEATHER_FLAG_OVERRIDE, WeatherKind};
use core::weather_areas::area_weather_for;

use crate::game_state::GameState;
use crate::network_manager::xsend;

/// Approximately one second at 36 TPS — how often the area-driven driver
/// re-evaluates each player's location.
#[allow(dead_code)]
const WEATHER_TICK_PERIOD: u32 = 36;

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
#[allow(dead_code)]
pub fn clear_weather(gs: &mut GameState, player_id: usize) {
    send_weather(gs, player_id, WeatherKind::None as u8, 0, 0, [0; 4], 0);
}

/// Per-player weather tick.
///
/// Called once per server tick from the main loop. Throttles itself to roughly
/// one update per second per player. Behavior:
///
/// 1. If the player has an admin override (`WEATHER_FLAG_OVERRIDE`) and it
///    has expired, clear it (so the area driver can take over again).
/// 2. Otherwise, look up the player's tile in the area-weather table and
///    transition to/from the area-default weather only if it actually
///    changes.
///
/// Skips players that aren't connected and in a normal play state.
///
/// # Arguments
///
/// * `gs` - Mutable game state.
/// * `nr` - Player index.
#[allow(dead_code)]
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

    let cur_flags = gs.players[nr].weather_flags;
    let expire = gs.players[nr].weather_expire_tick;
    let is_override = (cur_flags & WEATHER_FLAG_OVERRIDE) != 0;

    // Expire any timed weather (override or area-default).
    if expire != 0 && ticker >= expire {
        clear_weather(gs, nr);
        // Fall through to immediately apply the area default this tick.
    } else if is_override {
        // Active override — leave it alone until it expires.
        return;
    }

    let x = i32::from(gs.characters[cn].x);
    let y = i32::from(gs.characters[cn].y);
    let area = area_weather_for(x, y);

    let (target_kind, intensity, tint, flags) = match area {
        Some(a) => (
            a.kind as u8,
            a.intensity,
            a.tint.unwrap_or([0u8; 4]),
            a.flags,
        ),
        None => (WeatherKind::None as u8, 0u8, [0u8; 4], 0u8),
    };

    let new_cur_kind = gs.players[nr].weather_kind;
    let new_cur_intensity = gs.players[nr].weather_intensity;
    let new_cur_tint = gs.players[nr].weather_tint;
    let new_cur_flags = gs.players[nr].weather_flags;

    if new_cur_kind == target_kind
        && new_cur_intensity == intensity
        && new_cur_tint == tint
        && new_cur_flags == flags
    {
        return;
    }
    send_weather(gs, nr, target_kind, intensity, 0, tint, flags);
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
            gs.players[nr].weather_flags = WEATHER_FLAG_OVERRIDE;

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
            gs.players[nr].weather_flags = WEATHER_FLAG_OVERRIDE;
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
    fn weather_tick_applies_area_default_when_no_override() {
        with_test_gs(|gs| {
            let (_cn, nr) = add_test_player(gs);
            attach_test_socket(gs, nr);
            let cn = gs.players[nr].usnr;
            gs.characters[cn].x = 550; // Strange Forest
            gs.characters[cn].y = 320;

            // Force throttle window.
            let phase = (nr as u32) % WEATHER_TICK_PERIOD;
            gs.globals.ticker = phase as i32;
            gs.players[nr].tptr = 0;

            weather_tick(gs, nr);
            assert_eq!(
                gs.players[nr].weather_kind,
                WeatherKind::Fireflies as u8,
                "Strange Forest should set fireflies"
            );
            assert_eq!(gs.players[nr].weather_intensity, 64);
            assert!(gs.players[nr].tptr >= 10);
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
            gs.players[nr].weather_flags = WEATHER_FLAG_OVERRIDE;
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
