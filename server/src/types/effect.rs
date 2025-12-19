/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Effect structure

/// Effect structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C, packed)]
pub struct Effect {
    pub used: u8,
    pub flags: u8,

    pub effect_type: u8,  // what type of effect (FX_)

    pub duration: u32,  // time effect will stay

    pub data: [u32; 10],  // some data
}
