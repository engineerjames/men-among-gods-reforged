//! Static navigation tree for the in-game Journal (Guidebook) panel.
//!
//! This is hand-authored, compile-time data — there is no server-driven or
//! user-editable catalog. Content itself lives in external `.md` files under
//! `client/assets/journal/` (see [`crate::journal::content::load`]), so
//! editing copy does not require a rebuild; only adding/removing/reordering
//! nav entries does.

/// One leaf entry inside a category's subcategory list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalSubcategory {
    /// Display label shown in the middle nav column.
    pub label: &'static str,
    /// Path to the markdown content file, relative to
    /// `client/assets/journal/`.
    pub content_file: &'static str,
}

/// One top-level entry in the left nav column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JournalCategory {
    /// Display label shown in the left nav column.
    pub label: &'static str,
    /// Markdown content file to show when this category has no
    /// subcategories (or before one is selected). `None` when the category
    /// only ever shows subcategory content.
    pub content_file: Option<&'static str>,
    /// Nested subcategories shown in the middle nav column when this
    /// category is selected. Empty when the category has none.
    pub subcategories: &'static [JournalSubcategory],
}

/// The full Journal navigation tree, in display order.
pub static JOURNAL_CATALOG: &[JournalCategory] = &[
    JournalCategory {
        label: "Quest Log",
        content_file: Some("quest_log.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "First Kill",
        content_file: Some("first_kill.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Golden Shrines",
        content_file: Some("golden_shrines.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Labyrinth",
        content_file: Some("labyrinth/overview.md"),
        subcategories: &[
            JournalSubcategory {
                label: "Lab One: Grolms",
                content_file: "labyrinth/lab_one_grolms.md",
            },
            JournalSubcategory {
                label: "Lab Two: Lizards",
                content_file: "labyrinth/lab_two_lizards.md",
            },
            JournalSubcategory {
                label: "Lab Three: Wizards",
                content_file: "labyrinth/lab_three_wizards.md",
            },
        ],
    },
    JournalCategory {
        label: "Tower",
        content_file: Some("tower.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Titles",
        content_file: Some("titles.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Character Info",
        content_file: Some("character_info.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Pentagram Quest",
        content_file: Some("pentagram_quest.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Armor",
        content_file: Some("armor.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Weapons",
        content_file: Some("weapons.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Talents",
        content_file: Some("talents.md"),
        subcategories: &[],
    },
    JournalCategory {
        label: "Classes",
        content_file: Some("classes.md"),
        subcategories: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_non_empty() {
        assert!(!JOURNAL_CATALOG.is_empty());
    }

    #[test]
    fn every_category_has_content_file_or_subcategories() {
        for cat in JOURNAL_CATALOG {
            assert!(
                cat.content_file.is_some() || !cat.subcategories.is_empty(),
                "category {:?} has neither a content file nor subcategories",
                cat.label
            );
        }
    }

    #[test]
    fn labyrinth_has_three_subcategories() {
        let labyrinth = JOURNAL_CATALOG
            .iter()
            .find(|c| c.label == "Labyrinth")
            .expect("Labyrinth category should exist");
        assert_eq!(labyrinth.subcategories.len(), 3);
        assert_eq!(labyrinth.subcategories[2].label, "Lab Three: Wizards");
    }

    #[test]
    fn category_labels_are_unique() {
        let mut labels: Vec<&str> = JOURNAL_CATALOG.iter().map(|c| c.label).collect();
        let original_len = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), original_len);
    }

    /// Guards against the real bundling failure mode: a catalog entry
    /// pointing at a renamed/deleted `.md` file, which would otherwise
    /// silently fall back to "Content coming soon." in a shipped build with
    /// no build-time signal.
    #[test]
    fn all_content_files_exist_on_disk() {
        let journal_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/journal");
        for cat in JOURNAL_CATALOG {
            if let Some(file) = cat.content_file {
                assert!(
                    journal_dir.join(file).is_file(),
                    "category {:?} references missing content file {:?}",
                    cat.label,
                    file
                );
            }
            for sub in cat.subcategories {
                assert!(
                    journal_dir.join(sub.content_file).is_file(),
                    "subcategory {:?} (under {:?}) references missing content file {:?}",
                    sub.label,
                    cat.label,
                    sub.content_file
                );
            }
        }
    }
}
