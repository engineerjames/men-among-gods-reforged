// Inventory/equipment UI systems live here.

use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::gfx_cache::GraphicsCache;
use crate::network::client_commands::ClientCommand;
use crate::network::NetworkRuntime;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::*;
use crate::states::gameplay::layout::*;
use crate::states::gameplay::resources::*;
use crate::states::gameplay::LastRender;
use crate::systems::magic_postprocess::MagicScreenCamera;

use mag_core::constants::{
    PL_ARMS, PL_BELT, PL_BODY, PL_CLOAK, PL_FEET, PL_HEAD, PL_LEGS, PL_NECK, PL_RING, PL_SHIELD,
    PL_TWOHAND, PL_WEAPON, SPR_EMPTY, WN_ARMS, WN_BELT, WN_BODY, WN_CLOAK, WN_FEET, WN_HEAD,
    WN_LEGS, WN_LHAND, WN_LRING, WN_NECK, WN_RHAND, WN_RRING,
};

use super::super::cursor_game_pos;
use super::super::world_render::screen_to_world;

/// Spawns the visible backpack slot sprite entities.
pub(crate) fn spawn_ui_backpack(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win`: copyspritex(pl.item[n+inv_pos],220+(n%2)*35,2+(n/2)*35,...)
    // We spawn one stable entity per visible backpack slot and update its sprite each frame.
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    for n in 0..10usize {
        let sx = 220.0 + ((n % 2) as f32) * 35.0;
        let sy = 2.0 + ((n / 2) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiBackpackSlot { index: n },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_INV)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns the equipment (worn item) slot sprite entities.
pub(crate) fn spawn_ui_equipment(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win`: copyspritex(pl.worn[wntab[n]],303+(n%2)*35,2+(n/2)*35,...)
    // We spawn one stable entity per slot and update its sprite each frame.
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    for (n, worn_index) in EQUIP_WNTAB.into_iter().enumerate() {
        let sx = 303.0 + (n as f32 % 2.0) * 35.0;
        let sy = 2.0 + ((n / 2) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiEquipmentSlot { worn_index },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_EQUIP)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns overlay entities indicating equipment slots blocked by a carried item.
pub(crate) fn spawn_ui_equipment_blocks(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches engine.c: if (inv_block[wntab[n]]) copyspritex(4,303+(n%2)*35,2+(n/2)*35,0);
    // Use sprite 4 (block overlay) when available.
    let Some(block) = gfx.get_sprite(4) else {
        return;
    };
    for (n, worn_index) in EQUIP_WNTAB.into_iter().enumerate() {
        let sx = 303.0 + ((n % 2) as f32) * 35.0;
        let sy = 2.0 + ((n / 2) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiEquipmentBlock { worn_index },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            block.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_EQUIP + 0.05)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns the spell icon slot sprite entities.
pub(crate) fn spawn_ui_spells(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win`: copyspritex(pl.spell[n],374+(n%5)*24,4+(n/5)*24,...)
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    for n in 0..20usize {
        let sx = 374.0 + ((n % 5) as f32) * 24.0;
        let sy = 4.0 + ((n / 5) as f32) * 24.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiSpellSlot { index: n },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_SPELLS)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Updates equipment-slot overlay blocks (e.g., blocked slots for carried items/two-handers).
pub(crate) fn run_gameplay_update_equipment_blocks(
    gfx: Res<GraphicsCache>,
    player_state: Res<PlayerState>,
    mut q: Query<(
        &GameplayUiEquipmentBlock,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    // These overlays reflect *local* equipment restrictions (carried item, two-handers).
    // When shop/look UI is active we draw another character's equipment, so hide blocks.
    if player_state.should_show_shop() || player_state.should_show_look() {
        for (_, _, mut vis, _) in &mut q {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let pl = player_state.character_info();
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

    const BLOCK_SPRITE_ID: i32 = 4;
    for (slot, mut sprite, mut vis, mut last) in &mut q {
        let blocked = inv_block.get(slot.worn_index).copied().unwrap_or(false);
        if !blocked {
            *vis = Visibility::Hidden;
            continue;
        }
        if last.sprite_id != BLOCK_SPRITE_ID {
            if let Some(src) = gfx.get_sprite(BLOCK_SPRITE_ID as usize) {
                *sprite = src.clone();
                last.sprite_id = BLOCK_SPRITE_ID;
                *vis = Visibility::Visible;
            } else {
                *vis = Visibility::Hidden;
            }
        } else {
            *vis = Visibility::Visible;
        }
        sprite.color = Color::WHITE;
    }
}

/// Handles inventory UI hover and click interactions (equipment, backpack, money).
pub(crate) fn run_gameplay_inventory_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, (With<Camera2d>, With<MagicScreenCamera>)>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    inv_scroll: Res<GameplayInventoryScrollState>,
    mut hover: ResMut<GameplayInventoryHoverState>,
    mut cursor_state: ResMut<GameplayCursorTypeState>,
) {
    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        hover.backpack_slot = None;
        hover.equipment_worn_index = None;
        hover.money = None;
        cursor_state.cursor = GameplayCursorType::None;
        return;
    };

    // Hover detection order mirrors orig/inter.c::mouse_inventory: money first, then backpack.
    hover.backpack_slot = None;
    hover.equipment_worn_index = None;
    hover.money = None;
    cursor_state.cursor = GameplayCursorType::None;

    let x = game.x;
    let y = game.y;

    // Money (orig/inter.c::mouse_inventory): click regions around x=219..301,y=176..259.
    if (176.0..=203.0).contains(&y) {
        if (219.0..=246.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Silver1);
        } else if (247.0..=274.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Silver10);
        } else if (275.0..=301.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold1);
        }
    } else if (205.0..=231.0).contains(&y) {
        if (219.0..=246.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold10);
        } else if (247.0..=274.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold100);
        } else if (275.0..=301.0).contains(&x) {
            hover.money = Some(GameplayMoneyHoverKind::Gold1000);
        }
    } else if (232.0..=259.0).contains(&y) && (219.0..=246.0).contains(&x) {
        hover.money = Some(GameplayMoneyHoverKind::Gold10000);
    }

    // Backpack hover (orig/inter.c::mouse_inventory): x 219..288, y 1..175.
    if hover.money.is_none() && (219.0..=288.0).contains(&x) && (1.0..=175.0).contains(&y) {
        let tx = ((x - 219.0) / 35.0).floor() as i32;
        let ty = ((y - 1.0) / 35.0).floor() as i32;
        if (0..2).contains(&tx) && (0..5).contains(&ty) {
            let slot = (tx + ty * 2) as usize;
            if slot < 10 {
                hover.backpack_slot = Some(slot);
            }
        }
    }

    // Worn equipment hover (orig/inter.c::mouse_inventory): x>302 && x<371 && y>1 && y<175+35.
    if hover.money.is_none()
        && hover.backpack_slot.is_none()
        && (302.0..=371.0).contains(&x)
        && (1.0..=210.0).contains(&y)
    {
        let tx = ((x - 302.0) / 35.0).floor() as i32;
        let ty = ((y - 1.0) / 35.0).floor() as i32;
        if (0..2).contains(&tx) && (0..6).contains(&ty) {
            let n = (tx + ty * 2) as usize;
            if n < EQUIP_WNTAB.len() {
                hover.equipment_worn_index = Some(EQUIP_WNTAB[n]);
            }
        }
    }

    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let keys_mask = (shift as u32) | ((ctrl as u32) << 1);

    // Cursor type (orig/inter.c::mouse_inventory cursor_type logic).
    let pl = player_state.character_info();
    let has_citem = pl.citem != 0;
    if hover.money.is_some() {
        cursor_state.cursor = GameplayCursorType::Take;
    } else if let Some(slot) = hover.backpack_slot {
        let idx = inv_scroll.inv_pos.saturating_add(slot);
        let has_item = pl.item.get(idx).copied().unwrap_or(0) != 0;
        cursor_state.cursor = match keys_mask {
            1 => {
                if has_item {
                    if has_citem {
                        GameplayCursorType::Swap
                    } else {
                        GameplayCursorType::Take
                    }
                } else if has_citem {
                    GameplayCursorType::Drop
                } else {
                    GameplayCursorType::None
                }
            }
            0 => {
                if has_item {
                    GameplayCursorType::Use
                } else {
                    GameplayCursorType::None
                }
            }
            _ => GameplayCursorType::None,
        };
    } else if let Some(worn_index) = hover.equipment_worn_index {
        let has_item = pl.worn.get(worn_index).copied().unwrap_or(0) != 0;
        cursor_state.cursor = match keys_mask {
            1 => {
                if has_item {
                    if has_citem {
                        GameplayCursorType::Swap
                    } else {
                        GameplayCursorType::Take
                    }
                } else if has_citem {
                    GameplayCursorType::Drop
                } else {
                    GameplayCursorType::None
                }
            }
            0 => {
                if has_item {
                    GameplayCursorType::Use
                } else {
                    GameplayCursorType::None
                }
            }
            _ => GameplayCursorType::None,
        };
    }

    // Right-click behavior: money tooltips + backpack look.
    if mouse.just_released(MouseButton::Right) {
        if let Some(kind) = hover.money {
            match kind {
                GameplayMoneyHoverKind::Silver1 => player_state.tlog(1, "1 silver coin."),
                GameplayMoneyHoverKind::Silver10 => player_state.tlog(1, "10 silver coins."),
                GameplayMoneyHoverKind::Gold1 => player_state.tlog(1, "1 gold coin."),
                GameplayMoneyHoverKind::Gold10 => player_state.tlog(1, "10 gold coins."),
                GameplayMoneyHoverKind::Gold100 => player_state.tlog(1, "100 gold coins."),
                GameplayMoneyHoverKind::Gold1000 => player_state.tlog(1, "1,000 gold coins."),
                GameplayMoneyHoverKind::Gold10000 => player_state.tlog(1, "10,000 gold coins."),
            }
            return;
        }

        if let Some(slot) = hover.backpack_slot {
            let selected_char = player_state.selected_char() as u32;
            let idx = inv_scroll.inv_pos.saturating_add(slot);
            net.send(ClientCommand::new_inv_look(idx as u32, 0, selected_char).to_bytes());
            return;
        }

        // Worn equipment right-click (orig/inter.c::mouse_inventory): cmd3(CL_CMD_INV,7,slot,selected_char).
        // Slot numbering here matches the original cmd3 usage (0..11), not the WN_* indices.
        if hover.equipment_worn_index.is_some() && (keys_mask == 0 || keys_mask == 1) {
            let tx = ((x - 302.0) / 35.0).floor() as i32;
            let ty = ((y - 1.0) / 35.0).floor() as i32;
            let slot_nr: Option<u32> = match (tx, ty) {
                (0, 0) => Some(0),  // head
                (1, 0) => Some(9),  // cloak
                (0, 1) => Some(2),  // body
                (1, 1) => Some(3),  // arms
                (0, 2) => Some(1),  // neck
                (1, 2) => Some(4),  // belt
                (0, 3) => Some(8),  // right hand
                (1, 3) => Some(7),  // left hand
                (0, 4) => Some(10), // right ring
                (1, 4) => Some(11), // left ring
                (0, 5) => Some(5),  // legs
                (1, 5) => Some(6),  // feet
                _ => None,
            };
            if let Some(slot_nr) = slot_nr {
                let selected_char = player_state.selected_char() as u32;
                net.send(ClientCommand::new_inv(7, slot_nr, selected_char).to_bytes());
            }
            return;
        }

        return;
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    // Left-click behavior.
    if let Some(kind) = hover.money {
        let selected_char = player_state.selected_char() as u32;
        let amount = match kind {
            GameplayMoneyHoverKind::Silver1 => 1u32,
            GameplayMoneyHoverKind::Silver10 => 10u32,
            GameplayMoneyHoverKind::Gold1 => 100u32,
            GameplayMoneyHoverKind::Gold10 => 1_000u32,
            GameplayMoneyHoverKind::Gold100 => 10_000u32,
            GameplayMoneyHoverKind::Gold1000 => 100_000u32,
            GameplayMoneyHoverKind::Gold10000 => 1_000_000u32,
        };
        // orig/inter.c uses: cmd3(CL_CMD_INV,2,amount,selected_char)
        net.send(ClientCommand::new_inv(2, amount, selected_char).to_bytes());
        return;
    }

    if let Some(slot) = hover.backpack_slot {
        // Backpack click behavior depends on Shift (orig/inter.c::mouse_inventory).
        // - Shift + LB_UP: cmd3(CL_CMD_INV,0,nr+inv_pos,selected_char)
        // - No mods + LB_UP: cmd3(CL_CMD_INV,6,nr+inv_pos,selected_char)
        // - Ctrl or other combos: do nothing.
        if keys_mask == 0 || keys_mask == 1 {
            let selected_char = player_state.selected_char() as u32;
            let idx = inv_scroll.inv_pos.saturating_add(slot) as u32;
            let a = if keys_mask == 1 { 0u32 } else { 6u32 };
            net.send(ClientCommand::new_inv(a, idx, selected_char).to_bytes());
        }
        return;
    }

    // Worn equipment left-click (orig/inter.c::mouse_inventory):
    // - Shift + LB_UP: cmd3(CL_CMD_INV,1,slot,selected_char)
    // - No mods + LB_UP: cmd3(CL_CMD_INV,5,slot,selected_char)
    if hover.equipment_worn_index.is_some() && (keys_mask == 0 || keys_mask == 1) {
        let tx = ((x - 302.0) / 35.0).floor() as i32;
        let ty = ((y - 1.0) / 35.0).floor() as i32;
        let slot_nr: Option<u32> = match (tx, ty) {
            (0, 0) => Some(0),  // head
            (1, 0) => Some(9),  // cloak
            (0, 1) => Some(2),  // body
            (1, 1) => Some(3),  // arms
            (0, 2) => Some(1),  // neck
            (1, 2) => Some(4),  // belt
            (0, 3) => Some(8),  // right hand
            (1, 3) => Some(7),  // left hand
            (0, 4) => Some(10), // right ring
            (1, 4) => Some(11), // left ring
            (0, 5) => Some(5),  // legs
            (1, 5) => Some(6),  // feet
            _ => None,
        };
        if let Some(slot_nr) = slot_nr {
            let selected_char = player_state.selected_char() as u32;
            let a = if keys_mask == 1 { 1u32 } else { 5u32 };
            net.send(ClientCommand::new_inv(a, slot_nr, selected_char).to_bytes());
        }
    }
}
