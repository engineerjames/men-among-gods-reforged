//! Pulse-based aura subsystem.
//!
//! An aura source periodically scans nearby map tiles and applies a short-lived
//! spell item to valid targets. Targets that leave the area stop getting
//! refreshed and the spell expires naturally. Overlapping auras of the same
//! type maintain a single instance on each target; the strongest aura wins,
//! and equal-strength auras do not fight.

use crate::game_state::GameState;
use core::constants::{CharacterFlags, ItemFlags, USE_ACTIVE};
use core::skills::SkillIndex;

pub mod logic;
pub mod templates;

/// Unique identifier for an aura template.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AuraId {
    /// Debuff aura that mimics a curse.
    CurseAura,
    /// Buff aura that improves armor and weapon values.
    WarBannerAura,
}

/// Whether an aura is beneficial or harmful.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuraKind {
    /// Applies to allies.
    Buff,
    /// Applies to enemies.
    Debuff,
}

/// Runtime state for an active aura source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuraState {
    /// Which aura template is active.
    pub id: AuraId,
    /// Tick on which the next pulse should occur.
    pub next_pulse_tick: i32,
}

/// Static description of an aura type.
#[derive(Clone, Copy, Debug)]
pub struct AuraTemplate {
    /// Unique identifier.
    pub id: AuraId,
    /// Buff or debuff.
    pub kind: AuraKind,
    /// Square scan radius in tiles.
    pub radius_tiles: i32,
    /// Ticks between pulses.
    pub pulse_interval_ticks: i32,
    /// Ticks the applied spell item remains active.
    pub spell_duration_ticks: i32,
    /// Display name for the applied spell item.
    pub name: &'static [u8],
    /// Client sprite index for the spell bar.
    pub sprite: i16,
    /// Skill/template marker used to identify this aura's spell items.
    pub temp: u16,
    /// Base power used when scaling modifiers and resolving "strongest wins".
    ///
    /// The actual power applied to a spell item also includes a contribution
    /// from the source character's aura skill total, so raising the aura skill
    /// strengthens the effect.
    pub power: u32,
}

impl AuraTemplate {
    /// Returns true if the target is a valid recipient for this aura.
    ///
    /// # Arguments
    ///
    /// * `gs` - Active game state.
    /// * `source_cn` - Character index emitting the aura.
    /// * `target_cn` - Character index being considered.
    pub fn is_valid_target(&self, gs: &GameState, source_cn: usize, target_cn: usize) -> bool {
        if target_cn == 0 || target_cn >= core::constants::MAXCHARS {
            return false;
        }
        if source_cn == target_cn {
            // Buff auras are intended to help the caster; debuffs should never
            // apply to their own source.
            return matches!(self.kind, AuraKind::Buff);
        }

        let target = &gs.characters[target_cn];
        if target.used != USE_ACTIVE {
            return false;
        }
        if target.flags & CharacterFlags::Body.bits() != 0 {
            return false;
        }
        if target.flags & CharacterFlags::Invisible.bits() != 0 {
            return false;
        }

        let same_group = gs.characters[source_cn].data[core::constants::CHD_GROUP]
            == target.data[core::constants::CHD_GROUP];
        match self.kind {
            AuraKind::Buff => same_group,
            AuraKind::Debuff => !same_group,
        }
    }

    /// Returns the effective power of this aura for a given source.
    ///
    /// The base template power is increased by half the source's total aura
    /// skill value, so raising the aura skill makes the aura stronger.
    ///
    /// # Arguments
    ///
    /// * `gs` - Active game state.
    /// * `source_cn` - Character index emitting the aura.
    ///
    /// # Returns
    ///
    /// * Effective power, always at least 1.
    pub fn aura_power(&self, gs: &GameState, source_cn: usize) -> u32 {
        let skill_idx = self.temp as usize;
        let skill_total =
            gs.characters[source_cn].skill[skill_idx][SkillIndex::TotalValue as usize];
        (self.power + u32::from(skill_total) / 2).max(1)
    }

    /// Applies aura-specific modifiers to a freshly created spell item.
    ///
    /// # Arguments
    ///
    /// * `gs` - Active game state.
    /// * `item_idx` - Index of the spell item to modify.
    pub fn apply_modifiers(&self, gs: &mut GameState, item_idx: usize) {
        let power = gs.items[item_idx].power as i16;
        let item = &mut gs.items[item_idx];
        match self.id {
            AuraId::CurseAura => {
                // Mirror the attribute penalty from spell_curse at reduced power.
                for n in 0..5 {
                    item.attrib[n][1] = -(power / 3);
                }
            }
            AuraId::WarBannerAura => {
                // Improve armor and weapon values while active.
                let bonus = (power / 10).max(1);
                item.armor[1] = bonus as i8;
                item.weapon[1] = bonus as i8;
            }
        }
    }

    /// Creates the spell item backing this aura.
    ///
    /// # Arguments
    ///
    /// * `gs` - Active game state.
    /// * `source_cn` - Character index emitting the aura.
    ///
    /// # Returns
    ///
    /// * `Some(item_idx)` when creation succeeds, otherwise `None`.
    pub fn create_spell_item(&self, gs: &mut GameState, source_cn: usize) -> Option<usize> {
        let in_opt = crate::god::God::create_item(gs, 1);
        if in_opt.is_none() {
            log::error!("god_create_item failed for aura {:?}", self.id);
            return None;
        }
        let in_idx = in_opt.unwrap();
        let power = self.aura_power(gs, source_cn);
        let item = &mut gs.items[in_idx];

        let name_len = self.name.len().min(40);
        item.name[..name_len].copy_from_slice(&self.name[..name_len]);
        item.flags |= ItemFlags::IF_SPELL.bits();
        item.sprite[1] = self.sprite;
        item.duration = self.spell_duration_ticks as u32;
        item.active = self.spell_duration_ticks as u32;
        item.temp = self.temp;
        item.power = power;
        item.data[0] = source_cn as u32;

        self.apply_modifiers(gs, in_idx);

        Some(in_idx)
    }
}
