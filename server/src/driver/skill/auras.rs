use super::common::{add_skill_cooldown, effective_resist, skill_on_cooldown};
use crate::aura::{self, AuraId};
use crate::chlog;
use crate::effect::EffectManager;
use crate::game_state::GameState;
use core::constants::{CharacterFlags, TICKS};
use core::skills::{SK_RESIST, SK_SOUL_REFLECTION};
use core::types::FontColor;

/// Active hostile AoE: Soul Reflection. Terrifies nearby NPCs, causing them to
/// flee from the caster for a short time. Has no effect on player characters.
///
/// # Arguments
///
/// * `gs` - Active game state used for endurance costs, nearby target lookup, and NPC panic state.
/// * `cn` - Caster character index.
pub fn skill_soul_reflection(gs: &mut GameState, cn: usize) {
    if skill_on_cooldown(gs, cn, SK_SOUL_REFLECTION as u16) {
        return;
    }
    if gs.characters[cn].a_end < 150 * 1000 {
        gs.do_character_log(cn, FontColor::Red, "You're too exhausted!\n");
        return;
    }
    gs.characters[cn].a_end -= 150 * 1000;

    let power = i32::from(gs.characters[cn].skill[SK_SOUL_REFLECTION][5]);
    let flee_until = gs.globals.ticker + TICKS * 10;

    let caster_x = i32::from(gs.characters[cn].x);
    let caster_y = i32::from(gs.characters[cn].y);
    let xf = std::cmp::max(1, caster_x - 8);
    let yf = std::cmp::max(1, caster_y - 8);
    let xt = std::cmp::min(core::constants::SERVER_MAPX - 1, caster_x + 8);
    let yt = std::cmp::min(core::constants::SERVER_MAPY - 1, caster_y + 8);

    let mut routed = 0;
    // Inclusive bounds so the radius is 8 tiles in every direction.
    for x in xf..=xt {
        for y in yf..=yt {
            let m = (x + y * core::constants::SERVER_MAPX) as usize;
            let co = gs.map[m].ch as usize;
            if co == 0 || co == cn {
                continue;
            }
            // NPCs only; players are unaffected by this fear effect.
            if (gs.characters[co].flags & CharacterFlags::Player.bits()) != 0 {
                continue;
            }
            if (gs.characters[co].flags & CharacterFlags::Immortal.bits()) != 0 {
                continue;
            }
            if !gs.may_aoe_attack(cn, co) {
                continue;
            }
            if power < effective_resist(gs, cn, i32::from(gs.characters[co].skill[SK_RESIST][5])) {
                continue;
            }

            // Reuse the existing NPC panic-flee timer and clear its current
            // target so `npc_driver_high` picks a flee destination next tick.
            gs.characters[co].data[78] = flee_until;
            gs.characters[co].attack_cn = 0;
            gs.characters[co].goto_x = 0;
            gs.characters[co].goto_y = 0;
            routed += 1;
        }
    }

    gs.do_character_log(
        cn,
        FontColor::Green,
        &format!(
            "Your presence unravels the courage of {} nearby foe(s).\n",
            routed
        ),
    );
    chlog!(cn, "Cast Soul Reflection ({} routed)", routed);

    EffectManager::fx_add_effect(gs, 7, 0, caster_x, caster_y, 0);

    add_skill_cooldown(
        gs,
        cn,
        TICKS * 30,
        SK_SOUL_REFLECTION as u16,
        b"Soul Reflection Cooldown",
    );
}

/// Toggles the Curse Aura on or off.
///
/// When active, the caster periodically curses nearby enemies.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character index toggling the aura.
pub fn skill_curse_aura(gs: &mut GameState, cn: usize) {
    aura::logic::toggle_aura(
        gs,
        cn,
        AuraId::CurseAura,
        "You surround yourself with an aura of curses.\n",
        "You dismiss your aura of curses.\n",
    );
}

/// Toggles the War Banner aura on or off.
///
/// When active, the caster periodically improves the armor and weapon values
/// of nearby allies.
///
/// # Arguments
///
/// * `gs` - Active game state.
/// * `cn` - Character index toggling the aura.
pub fn skill_war_banner_aura(gs: &mut GameState, cn: usize) {
    aura::logic::toggle_aura(
        gs,
        cn,
        AuraId::WarBannerAura,
        "You raise a war banner.\n",
        "You lower your war banner.\n",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aura::logic::has_aura;
    use crate::driver::skill::tests::{active_spell_item, place_attacker};
    use crate::test_helpers::{add_test_player, with_test_gs};
    use core::constants::SERVER_MAPX;
    use core::constants::USE_ACTIVE;
    use core::types::Character;

    #[test]
    fn skill_soul_reflection_routs_npc_but_spares_player() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);
            gs.characters[cn].skill[SK_SOUL_REFLECTION][0] = 1;
            gs.characters[cn].skill[SK_SOUL_REFLECTION][5] = 50;
            gs.characters[cn].a_end = 1_000_000;
            gs.globals.ticker = 1000;

            let npc = 2;
            gs.characters[npc] = Character::default();
            gs.characters[npc].used = USE_ACTIVE;
            gs.characters[npc].x = gs.characters[cn].x + 1;
            gs.characters[npc].y = gs.characters[cn].y;
            gs.characters[npc].attack_cn = cn as u16;
            gs.map[gs.characters[npc].x as usize
                + gs.characters[npc].y as usize * SERVER_MAPX as usize]
                .ch = npc as u32;

            let other_player = 3;
            gs.characters[other_player] = Character::default();
            gs.characters[other_player].used = USE_ACTIVE;
            gs.characters[other_player].flags = CharacterFlags::Player.bits();
            gs.characters[other_player].x = gs.characters[cn].x - 1;
            gs.characters[other_player].y = gs.characters[cn].y;
            gs.characters[other_player].attack_cn = cn as u16;
            gs.map[gs.characters[other_player].x as usize
                + gs.characters[other_player].y as usize * SERVER_MAPX as usize]
                .ch = other_player as u32;

            skill_soul_reflection(gs, cn);

            assert_eq!(gs.characters[npc].data[78], 1000 + TICKS * 10);
            assert_eq!(gs.characters[npc].attack_cn, 0);
            assert_eq!(gs.characters[other_player].data[78], 0);
            assert_eq!(gs.characters[other_player].attack_cn, cn as u16);
        });
    }

    /// Player with Soul Reflection 50 and an NPC attacker (index 2) next to them.
    fn reflector_with_npc(gs: &mut GameState) -> (usize, usize) {
        let (cn, _nr) = add_test_player(gs);
        gs.item_templates[1].used = USE_ACTIVE;
        gs.characters[cn].skill[SK_SOUL_REFLECTION][0] = 1;
        gs.characters[cn].skill[SK_SOUL_REFLECTION][5] = 50;
        gs.characters[cn].a_end = 400_000;
        gs.globals.ticker = 1000;
        place_attacker(gs, cn, 2, 1, 0);
        (cn, 2)
    }

    #[test]
    fn soul_reflection_requires_endurance() {
        with_test_gs(|gs| {
            let (cn, npc) = reflector_with_npc(gs);
            gs.characters[cn].a_end = 149_999;

            skill_soul_reflection(gs, cn);

            assert_eq!(gs.characters[cn].a_end, 149_999);
            assert_eq!(gs.characters[npc].attack_cn, cn as u16);
            assert!(active_spell_item(gs, cn, SK_SOUL_REFLECTION).is_none());
        });
    }

    #[test]
    fn soul_reflection_spends_endurance_and_blocks_recast() {
        with_test_gs(|gs| {
            let (cn, _npc) = reflector_with_npc(gs);

            skill_soul_reflection(gs, cn);
            assert_eq!(gs.characters[cn].a_end, 250_000);
            let cooldown =
                active_spell_item(gs, cn, SK_SOUL_REFLECTION).expect("cooldown should be added");
            assert_eq!(gs.items[cooldown].active, (TICKS * 30) as u32);

            skill_soul_reflection(gs, cn);
            assert_eq!(gs.characters[cn].a_end, 250_000);
        });
    }

    #[test]
    fn soul_reflection_spares_immortal_and_resistant_npcs() {
        with_test_gs(|gs| {
            let (cn, npc) = reflector_with_npc(gs);
            gs.characters[npc].flags |= CharacterFlags::Immortal.bits();
            let resistant = 3;
            place_attacker(gs, cn, resistant, 0, 1);
            gs.characters[resistant].skill[SK_RESIST][5] = 51;

            skill_soul_reflection(gs, cn);

            assert_eq!(gs.characters[npc].data[78], 0);
            assert_eq!(gs.characters[npc].attack_cn, cn as u16);
            assert_eq!(gs.characters[resistant].data[78], 0);
            assert_eq!(gs.characters[resistant].attack_cn, cn as u16);
        });
    }

    #[test]
    fn soul_reflection_routs_npcs_with_resist_equal_to_power() {
        with_test_gs(|gs| {
            let (cn, npc) = reflector_with_npc(gs);
            gs.characters[npc].skill[SK_RESIST][5] = 50;
            gs.characters[npc].goto_x = 5;
            gs.characters[npc].goto_y = 5;

            skill_soul_reflection(gs, cn);

            assert_eq!(gs.characters[npc].data[78], 1000 + TICKS * 10);
            assert_eq!(gs.characters[npc].goto_x, 0);
            assert_eq!(gs.characters[npc].goto_y, 0);
        });
    }

    #[test]
    fn curse_aura_toggles_on_and_off() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);

            skill_curse_aura(gs, cn);
            assert!(has_aura(gs, cn, AuraId::CurseAura));

            skill_curse_aura(gs, cn);
            assert!(!has_aura(gs, cn, AuraId::CurseAura));
        });
    }

    #[test]
    fn war_banner_aura_toggles_on_and_off() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);

            skill_war_banner_aura(gs, cn);
            assert!(has_aura(gs, cn, AuraId::WarBannerAura));

            skill_war_banner_aura(gs, cn);
            assert!(!has_aura(gs, cn, AuraId::WarBannerAura));
        });
    }

    #[test]
    fn switching_auras_replaces_the_active_one() {
        with_test_gs(|gs| {
            let (cn, _nr) = add_test_player(gs);

            skill_curse_aura(gs, cn);
            skill_war_banner_aura(gs, cn);

            assert!(has_aura(gs, cn, AuraId::WarBannerAura));
            assert!(!has_aura(gs, cn, AuraId::CurseAura));
        });
    }

    #[test]
    fn soul_reflection_reaches_eight_tiles_in_every_direction() {
        with_test_gs(|gs| {
            let (cn, _npc) = reflector_with_npc(gs);
            gs.characters[cn].x = 50;
            gs.characters[cn].y = 50;
            let edges = [(3, 8, 0), (4, 0, 8), (5, -8, 0), (6, 0, -8)];
            for (co, dx, dy) in edges {
                place_attacker(gs, cn, co, dx, dy);
            }
            let (outside_east, outside_south) = (7, 8);
            place_attacker(gs, cn, outside_east, 9, 0);
            place_attacker(gs, cn, outside_south, 0, 9);

            skill_soul_reflection(gs, cn);

            for (co, dx, dy) in edges {
                assert_eq!(gs.characters[co].attack_cn, 0, "offset ({dx}, {dy})");
            }
            assert_eq!(gs.characters[outside_east].attack_cn, cn as u16);
            assert_eq!(gs.characters[outside_south].attack_cn, cn as u16);
        });
    }
}
