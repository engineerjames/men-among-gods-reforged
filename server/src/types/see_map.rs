/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Visibility map for a character

/// Visibility map for a character
#[derive(Clone)]
pub struct SeeMap {
    pub x: i32,
    pub y: i32,
    pub vis: [i8; 40 * 40],
}

impl Default for SeeMap {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            vis: [0; 40 * 40],
        }
    }
}
