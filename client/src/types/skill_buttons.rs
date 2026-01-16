// xbutton from original C headers
#[derive(Copy, Clone)]
#[repr(C)]
pub struct SkillButtons {
    name: [u8; 8],
    skill_nr: u32,
}

const _: () = {
    assert!(std::mem::size_of::<SkillButtons>() == 12);
};

impl Default for SkillButtons {
    fn default() -> Self {
        Self {
            name: [0; 8],
            skill_nr: 0,
        }
    }
}
