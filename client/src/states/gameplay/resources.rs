use std::collections::HashMap;

use bevy::prelude::*;

use mag_core::constants::{
    PL_ARMS, PL_BELT, PL_BODY, PL_CLOAK, PL_FEET, PL_HEAD, PL_LEGS, PL_NECK, PL_RING, PL_SHIELD,
    PL_TWOHAND, PL_WEAPON, WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD, WN_LEGS,
    WN_LHAND, WN_LRING, WN_NECK, WN_RHAND, WN_RRING,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShopUiKind {
    Panel,
    Slot { index: usize },
}

#[derive(Resource, Default, Debug)]
pub(crate) struct GameplayTextInput {
    pub(crate) current: String,
    pub(crate) history: Vec<String>,
    pub(crate) history_pos: Option<usize>,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct GameplayExitState {
    pub(crate) firstquit: bool,
    pub(crate) wantquit: bool,
}

#[derive(Resource)]
pub(crate) struct GameplayStatboxState {
    /// Pending stat raises, indexed like the original client:
    /// 0..=4 attributes, 5 hitpoints, 6 endurance, 7 mana,
    /// and 8.. are skills in the (sorted) skill table order.
    pub(crate) stat_raised: [i32; 108],
    pub(crate) stat_points_used: i32,
    pub(crate) skill_pos: usize,
}

impl Default for GameplayStatboxState {
    fn default() -> Self {
        Self {
            stat_raised: [0; 108],
            stat_points_used: 0,
            skill_pos: 0,
        }
    }
}

impl GameplayStatboxState {
    pub(crate) fn available_points(&self, pl: &mag_core::types::ClientPlayer) -> i32 {
        (pl.points - self.stat_points_used).max(0)
    }

    pub(crate) fn clear(&mut self) {
        self.stat_raised.fill(0);
        self.stat_points_used = 0;
    }
}

#[derive(Resource, Default)]
pub(crate) struct GameplayInventoryScrollState {
    /// Inventory scroll position from the original client (0..=30).
    pub(crate) inv_pos: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayMoneyHoverKind {
    Silver1,
    Silver10,
    Gold1,
    Gold10,
    Gold100,
    Gold1000,
    Gold10000,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct GameplayInventoryHoverState {
    pub(crate) backpack_slot: Option<usize>,
    pub(crate) equipment_worn_index: Option<usize>,
    pub(crate) money: Option<GameplayMoneyHoverKind>,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct GameplayXButtonsState {
    /// Skilltab index selected via right-click in the skill list.
    pub(crate) pending_skill_id: Option<usize>,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct GameplayShopHoverState {
    pub(crate) slot: Option<usize>,
    pub(crate) over_close: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum GameplayCursorType {
    #[default]
    None,
    Take,
    Drop,
    Swap,
    Use,
}

#[derive(Resource, Default, Debug)]
pub(crate) struct GameplayCursorTypeState {
    pub(crate) cursor: GameplayCursorType,
}

// Equipment slot ordering used by the original client UI.
// Matches engine.c's `wntab[]` for drawing worn items.
pub(crate) const EQUIP_WNTAB: [usize; 12] = [
    WN_HEAD, WN_CLOAK, WN_BODY, WN_ARMS, WN_NECK, WN_BELT, WN_RHAND, WN_LHAND, WN_RRING, WN_LRING,
    WN_LEGS, WN_FEET,
];

#[inline]
pub(crate) fn equip_slot_is_blocked(pl: &mag_core::types::ClientPlayer, worn_index: usize) -> bool {
    let mut inv_block = [false; 20];
    let citem = pl.citem;
    let citem_p = pl.citem_p as u16;

    if citem != 0 {
        inv_block[WN_HEAD] = (citem_p & PL_HEAD) == 0;
        inv_block[WN_NECK] = (citem_p & PL_NECK) == 0;
        inv_block[WN_BODY] = (citem_p & PL_BODY) == 0;
        inv_block[WN_ARMS] = (citem_p & PL_ARMS) == 0;
        inv_block[WN_BELT] = (citem_p & PL_BELT) == 0;
        inv_block[WN_LEGS] = (citem_p & PL_LEGS) == 0;
        inv_block[WN_FEET] = (citem_p & PL_FEET) == 0;
        inv_block[WN_RHAND] = (citem_p & PL_WEAPON) == 0;
        inv_block[WN_LHAND] = (citem_p & PL_SHIELD) == 0;
        inv_block[WN_CLOAK] = (citem_p & PL_CLOAK) == 0;

        let ring_blocked = (citem_p & PL_RING) == 0;
        inv_block[WN_LRING] = ring_blocked;
        inv_block[WN_RRING] = ring_blocked;
    }

    // Two-handed weapon blocks the left hand slot.
    if (pl.worn_p[WN_RHAND] as u16 & PL_TWOHAND) != 0 {
        inv_block[WN_LHAND] = true;
    }

    inv_block.get(worn_index).copied().unwrap_or(false)
}

#[derive(Resource, Default, Debug)]
pub(crate) struct MiniMapCache {
    pub(crate) xmap: Vec<u16>,
    pub(crate) avg_cache: HashMap<usize, u16>,
}
