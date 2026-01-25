// xbutton from original C headers
use serde::{Deserialize, Serialize};

// xbutton from original C headers
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct SkillButtons {
    name: [u8; 8],
    skill_nr: u32,
}

const _: () = {
    assert!(std::mem::size_of::<SkillButtons>() == 12);
};

impl Default for SkillButtons {
    /// Create a default (unassigned) skill button entry.
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

    /// Return the stored skill number.
    pub fn skill_nr(&self) -> u32 {
        self.skill_nr
    }

    /// Set the stored skill number.
    pub fn set_skill_nr(&mut self, skill_nr: u32) {
        self.skill_nr = skill_nr;
    }

    /// Return the button name as a UTF-8 string.
    pub fn name_str(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        String::from_utf8_lossy(&self.name[..end]).to_string()
    }

    /// Set the button name, truncating to 7 bytes.
    pub fn set_name(&mut self, name: &str) {
        self.name = [0; 8];
        for (i, &b) in name.as_bytes().iter().take(7).enumerate() {
            self.name[i] = b;
        }
    }

    /// Return true if the slot is unassigned.
    pub fn is_unassigned(&self) -> bool {
        if self.skill_nr == Self::UNASSIGNED_SKILL_NR {
            return true;
        }
        let s = self.name_str();
        s.is_empty() || s == "-"
    }

    /// Mark the slot as unassigned using the legacy sentinel values.
    pub fn set_unassigned(&mut self) {
        self.skill_nr = Self::UNASSIGNED_SKILL_NR;
        self.set_name("-");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Ensure the default button is treated as unassigned.
    fn default_is_considered_unassigned() {
        let b = SkillButtons::default();
        assert!(b.is_unassigned());
    }

    #[test]
    /// Verify `set_unassigned` sets both sentinel fields.
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
    /// Confirm name truncation to 7 bytes plus NUL.
    fn set_name_truncates_to_7_bytes_plus_nul() {
        let mut b = SkillButtons::default();
        b.set_name("123456789");
        assert_eq!(b.name_str(), "1234567");
    }
}
