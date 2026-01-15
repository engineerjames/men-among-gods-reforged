use crate::types::skill_buttons::SkillButtons;

// pdata from original C headers
#[repr(C)]
pub struct PlayerData {
    cname: [u8; 80],
    reference: [u8; 80],
    desc: [u8; 160],
    changed: i8,
    hide: i32,
    show_names: i32,
    show_proz: i32,
    skill_buttons: [SkillButtons; 12],
}

const _: () = {
    assert!(std::mem::size_of::<PlayerData>() == 480);
};

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            cname: [0; 80],
            reference: [0; 80],
            desc: [0; 160],
            changed: 0,
            hide: 0,
            show_names: 0,
            show_proz: 0,
            skill_buttons: [SkillButtons::default(); 12],
        }
    }
}
