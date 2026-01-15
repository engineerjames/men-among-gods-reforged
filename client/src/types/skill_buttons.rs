// xbutton from original C headers
#[repr(C)]
pub struct SkillButtons {
    name: [u8; 8],
    skill_nr: u32,
}

const _: () = {
    assert!(std::mem::size_of::<SkillButtons>() == 12);
};
