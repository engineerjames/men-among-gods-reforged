/// HUD toggle flags and spell-bar bindings, matching the original C `pdata`
/// struct (484 bytes).
// pdata from original C headers

pub const NUMBER_OF_KEYBINDS: usize = 13;

#[derive(Copy, Clone, Debug)]
pub struct PlayerData {
    pub hide: bool,
    pub show_names: bool,
    pub show_proz: bool,
    pub are_shadows_enabled: bool,
    /// Whether context-sensitive helper text is shown near the cursor.
    pub show_helper_text: bool,
    /// Custom CTRL+1-9 skill keybinds. Index 0 = key "1", index 8 = key "9".
    /// `Some(skill_nr)` if bound, `None` if unbound.
    pub skill_keybinds: [Option<usize>; NUMBER_OF_KEYBINDS],
}

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            hide: false,
            show_names: true,
            show_proz: true,
            are_shadows_enabled: true,
            show_helper_text: true,
            skill_keybinds: [None; NUMBER_OF_KEYBINDS],
        }
    }
}
