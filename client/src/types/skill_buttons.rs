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

impl SkillButtons {
    /// Legacy client used `-1` (signed) as the sentinel for an unassigned hotbar slot.
    /// Stored as `u32` here; treat `0xFFFF_FFFF` as the equivalent.
    pub const UNASSIGNED_SKILL_NR: u32 = u32::MAX;

    pub fn skill_nr(&self) -> u32 {
        self.skill_nr
    }

    pub fn set_skill_nr(&mut self, skill_nr: u32) {
        self.skill_nr = skill_nr;
    }

    pub fn name_str(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        String::from_utf8_lossy(&self.name[..end]).to_string()
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = [0; 8];
        for (i, &b) in name.as_bytes().iter().take(7).enumerate() {
            self.name[i] = b;
        }
    }

    pub fn is_unassigned(&self) -> bool {
        if self.skill_nr == Self::UNASSIGNED_SKILL_NR {
            return true;
        }
        let s = self.name_str();
        s.is_empty() || s == "-"
    }

    pub fn set_unassigned(&mut self) {
        self.skill_nr = Self::UNASSIGNED_SKILL_NR;
        self.set_name("-");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_considered_unassigned() {
        let b = SkillButtons::default();
        assert!(b.is_unassigned());
    }

    #[test]
    fn set_unassigned_sets_sentinel_and_dash_name() {
        let mut b = SkillButtons::default();
        b.set_skill_nr(123);
        b.set_name("Fire");
        assert!(!b.is_unassigned());

        b.set_unassigned();
        assert!(b.is_unassigned());
        assert_eq!(b.skill_nr(), SkillButtons::UNASSIGNED_SKILL_NR);
        assert_eq!(b.name_str(), "-");
    }

    #[test]
    fn set_name_truncates_to_7_bytes_plus_nul() {
        let mut b = SkillButtons::default();
        b.set_name("123456789");
        assert_eq!(b.name_str(), "1234567");
    }
}
