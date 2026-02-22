/// A single spell-bar button binding, matching the original C `xbutton`
/// struct (12 bytes). Stores an 8-byte display name and the `skilltab`
/// index of the bound skill.
// xbutton from original C headers
#[derive(Copy, Clone, Debug)]
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

impl SkillButtons {
    /// Sentinel value indicating no skill is assigned to this button.
    pub const UNASSIGNED_SKILL_NR: u32 = u32::MAX;

    /// Returns the `skilltab` index of the bound skill.
    pub fn skill_nr(&self) -> u32 {
        self.skill_nr
    }

    /// Sets the bound skill index.
    pub fn set_skill_nr(&mut self, skill_nr: u32) {
        self.skill_nr = skill_nr;
    }

    /// Returns the display name as a `String`, stopping at the first null byte.
    pub fn name_str(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        String::from_utf8_lossy(&self.name[..end]).to_string()
    }

    /// Sets the display name, truncating to 7 characters.
    pub fn set_name(&mut self, name: &str) {
        self.name = [0; 8];
        for (i, &b) in name.as_bytes().iter().take(7).enumerate() {
            self.name[i] = b;
        }
    }

    /// Returns `true` if no skill is assigned to this button.
    pub fn is_unassigned(&self) -> bool {
        if self.skill_nr == Self::UNASSIGNED_SKILL_NR {
            return true;
        }
        let s = self.name_str();
        s.is_empty() || s == "-"
    }

    /// Marks this button as unassigned (sets sentinel skill_nr and "-" name).
    pub fn set_unassigned(&mut self) {
        self.skill_nr = Self::UNASSIGNED_SKILL_NR;
        self.set_name("-");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unassigned() {
        let btn = SkillButtons::default();
        assert!(btn.is_unassigned());
        assert_eq!(btn.skill_nr(), 0);
        assert_eq!(btn.name_str(), "");
    }

    #[test]
    fn set_name_and_skill_nr() {
        let mut btn = SkillButtons::default();
        btn.set_name("Fire");
        btn.set_skill_nr(5);
        assert_eq!(btn.name_str(), "Fire");
        assert_eq!(btn.skill_nr(), 5);
        assert!(!btn.is_unassigned());
    }

    #[test]
    fn name_truncated_to_7_chars() {
        let mut btn = SkillButtons::default();
        btn.set_name("LongSpellName");
        assert_eq!(btn.name_str(), "LongSpe");
    }

    #[test]
    fn is_unassigned_with_dash_name() {
        let mut btn = SkillButtons::default();
        btn.set_name("-");
        btn.set_skill_nr(42);
        assert!(btn.is_unassigned());
    }

    #[test]
    fn is_unassigned_with_sentinel_nr() {
        let mut btn = SkillButtons::default();
        btn.set_name("Fire");
        btn.set_skill_nr(SkillButtons::UNASSIGNED_SKILL_NR);
        assert!(btn.is_unassigned());
    }

    #[test]
    fn set_unassigned_clears() {
        let mut btn = SkillButtons::default();
        btn.set_name("Fire");
        btn.set_skill_nr(5);
        btn.set_unassigned();
        assert!(btn.is_unassigned());
        assert_eq!(btn.skill_nr(), SkillButtons::UNASSIGNED_SKILL_NR);
        assert_eq!(btn.name_str(), "-");
    }
}
