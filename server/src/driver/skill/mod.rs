mod auras;
mod blast;
mod buffs;
mod common;
mod companion;
mod curse;
mod harakim;
mod mercenary;
mod stun;
mod templar;
mod utility;

pub use auras::*;
pub use blast::*;
pub use buffs::*;
pub use common::*;
pub use companion::*;
pub use curse::*;
pub use harakim::*;
pub use mercenary::*;
pub use stun::*;
pub use templar::*;
pub use utility::*;

use crate::chlog;
use crate::game_state::GameState;
use crate::player_logging;
use core::constants::{
    CharacterFlags, DX_DOWN, DX_LEFT, DX_LEFTDOWN, DX_LEFTUP, DX_RIGHT, DX_RIGHTDOWN, DX_RIGHTUP,
    DX_UP, MAXCHARS, SERVER_MAPX, SERVER_MAPY, USE_ACTIVE,
};
use core::skills::{
    SK_ANGUISH_EARTH, SK_ANGUISH_ICE, SK_ANGUISH_LAVA, SK_AURA_CURSE, SK_AURA_WAR_BANNER, SK_AXE,
    SK_BLADE_DANCE, SK_BLAST, SK_BLESS, SK_CONCEN, SK_CONTAGION, SK_CURSE, SK_DAGGER,
    SK_DELIVER_DEATH, SK_DISARM, SK_DISPEL, SK_ENHANCE, SK_GASH, SK_GHOST, SK_GUNPOWDER_FLASH,
    SK_HEAL, SK_ICE_STUN, SK_IDENT, SK_IMMUN, SK_INNER_STRENGTH, SK_KINDRED_SPIRIT, SK_LAVA_BLAST,
    SK_LIGHT, SK_LOCK, SK_MEDIT, SK_MSHIELD, SK_PARASITE, SK_PROTECT, SK_RAINS_OF_RENEWAL,
    SK_RECALL, SK_REGEN, SK_REPAIR, SK_REST, SK_REVENANT_CONDUIT, SK_SEEING_RED,
    SK_SOUL_REFLECTION, SK_SPECTRAL_PACT, SK_SPELLCASTER_KINDRED_SPIRIT, SK_STAFF, SK_STUN,
    SK_SUNS_BLESSING, SK_SURROUND, SK_SWORD, SK_THUNDEROUS_FURY, SK_TWOHAND, SK_WARCRY, SK_WEAPON,
    SK_WIMPY,
};
use core::types::FontColor;

/// Resolves the active offensive target for a skill cast.
///
/// Mirrors the target-selection pattern shared by `skill_blast`, `skill_curse`
/// and `skill_stun`: prefer `skill_target1`, fall back to `attack_cn`, then to
/// an adjacent character that is currently attacking the caster (which is the
/// only signal available when auto-fightback is disabled), and finally to the
/// caster themselves so the individual skills can emit their "you cannot X
/// yourself" message.
fn resolve_offensive_target(gs: &mut GameState, cn: usize) -> usize {
    if gs.characters[cn].skill_target1 != 0 {
        return gs.characters[cn].skill_target1 as usize;
    }
    if gs.characters[cn].attack_cn != 0 {
        return gs.characters[cn].attack_cn as usize;
    }
    resolve_engaged_attacker(gs, cn).unwrap_or(cn)
}

/// Tile deltas for the eight neighbours of a character, ordered clockwise from up.
const ADJACENT_DELTAS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Inverse of [`helpers::drv_dcoor2dir`]: maps a facing direction to a tile delta.
///
/// # Arguments
///
/// * `dir` - One of the `DX_*` direction constants.
///
/// # Returns
///
/// * `Some((dx, dy))` for a known direction, `None` otherwise.
fn dir_to_delta(dir: u8) -> Option<(i32, i32)> {
    match dir {
        DX_RIGHT => Some((1, 0)),
        DX_LEFT => Some((-1, 0)),
        DX_UP => Some((0, -1)),
        DX_DOWN => Some((0, 1)),
        DX_LEFTUP => Some((-1, -1)),
        DX_LEFTDOWN => Some((-1, 1)),
        DX_RIGHTUP => Some((1, -1)),
        DX_RIGHTDOWN => Some((1, 1)),
        _ => None,
    }
}

/// Returns the character at `(x + dx, y + dy)` when it is actively attacking `cn`.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Caster character index.
/// * `x` - Caster X coordinate.
/// * `y` - Caster Y coordinate.
/// * `delta` - Tile offset to inspect.
///
/// # Returns
///
/// * `Some(co)` when a visible, living attacker occupies that tile.
fn attacker_at_delta(
    gs: &mut GameState,
    cn: usize,
    x: i32,
    y: i32,
    delta: (i32, i32),
) -> Option<usize> {
    let (tx, ty) = (x + delta.0, y + delta.1);
    if tx < 0 || ty < 0 || tx >= SERVER_MAPX || ty >= SERVER_MAPY {
        return None;
    }

    let co = gs.map[(tx + ty * SERVER_MAPX) as usize].ch as usize;
    if co == 0 || co == cn || co >= MAXCHARS {
        return None;
    }
    if gs.characters[co].used != USE_ACTIVE {
        return None;
    }
    if (gs.characters[co].flags & CharacterFlags::Body.bits()) != 0 {
        return None;
    }
    if gs.characters[co].attack_cn as usize != cn {
        return None;
    }
    if gs.do_char_can_see(cn, co) == 0 {
        return None;
    }

    Some(co)
}

/// Finds an adjacent character that is currently attacking `cn`.
///
/// Prefers the tile the caster is facing so offensive casts follow the
/// player's aim, then scans the remaining neighbours so a caster who is only
/// being attacked from behind still gets a target.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Caster character index.
///
/// # Returns
///
/// * `Some(co)` when an engaged attacker is adjacent, otherwise `None`.
fn resolve_engaged_attacker(gs: &mut GameState, cn: usize) -> Option<usize> {
    let x = i32::from(gs.characters[cn].x);
    let y = i32::from(gs.characters[cn].y);
    let facing = dir_to_delta(gs.characters[cn].dir);

    if let Some(delta) = facing
        && let Some(co) = attacker_at_delta(gs, cn, x, y, delta)
    {
        return Some(co);
    }

    for delta in ADJACENT_DELTAS {
        if Some(delta) == facing {
            continue;
        }
        if let Some(co) = attacker_at_delta(gs, cn, x, y, delta) {
            return Some(co);
        }
    }

    None
}

/// Common preflight checks for hostile single-target casts.
///
/// Validates visibility, self-target rejection, stoned target, attack
/// permission, and remembers the PvP exchange. Returns true if the cast may
/// proceed.
fn hostile_cast_preflight(gs: &mut GameState, cn: usize, co: usize, self_msg: &str) -> bool {
    if cn == co {
        gs.do_character_log(cn, FontColor::Red, self_msg);
        return false;
    }
    if gs.do_char_can_see(cn, co) == 0 {
        gs.do_character_log(cn, FontColor::Red, "You cannot see your target.\n");
        return false;
    }
    if (gs.characters[co].flags & CharacterFlags::Stoned.bits()) != 0 {
        gs.do_character_log(
            cn,
            FontColor::Green,
            "Your target is lagging. Try again later.\n",
        );
        return false;
    }
    if !gs.may_attack_msg(cn, co, true) {
        chlog!(
            cn,
            "Prevented from attacking {}",
            gs.characters[co].get_name().to_owned()
        );
        return false;
    }
    gs.remember_pvp(cn, co);
    true
}

/// Dispatches direct skill use to the matching skill handler.
///
/// # Arguments
///
/// * `gs` - Active game state used for skill lookup and handler execution.
/// * `cn` - Character index using the skill.
/// * `nr` - Skill number to dispatch.
///
/// # Panics
///
/// * Panics if `cn` is invalid or `nr` cannot be used as a skill-table index.
pub fn skill_driver(gs: &mut GameState, cn: usize, nr: i32) {
    let target = gs.characters[cn].skill_target1;
    player_logging::log_event(
        cn,
        player_logging::PlayerLogCategory::Spell,
        player_logging::PlayerLogOutcome::Attempt,
        &format!("skill={nr} target={target}"),
    );
    // Check whether the character can use this skill/spell
    if gs.characters[cn].skill[nr as usize][0] == 0 {
        player_logging::log_event(
            cn,
            player_logging::PlayerLogCategory::Spell,
            player_logging::PlayerLogOutcome::Rejected,
            &format!("skill_not_available={nr}"),
        );
        gs.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n");
        return;
    }

    match nr {
        x if x == SK_LIGHT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_light(gs, cn);
            }
        }
        x if x == SK_PROTECT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_protect(gs, cn);
            }
        }
        x if x == SK_ENHANCE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_enhance(gs, cn);
            }
        }
        x if x == SK_BLESS as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_bless(gs, cn);
            }
        }
        x if x == SK_CURSE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_curse(gs, cn);
            }
        }
        x if x == SK_IDENT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_identify(gs, cn);
            }
        }
        x if x == SK_BLAST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_blast(gs, cn);
            }
        }
        x if x == SK_REPAIR as i32 => skill_repair(gs, cn),
        x if x == SK_LOCK as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You cannot use this skill directly. Hold a lock-pick under your mouse cursor and click on the door.\n",
        ),
        x if x == SK_RECALL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_recall(gs, cn);
            }
        }
        x if x == SK_STUN as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_stun(gs, cn);
            }
        }
        x if x == SK_DISPEL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_dispel(gs, cn);
            }
        }
        x if x == SK_WIMPY as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_wimp(gs, cn);
            }
        }
        x if x == SK_HEAL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_heal(gs, cn);
            }
        }
        x if x == SK_GHOST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_ghost(gs, cn);
            }
        }
        x if x == SK_MSHIELD as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_mshield(gs, cn);
            }
        }
        x if x == SK_AURA_CURSE as i32 => skill_curse_aura(gs, cn),
        x if x == SK_AURA_WAR_BANNER as i32 => skill_war_banner_aura(gs, cn),
        x if x == SK_SOUL_REFLECTION as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_soul_reflection(gs, cn);
            }
        }
        x if x == SK_IMMUN as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You use this skill automatically when someone casts evil spells on you.\n",
        ),
        x if x == SK_REGEN as i32 || x == SK_REST as i32 || x == SK_MEDIT as i32 => {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you stand still.\n",
            );
        }
        x if x == SK_WEAPON as i32
            || x == SK_DAGGER as i32
            || x == SK_SWORD as i32
            || x == SK_AXE as i32
            || x == SK_STAFF as i32
            || x == SK_TWOHAND as i32
            || x == SK_SURROUND as i32
            || x == SK_DISARM as i32 =>
        {
            gs.do_character_log(
                cn,
                FontColor::Green,
                "You use this skill automatically when you fight.\n",
            );
        }
        x if x == SK_CONCEN as i32 => gs.do_character_log(
            cn,
            FontColor::Green,
            "You use this skill automatically when you cast spells.\n",
        ),
        x if x == SK_WARCRY as i32 => skill_warcry(gs, cn),
         x if x == SK_PARASITE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_parasite(gs, cn);
            }
        }
        x if x == SK_GUNPOWDER_FLASH as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_gunpowder_flash(gs, cn);
            }
        }
        x if x == SK_DELIVER_DEATH as i32 => skill_deliver_death(gs, cn),
        x if x == SK_CONTAGION as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_contagion(gs, cn);
            }
        }
        x if x == SK_BLADE_DANCE as i32 => skill_blade_dance(gs, cn),
        x if x == SK_RAINS_OF_RENEWAL as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_rains_of_renewal(gs, cn);
            }
        }
        x if x == SK_GASH as i32 => skill_gash(gs, cn),
        x if x == SK_SUNS_BLESSING as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_suns_blessing(gs, cn);
            }
        }
        x if x == SK_SEEING_RED as i32 => skill_seeing_red(gs, cn),
        x if x == SK_THUNDEROUS_FURY as i32 => skill_thunderous_fury(gs, cn),
        x if x == SK_INNER_STRENGTH as i32 => skill_inner_strength(gs, cn),
        x if x == SK_REVENANT_CONDUIT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_revenant_conduit(gs, cn);
            }
        }
        x if x == SK_KINDRED_SPIRIT as i32 => skill_kindred_spirit(gs, cn),
        x if x == SK_SPECTRAL_PACT as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_spectral_pact(gs, cn);
            }
        }
        x if x == SK_ANGUISH_LAVA as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_anguish_lava(gs, cn);
            }
        }
        x if x == SK_ANGUISH_EARTH as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_anguish_earth(gs, cn);
            }
        }
        x if x == SK_ANGUISH_ICE as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_anguish_ice(gs, cn);
            }
        }
        x if x == SK_LAVA_BLAST as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_lava_blast(gs, cn);
            }
        }
        x if x == SK_ICE_STUN as i32 => {
            if (gs.characters[cn].flags & CharacterFlags::NoMagic.bits()) != 0 {
                nomagic(gs, cn);
            } else {
                skill_ice_stun(gs, cn);
            }
        }
        x if x == SK_SPELLCASTER_KINDRED_SPIRIT as i32 => {
            skill_spellcaster_kindred_spirit(gs, cn);
        }
        _ => {
            gs.do_character_log(cn, FontColor::Green, "You cannot use this skill/spell.\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::USE_ACTIVE;
    use core::types::Character;

    /// Place `co` on the map next to `cn` and mark it as attacking `cn`.
    pub(super) fn place_attacker(gs: &mut GameState, cn: usize, co: usize, dx: i16, dy: i16) {
        let (x, y) = (gs.characters[cn].x + dx, gs.characters[cn].y + dy);
        gs.characters[co] = Character::default();
        gs.characters[co].used = USE_ACTIVE;
        gs.characters[co].x = x;
        gs.characters[co].y = y;
        gs.characters[co].attack_cn = cn as u16;
        gs.map[x as usize + y as usize * SERVER_MAPX as usize].ch = co as u32;
    }

    pub(super) fn attach_marker(
        gs: &mut GameState,
        cn: usize,
        slot: usize,
        item_idx: usize,
        temp: u16,
    ) {
        gs.items[item_idx] = core::types::Item::default();
        gs.items[item_idx].used = USE_ACTIVE;
        gs.items[item_idx].temp = temp;
        gs.items[item_idx].power = 25;
        gs.items[item_idx].active = 100;
        gs.items[item_idx].duration = 100;
        gs.characters[cn].spell[slot] = item_idx as u32;
    }

    /// Set up a player caster with neutral spell modifiers and an adjacent hostile
    /// NPC (index 2) selected as the skill target. Returns `(cn, co)`.
    pub(super) fn caster_and_target(gs: &mut GameState) -> (usize, usize) {
        let (cn, _nr) = add_test_player(gs);
        gs.item_templates[1].used = USE_ACTIVE;
        gs.globals.newmoon = 0;
        gs.globals.fullmoon = 0;
        gs.characters[cn].kindred = 0;
        gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
        place_attacker(gs, cn, 2, 1, 0);
        gs.characters[cn].skill_target1 = 2;
        (cn, 2)
    }

    /// Return the item index of `cn`'s active spell with the given `temp`, if any.
    pub(super) fn active_spell_item(gs: &GameState, cn: usize, temp: usize) -> Option<usize> {
        gs.characters[cn]
            .spell
            .iter()
            .map(|&item_idx| item_idx as usize)
            .find(|&item_idx| item_idx != 0 && gs.items[item_idx].temp == temp as u16)
    }

    #[test]
    fn explicit_target_and_attack_cn_take_priority() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            place_attacker(gs, cn, 2, 1, 0);

            gs.characters[cn].skill_target1 = 7;
            gs.characters[cn].attack_cn = 9;
            assert_eq!(resolve_offensive_target(gs, cn), 7);

            gs.characters[cn].skill_target1 = 0;
            assert_eq!(resolve_offensive_target(gs, cn), 9);
        });
    }

    #[test]
    fn falls_back_to_adjacent_attacker_preferring_the_facing_tile() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            gs.characters[cn].skill_target1 = 0;
            gs.characters[cn].attack_cn = 0;

            let front = 2;
            let behind = 3;
            place_attacker(gs, cn, front, 1, 0);
            place_attacker(gs, cn, behind, -1, 0);

            gs.characters[cn].dir = DX_RIGHT;
            assert_eq!(resolve_offensive_target(gs, cn), front);

            gs.characters[cn].dir = DX_LEFT;
            assert_eq!(resolve_offensive_target(gs, cn), behind);
        });
    }

    #[test]
    fn falls_back_to_attacker_behind_when_facing_tile_is_empty() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            gs.characters[cn].skill_target1 = 0;
            gs.characters[cn].attack_cn = 0;
            gs.characters[cn].dir = DX_UP;

            let behind = 2;
            place_attacker(gs, cn, behind, 0, 1);

            assert_eq!(resolve_offensive_target(gs, cn), behind);
        });
    }

    #[test]
    fn ignores_adjacent_characters_that_are_not_attacking_us() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            gs.characters[cn].skill_target1 = 0;
            gs.characters[cn].attack_cn = 0;
            gs.characters[cn].dir = DX_RIGHT;

            let bystander = 2;
            place_attacker(gs, cn, bystander, 1, 0);
            gs.characters[bystander].attack_cn = 0;

            assert_eq!(resolve_offensive_target(gs, cn), cn);
        });
    }

    #[test]
    fn dir_to_delta_covers_every_adjacent_tile_once() {
        let dirs = [
            DX_UP,
            DX_RIGHTUP,
            DX_RIGHT,
            DX_RIGHTDOWN,
            DX_DOWN,
            DX_LEFTDOWN,
            DX_LEFT,
            DX_LEFTUP,
        ];
        let deltas: Vec<_> = dirs
            .iter()
            .map(|&dir| dir_to_delta(dir).expect("valid direction"))
            .collect();
        assert_eq!(deltas, ADJACENT_DELTAS.to_vec());
        assert_eq!(dir_to_delta(0), None);
    }

    #[test]
    fn attacker_at_delta_ignores_bodies_and_out_of_bounds_tiles() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            place_attacker(gs, cn, 2, 1, 0);

            assert_eq!(attacker_at_delta(gs, cn, 10, 10, (1, 0)), Some(2));
            assert_eq!(attacker_at_delta(gs, cn, 0, 0, (-1, 0)), None);

            gs.characters[2].flags |= CharacterFlags::Body.bits();
            assert_eq!(attacker_at_delta(gs, cn, 10, 10, (1, 0)), None);
        });
    }

    #[test]
    fn hostile_cast_preflight_rejects_self_and_stoned_targets() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            assert!(!hostile_cast_preflight(gs, cn, cn, "self\n"));

            place_attacker(gs, cn, 2, 1, 0);
            gs.characters[2].flags |= CharacterFlags::Stoned.bits();
            assert!(!hostile_cast_preflight(gs, cn, 2, "self\n"));

            gs.characters[2].flags &= !CharacterFlags::Stoned.bits();
            assert!(hostile_cast_preflight(gs, cn, 2, "self\n"));
        });
    }

    #[test]
    fn hostile_cast_preflight_rejects_targets_out_of_sight() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].flags |= CharacterFlags::Infrared.bits();
            place_attacker(gs, cn, 2, 60, 60);

            assert!(!hostile_cast_preflight(gs, cn, 2, "self\n"));
        });
    }

    #[test]
    fn skill_driver_rejects_skills_the_character_lacks() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].a_mana = 100_000;

            skill_driver(gs, cn, SK_MSHIELD as i32);

            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(active_spell_item(gs, cn, SK_MSHIELD).is_none());
        });
    }

    #[test]
    fn skill_driver_dispatches_to_the_matching_skill() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].skill[SK_MSHIELD][0] = 1;
            gs.characters[cn].skill[SK_MSHIELD][5] = 40;
            gs.characters[cn].a_mana = 100_000;

            skill_driver(gs, cn, SK_MSHIELD as i32);

            assert_eq!(gs.characters[cn].a_mana, 75_000);
            assert!(active_spell_item(gs, cn, SK_MSHIELD).is_some());
        });
    }

    #[test]
    fn skill_driver_blocks_spells_but_not_physical_skills_under_nomagic() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.item_templates[1].used = USE_ACTIVE;
            gs.characters[cn].flags |= CharacterFlags::NoMagic.bits();
            gs.characters[cn].skill[SK_MSHIELD][0] = 1;
            gs.characters[cn].skill[SK_MSHIELD][5] = 40;
            gs.characters[cn].skill[SK_WARCRY][0] = 1;
            gs.characters[cn].a_mana = 100_000;
            gs.characters[cn].a_end = 200_000;

            skill_driver(gs, cn, SK_MSHIELD as i32);
            assert_eq!(gs.characters[cn].a_mana, 100_000);
            assert!(active_spell_item(gs, cn, SK_MSHIELD).is_none());

            skill_driver(gs, cn, SK_WARCRY as i32);
            assert_eq!(gs.characters[cn].a_end, 50_000);
        });
    }
}
