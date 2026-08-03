//! In-game Journal (Guidebook) panel.
//!
//! A three-column, multi-nested tab layout: categories (left) →
//! subcategories (middle, only shown when the selected category has any) →
//! scrollable markdown content (right). Purely client-local and static —
//! there is no network traffic or server-driven state. Content is loaded
//! on demand from `.md` files via [`crate::journal::content::load`].

use std::collections::HashMap;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;

use crate::font_cache;
use crate::journal::catalog::JOURNAL_CATALOG;
use crate::journal::content;
use crate::journal::markdown::{MdBlock, MdInline};
use crate::player_state::CompletionSnapshot;
use crate::ui::RenderContext;
use crate::ui::widget::{Bounds, EventResponse, HudPanel, UiEvent, Widget, WidgetAction};
use crate::ui::widgets::title_bar::{TITLE_BAR_H, TitleBar, clamp_to_viewport};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// Panel width.
pub const JOURNAL_PANEL_W: u32 = 520;
/// Panel height.
pub const JOURNAL_PANEL_H: u32 = 360;

/// Horizontal inset from panel edges.
const H_INSET: i32 = 8;
/// Width of each nav column (categories, subcategories).
const NAV_COL_W: i32 = 130;
/// Row height for nav entries.
const ROW_H: i32 = 18;
/// Y offset of the first nav row below the title bar.
const Y_FIRST_ROW: i32 = TITLE_BAR_H + 8;
/// Padding inside the content column.
const CONTENT_PAD: i32 = 8;
/// Vertical gap between content blocks.
const PARAGRAPH_GAP: i32 = 6;
/// Maximum rendered height of an image block, in pixels.
const IMAGE_MAX_H: u32 = 120;
/// Pixels scrolled per wheel notch.
const SCROLL_STEP_PX: i32 = 24;

const NAV_BG: Color = Color::RGBA(4, 4, 16, 160);
const NAV_SELECTED_BG: Color = Color::RGBA(60, 60, 110, 200);
const NAV_HOVER_BG: Color = Color::RGBA(40, 40, 80, 160);
const SEPARATOR_COLOR: Color = Color::RGBA(120, 120, 140, 160);
/// Tint applied to `**bold**` runs (bitmap font has no true bold glyphs).
const BOLD_TINT: Color = Color::RGBA(255, 210, 110, 255);
/// Bitmap font index (yellow, sprite 701) used throughout the panel.
const UI_FONT: usize = 0;

/// Per-frame data pushed into [`JournalPanel::update_data`].
pub struct JournalPanelData {
    /// Latest server-driven Journal completion snapshot.
    pub completion: CompletionSnapshot,
}

/// Returns whether `key` is a checklist key this panel knows how to resolve.
fn checklist_key_is_known(key: &str) -> bool {
    matches!(
        key,
        "first_kill" | "explorer_points" | "quests" | "labyrinth_overview"
    )
}

/// Resolves whether checklist item `index` (for the given `key`) is checked,
/// given the current completion snapshot. Returns `false` for unknown keys
/// (callers should gate on [`checklist_key_is_known`] first to show a
/// dedicated "unknown key" message instead).
fn resolve_checklist_bit(completion: &CompletionSnapshot, key: &str, index: u16) -> bool {
    match key {
        "first_kill" => bit_at(&completion.first_kill_bits, index),
        "explorer_points" => bit_at(&completion.explorer_point_bits, index),
        "quests" => index < 32 && (completion.quest_completion_bits >> index) & 1 != 0,
        "labyrinth_overview" => u16::from(completion.labyrinth_progress) >= index,
        _ => false,
    }
}

/// Reads bit `index` out of a 4x32-bit flag array (128 bits total), as used
/// by `first_kill_bits` / `explorer_point_bits`.
fn bit_at(bits: &[u32; 4], index: u16) -> bool {
    let slot = (index / 32) as usize;
    let bit = index % 32;
    slot < bits.len() && (bits[slot] >> bit) & 1 != 0
}

/// Returns whether `key` is a counter key this panel knows how to resolve.
fn counter_key_is_known(key: &str) -> bool {
    matches!(key, "pentagram_solves")
}

/// Computes the per-column display width (in characters) for a table, as
/// the max of each column's header and cell content lengths (missing cells
/// count as width 0). Uses the bitmap font's monospace character count
/// rather than pixel width since rows are laid out with padded spaces.
///
/// # Arguments
///
/// * `headers` - Column header labels (may be empty for header-less tables).
/// * `rows` - Data rows, each a list of cell values in column order.
///
/// # Returns
///
/// * One width per column, covering the widest header or row cell.
fn table_column_widths(headers: &[String], rows: &[&Vec<String>]) -> Vec<usize> {
    let ncols = headers
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0))
        .max(1);
    let mut widths = vec![0usize; ncols];
    for (i, h) in headers.iter().enumerate() {
        widths[i] = widths[i].max(h.chars().count());
    }
    for row in rows {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    widths
}

/// Formats one table row, padding each cell to its column width and
/// separating columns with a fixed gap, for monospace-aligned rendering.
///
/// # Arguments
///
/// * `cells` - The row's cell values, in column order.
/// * `widths` - Per-column widths, as computed by [`table_column_widths`].
///
/// # Returns
///
/// * The formatted, space-padded row text.
fn format_table_row(cells: &[String], widths: &[usize]) -> String {
    let mut out = String::new();
    for (i, width) in widths.iter().enumerate() {
        let cell = cells.get(i).map(|c| c.as_str()).unwrap_or("");
        out.push_str(cell);
        let pad = width.saturating_sub(cell.chars().count());
        out.push_str(&" ".repeat(pad));
        out.push_str("  ");
    }
    out
}

/// Resolves the numeric value for counter `key`, given the current
/// completion snapshot. Returns `0` for unknown keys (callers should gate on
/// [`counter_key_is_known`] first).
fn resolve_counter_value(completion: &CompletionSnapshot, key: &str) -> u32 {
    match key {
        "pentagram_solves" => completion.pentagram_solves,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// JournalPanel
// ---------------------------------------------------------------------------

/// The in-game Journal / Guidebook panel.
///
/// Note on inline rendering: since the bitmap font has no true bold glyph
/// variant, each parsed [`MdInline`] run within a paragraph is rendered on
/// its own wrapped line block (tinted for `Bold`), rather than flowing
/// inline with surrounding text. This keeps the hand-rolled renderer simple;
/// content authors should treat each run as its own line/paragraph.
pub struct JournalPanel {
    bounds: Bounds,
    bg_color: Color,
    border_color: Color,
    visible: bool,
    pending_actions: Vec<WidgetAction>,

    /// Draggable title bar.
    title_bar: TitleBar,

    /// Index into [`JOURNAL_CATALOG`] of the currently selected category.
    selected_category: usize,
    /// Index into the selected category's subcategories, if any is chosen.
    selected_subcategory: Option<usize>,
    /// Parsed content blocks for the current selection.
    content_blocks: Vec<MdBlock>,
    /// Current vertical scroll offset in pixels (0 = top).
    scroll_offset: i32,

    hovered_category: Option<usize>,
    hovered_subcategory: Option<usize>,

    /// Lazily loaded image textures, keyed by content-relative path.
    /// `None` records a load failure so it isn't retried every frame.
    image_textures: HashMap<String, Option<usize>>,

    /// Latest server-driven Journal completion snapshot, used to fill in
    /// checkbox/counter state for [`MdBlock::CompletionChecklist`] and
    /// [`MdBlock::CompletionCounter`] blocks.
    completion: CompletionSnapshot,
}

impl JournalPanel {
    /// Creates a new Journal panel with the first catalog category selected.
    ///
    /// # Arguments
    ///
    /// * `bounds` - Position and size of the panel.
    /// * `bg_color` - Semi-transparent background color.
    ///
    /// # Returns
    ///
    /// A new `JournalPanel`, initially hidden.
    pub fn new(bounds: Bounds, bg_color: Color) -> Self {
        let mut panel = Self {
            bounds,
            bg_color,
            border_color: Color::RGBA(120, 120, 140, 200),
            visible: false,
            pending_actions: Vec::new(),
            title_bar: TitleBar::new("Journal", bounds.x, bounds.y, bounds.width),
            selected_category: 0,
            selected_subcategory: None,
            content_blocks: Vec::new(),
            scroll_offset: 0,
            hovered_category: None,
            hovered_subcategory: None,
            image_textures: HashMap::new(),
            completion: CompletionSnapshot::default(),
        };
        panel.select_category(0);
        panel
    }

    /// Per-frame data pushed into the Journal panel from `AppState`.
    ///
    /// Toggles the panel's visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Updates the panel's cached Journal completion snapshot.
    ///
    /// Call every frame (like other HUD panels' `update_data`), typically
    /// sourced from `PlayerState::completion()`.
    ///
    /// # Arguments
    ///
    /// * `data` - The latest completion snapshot to render against.
    pub fn update_data(&mut self, data: JournalPanelData) {
        self.completion = data.completion;
    }

    /// Returns whether the panel is currently visible.
    ///
    /// # Returns
    ///
    /// * `true` when the panel is visible, otherwise `false`.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Selects category `idx`, resetting the subcategory selection to the
    /// first subcategory (if any) and reloading content. No-op when `idx` is
    /// out of range.
    ///
    /// # Arguments
    ///
    /// * `idx` - Index into [`JOURNAL_CATALOG`].
    fn select_category(&mut self, idx: usize) {
        let Some(cat) = JOURNAL_CATALOG.get(idx) else {
            return;
        };
        self.selected_category = idx;
        self.selected_subcategory = if cat.subcategories.is_empty() {
            None
        } else {
            Some(0)
        };
        self.reload_content();
    }

    /// Selects subcategory `idx` within the currently selected category and
    /// reloads content. No-op when `idx` is out of range.
    ///
    /// # Arguments
    ///
    /// * `idx` - Index into the selected category's subcategories.
    fn select_subcategory(&mut self, idx: usize) {
        if let Some(cat) = JOURNAL_CATALOG.get(self.selected_category)
            && idx < cat.subcategories.len()
        {
            self.selected_subcategory = Some(idx);
            self.reload_content();
        }
    }

    /// Returns the content file for the current category/subcategory
    /// selection, if any.
    fn current_content_file(&self) -> Option<&'static str> {
        let cat = JOURNAL_CATALOG.get(self.selected_category)?;
        match self.selected_subcategory {
            Some(sub_idx) => cat.subcategories.get(sub_idx).map(|s| s.content_file),
            None => cat.content_file,
        }
    }

    /// Reloads `content_blocks` from the current selection's content file
    /// and resets scroll to the top.
    fn reload_content(&mut self) {
        self.scroll_offset = 0;
        self.content_blocks = match self.current_content_file() {
            Some(file) => content::load(file),
            None => Vec::new(),
        };
    }

    /// Returns `true` when the currently selected category has any
    /// subcategories (i.e. the middle nav column should be shown).
    fn has_subcategories(&self) -> bool {
        JOURNAL_CATALOG
            .get(self.selected_category)
            .is_some_and(|c| !c.subcategories.is_empty())
    }

    // -- Layout ---------------------------------------------------------

    /// Bounds of the left (categories) nav column.
    fn nav_categories_bounds(&self) -> Bounds {
        Bounds::new(
            self.bounds.x + H_INSET,
            self.bounds.y + Y_FIRST_ROW,
            NAV_COL_W as u32,
            self.bounds
                .height
                .saturating_sub((Y_FIRST_ROW + H_INSET) as u32),
        )
    }

    /// Bounds of the middle (subcategories) nav column, when shown.
    fn nav_subcategories_bounds(&self) -> Option<Bounds> {
        if !self.has_subcategories() {
            return None;
        }
        let cats = self.nav_categories_bounds();
        Some(Bounds::new(
            cats.x + NAV_COL_W,
            cats.y,
            NAV_COL_W as u32,
            cats.height,
        ))
    }

    /// Bounds of the right (content) column.
    fn content_bounds(&self) -> Bounds {
        let cats = self.nav_categories_bounds();
        let left = match self.nav_subcategories_bounds() {
            Some(subs) => subs.x + subs.width as i32,
            None => cats.x + cats.width as i32,
        };
        let right = self.bounds.x + self.bounds.width as i32 - H_INSET;
        Bounds::new(
            left + H_INSET,
            cats.y,
            (right - left - H_INSET).max(0) as u32,
            cats.height,
        )
    }

    /// Bounds of nav row `row_idx` within column `col`.
    fn row_bounds(col: &Bounds, row_idx: usize) -> Bounds {
        Bounds::new(
            col.x,
            col.y + row_idx as i32 * ROW_H,
            col.width,
            ROW_H as u32,
        )
    }

    // -- Rendering --------------------------------------------------------

    /// Renders the left categories nav column.
    fn render_categories(
        &self,
        ctx: &mut RenderContext<'_, '_>,
        cats: &Bounds,
    ) -> Result<(), String> {
        for (i, cat) in JOURNAL_CATALOG.iter().enumerate() {
            let row = Self::row_bounds(cats, i);
            let bg = if self.selected_category == i {
                NAV_SELECTED_BG
            } else if self.hovered_category == Some(i) {
                NAV_HOVER_BG
            } else {
                NAV_BG
            };
            ctx.canvas.set_blend_mode(BlendMode::Blend);
            ctx.canvas.set_draw_color(bg);
            ctx.canvas
                .fill_rect(sdl2::rect::Rect::new(row.x, row.y, row.width, row.height))?;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                UI_FONT,
                cat.label,
                row.x + 4,
                row.y + 4,
                font_cache::TextStyle::default(),
            )?;
        }
        Ok(())
    }

    /// Renders the middle subcategories nav column, when present.
    fn render_subcategories(
        &self,
        ctx: &mut RenderContext<'_, '_>,
        subs: &Bounds,
    ) -> Result<(), String> {
        let Some(cat) = JOURNAL_CATALOG.get(self.selected_category) else {
            return Ok(());
        };
        for (i, sub) in cat.subcategories.iter().enumerate() {
            let row = Self::row_bounds(subs, i);
            let bg = if self.selected_subcategory == Some(i) {
                NAV_SELECTED_BG
            } else if self.hovered_subcategory == Some(i) {
                NAV_HOVER_BG
            } else {
                NAV_BG
            };
            ctx.canvas.set_blend_mode(BlendMode::Blend);
            ctx.canvas.set_draw_color(bg);
            ctx.canvas
                .fill_rect(sdl2::rect::Rect::new(row.x, row.y, row.width, row.height))?;
            font_cache::draw_text(
                ctx.canvas,
                ctx.gfx,
                UI_FONT,
                sub.label,
                row.x + 4,
                row.y + 4,
                font_cache::TextStyle::default(),
            )?;
        }
        Ok(())
    }

    /// Ensures the image at `relative_path` (relative to
    /// `client/assets/journal/`) is loaded, caching the result (including
    /// failures) so it is not retried every frame.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Render context providing the graphics cache.
    /// * `relative_path` - Path to the image, relative to the journal
    ///   assets directory.
    ///
    /// # Returns
    ///
    /// * `Some(texture_id)` when the image is loaded (now or previously);
    ///   `None` when loading failed.
    fn ensure_image_texture(
        &mut self,
        ctx: &mut RenderContext<'_, '_>,
        relative_path: &str,
    ) -> Option<usize> {
        if let Some(cached) = self.image_textures.get(relative_path) {
            return *cached;
        }
        let full_path = crate::filepaths::get_asset_directory()
            .join("journal")
            .join(relative_path);
        let id = match ctx.gfx.load_texture_from_path(&full_path) {
            Ok(id) => Some(id),
            Err(err) => {
                log::warn!(
                    "Failed to load journal image {}: {}",
                    full_path.display(),
                    err
                );
                None
            }
        };
        self.image_textures.insert(relative_path.to_owned(), id);
        id
    }

    /// Computes the on-screen `(width, height)` for an image with natural
    /// size `(tw, th)`, scaled down to fit within `max_width` and
    /// [`IMAGE_MAX_H`] while preserving aspect ratio. Never scales up.
    fn scaled_image_size(tw: u32, th: u32, max_width: u32) -> (u32, u32) {
        if tw == 0 || th == 0 {
            return (0, 0);
        }
        let width_scale = if tw > max_width {
            max_width as f32 / tw as f32
        } else {
            1.0
        };
        let scaled_h = th as f32 * width_scale;
        let height_scale = if scaled_h > IMAGE_MAX_H as f32 {
            IMAGE_MAX_H as f32 / scaled_h
        } else {
            1.0
        };
        let scale = width_scale * height_scale;
        (
            (tw as f32 * scale).round() as u32,
            (th as f32 * scale).round() as u32,
        )
    }

    /// Computes the on-screen `(width, height)` for an image with natural
    /// size `(tw, th)`, honoring explicit `width`/`height` overrides parsed
    /// from the source (a missing dimension is derived from the natural
    /// aspect ratio). Falls back to [`Self::scaled_image_size`]'s auto-fit
    /// behavior when neither override is present.
    fn resolved_image_size(
        tw: u32,
        th: u32,
        max_width: u32,
        width: Option<u32>,
        height: Option<u32>,
    ) -> (u32, u32) {
        match (width, height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let h = if tw > 0 {
                    (th as f32 * (w as f32 / tw as f32)).round() as u32
                } else {
                    0
                };
                (w, h)
            }
            (None, Some(h)) => {
                let w = if th > 0 {
                    (tw as f32 * (h as f32 / th as f32)).round() as u32
                } else {
                    0
                };
                (w, h)
            }
            (None, None) => Self::scaled_image_size(tw, th, max_width),
        }
    }

    /// Renders the scrollable content column, computing (and clamping)
    /// scroll bounds from the current content's total rendered height.
    fn render_content(
        &mut self,
        ctx: &mut RenderContext<'_, '_>,
        content: &Bounds,
    ) -> Result<(), String> {
        let text_width = content.width.saturating_sub((2 * CONTENT_PAD) as u32);
        let glyph_h = font_cache::BITMAP_GLYPH_H as i32;

        // Pass 1: measure each block's rendered height (loading any images).
        let mut heights: Vec<i32> = Vec::with_capacity(self.content_blocks.len());
        for i in 0..self.content_blocks.len() {
            let height = match &self.content_blocks[i] {
                MdBlock::Paragraph(runs) => {
                    let mut h = 0;
                    for run in runs {
                        let text = match run {
                            MdInline::Text(t) | MdInline::Bold(t) => t,
                        };
                        let lines = font_cache::wrap_lines_bitmap(text, text_width);
                        h += lines.len().max(1) as i32 * glyph_h;
                    }
                    h
                }
                MdBlock::Image {
                    path,
                    width,
                    height,
                    ..
                } => {
                    let path = path.clone();
                    let (width, height) = (*width, *height);
                    match self.ensure_image_texture(ctx, &path) {
                        Some(id) => {
                            let (tw, th) = ctx.gfx.query_texture_size(id);
                            let (_, dh) =
                                Self::resolved_image_size(tw, th, text_width, width, height);
                            dh.max(1) as i32
                        }
                        None => glyph_h,
                    }
                }
                MdBlock::Table { rows, .. } => (1 + rows.len()).max(1) as i32 * glyph_h,
                MdBlock::CompletionChecklist { key, items } => {
                    if !checklist_key_is_known(key) {
                        glyph_h
                    } else {
                        let mut h = 0;
                        for (_, label) in items {
                            let text = format!("[ ] {label}");
                            let lines = font_cache::wrap_lines_bitmap(&text, text_width);
                            h += lines.len().max(1) as i32 * glyph_h;
                        }
                        h.max(glyph_h)
                    }
                }
                MdBlock::CompletionTable { key, headers, rows } => {
                    if !checklist_key_is_known(key) {
                        glyph_h
                    } else {
                        (rows.len() + usize::from(!headers.is_empty())).max(1) as i32 * glyph_h
                    }
                }
                MdBlock::CompletionCounter { .. } => glyph_h,
            };
            heights.push(height);
        }
        let total_height: i32 =
            heights.iter().sum::<i32>() + PARAGRAPH_GAP * heights.len().saturating_sub(1) as i32;
        let max_scroll = (total_height - content.height as i32).max(0);
        self.scroll_offset = self.scroll_offset.clamp(0, max_scroll);

        // Pass 2: draw, clipped to the content bounds.
        let clip_rect = sdl2::rect::Rect::new(content.x, content.y, content.width, content.height);
        let previous_clip = ctx.canvas.clip_rect();
        ctx.canvas.set_clip_rect(clip_rect);

        let mut y = content.y - self.scroll_offset;
        for (block, height) in self.content_blocks.iter().zip(heights.iter()) {
            let block_bottom = y + height;
            if block_bottom >= content.y && y <= content.y + content.height as i32 {
                match block {
                    MdBlock::Paragraph(runs) => {
                        let mut ry = y;
                        for run in runs {
                            let (text, style) = match run {
                                MdInline::Text(t) => (t, font_cache::TextStyle::default()),
                                MdInline::Bold(t) => {
                                    (t, font_cache::TextStyle::default().with_tint(BOLD_TINT))
                                }
                            };
                            let lines_drawn = font_cache::draw_wrapped_text(
                                ctx.canvas,
                                ctx.gfx,
                                UI_FONT,
                                text,
                                content.x + CONTENT_PAD,
                                ry,
                                text_width,
                                style,
                            )?;
                            ry += lines_drawn.max(1) as i32 * glyph_h;
                        }
                    }
                    MdBlock::Image {
                        path,
                        alt,
                        width,
                        height,
                    } => {
                        if let Some(Some(id)) = self.image_textures.get(path).copied() {
                            let (tw, th) = ctx.gfx.query_texture_size(id);
                            let (dw, dh) =
                                Self::resolved_image_size(tw, th, text_width, *width, *height);
                            let dst = sdl2::rect::Rect::new(content.x + CONTENT_PAD, y, dw, dh);
                            let tex = ctx.gfx.get_texture(id);
                            ctx.canvas.copy(tex, None, Some(dst))?;
                        } else {
                            font_cache::draw_text(
                                ctx.canvas,
                                ctx.gfx,
                                UI_FONT,
                                &format!("[image: {alt}]"),
                                content.x + CONTENT_PAD,
                                y,
                                font_cache::TextStyle::default(),
                            )?;
                        }
                    }
                    MdBlock::Table { headers, rows } => {
                        let row_refs: Vec<&Vec<String>> = rows.iter().collect();
                        let widths = table_column_widths(headers, &row_refs);
                        let mut ry = y;
                        if !headers.is_empty() {
                            font_cache::draw_text(
                                ctx.canvas,
                                ctx.gfx,
                                UI_FONT,
                                &format_table_row(headers, &widths),
                                content.x + CONTENT_PAD,
                                ry,
                                font_cache::TextStyle::default().with_tint(BOLD_TINT),
                            )?;
                            ry += glyph_h;
                        }
                        for row in rows {
                            font_cache::draw_text(
                                ctx.canvas,
                                ctx.gfx,
                                UI_FONT,
                                &format_table_row(row, &widths),
                                content.x + CONTENT_PAD,
                                ry,
                                font_cache::TextStyle::default(),
                            )?;
                            ry += glyph_h;
                        }
                    }
                    MdBlock::CompletionTable { key, headers, rows } => {
                        if !checklist_key_is_known(key) {
                            font_cache::draw_text(
                                ctx.canvas,
                                ctx.gfx,
                                UI_FONT,
                                &format!("Unknown completion key: {key}"),
                                content.x + CONTENT_PAD,
                                y,
                                font_cache::TextStyle::default(),
                            )?;
                        } else {
                            let row_cols: Vec<&Vec<String>> =
                                rows.iter().map(|(_, cols)| cols).collect();
                            let widths = table_column_widths(headers, &row_cols);
                            let mut ry = y;
                            if !headers.is_empty() {
                                font_cache::draw_text(
                                    ctx.canvas,
                                    ctx.gfx,
                                    UI_FONT,
                                    &format!("    {}", format_table_row(headers, &widths)),
                                    content.x + CONTENT_PAD,
                                    ry,
                                    font_cache::TextStyle::default().with_tint(BOLD_TINT),
                                )?;
                                ry += glyph_h;
                            }
                            for (index, cols) in rows {
                                let checked = resolve_checklist_bit(&self.completion, key, *index);
                                let prefix = if checked { "[x] " } else { "[ ] " };
                                let text = format!("{prefix}{}", format_table_row(cols, &widths));
                                font_cache::draw_text(
                                    ctx.canvas,
                                    ctx.gfx,
                                    UI_FONT,
                                    &text,
                                    content.x + CONTENT_PAD,
                                    ry,
                                    font_cache::TextStyle::default(),
                                )?;
                                ry += glyph_h;
                            }
                        }
                    }
                    MdBlock::CompletionChecklist { key, items } => {
                        if !checklist_key_is_known(key) {
                            font_cache::draw_text(
                                ctx.canvas,
                                ctx.gfx,
                                UI_FONT,
                                &format!("Unknown completion key: {key}"),
                                content.x + CONTENT_PAD,
                                y,
                                font_cache::TextStyle::default(),
                            )?;
                        } else {
                            let mut ry = y;
                            for (index, label) in items {
                                let checked = resolve_checklist_bit(&self.completion, key, *index);
                                let prefix = if checked { "[x] " } else { "[ ] " };
                                let text = format!("{prefix}{label}");
                                let lines_drawn = font_cache::draw_wrapped_text(
                                    ctx.canvas,
                                    ctx.gfx,
                                    UI_FONT,
                                    &text,
                                    content.x + CONTENT_PAD,
                                    ry,
                                    text_width,
                                    font_cache::TextStyle::default(),
                                )?;
                                ry += lines_drawn.max(1) as i32 * glyph_h;
                            }
                        }
                    }
                    MdBlock::CompletionCounter { key, max, label } => {
                        let text = if counter_key_is_known(key) {
                            let value = resolve_counter_value(&self.completion, key);
                            match max {
                                Some(m) => format!("{label}: {value}/{m}"),
                                None => format!("{label}: {value}"),
                            }
                        } else {
                            format!("Unknown completion key: {key}")
                        };
                        font_cache::draw_text(
                            ctx.canvas,
                            ctx.gfx,
                            UI_FONT,
                            &text,
                            content.x + CONTENT_PAD,
                            y,
                            font_cache::TextStyle::default(),
                        )?;
                    }
                }
            }
            y = block_bottom + PARAGRAPH_GAP;
        }

        ctx.canvas.set_clip_rect(previous_clip);
        Ok(())
    }
}

impl Widget for JournalPanel {
    fn bounds(&self) -> &Bounds {
        &self.bounds
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.bounds.x = x;
        self.bounds.y = y;
        self.title_bar.set_bar_position(x, y);
    }

    fn handle_event(&mut self, event: &UiEvent) -> EventResponse {
        if !self.visible {
            return EventResponse::Ignored;
        }

        let (tb_resp, drag_pos) = self.title_bar.handle_event(event);
        if let Some((nx, ny)) = drag_pos {
            let (cx, cy) = clamp_to_viewport(nx, ny, self.bounds.width, self.bounds.height);
            self.set_position(cx, cy);
            return EventResponse::Consumed;
        }
        if self.title_bar.was_close_requested() {
            self.visible = false;
            self.pending_actions
                .push(WidgetAction::TogglePanel(HudPanel::Journal));
            return EventResponse::Consumed;
        }
        if tb_resp == EventResponse::Consumed {
            return EventResponse::Consumed;
        }

        match event {
            UiEvent::MouseMove { x, y } => {
                let cats = self.nav_categories_bounds();
                self.hovered_category = (0..JOURNAL_CATALOG.len())
                    .find(|&i| Self::row_bounds(&cats, i).contains_point(*x, *y));
                self.hovered_subcategory = self.nav_subcategories_bounds().and_then(|subs| {
                    let count = JOURNAL_CATALOG[self.selected_category].subcategories.len();
                    (0..count).find(|&i| Self::row_bounds(&subs, i).contains_point(*x, *y))
                });
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            UiEvent::MouseClick { x, y, .. } => {
                let cats = self.nav_categories_bounds();
                if let Some(idx) = (0..JOURNAL_CATALOG.len())
                    .find(|&i| Self::row_bounds(&cats, i).contains_point(*x, *y))
                {
                    self.select_category(idx);
                    return EventResponse::Consumed;
                }
                if let Some(subs) = self.nav_subcategories_bounds() {
                    let count = JOURNAL_CATALOG[self.selected_category].subcategories.len();
                    if let Some(idx) =
                        (0..count).find(|&i| Self::row_bounds(&subs, i).contains_point(*x, *y))
                    {
                        self.select_subcategory(idx);
                        return EventResponse::Consumed;
                    }
                }
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            UiEvent::MouseWheel { x, y, delta } => {
                let content = self.content_bounds();
                if content.contains_point(*x, *y) {
                    if *delta > 0 {
                        self.scroll_offset = (self.scroll_offset - SCROLL_STEP_PX).max(0);
                    } else if *delta < 0 {
                        self.scroll_offset += SCROLL_STEP_PX;
                    }
                    EventResponse::Consumed
                } else if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            UiEvent::MouseDown { x, y, .. } => {
                if self.bounds.contains_point(*x, *y) {
                    EventResponse::Consumed
                } else {
                    EventResponse::Ignored
                }
            }
            _ => EventResponse::Ignored,
        }
    }

    fn render(&mut self, ctx: &mut RenderContext<'_, '_>) -> Result<(), String> {
        if !self.visible {
            return Ok(());
        }

        let rect = sdl2::rect::Rect::new(
            self.bounds.x,
            self.bounds.y,
            self.bounds.width,
            self.bounds.height,
        );
        ctx.canvas.set_blend_mode(BlendMode::Blend);
        ctx.canvas.set_draw_color(self.bg_color);
        ctx.canvas.fill_rect(rect)?;
        ctx.canvas.set_draw_color(self.border_color);
        ctx.canvas.draw_rect(rect)?;

        self.title_bar.render(ctx)?;

        let cats = self.nav_categories_bounds();
        self.render_categories(ctx, &cats)?;

        ctx.canvas.set_draw_color(SEPARATOR_COLOR);
        ctx.canvas.draw_line(
            sdl2::rect::Point::new(cats.x + cats.width as i32, cats.y),
            sdl2::rect::Point::new(cats.x + cats.width as i32, cats.y + cats.height as i32),
        )?;

        if let Some(subs) = self.nav_subcategories_bounds() {
            self.render_subcategories(ctx, &subs)?;
            ctx.canvas.set_draw_color(SEPARATOR_COLOR);
            ctx.canvas.draw_line(
                sdl2::rect::Point::new(subs.x + subs.width as i32, subs.y),
                sdl2::rect::Point::new(subs.x + subs.width as i32, subs.y + subs.height as i32),
            )?;
        }

        let content = self.content_bounds();
        self.render_content(ctx, &content)?;

        Ok(())
    }

    fn take_actions(&mut self) -> Vec<WidgetAction> {
        std::mem::take(&mut self.pending_actions)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::widget::{KeyModifiers, MouseButton};

    fn make_panel() -> JournalPanel {
        JournalPanel::new(
            Bounds::new(0, 0, JOURNAL_PANEL_W, JOURNAL_PANEL_H),
            Color::RGBA(0, 0, 0, 180),
        )
    }

    #[test]
    fn starts_hidden() {
        let panel = make_panel();
        assert!(!panel.is_visible());
    }

    #[test]
    fn toggle_visibility() {
        let mut panel = make_panel();
        panel.toggle();
        assert!(panel.is_visible());
        panel.toggle();
        assert!(!panel.is_visible());
    }

    #[test]
    fn initial_selection_is_first_category_with_content() {
        let panel = make_panel();
        assert_eq!(panel.selected_category, 0);
        assert_eq!(panel.selected_subcategory, None);
        assert!(!panel.content_blocks.is_empty());
    }

    #[test]
    fn hidden_panel_ignores_events() {
        let mut panel = make_panel();
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: 5,
            y: 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });
        assert_eq!(resp, EventResponse::Ignored);
    }

    #[test]
    fn clicking_category_with_subcategories_selects_first_subcategory() {
        let mut panel = make_panel();
        panel.toggle();

        // "Labyrinth" is index 3 in JOURNAL_CATALOG and has 3 subcategories.
        let labyrinth_idx = JOURNAL_CATALOG
            .iter()
            .position(|c| c.label == "Labyrinth")
            .expect("Labyrinth category should exist");

        let cats = panel.nav_categories_bounds();
        let row = JournalPanel::row_bounds(&cats, labyrinth_idx);
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: row.x + 5,
            y: row.y + 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });

        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(panel.selected_category, labyrinth_idx);
        assert_eq!(panel.selected_subcategory, Some(0));
        assert!(panel.has_subcategories());
    }

    #[test]
    fn clicking_a_subcategory_row_selects_it() {
        let mut panel = make_panel();
        panel.toggle();

        let labyrinth_idx = JOURNAL_CATALOG
            .iter()
            .position(|c| c.label == "Labyrinth")
            .expect("Labyrinth category should exist");
        panel.select_category(labyrinth_idx);

        let subs = panel
            .nav_subcategories_bounds()
            .expect("Labyrinth should show a subcategory column");
        let row = JournalPanel::row_bounds(&subs, 2);
        let resp = panel.handle_event(&UiEvent::MouseClick {
            x: row.x + 5,
            y: row.y + 5,
            button: MouseButton::Left,
            modifiers: KeyModifiers::default(),
        });

        assert_eq!(resp, EventResponse::Consumed);
        assert_eq!(panel.selected_subcategory, Some(2));
    }

    #[test]
    fn selecting_category_without_subcategories_clears_subcategory_column() {
        let mut panel = make_panel();
        panel.toggle();
        panel.select_category(0); // "Quest Log" has no subcategories.
        assert_eq!(panel.selected_subcategory, None);
        assert!(!panel.has_subcategories());
        assert!(panel.nav_subcategories_bounds().is_none());
    }

    #[test]
    fn wheel_scroll_updates_offset_in_content_column() {
        let mut panel = make_panel();
        panel.toggle();
        let content = panel.content_bounds();
        let (cx, cy) = (content.x + 5, content.y + 5);

        // Wheel down (negative delta) scrolls further into the content.
        panel.handle_event(&UiEvent::MouseWheel {
            x: cx,
            y: cy,
            delta: -3,
        });
        assert_eq!(panel.scroll_offset, SCROLL_STEP_PX);

        // Wheel up (positive delta) scrolls back toward the top, clamped at 0.
        panel.handle_event(&UiEvent::MouseWheel {
            x: cx,
            y: cy,
            delta: 3,
        });
        assert_eq!(panel.scroll_offset, 0);

        panel.handle_event(&UiEvent::MouseWheel {
            x: cx,
            y: cy,
            delta: 3,
        });
        assert_eq!(panel.scroll_offset, 0);
    }

    #[test]
    fn wheel_scroll_outside_content_column_is_ignored_by_scroll() {
        let mut panel = make_panel();
        panel.toggle();
        let cats = panel.nav_categories_bounds();
        panel.handle_event(&UiEvent::MouseWheel {
            x: cats.x + 5,
            y: cats.y + 5,
            delta: -3,
        });
        assert_eq!(panel.scroll_offset, 0);
    }
}
