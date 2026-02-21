// xbutton from original C headers
#[derive(Copy, Clone, Debug)]
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
    pub const UNASSIGNED_SKILL_NR: u32 = u32::MAX;

    pub fn skill_nr(&self) -> u32 {
        self.skill_nr
    }

    #[allow(dead_code)]
    pub fn set_skill_nr(&mut self, skill_nr: u32) {
        self.skill_nr = skill_nr;
    }

    #[allow(dead_code)]
    pub fn name_str(&self) -> String {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        String::from_utf8_lossy(&self.name[..end]).to_string()
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn set_unassigned(&mut self) {
        self.skill_nr = Self::UNASSIGNED_SKILL_NR;
        self.set_name("-");
    }
}
