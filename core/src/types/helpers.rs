//! Helper functions for type validation and conversions

use super::Character;
use crate::constants::*;

/// Sanity checks on map locations x
#[inline]
pub fn sanex(x: i32) -> bool {
    x >= 0 && x < SERVER_MAPX
}

/// Sanity checks on map locations y
#[inline]
pub fn saney(y: i32) -> bool {
    y >= 0 && y < SERVER_MAPY
}

/// Sanity checks on map locations x and y
#[inline]
pub fn sanexy(x: i32, y: i32) -> bool {
    sanex(x) && saney(y)
}

/// Convert (x,y) coordinates to absolute position
#[inline]
pub fn xy2m(x: i32, y: i32) -> usize {
    (x + y * SERVER_MAPX) as usize
}

/// Sanity checks on item numbers
#[inline]
pub fn is_sane_item(index: usize) -> bool {
    index > 0 && index < MAXITEM
}

/// Sanity checks on character numbers
#[inline]
pub fn is_sane_char(cn: usize) -> bool {
    cn > 0 && cn < MAXCHARS
}

/// Sanity check on skill number
#[inline]
pub fn sane_skill(s: usize) -> bool {
    s < MAXSKILL
}

/// Sanity checks on item templates
#[inline]
pub fn is_sane_itemplate(tn: usize) -> bool {
    tn > 0 && tn < MAXTITEM
}

/// Sanity checks on character templates
#[inline]
pub fn is_sane_ctemplate(tn: usize) -> bool {
    tn > 0 && tn < MAXTCHARS
}

/// Check if this is a sane player character number
#[inline]
pub fn is_sane_player(cn: usize, ch: &[Character]) -> bool {
    is_sane_char(cn) && (ch[cn].flags & CharacterFlags::CF_PLAYER.bits()) != 0
}
