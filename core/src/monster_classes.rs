//! Monster class name table, shared between the server (for lookup/logging)
//! and the client (for the Journal's "First Kills" checklist labels).
//!
//! Index `0` is a blank placeholder (no monster uses class `0`); indices
//! `1..=76` are the real monster class names, ported from the original C++
//! server's `npc_class[]` table.

/// Monster class names indexed by class ID. Index `0` is unused/blank.
pub const NPC_CLASS: [&str; 77] = [
    "",
    "Weak Thief",
    "Thief",
    "Ghost",
    "Weak Skeleton",
    "Strong Skeleton",
    "Skeleton",
    "Outlaw",
    "Grolm Fighter",
    "Grolm Warrior",
    "Grolm Knight",
    "Lizard Youngster",
    "Lizard Youth",
    "Lizard Worker",
    "Lizard Fighter",
    "Lizard Warrior",
    "Lizard Mage",
    "Ratling",
    "Ratling Fighter",
    "Ratling Warrior",
    "Ratling Knight",
    "Ratling Baron",
    "Ratling Count",
    "Ratling Duke",
    "Ratling Prince",
    "Ratling King",
    "Spellcaster",
    "Knight",
    "Weak Golem",
    "Captain Gargoyle",
    "Undead",
    "Very Strong Ice Gargoyle",
    "Strong Outlaw",
    "Private Grolm",
    "PFC Grolm",
    "Lance Corp Grolm",
    "Corporal Grolm",
    "Sergeant Grolm",
    "Staff Sergeant Grolm",
    "Master Sergeant Grolm",
    "First Sergeant Grolm",
    "Sergeant Major Grolm",
    "2nd Lieutenant Grolm",
    "1st Lieutenant Grolm",
    "Major Gargoyle",
    "Lt. Colonel Gargoyle",
    "Colonel Gargoyle",
    "Brig. General Gargoyle",
    "Major General Gargoyle",
    "Lieutenant Gargoyle",
    "Weak Spider",
    "Spider",
    "Strong Spider",
    "Very Strong Outlaw",
    "Lizard Knight",
    "Lizard Archmage",
    "Undead Lord",
    "Undead King",
    "Very Weak Ice Gargoyle",
    "Strong Golem",
    "Strong Ghost",
    "Shiva",
    "Flame",
    "Weak Ice Gargoyle",
    "Ice Gargoyle",
    "Strong Ice Gargoyle",
    "Greenling",
    "Greenling Fighter",
    "Greenling Warrior",
    "Greenling Knight",
    "Greenling Baron",
    "Greenling Count",
    "Greenling Duke",
    "Greenling Prince",
    "Greenling King",
    "Strong Thief",
    "Major Grolm",
];

/// Returns the monster class name for a given class number, or an error
/// string if `nr` is out of bounds.
///
/// # Arguments
/// * `nr` - Numeric monster class identifier.
///
/// # Returns
/// * The class's display name, or a placeholder error string if `nr` is
///   negative or exceeds the table's bounds.
pub fn get_class_name(nr: i32) -> &'static str {
    if nr < 0 {
        return "err... nothing";
    }
    let nr = nr as usize;
    if nr >= NPC_CLASS.len() {
        return "umm... whatzit";
    }
    NPC_CLASS[nr]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_zero_is_blank() {
        assert_eq!(get_class_name(0), "");
    }

    #[test]
    fn known_index_returns_name() {
        assert_eq!(get_class_name(1), "Weak Thief");
        assert_eq!(get_class_name(76), "Major Grolm");
    }

    #[test]
    fn negative_index_returns_error_string() {
        assert_eq!(get_class_name(-1), "err... nothing");
    }

    #[test]
    fn out_of_bounds_index_returns_error_string() {
        assert_eq!(get_class_name(77), "umm... whatzit");
        assert_eq!(get_class_name(1000), "umm... whatzit");
    }

    #[test]
    fn table_has_77_entries() {
        assert_eq!(NPC_CLASS.len(), 77);
    }
}
