//! Aura template registry.
//!
//! Provides the static configuration for each aura type. New auras are added
//! here; the rest of the subsystem refers to them through [`super::AuraId`].

use super::{AuraId, AuraKind, AuraTemplate};
use core::constants::TICKS;
use core::skills::{SK_AURA_CURSE, SK_AURA_WAR_BANNER};

const CURSE_AURA_NAME: &[u8] = b"Aura of Curses";
const WAR_BANNER_AURA_NAME: &[u8] = b"War Banner";

/// Returns the template for the given aura id.
///
/// # Arguments
///
/// * `id` - Aura type to look up.
///
/// # Panics
///
/// * Panics if `id` is not a known aura. This should never happen in practice
///   because the enum is exhaustive within this module.
pub fn aura_template(id: AuraId) -> AuraTemplate {
    match id {
        AuraId::CurseAura => AuraTemplate {
            id,
            kind: AuraKind::Debuff,
            radius_tiles: 12,
            pulse_interval_ticks: TICKS,
            spell_duration_ticks: TICKS * 2,
            name: CURSE_AURA_NAME,
            sprite: 89,
            temp: SK_AURA_CURSE as u16,
            power: 50,
        },
        AuraId::WarBannerAura => AuraTemplate {
            id,
            kind: AuraKind::Buff,
            radius_tiles: 12,
            pulse_interval_ticks: TICKS,
            spell_duration_ticks: TICKS * 2,
            name: WAR_BANNER_AURA_NAME,
            sprite: 90,
            temp: SK_AURA_WAR_BANNER as u16,
            power: 50,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curse_aura_is_debuff() {
        let template = aura_template(AuraId::CurseAura);
        assert_eq!(template.id, AuraId::CurseAura);
        assert!(matches!(template.kind, AuraKind::Debuff));
        assert_eq!(template.temp, SK_AURA_CURSE as u16);
    }

    #[test]
    fn war_banner_aura_is_buff() {
        let template = aura_template(AuraId::WarBannerAura);
        assert_eq!(template.id, AuraId::WarBannerAura);
        assert!(matches!(template.kind, AuraKind::Buff));
        assert_eq!(template.temp, SK_AURA_WAR_BANNER as u16);
    }
}
