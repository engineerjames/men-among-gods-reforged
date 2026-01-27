// Gameplay UI/world layout constants.
//
// These are ported from the original client and kept centralized so the gameplay
// state logic can stay readable.

// In the original client, xoff starts with `-176` (to account for UI layout).
// Keeping this makes it easier to compare screenshots while we port rendering.
pub(in crate::states::gameplay) const MAP_X_SHIFT: f32 = -176.0;

// World draw order: match engine.c's two-pass painter order.
// Pass 1: all backgrounds in (y desc, x asc) order.
// Pass 2: objects -> character shadow -> characters in the same order.
// We encode this by giving each pass its own z-range and then adding draw_order.
// Within pass-2, objects/shadows/characters must share the same z-range (with small per-layer
// biases) so that later tiles can occlude earlier tiles (e.g. pillars/walls in front of players).
pub(in crate::states::gameplay) const Z_WORLD_STEP: f32 = 0.01;
pub(in crate::states::gameplay) const Z_BG_BASE: f32 = 0.0;
pub(in crate::states::gameplay) const Z_OBJ_BASE: f32 = 100.0;
pub(in crate::states::gameplay) const Z_FX_BASE: f32 = 100.25;
// Must stay within the Camera2d default orthographic near/far (default_2d far is 1000).
pub(in crate::states::gameplay) const Z_UI: f32 = 900.0;
pub(in crate::states::gameplay) const Z_UI_PORTRAIT: f32 = 910.0;
pub(in crate::states::gameplay) const Z_UI_RANK: f32 = 911.0;
pub(in crate::states::gameplay) const Z_UI_INV: f32 = 919.0;
pub(in crate::states::gameplay) const Z_UI_EQUIP: f32 = 920.0;
pub(in crate::states::gameplay) const Z_UI_SPELLS: f32 = 921.0;
pub(in crate::states::gameplay) const Z_UI_SHOP_PANEL: f32 = 930.0;
pub(in crate::states::gameplay) const Z_UI_SHOP_ITEMS: f32 = 931.0;
pub(in crate::states::gameplay) const Z_UI_TEXT: f32 = 940.0;
pub(in crate::states::gameplay) const Z_UI_MINIMAP: f32 = 905.0;
pub(in crate::states::gameplay) const Z_UI_SCROLL: f32 = 939.0;
pub(in crate::states::gameplay) const Z_UI_CURSOR: f32 = 950.0;

// Matches dd_show_map() placement in dd.c: top-left at (3,471), size 128x128.
pub(in crate::states::gameplay) const MINIMAP_X: f32 = 3.0;
pub(in crate::states::gameplay) const MINIMAP_Y: f32 = 471.0;
pub(in crate::states::gameplay) const MINIMAP_SIZE: u32 = 128;

pub(in crate::states::gameplay) const LOG_X: f32 = 500.0;
pub(in crate::states::gameplay) const LOG_Y: f32 = 4.0;
pub(in crate::states::gameplay) const LOG_LINE_H: f32 = 10.0;
pub(in crate::states::gameplay) const LOG_LINES: usize = 22;
pub(in crate::states::gameplay) const INPUT_X: f32 = 500.0;
pub(in crate::states::gameplay) const INPUT_Y: f32 = 9.0 + LOG_LINE_H * (LOG_LINES as f32);

// Bitmap font index (0..3) maps to sprite IDs 700..703. Yellow is 701 => index 1.
pub(in crate::states::gameplay) const UI_BITMAP_FONT: usize = 1;

// HUD stat label positions (engine.c: eng_display_win layout)
pub(in crate::states::gameplay) const HUD_HITPOINTS_X: f32 = 5.0;
pub(in crate::states::gameplay) const HUD_HITPOINTS_Y: f32 = 270.0;
pub(in crate::states::gameplay) const HUD_ENDURANCE_X: f32 = 5.0;
pub(in crate::states::gameplay) const HUD_ENDURANCE_Y: f32 = 284.0;
pub(in crate::states::gameplay) const HUD_MANA_X: f32 = 5.0;
pub(in crate::states::gameplay) const HUD_MANA_Y: f32 = 298.0;
pub(in crate::states::gameplay) const HUD_MONEY_X: f32 = 375.0;
pub(in crate::states::gameplay) const HUD_MONEY_Y: f32 = 190.0;
pub(in crate::states::gameplay) const HUD_UPDATE_LABEL_X: f32 = 117.0;
pub(in crate::states::gameplay) const HUD_UPDATE_LABEL_Y: f32 = 256.0;
pub(in crate::states::gameplay) const HUD_UPDATE_VALUE_X: f32 = 162.0;
pub(in crate::states::gameplay) const HUD_UPDATE_VALUE_Y: f32 = 256.0;
pub(in crate::states::gameplay) const HUD_WEAPON_VALUE_X: f32 = 646.0;
pub(in crate::states::gameplay) const HUD_WEAPON_VALUE_Y: f32 = 243.0;
pub(in crate::states::gameplay) const HUD_ARMOR_VALUE_X: f32 = 646.0;
pub(in crate::states::gameplay) const HUD_ARMOR_VALUE_Y: f32 = 257.0;
pub(in crate::states::gameplay) const HUD_EXPERIENCE_X: f32 = 646.0;
pub(in crate::states::gameplay) const HUD_EXPERIENCE_Y: f32 = 271.0;
pub(in crate::states::gameplay) const HUD_ATTRIBUTES_X: f32 = 5.0;
pub(in crate::states::gameplay) const HUD_ATTRIBUTES_Y_START: f32 = 4.0;
pub(in crate::states::gameplay) const HUD_ATTRIBUTES_SPACING: f32 = 14.0;
// Update-panel (stat raise) rows in engine.c are at y=74,88,102 with +/- markers and cost column.
pub(in crate::states::gameplay) const HUD_RAISE_STATS_X: f32 = 5.0;
pub(in crate::states::gameplay) const HUD_RAISE_STATS_PLUS_X: f32 = 136.0;
pub(in crate::states::gameplay) const HUD_RAISE_STATS_MINUS_X: f32 = 150.0;
pub(in crate::states::gameplay) const HUD_RAISE_STATS_COST_X: f32 = 162.0;
pub(in crate::states::gameplay) const HUD_RAISE_HP_Y: f32 = 74.0;
pub(in crate::states::gameplay) const HUD_RAISE_END_Y: f32 = 88.0;
pub(in crate::states::gameplay) const HUD_RAISE_MANA_Y: f32 = 102.0;
pub(in crate::states::gameplay) const HUD_SKILLS_X: f32 = 5.0;
pub(in crate::states::gameplay) const HUD_SKILLS_Y_START: f32 = 116.0;
pub(in crate::states::gameplay) const HUD_SKILLS_SPACING: f32 = 14.0;
// engine.c: dd_xputtext(374+(125-strlen(tmp)*6)/2, 28, 1, tmp)
pub(in crate::states::gameplay) const HUD_TOP_NAME_AREA_X: f32 = 374.0;
pub(in crate::states::gameplay) const HUD_TOP_NAME_AREA_W: f32 = 125.0;
pub(in crate::states::gameplay) const HUD_TOP_NAME_Y: f32 = 28.0;

// engine.c: dd_xputtext(374+(125-strlen(pl.name)*6)/2,152,1,pl.name);
//           dd_xputtext(374+(125-strlen(rank[points2rank(pl.points_tot)])*6)/2,172,1,rank[...]);
pub(in crate::states::gameplay) const HUD_PORTRAIT_TEXT_AREA_X: f32 = 374.0;
pub(in crate::states::gameplay) const HUD_PORTRAIT_TEXT_AREA_W: f32 = 125.0;
pub(in crate::states::gameplay) const HUD_PORTRAIT_NAME_Y: f32 = 152.0;
pub(in crate::states::gameplay) const HUD_PORTRAIT_RANK_Y: f32 = 172.0;
// engine.c:
// dd_showbar(373,127,n,6, BLUE/GREEN/RED)
// dd_showbar(373,134,n,6, BLUE/GREEN/RED)
// dd_showbar(373,141,n,6, BLUE/GREEN/RED)
pub(in crate::states::gameplay) const HUD_BAR_X: f32 = 373.0;
pub(in crate::states::gameplay) const HUD_HP_BAR_Y: f32 = 127.0;
pub(in crate::states::gameplay) const HUD_END_BAR_Y: f32 = 134.0;
pub(in crate::states::gameplay) const HUD_MANA_BAR_Y: f32 = 141.0;
pub(in crate::states::gameplay) const HUD_BAR_H: f32 = 6.0;

pub(in crate::states::gameplay) const BAR_SCALE_NUM: u32 = 62;
pub(in crate::states::gameplay) const BAR_W_MAX: u32 = 124;

// UI buttonbox (ported from orig/inter.c::trans_button and engine.c::dd_showbox).
pub(in crate::states::gameplay) const BUTTONBOX_X: f32 = 604.0;
pub(in crate::states::gameplay) const BUTTONBOX_Y_ROW1: f32 = 552.0; // F1..F4
pub(in crate::states::gameplay) const BUTTONBOX_Y_ROW2: f32 = 568.0; // F5..F8
pub(in crate::states::gameplay) const BUTTONBOX_Y_ROW3: f32 = 584.0; // F9..F12
pub(in crate::states::gameplay) const BUTTONBOX_BUTTON_W: f32 = 41.0;
pub(in crate::states::gameplay) const BUTTONBOX_BUTTON_H: f32 = 14.0;
pub(in crate::states::gameplay) const BUTTONBOX_STEP_X: f32 = 49.0;

pub(in crate::states::gameplay) const TOGGLE_BOX_W: f32 = 46.0;
pub(in crate::states::gameplay) const TOGGLE_BOX_H: f32 = 13.0;
pub(in crate::states::gameplay) const TOGGLE_SHOW_PROZ_X: f32 = 753.0;
pub(in crate::states::gameplay) const TOGGLE_SHOW_PROZ_Y: f32 = 554.0;
pub(in crate::states::gameplay) const TOGGLE_SHOW_NAMES_X: f32 = 704.0;
pub(in crate::states::gameplay) const TOGGLE_SHOW_NAMES_Y: f32 = 569.0;
pub(in crate::states::gameplay) const TOGGLE_HIDE_X: f32 = 656.0;
pub(in crate::states::gameplay) const TOGGLE_HIDE_Y: f32 = 569.0;

pub(in crate::states::gameplay) const MODE_BOX_W: f32 = 46.0;
pub(in crate::states::gameplay) const MODE_BOX_H: f32 = 13.0;
pub(in crate::states::gameplay) const MODE_FAST_X: f32 = 608.0; // pl.mode==2
pub(in crate::states::gameplay) const MODE_NORMAL_X: f32 = 656.0; // pl.mode==1
pub(in crate::states::gameplay) const MODE_SLOW_X: f32 = 704.0; // pl.mode==0
pub(in crate::states::gameplay) const MODE_BOX_Y: f32 = 554.0;

// Skill hotbar (xbuttons) grid: 12 slots, 4 columns x 3 rows.
// Ported from orig/engine.c draw loop and orig/inter.c::trans_button hitboxes.
pub(in crate::states::gameplay) const XBUTTONS_X: f32 = 604.0;
pub(in crate::states::gameplay) const XBUTTONS_Y_ROW1: f32 = 504.0;
pub(in crate::states::gameplay) const XBUTTONS_Y_ROW2: f32 = 519.0;
pub(in crate::states::gameplay) const XBUTTONS_Y_ROW3: f32 = 534.0;
pub(in crate::states::gameplay) const XBUTTONS_BUTTON_W: f32 = 41.0;
pub(in crate::states::gameplay) const XBUTTONS_BUTTON_H: f32 = 14.0;
pub(in crate::states::gameplay) const XBUTTONS_STEP_X: f32 = 49.0;

pub(in crate::states::gameplay) const XBUTTONS_LABEL_X: f32 = 610.0;
pub(in crate::states::gameplay) const XBUTTONS_LABEL_Y: f32 = 508.0;
pub(in crate::states::gameplay) const XBUTTONS_LABEL_STEP_Y: f32 = 15.0;

// Scroll knob rectangles (engine.c: dd_showbar calls for inv/skill scroll).
pub(in crate::states::gameplay) const SCROLL_KNOB_W: f32 = 11.0;
pub(in crate::states::gameplay) const SCROLL_KNOB_H: f32 = 11.0;
pub(in crate::states::gameplay) const SKILL_SCROLL_X: f32 = 207.0;
pub(in crate::states::gameplay) const SKILL_SCROLL_Y_BASE: f32 = 149.0;
pub(in crate::states::gameplay) const SKILL_SCROLL_RANGE: i32 = 58;
pub(in crate::states::gameplay) const SKILL_SCROLL_MAX: i32 = 40;
pub(in crate::states::gameplay) const INV_SCROLL_X: f32 = 290.0;
pub(in crate::states::gameplay) const INV_SCROLL_Y_BASE: f32 = 36.0;
pub(in crate::states::gameplay) const INV_SCROLL_RANGE: i32 = 94;
pub(in crate::states::gameplay) const INV_SCROLL_MAX: i32 = 30;
