// Inventory/equipment/spell UI draw helpers live here.

use bevy::prelude::*;

use crate::gfx_cache::GraphicsCache;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::*;
use crate::states::gameplay::resources::*;
use crate::states::gameplay::LastRender;

/// Updates backpack UI sprites/visibility based on current inventory scroll/hover state.
pub(crate) fn draw_inventory_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    inv_scroll: &GameplayInventoryScrollState,
    hover: &GameplayInventoryHoverState,
    q: &mut Query<(
        &GameplayUiBackpackSlot,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let pl = player_state.character_info();
    let inv_pos = inv_scroll.inv_pos.min(30);

    for (slot, mut sprite, mut visibility, mut last) in q.iter_mut() {
        let idx = inv_pos.saturating_add(slot.index);
        let sprite_id = pl.item.get(idx).copied().unwrap_or(0);

        // Highlight the currently hovered visible slot.
        let is_hovered = hover.backpack_slot == Some(slot.index);
        sprite.color = if is_hovered {
            Color::srgba(0.6, 1.0, 0.6, 1.0)
        } else {
            Color::WHITE
        };

        if sprite_id <= 0 {
            last.sprite_id = sprite_id;
            *visibility = Visibility::Hidden;
            continue;
        }

        if last.sprite_id != sprite_id {
            if let Some(src) = gfx.get_sprite(sprite_id as usize) {
                *sprite = src.clone();
                last.sprite_id = sprite_id;
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
            }
        } else {
            *visibility = Visibility::Visible;
        }
    }
}

/// Updates worn equipment UI sprites/visibility based on current hover state.
pub(crate) fn draw_equipment_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    hover: &GameplayInventoryHoverState,
    q: &mut Query<(
        &GameplayUiEquipmentSlot,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    // Match portrait behavior: while shop/look UI is active, show that target's equipment.
    enum EquipSource<'a> {
        Player(&'a mag_core::types::ClientPlayer),
        Look(&'a crate::types::look::Look),
    }

    let src = if player_state.should_show_shop() {
        EquipSource::Look(player_state.shop_target())
    } else if player_state.should_show_look() {
        EquipSource::Look(player_state.look_target())
    } else {
        EquipSource::Player(player_state.character_info())
    };

    for (slot, mut sprite, mut visibility, mut last) in q.iter_mut() {
        let sprite_id: i32 = match src {
            EquipSource::Player(pl) => pl.worn.get(slot.worn_index).copied().unwrap_or(0),
            EquipSource::Look(look) => look.worn(slot.worn_index) as i32,
        };

        // Hover highlight tint.
        let is_hovered = hover.equipment_worn_index == Some(slot.worn_index);
        sprite.color = if is_hovered {
            Color::srgba(0.6, 1.0, 0.6, 1.0)
        } else {
            Color::WHITE
        };

        if sprite_id <= 0 {
            last.sprite_id = sprite_id;
            *visibility = Visibility::Hidden;
            continue;
        }

        if last.sprite_id != sprite_id {
            if let Some(src) = gfx.get_sprite(sprite_id as usize) {
                *sprite = src.clone();
                last.sprite_id = sprite_id;
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
            }
        } else {
            *visibility = Visibility::Visible;
        }
    }
}

/// Updates the active-spells UI sprites/visibility and applies dimming by duration.
pub(crate) fn draw_active_spells_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    q: &mut Query<(
        &GameplayUiSpellSlot,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let pl = player_state.character_info();

    for (slot, mut sprite, mut visibility, mut last) in q.iter_mut() {
        let sprite_id = pl.spell.get(slot.index).copied().unwrap_or(0);
        if sprite_id <= 0 {
            last.sprite_id = sprite_id;
            *visibility = Visibility::Hidden;
            continue;
        }

        if last.sprite_id != sprite_id {
            if let Some(src) = gfx.get_sprite(sprite_id as usize) {
                *sprite = src.clone();
                last.sprite_id = sprite_id;
            } else {
                *visibility = Visibility::Hidden;
                continue;
            }
        }

        // dd.c shading (approx): engine.c uses effect = 15 - min(15, active[n]).
        // active==0 => effect=15 => dim; active>=15 => effect=0 => bright.
        let active = pl.active.get(slot.index).copied().unwrap_or(0).max(0) as i32;
        let effect = 15 - active.min(15);
        let shade = 1.0 - (effect as f32 / 15.0) * 0.6;
        sprite.color = Color::srgba(shade, shade, shade, 1.0);
        *visibility = Visibility::Visible;
    }
}
