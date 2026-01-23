use crate::types::skill_buttons::SkillButtons;

// pdata from original C headers
#[derive(Copy, Clone)]
#[repr(C)]
pub struct PlayerData {
    pub cname: [u8; 80],
    pub reference: [u8; 80],
    pub desc: [u8; 160],
    pub changed: i8,
    pub hide: i32,
    pub show_names: i32, // TODO: Change these to bools
    pub show_proz: i32,
    pub are_shadows_enabled: i32,
    pub skill_buttons: [SkillButtons; 12],
}

const _: () = {
    assert!(std::mem::size_of::<PlayerData>() == 484);
};

impl Default for PlayerData {
    /// Create a default player data record with sane client settings.
    fn default() -> Self {
        Self {
            cname: [0; 80],
            reference: [0; 80],
            desc: [0; 160],
            changed: 0,
            hide: 0,
            show_names: 1,
            show_proz: 0,
            are_shadows_enabled: 1,
            skill_buttons: [SkillButtons::default(); 12],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_shadows_enabled() {
        let pdata = PlayerData::default();
        assert_eq!(pdata.are_shadows_enabled, 1);
    }
}
