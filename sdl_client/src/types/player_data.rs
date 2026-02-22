use crate::types::skill_buttons::SkillButtons;

/// HUD toggle flags and spell-bar bindings, matching the original C `pdata`
/// struct (484 bytes).
// pdata from original C headers
#[derive(Copy, Clone, Debug)]
pub struct PlayerData {
    pub hide: i32,
    pub show_names: i32,
    pub show_proz: i32,
    pub are_shadows_enabled: i32,
    pub skill_buttons: [SkillButtons; 12],
}

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            hide: 0,
            show_names: 1,
            show_proz: 0,
            are_shadows_enabled: 1,
            skill_buttons: [SkillButtons::default(); 12],
        }
    }
}
