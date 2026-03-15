/// HUD toggle flags and spell-bar bindings, matching the original C `pdata`
/// struct (484 bytes).
// pdata from original C headers
#[derive(Copy, Clone, Debug)]
pub struct PlayerData {
    pub hide: i32,
    pub show_names: i32,
    pub show_proz: i32,
    pub are_shadows_enabled: i32,
    /// Whether context-sensitive helper text is shown near the cursor.
    pub show_helper_text: i32,
    /// Custom CTRL+1-9 skill keybinds. Index 0 = key "1", index 8 = key "9".
    /// `Some(skill_nr)` if bound, `None` if unbound.
    pub skill_keybinds: [Option<u32>; 9],
}

impl Default for PlayerData {
    fn default() -> Self {
        Self {
            hide: 0,
            show_names: 1,
            show_proz: 0,
            are_shadows_enabled: 1,
            show_helper_text: 1,
            skill_keybinds: [None; 9],
        }
    }
}
