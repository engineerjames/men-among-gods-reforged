use bevy::prelude::*;

use mag_core::constants::{
    WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD, WN_LEGS, WN_LHAND, WN_LRING, WN_NECK,
    WN_RHAND, WN_RRING,
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
pub(crate) struct GameplayLogScrollState {
    /// Number of log lines scrolled up from the most recent message.
    ///
    /// 0 means "follow the tail" (show newest messages). Larger values show older messages.
    pub(crate) offset: usize,

    /// Tracks `PlayerState::log_revision()` so we can keep the viewport stable when new log
    /// messages arrive while the user is scrolled up.
    pub(crate) last_log_revision: u64,
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

#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct CursorActionTextSettings {
    pub(crate) enabled: bool,
}

impl Default for CursorActionTextSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
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
    WN_HEAD, WN_CLOAK, WN_BODY, WN_ARMS, WN_NECK, WN_BELT, WN_RHAND, WN_LHAND, WN_LRING, WN_RRING,
    WN_LEGS, WN_FEET,
];
