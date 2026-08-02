//! Minimal markdown-subset parser for Journal content.
//!
//! Deliberately hand-rolled and intentionally small: paragraphs separated by
//! blank lines, `**bold**` emphasis spans, standalone `![alt](path)` image
//! lines, and two Journal-specific fenced-code-block extensions used to
//! render server-driven completion data (see [`MdBlock::CompletionChecklist`]
//! and [`MdBlock::CompletionCounter`]). No headings, links, lists, or nested
//! markup are supported — anything else is treated as plain text.
//!
//! The two completion fences (` ```completion_checklist:<key> ` /
//! ` ```completion_counter:<key> `) are plain fenced code blocks by design:
//! an external, generic markdown viewer (e.g. GitHub, VS Code preview) will
//! render their body as inert code text rather than crashing or mis-parsing,
//! satisfying the "future portability" goal without needing a separate
//! strip/post-process step.

/// One inline run of text within a [`MdBlock::Paragraph`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MdInline {
    /// Plain, unstyled text.
    Text(String),
    /// Text that was wrapped in `**...**` in the source.
    Bold(String),
}

/// One block-level element of parsed Journal content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MdBlock {
    /// A paragraph made up of one or more inline runs.
    Paragraph(Vec<MdInline>),
    /// A standalone image reference, on its own line in the source as
    /// `![alt](path)`.
    Image {
        /// Path to the image asset (interpretation is up to the renderer).
        path: String,
        /// Alt text, shown as a fallback when the image fails to load.
        alt: String,
    },
    /// A completion checklist, parsed from a
    /// ` ```completion_checklist:<key> ` fenced block whose body lines are
    /// `index: Label`.
    CompletionChecklist {
        /// Identifies which completion bitset this checklist renders against
        /// (e.g. `"first_kill"`, `"explorer_points"`, `"quests"`,
        /// `"labyrinth_overview"`). Interpreted by the renderer.
        key: String,
        /// `(index, label)` pairs, in source order.
        items: Vec<(u16, String)>,
    },
    /// A completion counter, parsed from a
    /// ` ```completion_counter:<key> ` fenced block whose body lines are
    /// `label: ...` and optionally `max: N`.
    CompletionCounter {
        /// Identifies which completion value this counter renders (e.g.
        /// `"pentagram_solves"`). Interpreted by the renderer.
        key: String,
        /// Optional upper bound, rendered as `value/max` when present.
        max: Option<u32>,
        /// Display label shown before the value.
        label: String,
    },
}

/// Parses `source` into a sequence of [`MdBlock`]s.
///
/// Blank lines (lines that are empty after trimming) separate blocks. A
/// block consisting of exactly one line matching `![alt](path)` becomes an
/// [`MdBlock::Image`]; any other block becomes an [`MdBlock::Paragraph`]
/// with its lines joined by spaces and `**bold**` spans extracted.
///
/// Lines starting with `` ``` `` open a fenced block that runs until a line
/// that is exactly `` ``` `` (or end of input). Fences tagged
/// `completion_checklist:<key>` or `completion_counter:<key>` are parsed into
/// the matching [`MdBlock`] variant; any other fenced content (unknown tag,
/// plain code fence, or an unterminated fence) is treated as plain paragraph
/// text instead, so this parser never panics on unexpected fenced content.
///
/// # Arguments
///
/// * `source` - Raw markdown-subset text to parse.
///
/// # Returns
///
/// * The parsed blocks, in source order. Returns an empty `Vec` for empty or
///   all-blank input.
pub fn parse(source: &str) -> Vec<MdBlock> {
    let mut blocks = Vec::new();
    let mut current_lines: Vec<&str> = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        if let Some(tag) = line.trim().strip_prefix("```") {
            let fence_start = i;
            let close_idx = lines[(i + 1)..].iter().position(|l| l.trim() == "```");

            match close_idx {
                Some(offset) => {
                    let close_idx = fence_start + 1 + offset;
                    let body_lines = &lines[(fence_start + 1)..close_idx];
                    let tag = tag.trim();
                    if let Some(key) = tag.strip_prefix("completion_checklist:") {
                        flush_paragraph(&mut current_lines, &mut blocks);
                        blocks.push(parse_completion_checklist(key, body_lines));
                    } else if let Some(key) = tag.strip_prefix("completion_counter:") {
                        flush_paragraph(&mut current_lines, &mut blocks);
                        blocks.push(parse_completion_counter(key, body_lines));
                    } else {
                        // Unknown tag or plain code fence: keep the fence
                        // markers and body as plain paragraph text.
                        current_lines.extend_from_slice(&lines[fence_start..=close_idx]);
                    }
                    i = close_idx + 1;
                    continue;
                }
                None => {
                    // Unterminated fence: treat the rest of the input as
                    // plain paragraph text instead of panicking/looping.
                    current_lines.extend_from_slice(&lines[fence_start..]);
                    i = lines.len();
                    continue;
                }
            }
        }

        if line.trim().is_empty() {
            flush_paragraph(&mut current_lines, &mut blocks);
        } else {
            current_lines.push(line);
        }
        i += 1;
    }
    flush_paragraph(&mut current_lines, &mut blocks);

    blocks
}

/// Parses the body of a `completion_checklist:<key>` fenced block.
///
/// Each non-blank body line is expected to be `index: Label`; lines that
/// don't parse as `u16: text` are silently skipped (defensive against
/// hand-typo'd markdown, never panics).
///
/// # Arguments
///
/// * `key` - The checklist key (text after the `completion_checklist:` tag).
/// * `body_lines` - The fenced block's body lines (excluding the fence
///   markers).
///
/// # Returns
///
/// * An [`MdBlock::CompletionChecklist`] with `key` and the parsed items.
fn parse_completion_checklist(key: &str, body_lines: &[&str]) -> MdBlock {
    let mut items = Vec::new();
    for line in body_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((index_str, label)) = trimmed.split_once(':')
            && let Ok(index) = index_str.trim().parse::<u16>()
        {
            items.push((index, label.trim().to_owned()));
        }
    }
    MdBlock::CompletionChecklist {
        key: key.to_owned(),
        items,
    }
}

/// Parses the body of a `completion_counter:<key>` fenced block.
///
/// Recognizes `label: ...` and `max: N` lines (any order, both optional);
/// unrecognized lines are silently skipped.
///
/// # Arguments
///
/// * `key` - The counter key (text after the `completion_counter:` tag).
/// * `body_lines` - The fenced block's body lines (excluding the fence
///   markers).
///
/// # Returns
///
/// * An [`MdBlock::CompletionCounter`] with `key`, the parsed `max` (if any),
///   and `label` (empty string if not specified).
fn parse_completion_counter(key: &str, body_lines: &[&str]) -> MdBlock {
    let mut max = None;
    let mut label = String::new();
    for line in body_lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("max:") {
            if let Ok(parsed) = rest.trim().parse::<u32>() {
                max = Some(parsed);
            }
        } else if let Some(rest) = trimmed.strip_prefix("label:") {
            label = rest.trim().to_owned();
        }
    }
    MdBlock::CompletionCounter {
        key: key.to_owned(),
        max,
        label,
    }
}

/// Flushes the currently accumulated non-blank `lines` into `blocks` as
/// either a single [`MdBlock::Image`] (when `lines` is exactly one
/// `![alt](path)` line) or an [`MdBlock::Paragraph`]. No-op when `lines` is
/// empty. Clears `lines` afterward.
///
/// # Arguments
///
/// * `lines` - Accumulated non-blank source lines for the current block.
/// * `blocks` - Output block list to append to.
fn flush_paragraph<'a>(lines: &mut Vec<&'a str>, blocks: &mut Vec<MdBlock>) {
    if lines.is_empty() {
        return;
    }
    if lines.len() == 1
        && let Some((alt, path)) = parse_image_line(lines[0].trim())
    {
        blocks.push(MdBlock::Image { path, alt });
        lines.clear();
        return;
    }
    let joined = lines.join(" ");
    blocks.push(MdBlock::Paragraph(parse_inline(&joined)));
    lines.clear();
}

/// Attempts to parse `line` as a standalone `![alt](path)` image reference.
///
/// # Arguments
///
/// * `line` - A single, already-trimmed source line.
///
/// # Returns
///
/// * `Some((alt, path))` when the entire line matches the image syntax;
///   `None` otherwise (including malformed/partial matches), so callers can
///   fall back to treating it as plain text.
fn parse_image_line(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("![")?;
    let close_bracket = rest.find(']')?;
    let alt = &rest[..close_bracket];
    let after_alt = &rest[close_bracket + 1..];
    let paren_rest = after_alt.strip_prefix('(')?;
    let close_paren = paren_rest.find(')')?;
    // Require the closing paren to be the last character so trailing
    // garbage on the line doesn't get silently treated as an image.
    if close_paren != paren_rest.len() - 1 {
        return None;
    }
    let path = &paren_rest[..close_paren];
    Some((alt.to_owned(), path.to_owned()))
}

/// Splits `text` into inline runs, extracting `**bold**` spans.
///
/// An unmatched `**` (no closing marker) is treated as literal text rather
/// than starting a bold span.
///
/// # Arguments
///
/// * `text` - Plain paragraph text (already joined from source lines).
///
/// # Returns
///
/// * The parsed inline runs, in source order.
fn parse_inline(text: &str) -> Vec<MdInline> {
    let mut runs = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("**") {
        if start > 0 {
            runs.push(MdInline::Text(remaining[..start].to_owned()));
        }
        let after_marker = &remaining[start + 2..];
        match after_marker.find("**") {
            Some(end) => {
                runs.push(MdInline::Bold(after_marker[..end].to_owned()));
                remaining = &after_marker[end + 2..];
            }
            None => {
                // No closing marker: keep the rest of the text, including
                // the stray "**", as plain text.
                runs.push(MdInline::Text(remaining[start..].to_owned()));
                remaining = "";
                break;
            }
        }
    }
    if !remaining.is_empty() {
        runs.push(MdInline::Text(remaining.to_owned()));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_no_blocks() {
        assert_eq!(parse(""), vec![]);
        assert_eq!(parse("\n\n   \n"), vec![]);
    }

    #[test]
    fn single_paragraph() {
        let blocks = parse("Hello world.");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "Hello world.".to_owned()
            )])]
        );
    }

    #[test]
    fn multiple_paragraphs_separated_by_blank_lines() {
        let blocks = parse("First paragraph.\n\nSecond paragraph.");
        assert_eq!(
            blocks,
            vec![
                MdBlock::Paragraph(vec![MdInline::Text("First paragraph.".to_owned())]),
                MdBlock::Paragraph(vec![MdInline::Text("Second paragraph.".to_owned())]),
            ]
        );
    }

    #[test]
    fn multiline_paragraph_is_joined_with_spaces() {
        let blocks = parse("Line one\nLine two");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "Line one Line two".to_owned()
            )])]
        );
    }

    #[test]
    fn image_line_becomes_image_block() {
        let blocks = parse("![A cool statue](gfx/journal/statue.png)");
        assert_eq!(
            blocks,
            vec![MdBlock::Image {
                path: "gfx/journal/statue.png".to_owned(),
                alt: "A cool statue".to_owned(),
            }]
        );
    }

    #[test]
    fn bold_span_is_extracted() {
        let blocks = parse("This is **very** important.");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![
                MdInline::Text("This is ".to_owned()),
                MdInline::Bold("very".to_owned()),
                MdInline::Text(" important.".to_owned()),
            ])]
        );
    }

    #[test]
    fn mixed_content_paragraph_image_paragraph() {
        let blocks = parse("Intro text.\n\n![alt](img.png)\n\nOutro text.");
        assert_eq!(
            blocks,
            vec![
                MdBlock::Paragraph(vec![MdInline::Text("Intro text.".to_owned())]),
                MdBlock::Image {
                    path: "img.png".to_owned(),
                    alt: "alt".to_owned(),
                },
                MdBlock::Paragraph(vec![MdInline::Text("Outro text.".to_owned())]),
            ]
        );
    }

    #[test]
    fn malformed_image_syntax_falls_back_to_plain_text() {
        // Missing closing paren.
        let blocks = parse("![alt](img.png");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "![alt](img.png".to_owned()
            )])]
        );

        // Trailing garbage after the closing paren.
        let blocks = parse("![alt](img.png) extra text");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "![alt](img.png) extra text".to_owned()
            )])]
        );
    }

    #[test]
    fn unmatched_bold_marker_is_literal() {
        let blocks = parse("This has an **unmatched marker.");
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![
                MdInline::Text("This has an ".to_owned()),
                MdInline::Text("**unmatched marker.".to_owned()),
            ])]
        );
    }

    #[test]
    fn completion_checklist_roundtrip() {
        let src = "```completion_checklist:first_kill\n1: Weak Thief\n2: Thief\n```";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![MdBlock::CompletionChecklist {
                key: "first_kill".to_owned(),
                items: vec![(1, "Weak Thief".to_owned()), (2, "Thief".to_owned()),],
            }]
        );
    }

    #[test]
    fn completion_checklist_skips_malformed_lines() {
        let src = "```completion_checklist:quests\nnot_a_valid_line\n1: Valid Quest\n```";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![MdBlock::CompletionChecklist {
                key: "quests".to_owned(),
                items: vec![(1, "Valid Quest".to_owned())],
            }]
        );
    }

    #[test]
    fn completion_counter_roundtrip() {
        let src = "```completion_counter:pentagram_solves\nlabel: Times Solved\nmax: 10\n```";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![MdBlock::CompletionCounter {
                key: "pentagram_solves".to_owned(),
                max: Some(10),
                label: "Times Solved".to_owned(),
            }]
        );
    }

    #[test]
    fn completion_counter_without_max_is_none() {
        let src = "```completion_counter:pentagram_solves\nlabel: Times Solved\n```";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![MdBlock::CompletionCounter {
                key: "pentagram_solves".to_owned(),
                max: None,
                label: "Times Solved".to_owned(),
            }]
        );
    }

    #[test]
    fn unknown_fence_tag_falls_back_to_paragraph() {
        let src = "```rust\nfn main() {}\n```";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "```rust fn main() {} ```".to_owned()
            )])]
        );
    }

    #[test]
    fn unterminated_completion_fence_falls_back_to_paragraph() {
        let src = "```completion_checklist:first_kill\n1: Weak Thief";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![MdBlock::Paragraph(vec![MdInline::Text(
                "```completion_checklist:first_kill 1: Weak Thief".to_owned()
            )])]
        );
    }

    #[test]
    fn completion_block_surrounded_by_paragraphs() {
        let src =
            "Intro.\n\n```completion_counter:pentagram_solves\nlabel: Times Solved\n```\n\nOutro.";
        let blocks = parse(src);
        assert_eq!(
            blocks,
            vec![
                MdBlock::Paragraph(vec![MdInline::Text("Intro.".to_owned())]),
                MdBlock::CompletionCounter {
                    key: "pentagram_solves".to_owned(),
                    max: None,
                    label: "Times Solved".to_owned(),
                },
                MdBlock::Paragraph(vec![MdInline::Text("Outro.".to_owned())]),
            ]
        );
    }
}
