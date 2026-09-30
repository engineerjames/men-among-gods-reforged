//! Per-bot identity and in-world action selection.
//!
//! [`BotProfile`] is the single place a bot's tracked identity lives: the
//! class/sex/name it plays as (from the account API) plus the skills the
//! server has reported as known. Everything that decides *what* a bot does
//! (which spell to cast, what to say, what to interact with) reads from this
//! profile, so later behaviour-tree work can key off the same data.

use mag_core::skills::{
    MAX_SKILLS, SK_AURA_WAR_BANNER, SK_BLADE_DANCE, SK_BLESS, SK_DISPEL, SK_ENHANCE, SK_GHOST,
    SK_HEAL, SK_INNER_STRENGTH, SK_KINDRED_SPIRIT, SK_LIGHT, SK_MSHIELD, SK_PROTECT,
    SK_RAINS_OF_RENEWAL, SK_REVENANT_CONDUIT, SK_SEEING_RED, SK_SPECTRAL_PACT, SK_SUNS_BLESSING,
    SK_THUNDEROUS_FURY, SK_WARCRY, SK_WIMPY, get_skill_name, is_hostile_skill,
};
use mag_core::types::api::Class;
use rand::Rng;
use rand::seq::IndexedRandom;

use crate::api_bootstrap::BotCharacter;
use crate::world_view::WorldView;

/// Skills a bot may cast on itself without any extra target or held item.
///
/// Excludes: `SK_RECALL` (teleports away, undoing dispersion), `SK_IDENT` and
/// `SK_REPAIR` (need an item under the cursor), `SK_LOCK` (needs a lock-pick
/// and a door), and every passive/automatic skill the server refuses to cast
/// directly (`SK_REGEN`, `SK_REST`, `SK_MEDIT`, weapon skills, ...).
pub const SELF_CAST_SKILLS: &[usize] = &[
    SK_LIGHT,
    SK_PROTECT,
    SK_ENHANCE,
    SK_BLESS,
    SK_HEAL,
    SK_MSHIELD,
    SK_WIMPY,
    SK_DISPEL,
    SK_GHOST,
    SK_WARCRY,
    SK_BLADE_DANCE,
    SK_RAINS_OF_RENEWAL,
    SK_SUNS_BLESSING,
    SK_SEEING_RED,
    SK_THUNDEROUS_FURY,
    SK_INNER_STRENGTH,
    SK_REVENANT_CONDUIT,
    SK_KINDRED_SPIRIT,
    SK_SPECTRAL_PACT,
    SK_AURA_WAR_BANNER,
];

/// Fallback chat lines used when `behavior.chat.messages` is empty.
pub const DEFAULT_CHAT_MESSAGES: &[&str] = &[
    "hello there",
    "anyone around?",
    "nice weather today",
    "where is the temple?",
    "need a group for the dungeon",
    "brb",
    "gg",
    "does anyone sell potions?",
    "lag?",
    "follow me",
];

/// Tile radius scanned for a hostile-cast target.
const HOSTILE_TARGET_RADIUS: i32 = 6;

/// A single cast decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastChoice {
    /// Skill number (`SK_*`).
    pub skill: usize,
    /// Target character number; `0` lets the server default to the caster.
    pub target: u32,
}

/// Tracked identity and server-reported state for one bot.
#[derive(Debug, Clone)]
pub struct BotProfile {
    /// Character identity from the account API (id, name, class, sex).
    pub character: BotCharacter,
    /// Base skill value per skill (`skill[n][0]`); non-zero means known.
    pub skill_base: [u16; MAX_SKILLS],
    /// Current total braveness (`attrib[0][5]`), sent with `CL_SKILL`.
    pub braveness: u16,
}

impl BotProfile {
    /// Creates a profile with no known skills yet.
    ///
    /// # Arguments
    ///
    /// * `character` - Character identity returned by bootstrap.
    ///
    /// # Returns
    ///
    /// * A new profile awaiting `SV_SETCHARSKILL` updates.
    pub fn new(character: BotCharacter) -> Self {
        Self {
            character,
            skill_base: [0; MAX_SKILLS],
            braveness: 0,
        }
    }

    /// Character class this bot plays.
    ///
    /// # Returns
    ///
    /// * The class recorded by the account API.
    pub fn class(&self) -> Class {
        self.character.class
    }

    /// Records a `SV_SETCHARSKILL` update.
    ///
    /// # Arguments
    ///
    /// * `index` - Skill number.
    /// * `values` - The six per-skill values; only `[0]` (base) is kept.
    pub fn set_skill(&mut self, index: u8, values: &[u16; 6]) {
        if let Some(slot) = self.skill_base.get_mut(index as usize) {
            *slot = values[0];
        }
    }

    /// Records a `SV_SETCHARATTRIB` update for braveness (attribute `0`).
    ///
    /// # Arguments
    ///
    /// * `index` - Attribute number.
    /// * `values` - The six per-attribute values; `[5]` is the total.
    pub fn set_attrib(&mut self, index: u8, values: &[u16; 6]) {
        if index == 0 {
            self.braveness = values[5];
        }
    }

    /// Whether the server has reported `skill` as known.
    ///
    /// # Arguments
    ///
    /// * `skill` - Skill number.
    ///
    /// # Returns
    ///
    /// * `true` if the base value is non-zero.
    pub fn knows(&self, skill: usize) -> bool {
        self.skill_base.get(skill).is_some_and(|&v| v != 0)
    }

    /// Known skills that may be cast on the bot itself.
    ///
    /// # Returns
    ///
    /// * Skill numbers from [`SELF_CAST_SKILLS`] the bot knows.
    pub fn self_cast_skills(&self) -> Vec<usize> {
        SELF_CAST_SKILLS
            .iter()
            .copied()
            .filter(|&s| self.knows(s))
            .collect()
    }

    /// Known hostile skills (see [`is_hostile_skill`]).
    ///
    /// # Returns
    ///
    /// * Skill numbers the bot knows that require an enemy target.
    pub fn hostile_skills(&self) -> Vec<usize> {
        (0..MAX_SKILLS)
            .filter(|&s| is_hostile_skill(s) && self.knows(s))
            .collect()
    }

    /// Picks a skill to cast right now, if any is available.
    ///
    /// Self-cast skills target `0` (the server defaults to the caster).
    /// When `allow_hostile` is set and another character is visible nearby,
    /// hostile skills are pooled in too and target that character.
    ///
    /// # Arguments
    ///
    /// * `world` - Current map view, used to find a hostile target.
    /// * `allow_hostile` - Whether hostile skills may be chosen.
    /// * `rng` - RNG for the random pick.
    ///
    /// # Returns
    ///
    /// * `Some(choice)` if the bot knows at least one eligible skill.
    pub fn pick_cast(
        &self,
        world: &WorldView,
        allow_hostile: bool,
        rng: &mut impl Rng,
    ) -> Option<CastChoice> {
        let mut pool: Vec<CastChoice> = self
            .self_cast_skills()
            .into_iter()
            .map(|skill| CastChoice { skill, target: 0 })
            .collect();

        if allow_hostile {
            let targets = world.chars_within(HOSTILE_TARGET_RADIUS);
            if let Some(target) = targets.choose(rng) {
                pool.extend(self.hostile_skills().into_iter().map(|skill| CastChoice {
                    skill,
                    target: u32::from(target.ch_nr),
                }));
            }
        }

        pool.choose(rng).copied()
    }

    /// Human-readable summary of known castable skills, for logging.
    ///
    /// # Returns
    ///
    /// * Comma-separated skill names.
    pub fn describe_skills(&self) -> String {
        let mut names: Vec<&str> = self
            .self_cast_skills()
            .into_iter()
            .chain(self.hostile_skills())
            .map(get_skill_name)
            .filter(|n| !n.is_empty())
            .collect();
        names.sort_unstable();
        names.dedup();
        names.join(", ")
    }
}

/// Picks a chat line from `messages`, or from [`DEFAULT_CHAT_MESSAGES`] when
/// that pool is empty.
///
/// # Arguments
///
/// * `messages` - Configured message pool.
/// * `rng` - RNG for the random pick.
///
/// # Returns
///
/// * The chosen message text.
pub fn pick_chat_message<'a>(messages: &'a [String], rng: &mut impl Rng) -> &'a str {
    if let Some(m) = messages.choose(rng) {
        return m.as_str();
    }
    DEFAULT_CHAT_MESSAGES
        .choose(rng)
        .copied()
        .unwrap_or("hello")
}

/// Picks a usable tile within `radius` of the bot to interact with.
///
/// # Arguments
///
/// * `world` - Current map view.
/// * `radius` - Maximum tile distance from the bot.
/// * `rng` - RNG for the random pick.
///
/// # Returns
///
/// * `Some((x, y))` world coordinates of a usable item, or `None` if none
///   is visible within range.
pub fn pick_interact_target(
    world: &WorldView,
    radius: i32,
    rng: &mut impl Rng,
) -> Option<(i32, i32)> {
    world.usable_tiles_within(radius).choose(rng).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mag_core::constants::{ISCHAR, ISUSABLE, TILEX, TILEY};
    use mag_core::skills::{SK_BLAST, SK_RECALL, SK_WEAPON};
    use mag_core::types::api::Sex;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn profile() -> BotProfile {
        BotProfile::new(BotCharacter {
            id: 77,
            name: "loadtestd".into(),
            class: Class::Harakim,
            sex: Sex::Female,
        })
    }

    fn known(values0: u16) -> [u16; 6] {
        [values0, 0, 0, 0, 0, 0]
    }

    #[test]
    fn profile_tracks_identity_from_bootstrap() {
        let p = profile();
        assert_eq!(p.class(), Class::Harakim);
        assert_eq!(p.character.sex, Sex::Female);
        assert_eq!(p.character.id, 77);
        assert!(p.self_cast_skills().is_empty());
    }

    #[test]
    fn set_skill_marks_known_and_ignores_out_of_range() {
        let mut p = profile();
        p.set_skill(SK_BLESS as u8, &known(5));
        p.set_skill(u8::MAX, &known(5));
        assert!(p.knows(SK_BLESS));
        assert!(!p.knows(SK_HEAL));
        assert!(!p.knows(usize::MAX));
    }

    #[test]
    fn self_cast_pool_excludes_recall_weapon_and_hostile() {
        let mut p = profile();
        for s in [SK_BLESS, SK_RECALL, SK_WEAPON, SK_BLAST, SK_LIGHT] {
            p.set_skill(s as u8, &known(1));
        }
        let mut pool = p.self_cast_skills();
        pool.sort_unstable();
        assert_eq!(pool, vec![SK_LIGHT, SK_BLESS]);
        assert_eq!(p.hostile_skills(), vec![SK_BLAST]);
    }

    #[test]
    fn pick_cast_none_when_nothing_known() {
        let p = profile();
        let mut rng = StdRng::seed_from_u64(1);
        assert!(p.pick_cast(&WorldView::new(), true, &mut rng).is_none());
    }

    #[test]
    fn pick_cast_self_targets_zero() {
        let mut p = profile();
        p.set_skill(SK_PROTECT as u8, &known(1));
        let mut rng = StdRng::seed_from_u64(1);
        let c = p.pick_cast(&WorldView::new(), false, &mut rng).unwrap();
        assert_eq!(
            c,
            CastChoice {
                skill: SK_PROTECT,
                target: 0
            }
        );
    }

    #[test]
    fn hostile_only_cast_requires_visible_target() {
        let mut p = profile();
        p.set_skill(SK_BLAST as u8, &known(1));
        let mut rng = StdRng::seed_from_u64(1);

        let mut world = WorldView::new();
        assert!(p.pick_cast(&world, true, &mut rng).is_none());
        assert!(p.pick_cast(&world, false, &mut rng).is_none());

        world.apply_set_map(0, Some(0), None, None);
        world.set_origin(0, 0);
        let neighbour = (TILEX / 2 + 1 + (TILEY / 2) * TILEX) as u16;
        world.apply_set_map(0, Some(neighbour), Some(ISCHAR), Some(31));

        let c = p.pick_cast(&world, true, &mut rng).unwrap();
        assert_eq!(
            c,
            CastChoice {
                skill: SK_BLAST,
                target: 31
            }
        );
        assert!(p.pick_cast(&world, false, &mut rng).is_none());
    }

    #[test]
    fn chat_message_falls_back_to_defaults() {
        let mut rng = StdRng::seed_from_u64(2);
        let m = pick_chat_message(&[], &mut rng);
        assert!(DEFAULT_CHAT_MESSAGES.contains(&m));
        let pool = vec!["only".to_owned()];
        assert_eq!(pick_chat_message(&pool, &mut rng), "only");
    }

    #[test]
    fn interact_target_from_usable_tiles() {
        let mut rng = StdRng::seed_from_u64(3);
        let mut world = WorldView::new();
        assert!(pick_interact_target(&world, 5, &mut rng).is_none());
        world.set_origin(10, 20);
        let idx = (TILEX / 2 + 2 + (TILEY / 2) * TILEX) as u16;
        world.apply_set_map(0, Some(idx), Some(ISUSABLE), None);
        assert_eq!(
            pick_interact_target(&world, 5, &mut rng),
            Some((10 + TILEX as i32 / 2 + 2, 20 + TILEY as i32 / 2))
        );
        assert!(pick_interact_target(&world, 1, &mut rng).is_none());
    }

    #[test]
    fn describe_skills_lists_names() {
        let mut p = profile();
        p.set_skill(SK_BLESS as u8, &known(1));
        p.set_skill(SK_HEAL as u8, &known(1));
        let d = p.describe_skills();
        assert!(d.contains(get_skill_name(SK_BLESS)));
        assert!(d.contains(get_skill_name(SK_HEAL)));
    }
}
