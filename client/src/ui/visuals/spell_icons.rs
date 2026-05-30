//! Shared spell icon metadata for HUD widgets.
//!
//! Spell assets currently live as standalone PNG files under
//! `assets/gfx/spells`.  This module centralizes the skill-to-filename mapping
//! so temporary filesystem loading stays consistent until these graphics are
//! merged into the packed graphics cache.

use std::path::PathBuf;

use mag_core::skills;
use sdl2::pixels::Color;

use crate::filepaths;

/// Display metadata for a spell icon asset.
#[derive(Clone, Copy, Debug)]
pub struct SpellIconMeta {
    /// Short display name for tooltips or diagnostics.
    pub name: &'static str,
    /// Fallback tile color when the PNG cannot be loaded.
    pub color: Color,
    /// Filesystem icon filename under `assets/gfx/spells`.
    pub icon_filename: &'static str,
}

/// Returns the skill-bar icon metadata for a bindable spell skill.
///
/// This lookup covers every entry in [`crate::ui::hud::skill_picker_popup::BINDABLE_SKILLS`].
/// Active spell-effect indicators use [`active_spell_effect_icon_meta`] because
/// some effects intentionally differ from their castable skill icon.
///
/// # Arguments
///
/// * `skill_nr` - Protocol skill number matching one of the `SK_*` constants.
///
/// # Returns
///
/// * `Some(SpellIconMeta)` when the skill has a skill-bar icon, `None` otherwise.
pub fn spell_icon_meta(skill_nr: usize) -> Option<SpellIconMeta> {
    match skill_nr {
        skills::SK_MSHIELD => Some(SpellIconMeta {
            name: "Magic Shield",
            color: Color::RGB(100, 140, 255),
            icon_filename: "mshield_icon.png",
        }),
        skills::SK_REPAIR => Some(SpellIconMeta {
            name: "Repair",
            color: Color::RGB(170, 145, 90),
            icon_filename: "repair_icon.png",
        }),
        skills::SK_LIGHT => Some(SpellIconMeta {
            name: "Light",
            color: Color::RGB(240, 230, 100),
            icon_filename: "light_icon.png",
        }),
        skills::SK_RECALL => Some(SpellIconMeta {
            name: "Recall",
            color: Color::RGB(160, 200, 255),
            icon_filename: "recall_icon.png",
        }),
        skills::SK_WIMPY => Some(SpellIconMeta {
            name: "Guardian Angel",
            color: Color::RGB(160, 80, 40),
            icon_filename: "wimpy_icon.png",
        }),
        skills::SK_PROTECT => Some(SpellIconMeta {
            name: "Protection",
            color: Color::RGB(80, 160, 240),
            icon_filename: "protect_icon.png",
        }),
        skills::SK_ENHANCE => Some(SpellIconMeta {
            name: "Enhance Weapon",
            color: Color::RGB(100, 220, 100),
            icon_filename: "enhance_icon.png",
        }),
        skills::SK_STUN => Some(SpellIconMeta {
            name: "Stun",
            color: Color::RGB(200, 60, 60),
            icon_filename: "stun_icon.png",
        }),
        skills::SK_CURSE => Some(SpellIconMeta {
            name: "Curse",
            color: Color::RGB(180, 40, 200),
            icon_filename: "curse_icon.png",
        }),
        skills::SK_BLESS => Some(SpellIconMeta {
            name: "Bless",
            color: Color::RGB(200, 200, 255),
            icon_filename: "bless_icon.png",
        }),
        skills::SK_IDENT => Some(SpellIconMeta {
            name: "Identify",
            color: Color::RGB(220, 190, 80),
            icon_filename: "identify_icon.png",
        }),
        skills::SK_BLAST => Some(SpellIconMeta {
            name: "Blast",
            color: Color::RGB(230, 120, 70),
            icon_filename: "blast_icon.png",
        }),
        skills::SK_DISPEL => Some(SpellIconMeta {
            name: "Dispel Magic",
            color: Color::RGB(130, 210, 230),
            icon_filename: "dispel_icon.png",
        }),
        skills::SK_HEAL => Some(SpellIconMeta {
            name: "Heal",
            color: Color::RGB(90, 220, 140),
            icon_filename: "heal_icon.png",
        }),
        skills::SK_GHOST => Some(SpellIconMeta {
            name: "Ghost Companion",
            color: Color::RGB(160, 170, 210),
            icon_filename: "ghost_icon.png",
        }),
        skills::SK_WARCRY => Some(SpellIconMeta {
            name: "Warcry",
            color: Color::RGB(220, 110, 70),
            icon_filename: "warcry_icon.png",
        }),
        skills::SK_PARASITE => Some(SpellIconMeta {
            name: "Parasite",
            color: Color::RGB(140, 200, 80),
            icon_filename: "parasite_icon.png",
        }),
        skills::SK_DISTRACT => Some(SpellIconMeta {
            name: "Distract",
            color: Color::RGB(220, 220, 120),
            icon_filename: "distract_icon.png",
        }),
        skills::SK_DELIVER_DEATH => Some(SpellIconMeta {
            name: "Deliver Death",
            color: Color::RGB(200, 40, 40),
            icon_filename: "deliver_death_icon.png",
        }),
        skills::SK_DISARM => Some(SpellIconMeta {
            name: "Disarm",
            color: Color::RGB(180, 180, 200),
            icon_filename: "disarm_icon.png",
        }),
        skills::SK_CONTAGION => Some(SpellIconMeta {
            name: "Contagion",
            color: Color::RGB(120, 180, 80),
            icon_filename: "contagion_icon.png",
        }),
        skills::SK_BLADE_DANCE => Some(SpellIconMeta {
            name: "Blade Dance",
            color: Color::RGB(220, 160, 80),
            icon_filename: "blade_dance_icon.png",
        }),
        _ => None,
    }
}

/// Returns active spell-effect icon metadata for server-reported effects.
///
/// This intentionally excludes passive or non-indicator skills, even if they
/// are bindable on the skill bar. `SK_BLAST` maps to the exhaustion indicator
/// rather than the castable blast icon.
///
/// For potion effects where multiple potions share the same `skill_nr` (e.g.
/// skill 254 is used by both Greenling Eye and Ratling Eye potions), `sprite`
/// is used as a tiebreaker.  Pass `0` when the sprite is not available.
///
/// # Arguments
///
/// * `skill_nr` - Protocol skill number matching one of the `SK_*` constants.
/// * `sprite` - Sprite tile number from the `SetCharSpell` packet (bytes 5–6);
///   used to disambiguate effects that share a `skill_nr`. TODO: The server
///   already tells us the sprite id so just use that ffs.
///
/// # Returns
///
/// * `Some(SpellIconMeta)` when the skill has an active-effect indicator.
pub fn active_spell_effect_icon_meta(skill_nr: usize, sprite: i16) -> Option<SpellIconMeta> {
    match skill_nr {
        skills::SK_LIGHT
        | skills::SK_PROTECT
        | skills::SK_ENHANCE
        | skills::SK_BLESS
        | skills::SK_MSHIELD
        | skills::SK_RECALL
        | skills::SK_CURSE
        | skills::SK_STUN
        | skills::SK_WIMPY => spell_icon_meta(skill_nr),
        skills::SK_BLAST => Some(SpellIconMeta {
            name: "Spell Exhaustion",
            color: Color::RGB(200, 140, 40),
            icon_filename: "exhaustion_icon.png",
        }),
        // Eye potions share skill_nr 254 (data[1]); the sprite (data[0])
        // distinguishes them: Greenling Eye = 16741, Ratling Eye = 96.
        // Other consumables (Astonian Ale, Dragon's Breath, Mana Lite) also
        // use skill_nr 254 and fall through to the generic potion icon.
        254 => match sprite {
            16741 => Some(SpellIconMeta {
                name: "Greenling Eye Potion",
                color: Color::RGB(100, 210, 140),
                icon_filename: "gpot_icon.png",
            }),
            96 => Some(SpellIconMeta {
                name: "Ratling Eye Potion",
                color: Color::RGB(200, 160, 80),
                icon_filename: "rpot_icon.png",
            }),
            _ => Some(SpellIconMeta {
                name: "Potion Effect",
                color: Color::RGB(140, 200, 200),
                icon_filename: "potion_icon.png",
            }),
        },
        // Potion of Golem uses data[1]=449.
        449 => Some(SpellIconMeta {
            name: "Golem Potion",
            color: Color::RGB(160, 200, 100),
            icon_filename: "golempot_icon.png",
        }),
        _ => None,
    }
}

/// Builds the filesystem path for a spell icon asset.
///
/// # Arguments
///
/// * `meta` - Spell icon metadata containing the asset filename.
///
/// # Returns
///
/// * Path to the PNG under `assets/gfx/spells`.
pub fn spell_icon_path(meta: SpellIconMeta) -> PathBuf {
    filepaths::get_asset_directory()
        .join("gfx")
        .join("spells")
        .join(meta.icon_filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::hud::skill_picker_popup::BINDABLE_SKILLS;

    #[test]
    fn bindable_skills_have_skill_bar_icon_metadata() {
        for skill_nr in BINDABLE_SKILLS {
            let meta = spell_icon_meta(*skill_nr)
                .unwrap_or_else(|| panic!("missing icon metadata for skill {skill_nr}"));
            assert!(meta.icon_filename.ends_with("_icon.png"));
            assert!(!meta.name.is_empty());
        }
    }

    #[test]
    fn bindable_skill_icon_assets_exist() {
        for skill_nr in BINDABLE_SKILLS {
            let meta = spell_icon_meta(*skill_nr).unwrap();
            let path = spell_icon_path(meta);
            assert!(
                path.exists(),
                "missing icon asset for skill {} at {}",
                skill_nr,
                path.display()
            );
        }
    }

    #[test]
    fn blast_uses_distinct_skill_and_effect_icons() {
        let skill_meta = spell_icon_meta(skills::SK_BLAST).unwrap();
        let effect_meta = active_spell_effect_icon_meta(skills::SK_BLAST, 0).unwrap();
        assert_eq!(skill_meta.icon_filename, "blast_icon.png");
        assert_eq!(effect_meta.icon_filename, "exhaustion_icon.png");
    }

    #[test]
    fn passive_effects_do_not_have_active_indicators() {
        let passive = [
            skills::SK_REGEN,
            skills::SK_REST,
            skills::SK_MEDIT,
            skills::SK_GHOST,
            skills::SK_IMMUN,
            skills::SK_CONCEN,
            skills::SK_WARCRY,
        ];

        for skill_nr in passive {
            assert!(active_spell_effect_icon_meta(skill_nr, 0).is_none());
        }
    }
}
