//! Minimal markdown-subset parser for Journal content.
//!
//! Deliberately hand-rolled and intentionally small: paragraphs separated by
//! blank lines, `**bold**` emphasis spans, and standalone `![alt](path)`
//! image lines. No headings, links, lists, or nested markup are supported —
//! anything else is treated as plain text.

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
}

/// Parses `source` into a sequence of [`MdBlock`]s.
///
/// Blank lines (lines that are empty after trimming) separate blocks. A
/// block consisting of exactly one line matching `![alt](path)` becomes an
/// [`MdBlock::Image`]; any other block becomes an [`MdBlock::Paragraph`]
/// with its lines joined by spaces and `**bold**` spans extracted.
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

    for line in source.lines() {
        if line.trim().is_empty() {
            flush_paragraph(&mut current_lines, &mut blocks);
        } else {
            current_lines.push(line);
        }
    }
    flush_paragraph(&mut current_lines, &mut blocks);

    blocks
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
}
