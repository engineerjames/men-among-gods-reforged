/// Effect structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct Effect {
    pub used: u8,
    pub flags: u8,

    pub effect_type: u8, // what type of effect (FX_)

    pub duration: u32, // time effect will stay

    pub data: [u32; 10], // some data
}
