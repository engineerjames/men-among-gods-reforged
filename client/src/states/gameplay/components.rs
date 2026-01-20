use bevy::prelude::*;

use crate::states::gameplay::resources::ShopUiKind;

#[derive(Component)]
pub struct GameplayRenderEntity;

#[derive(Component)]
pub(crate) struct GameplayUiOverlay;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayToggleBoxKind {
    ShowProz,
    ShowNames,
    Hide,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiToggleBox {
    pub(crate) kind: GameplayToggleBoxKind,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiModeBox {
    pub(crate) mode: i32,
}

#[derive(Component)]
pub(crate) struct GameplayUiPortrait;

#[derive(Component)]
pub(crate) struct GameplayUiRank;

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiEquipmentSlot {
    pub(crate) worn_index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiEquipmentBlock {
    pub(crate) worn_index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiBackpackSlot {
    pub(crate) index: usize,
}

#[derive(Component)]
pub(crate) struct GameplayUiCarriedItem;

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiSpellSlot {
    pub(crate) index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiShop {
    pub(crate) kind: ShopUiKind,
}

#[derive(Component)]
pub(crate) struct GameplayUiLogLine {
    pub(crate) line: usize,
}

#[derive(Component)]
pub(crate) struct GameplayUiInputText;

#[derive(Component)]
pub(crate) struct GameplayUiMinimap;

#[derive(Component, Clone, Debug)]
pub(crate) struct BitmapText {
    pub(crate) text: String,
    pub(crate) color: Color,
    pub(crate) font: usize,
}

#[derive(Component)]
pub(crate) struct BitmapGlyph;

// HUD stat label components
#[derive(Component)]
pub(crate) struct GameplayUiHitpointsLabel;

#[derive(Component)]
pub(crate) struct GameplayUiEnduranceLabel;

#[derive(Component)]
pub(crate) struct GameplayUiManaLabel;

#[derive(Component)]
pub(crate) struct GameplayUiMoneyLabel;

#[derive(Component)]
pub(crate) struct GameplayUiShopSellPriceLabel;

#[derive(Component)]
pub(crate) struct GameplayUiShopBuyPriceLabel;

#[derive(Component)]
pub(crate) struct GameplayUiUpdateLabel;

#[derive(Component)]
pub(crate) struct GameplayUiUpdateValue;

#[derive(Component)]
pub(crate) struct GameplayUiWeaponValueLabel;

#[derive(Component)]
pub(crate) struct GameplayUiArmorValueLabel;

#[derive(Component)]
pub(crate) struct GameplayUiExperienceLabel;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayUiRaiseStat {
    Hitpoints,
    Endurance,
    Mana,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayUiRaiseStatColumn {
    Value,
    Plus,
    Minus,
    Cost,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiRaiseStatText {
    pub(crate) stat: GameplayUiRaiseStat,
    pub(crate) col: GameplayUiRaiseStatColumn,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiAttributeLabel {
    pub(crate) attrib_index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiSkillLabel {
    pub(crate) skill_index: usize,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiXButtonLabel {
    pub(crate) index: usize,
}

#[derive(Component)]
pub(crate) struct GameplayUiTopSelectedNameLabel;

#[derive(Component)]
pub(crate) struct GameplayUiPortraitNameLabel;

#[derive(Component)]
pub(crate) struct GameplayUiPortraitRankLabel;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayUiBarKind {
    Hitpoints,
    Endurance,
    Mana,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayUiBarLayer {
    Background,
    Fill,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiBar {
    pub(crate) kind: GameplayUiBarKind,
    pub(crate) layer: GameplayUiBarLayer,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiAttributeAuxText {
    pub(crate) attrib_index: usize,
    pub(crate) col: GameplayUiRaiseStatColumn,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiSkillAuxText {
    pub(crate) row: usize,
    pub(crate) col: GameplayUiRaiseStatColumn,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GameplayUiScrollKnobKind {
    Skill,
    Inventory,
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GameplayUiScrollKnob {
    pub(crate) kind: GameplayUiScrollKnobKind,
}
