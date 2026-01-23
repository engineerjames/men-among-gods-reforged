// Shop UI systems live here.

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

use mag_core::constants::SPR_EMPTY;

use super::super::cursor_game_pos;
use super::super::world_render::screen_to_world;

/// Updates shop sell/buy price labels based on the currently hovered shop slot.
///
/// When the shop UI is closed, hides the price labels.
pub(crate) fn run_gameplay_update_shop_price_labels(
    player_state: Res<PlayerState>,
    shop_hover: Res<GameplayShopHoverState>,
    mut q_sell: Query<
        (&mut BitmapText, &mut Visibility),
        (
            With<GameplayUiShopSellPriceLabel>,
            Without<GameplayUiShopBuyPriceLabel>,
        ),
    >,
    mut q_buy: Query<
        (&mut BitmapText, &mut Visibility),
        (
            With<GameplayUiShopBuyPriceLabel>,
            Without<GameplayUiShopSellPriceLabel>,
        ),
    >,
) {
    if !player_state.should_show_shop() {
        for (mut text, mut vis) in &mut q_sell {
            if !text.text.is_empty() {
                text.text.clear();
            }
            *vis = Visibility::Hidden;
        }
        for (mut text, mut vis) in &mut q_buy {
            if !text.text.is_empty() {
                text.text.clear();
            }
            *vis = Visibility::Hidden;
        }
        return;
    }

    let shop = player_state.shop_target();

    // Sell price: only when hovering an item slot with a non-zero price.
    let sell_text = shop_hover
        .slot
        .and_then(|idx| {
            let price = shop.price(idx);
            if price == 0 {
                return None;
            }
            Some(format!("Sell: {}G {}S", price / 100, price % 100))
        })
        .unwrap_or_default();

    for (mut text, mut vis) in &mut q_sell {
        if text.text != sell_text {
            text.text = sell_text.clone();
        }
        *vis = if text.text.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }

    // Buy price: when carrying an item and shop has a non-zero pl_price.
    let pl = player_state.character_info();
    let buy_price = if pl.citem != 0 { shop.pl_price() } else { 0 };
    let buy_text = if buy_price != 0 {
        // Keep the original spacing ("Buy:  ") for alignment with the classic UI.
        format!("Buy:  {}G {}S", buy_price / 100, buy_price % 100)
    } else {
        String::new()
    };

    for (mut text, mut vis) in &mut q_buy {
        if text.text != buy_text {
            text.text = buy_text.clone();
        }
        *vis = if text.text.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Spawns the shop window panel and item slot sprite entities.
pub(crate) fn spawn_ui_shop_window(commands: &mut Commands, gfx: &GraphicsCache) {
    // Matches `eng_display_win` shop layout:
    // - copyspritex(92,220,260,0);
    // - for n in 0..62: copyspritex(shop.item[n],222+(n%8)*35,262+(n/8)*35, ...)
    let Some(empty) = gfx.get_sprite(SPR_EMPTY as usize) else {
        return;
    };

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiShop {
            kind: ShopUiKind::Panel,
        },
        LastRender {
            sprite_id: i32::MIN,
            sx: f32::NAN,
            sy: f32::NAN,
        },
        empty.clone(),
        Anchor::TOP_LEFT,
        Transform::from_translation(screen_to_world(220.0, 260.0, Z_UI_SHOP_PANEL)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    for n in 0..62usize {
        let sx = 222.0 + ((n % 8) as f32) * 35.0;
        let sy = 262.0 + ((n / 8) as f32) * 35.0;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiShop {
                kind: ShopUiKind::Slot { index: n },
            },
            LastRender {
                sprite_id: i32::MIN,
                sx: f32::NAN,
                sy: f32::NAN,
            },
            empty.clone(),
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(sx, sy, Z_UI_SHOP_ITEMS)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Updates shop window UI panel/items sprites/visibility and hover highlighting.
pub(crate) fn draw_shop_window_ui(
    gfx: &GraphicsCache,
    player_state: &PlayerState,
    shop_hover: &GameplayShopHoverState,
    q: &mut Query<(
        &GameplayUiShop,
        &mut Sprite,
        &mut Visibility,
        &mut LastRender,
    )>,
) {
    let show_shop = player_state.should_show_shop();

    if !show_shop {
        for (_shop_ui, _sprite, mut visibility, mut last) in q.iter_mut() {
            last.sprite_id = 0;
            *visibility = Visibility::Hidden;
        }
        return;
    }

    let shop = player_state.shop_target();

    for (shop_ui, mut sprite, mut visibility, mut last) in q.iter_mut() {
        match shop_ui.kind {
            ShopUiKind::Panel => {
                const SHOP_PANEL_SPRITE: i32 = 92;
                if last.sprite_id != SHOP_PANEL_SPRITE {
                    if let Some(src) = gfx.get_sprite(SHOP_PANEL_SPRITE as usize) {
                        *sprite = src.clone();
                        last.sprite_id = SHOP_PANEL_SPRITE;
                    } else {
                        *visibility = Visibility::Hidden;
                        continue;
                    }
                }
                *visibility = Visibility::Visible;
            }
            ShopUiKind::Slot { index } => {
                let sprite_id = shop.item(index) as i32;
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

                let is_hovered = shop_hover.slot == Some(index);
                sprite.color = if is_hovered {
                    // Approximate engine.c's highlight effect=16.
                    Color::srgba(0.6, 1.0, 0.6, 1.0)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 1.0)
                };
                *visibility = Visibility::Visible;
            }
        }
    }
}

/// Handles mouse interactions with the shop UI (hover, close, buy/sell actions).
pub(crate) fn run_gameplay_shop_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, (With<Camera2d>, With<MagicScreenCamera>)>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut shop_hover: ResMut<GameplayShopHoverState>,
    mut cursor_state: ResMut<GameplayCursorTypeState>,
) {
    shop_hover.slot = None;
    shop_hover.over_close = false;

    if !player_state.should_show_shop() {
        return;
    }

    let Some(game) = cursor_game_pos(&windows, &cameras) else {
        return;
    };

    let x = game.x;
    let y = game.y;

    // Close button (orig/inter.c::mouse_shop): x 499..516, y 260..274.
    if (499.0..=516.0).contains(&x) && (260.0..=274.0).contains(&y) {
        shop_hover.over_close = true;
        if mouse.just_released(MouseButton::Left) {
            player_state.close_shop();
        }
        return;
    }

    // Shop grid (orig/inter.c::mouse_shop): x 220..500, y 261..(485+32+35).
    // That bottom bound evaluates to 552.
    if (220.0..=500.0).contains(&x) && (261.0..=552.0).contains(&y) {
        let tx = ((x - 220.0) / 35.0).floor() as i32;
        let ty = ((y - 261.0) / 35.0).floor() as i32;
        if (0..8).contains(&tx) && ty >= 0 {
            let nr = (tx + ty * 8) as usize;
            if nr < 62 {
                shop_hover.slot = Some(nr);

                let shop = player_state.shop_target();
                let pl = player_state.character_info();

                if shop.item(nr) != 0 {
                    cursor_state.cursor = GameplayCursorType::Take;
                } else if pl.citem != 0 {
                    cursor_state.cursor = GameplayCursorType::Drop;
                }

                let shop_nr = shop.nr() as i16;
                if mouse.just_released(MouseButton::Left) {
                    net.send(ClientCommand::new_shop(shop_nr, nr as i32).to_bytes());
                } else if mouse.just_released(MouseButton::Right) {
                    net.send(ClientCommand::new_shop(shop_nr, (nr + 62) as i32).to_bytes());
                }
            }
        }
    }
}
