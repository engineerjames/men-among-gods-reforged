//! File-backed loading of Journal content.

use crate::filepaths;
use crate::journal::markdown::{self, MdBlock, MdInline};

/// Loads and parses the markdown content file at `relative_path` (relative
/// to `client/assets/journal/`).
///
/// # Arguments
///
/// * `relative_path` - Path to the content file, relative to
///   `client/assets/journal/` (e.g. `"labyrinth/lab_one_grolms.md"`).
///
/// # Returns
///
/// * The parsed blocks on success. On any I/O error (missing file,
///   permissions, etc.) returns a single placeholder paragraph rather than
///   panicking, so a missing content file never crashes the client.
pub fn load(relative_path: &str) -> Vec<MdBlock> {
    let path = filepaths::get_asset_directory()
        .join("journal")
        .join(relative_path);

    match std::fs::read_to_string(&path) {
        Ok(source) => markdown::parse(&source),
        Err(_) => vec![MdBlock::Paragraph(vec![MdInline::Text(
            "Content coming soon.".to_owned(),
        )])],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_shrines_parses_as_completion_table() {
        let blocks = load("golden_shrines.md");
        let table = blocks
            .iter()
            .find_map(|b| match b {
                MdBlock::CompletionTable { key, headers, rows } if key == "explorer_points" => {
                    Some((headers, rows))
                }
                _ => None,
            })
            .expect("golden_shrines.md should contain an explorer_points completion_table");
        assert_eq!(table.0, &vec!["X".to_owned(), "Y".to_owned()]);
        assert_eq!(table.1.len(), 77);
    }

    #[test]
    fn missing_file_returns_placeholder() {
        let blocks = load("this/file/does/not/exist.md");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "Content coming soon.".to_owned()
            )])]
        );
    }
}
