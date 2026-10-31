//! Builds the public [`LiveStatus`] snapshot from in-memory game state.
//!
//! The snapshot applies the same visibility rules as the in-game `#who`
//! command for normal players ([`crate::god::God::user_who`]): invisible or
//! `NoWho` players are omitted entirely, and the area is withheld for gods
//! and purple-kindred characters.

use core::constants::{CharacterFlags, MAXCHARS, USE_ACTIVE};
use core::live_status_store::{CharacterRoles, LiveStatus, OnlinePlayer, WorldTime};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::area;
use crate::game_state::GameState;

/// Build the live status snapshot for the current tick.
///
/// # Arguments
///
/// * `gs` - Active game state (read only).
///
/// # Returns
///
/// * A [`LiveStatus`] ready to be published to KeyDB.
pub fn build(gs: &GameState) -> LiveStatus {
    let hidden = (CharacterFlags::Invisible | CharacterFlags::NoWho).bits();
    let online = (1..MAXCHARS)
        .map(|cn| &gs.characters[cn])
        .filter(|c| {
            c.used == USE_ACTIVE
                && c.flags & CharacterFlags::Player.bits() != 0
                && c.flags & hidden == 0
        })
        .map(|c| {
            let roles = CharacterRoles::from_flags(c.flags, c.kindred);
            let area = (!roles.god && !roles.purple)
                .then(|| area::get_area_m(i32::from(c.x), i32::from(c.y), false));
            OnlinePlayer {
                name: c.get_name().to_owned(),
                points: c.points_tot,
                rank: core::ranks::rank_name(c.points_tot.unsigned_abs()).to_owned(),
                area,
                roles,
            }
        })
        .collect();

    LiveStatus {
        published_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        time: WorldTime::from_global(&gs.globals),
        online,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::constants::USE_NONACTIVE;
    use core::traits::KIN_PURPLE;

    fn player(gs: &mut GameState, cn: usize, name: &str, flags: CharacterFlags) {
        let c = &mut gs.characters[cn];
        c.used = USE_ACTIVE;
        c.flags = (CharacterFlags::Player | flags).bits();
        c.set_name(name);
        c.x = 512;
        c.y = 512;
    }

    #[test]
    fn build_applies_who_visibility_rules() {
        crate::test_helpers::with_test_gs(|gs| {
            player(gs, 1, "Alice", CharacterFlags::empty());
            player(gs, 2, "Bob", CharacterFlags::Invisible);
            player(gs, 3, "Carol", CharacterFlags::NoWho);
            player(gs, 4, "Dave", CharacterFlags::God);
            player(gs, 5, "Eve", CharacterFlags::empty());
            gs.characters[5].kindred = KIN_PURPLE as i32;
            player(gs, 6, "Frank", CharacterFlags::empty());
            gs.characters[6].used = USE_NONACTIVE;
            // NPC: active but not a player.
            gs.characters[7].used = USE_ACTIVE;
            gs.characters[7].set_name("Guard");

            let status = build(gs);
            let names: Vec<&str> = status.online.iter().map(|p| p.name.as_str()).collect();
            assert_eq!(names, ["Alice", "Dave", "Eve"]);
            assert!(status.online[0].area.is_some());
            assert!(status.online[1].roles.god && status.online[1].area.is_none());
            assert!(status.online[2].roles.purple && status.online[2].area.is_none());
            assert!(status.published_at > 0);
        });
    }
}
