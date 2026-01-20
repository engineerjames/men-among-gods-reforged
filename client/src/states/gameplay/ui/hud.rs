// HUD label/bar systems live here.

use bevy::asset::RenderAssetUsages;
use bevy::ecs::query::Without;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;

use crate::network::client_commands::ClientCommand;
use crate::network::NetworkRuntime;
use crate::player_state::PlayerState;
use crate::states::gameplay::components::*;
use crate::states::gameplay::layout::*;
use crate::states::gameplay::resources::*;

use mag_core::types::skilltab::{get_skill_name, get_skill_sortkey};

use super::super::world_render::screen_to_world;

use super::super::{
    build_sorted_skills, cmd_exit, cursor_game_pos, end_needed, hp_needed, in_rect, mana_needed,
    ui_bar_colors, ATTRIBUTE_NAMES,
};

/// Spawns HUD labels used in gameplay (HP/End/Mana, stats, skills, etc.).
pub(crate) fn spawn_ui_hud_labels(commands: &mut Commands) {
    // Hitpoints label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiHitpointsLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_HITPOINTS_X, HUD_HITPOINTS_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Endurance label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiEnduranceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_ENDURANCE_X, HUD_ENDURANCE_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Mana label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiManaLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_MANA_X, HUD_MANA_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Money label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiMoneyLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(HUD_MONEY_X, HUD_MONEY_Y, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Update label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiUpdateLabel,
        BitmapText {
            text: String::from("Update"),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_UPDATE_LABEL_X,
            HUD_UPDATE_LABEL_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Update value
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiUpdateValue,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_UPDATE_VALUE_X,
            HUD_UPDATE_VALUE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Weapon value label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiWeaponValueLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_WEAPON_VALUE_X,
            HUD_WEAPON_VALUE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Armor value label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiArmorValueLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_ARMOR_VALUE_X,
            HUD_ARMOR_VALUE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Experience label
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiExperienceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_EXPERIENCE_X,
            HUD_EXPERIENCE_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Attribute labels (5 attributes: Braveness, Willpower, Intuition, Agility, Strength)
    for i in 0..5 {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiAttributeLabel { attrib_index: i },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(
                HUD_ATTRIBUTES_X,
                HUD_ATTRIBUTES_Y_START + (i as f32) * HUD_ATTRIBUTES_SPACING,
                Z_UI_TEXT,
            )),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));

        // Attribute +/- and cost columns (engine.c uses x=136/150/162).
        let y = HUD_ATTRIBUTES_Y_START + (i as f32) * HUD_ATTRIBUTES_SPACING;
        for (col, x) in [
            (GameplayUiRaiseStatColumn::Plus, HUD_RAISE_STATS_PLUS_X),
            (GameplayUiRaiseStatColumn::Minus, HUD_RAISE_STATS_MINUS_X),
            (GameplayUiRaiseStatColumn::Cost, HUD_RAISE_STATS_COST_X),
        ] {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiAttributeAuxText {
                    attrib_index: i,
                    col,
                },
                BitmapText {
                    text: String::new(),
                    color: Color::WHITE,
                    font: UI_BITMAP_FONT,
                },
                Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }

    // Raise-stat rows (Hitpoints / Endurance / Mana) with +/- markers and exp cost.
    // Matches engine.c lines at y=74,88,102 and columns x=5,136,150,162.
    let raise_rows = [
        (GameplayUiRaiseStat::Hitpoints, HUD_RAISE_HP_Y),
        (GameplayUiRaiseStat::Endurance, HUD_RAISE_END_Y),
        (GameplayUiRaiseStat::Mana, HUD_RAISE_MANA_Y),
    ];
    for (stat, y) in raise_rows {
        for (col, x) in [
            (GameplayUiRaiseStatColumn::Value, HUD_RAISE_STATS_X),
            (GameplayUiRaiseStatColumn::Plus, HUD_RAISE_STATS_PLUS_X),
            (GameplayUiRaiseStatColumn::Minus, HUD_RAISE_STATS_MINUS_X),
            (GameplayUiRaiseStatColumn::Cost, HUD_RAISE_STATS_COST_X),
        ] {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiRaiseStatText { stat, col },
                BitmapText {
                    text: String::new(),
                    color: Color::WHITE,
                    font: UI_BITMAP_FONT,
                },
                Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }

    // Skill labels (10 skills visible at a time)
    for i in 0..10 {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiSkillLabel { skill_index: i },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(
                HUD_SKILLS_X,
                HUD_SKILLS_Y_START + (i as f32) * HUD_SKILLS_SPACING,
                Z_UI_TEXT,
            )),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));

        // Skill +/- and cost columns (engine.c uses x=136/150/162 at y=116+n*14).
        let y = HUD_SKILLS_Y_START + (i as f32) * HUD_SKILLS_SPACING;
        for (col, x) in [
            (GameplayUiRaiseStatColumn::Plus, HUD_RAISE_STATS_PLUS_X),
            (GameplayUiRaiseStatColumn::Minus, HUD_RAISE_STATS_MINUS_X),
            (GameplayUiRaiseStatColumn::Cost, HUD_RAISE_STATS_COST_X),
        ] {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiSkillAuxText { row: i, col },
                BitmapText {
                    text: String::new(),
                    color: Color::WHITE,
                    font: UI_BITMAP_FONT,
                },
                Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }

    // Top-center selected/player name (engine.c y=28).
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiTopSelectedNameLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(
            HUD_TOP_NAME_AREA_X,
            HUD_TOP_NAME_Y,
            Z_UI_TEXT,
        )),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // (No "Available Points" label; original client doesn't show it here.)

    // Shop price labels (engine.c):
    // - dd_xputtext(225,549,1,"Sell: %dG %dS", ...)
    // - dd_xputtext(225,559,1,"Buy:  %dG %dS", ...)
    // These are only visible while the shop UI is open.
    commands.spawn((
        GameplayRenderEntity,
        GameplayUiShopSellPriceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(225.0, 549.0, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    commands.spawn((
        GameplayRenderEntity,
        GameplayUiShopBuyPriceLabel,
        BitmapText {
            text: String::new(),
            color: Color::WHITE,
            font: UI_BITMAP_FONT,
        },
        Transform::from_translation(screen_to_world(225.0, 559.0, Z_UI_TEXT)),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
    ));

    // Skill hotbar (xbuttons) labels (engine.c: dd_xputtext(610+(n%4)*49,508+(n/4)*15,...)).
    for n in 0..12 {
        let col = n % 4;
        let row = n / 4;
        let x = XBUTTONS_LABEL_X + (col as f32) * XBUTTONS_STEP_X;
        let y = XBUTTONS_LABEL_Y + (row as f32) * XBUTTONS_LABEL_STEP_Y;
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiXButtonLabel { index: n },
            BitmapText {
                text: String::new(),
                color: Color::WHITE,
                font: UI_BITMAP_FONT,
            },
            Transform::from_translation(screen_to_world(x, y, Z_UI_TEXT)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Spawns the orange outline boxes used for gameplay toggles and mode selection.
pub(crate) fn spawn_ui_toggle_boxes(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    // A single white pixel stretched + tinted to match dd_showbox's outline.
    let pixel = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let pixel_handle = image_assets.add(pixel);

    let orange = Color::srgb(1.0, 0.55, 0.0);
    let z = Z_UI_TEXT - 2.0;

    let spawn_box = |commands: &mut Commands, kind: GameplayToggleBoxKind, sx: f32, sy: f32| {
        let w = TOGGLE_BOX_W;
        let h = TOGGLE_BOX_H;
        let t = 1.0;

        let spawn_seg = |commands: &mut Commands, ox: f32, oy: f32, sw: f32, sh: f32| {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiToggleBox { kind },
                Sprite {
                    image: pixel_handle.clone(),
                    color: orange,
                    custom_size: Some(Vec2::new(sw.max(0.0), sh.max(0.0))),
                    ..default()
                },
                Anchor::TOP_LEFT,
                Transform::from_translation(screen_to_world(sx + ox, sy + oy, z)),
                GlobalTransform::default(),
                Visibility::Hidden,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        };

        // Top / bottom
        spawn_seg(commands, 0.0, 0.0, w, t);
        spawn_seg(commands, 0.0, h - t, w, t);
        // Left / right
        spawn_seg(commands, 0.0, 0.0, t, h);
        spawn_seg(commands, w - t, 0.0, t, h);
    };

    spawn_box(
        commands,
        GameplayToggleBoxKind::ShowProz,
        TOGGLE_SHOW_PROZ_X,
        TOGGLE_SHOW_PROZ_Y,
    );
    spawn_box(
        commands,
        GameplayToggleBoxKind::ShowNames,
        TOGGLE_SHOW_NAMES_X,
        TOGGLE_SHOW_NAMES_Y,
    );
    spawn_box(
        commands,
        GameplayToggleBoxKind::Hide,
        TOGGLE_HIDE_X,
        TOGGLE_HIDE_Y,
    );

    // Mode selection boxes (orig/engine.c: dd_showbox based on pl.mode).
    let spawn_mode_box = |commands: &mut Commands, mode: i32, sx: f32, sy: f32| {
        let w = MODE_BOX_W;
        let h = MODE_BOX_H;
        let t = 1.0;

        let spawn_seg = |commands: &mut Commands, ox: f32, oy: f32, sw: f32, sh: f32| {
            commands.spawn((
                GameplayRenderEntity,
                GameplayUiModeBox { mode },
                Sprite {
                    image: pixel_handle.clone(),
                    color: orange,
                    custom_size: Some(Vec2::new(sw.max(0.0), sh.max(0.0))),
                    ..default()
                },
                Anchor::TOP_LEFT,
                Transform::from_translation(screen_to_world(sx + ox, sy + oy, z)),
                GlobalTransform::default(),
                Visibility::Hidden,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        };

        spawn_seg(commands, 0.0, 0.0, w, t);
        spawn_seg(commands, 0.0, h - t, w, t);
        spawn_seg(commands, 0.0, 0.0, t, h);
        spawn_seg(commands, w - t, 0.0, t, h);
    };

    spawn_mode_box(commands, 2, MODE_FAST_X, MODE_BOX_Y);
    spawn_mode_box(commands, 1, MODE_NORMAL_X, MODE_BOX_Y);
    spawn_mode_box(commands, 0, MODE_SLOW_X, MODE_BOX_Y);
}

/// Spawns the HUD stat bars (background + fill rectangles).
pub(crate) fn spawn_ui_stat_bars(commands: &mut Commands, image_assets: &mut Assets<Image>) {
    // A single white pixel stretched + tinted for dd_showbar-like rectangles.
    let pixel = Image::new(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let pixel_handle = image_assets.add(pixel);

    let (blue, green, _red) = ui_bar_colors();

    let z_bg = Z_UI_TEXT - 6.0;
    let z_fg = Z_UI_TEXT - 5.9;

    let spawn_bar = |commands: &mut Commands,
                     kind: GameplayUiBarKind,
                     layer: GameplayUiBarLayer,
                     x: f32,
                     y: f32,
                     z: f32,
                     color: Color| {
        commands.spawn((
            GameplayRenderEntity,
            GameplayUiBar { kind, layer },
            Sprite {
                image: pixel_handle.clone(),
                color,
                custom_size: Some(Vec2::new(0.0, HUD_BAR_H)),
                ..default()
            },
            Anchor::TOP_LEFT,
            Transform::from_translation(screen_to_world(x, y, z)),
            GlobalTransform::default(),
            Visibility::Hidden,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    };

    for (kind, y) in [
        (GameplayUiBarKind::Hitpoints, HUD_HP_BAR_Y),
        (GameplayUiBarKind::Endurance, HUD_END_BAR_Y),
        (GameplayUiBarKind::Mana, HUD_MANA_BAR_Y),
    ] {
        spawn_bar(
            commands,
            kind,
            GameplayUiBarLayer::Background,
            HUD_BAR_X,
            y,
            z_bg,
            blue,
        );
        // Fill color is updated dynamically (green for self, red for look).
        spawn_bar(
            commands,
            kind,
            GameplayUiBarLayer::Fill,
            HUD_BAR_X,
            y,
            z_fg,
            green,
        );
    }
}

/// Updates HUD stat bar fill widths and visibility (HP/Endurance/Mana).
pub(crate) fn run_gameplay_update_stat_bars(
    player_state: Res<PlayerState>,
    mut q: Query<(&GameplayUiBar, &mut Sprite, &mut Visibility)>,
) {
    let (blue, green, red) = ui_bar_colors();

    #[inline]
    fn scaled_width(numer: u32, denom: u32) -> f32 {
        if denom == 0 {
            return 0.0;
        }
        let w = ((numer as u64) * (BAR_SCALE_NUM as u64) / (denom as u64)).min(BAR_W_MAX as u64);
        w as f32
    }

    let pl = player_state.character_info();
    let look_mode = player_state.should_show_look();
    let look = player_state.look_target();

    let hp_max = pl.hp[5] as u32;
    let end_max = pl.end[5] as u32;
    let mana_max = pl.mana[5] as u32;

    let self_hp_cur = pl.a_hp.max(0) as u32;
    let self_end_cur = pl.a_end.max(0) as u32;
    let self_mana_cur = pl.a_mana.max(0) as u32;

    let (hp_bg, hp_fg, end_bg, end_fg, mana_bg, mana_fg, fill_color) = if look_mode {
        (
            scaled_width(look.hp(), hp_max),
            scaled_width(look.a_hp(), hp_max),
            scaled_width(look.end(), end_max),
            scaled_width(look.a_end(), end_max),
            scaled_width(look.mana(), mana_max),
            scaled_width(look.a_mana(), mana_max),
            red,
        )
    } else {
        (
            scaled_width(hp_max, hp_max),
            scaled_width(self_hp_cur, hp_max),
            scaled_width(end_max, end_max),
            scaled_width(self_end_cur, end_max),
            scaled_width(mana_max, mana_max),
            scaled_width(self_mana_cur, mana_max),
            green,
        )
    };

    for (bar, mut sprite, mut vis) in &mut q {
        let (w, color) = match (bar.kind, bar.layer) {
            (GameplayUiBarKind::Hitpoints, GameplayUiBarLayer::Background) => (hp_bg, blue),
            (GameplayUiBarKind::Hitpoints, GameplayUiBarLayer::Fill) => (hp_fg, fill_color),
            (GameplayUiBarKind::Endurance, GameplayUiBarLayer::Background) => (end_bg, blue),
            (GameplayUiBarKind::Endurance, GameplayUiBarLayer::Fill) => (end_fg, fill_color),
            (GameplayUiBarKind::Mana, GameplayUiBarLayer::Background) => (mana_bg, blue),
            (GameplayUiBarKind::Mana, GameplayUiBarLayer::Fill) => (mana_fg, fill_color),
        };

        sprite.color = color;
        sprite.custom_size = Some(Vec2::new(w.max(0.0), HUD_BAR_H));
        *vis = if w >= 1.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Handles UI toggle buttons and mode buttons (keyboard + mouse), including exit/reset.
pub(crate) fn run_gameplay_buttonbox_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<&Camera, With<Camera2d>>,
    net: Res<NetworkRuntime>,
    mut player_state: ResMut<PlayerState>,
    mut exit_state: ResMut<GameplayExitState>,
    mut q_boxes: Query<(&GameplayUiToggleBox, &mut Visibility), Without<GameplayUiModeBox>>,
    mut q_mode_boxes: Query<(&GameplayUiModeBox, &mut Visibility), Without<GameplayUiToggleBox>>,
) {
    // Reset (orig/main.c ESC): cmd(CL_CMD_RESET,0,0); show_shop=0; noshop=QSIZE*12; xmove=0;
    if keys.just_pressed(KeyCode::Escape) {
        net.send(ClientCommand::new_reset().to_bytes());
        player_state.close_shop();
    }

    // Keyboard shortcuts for mode buttons (orig client: F1/F2/F3).
    if keys.just_pressed(KeyCode::F1) {
        net.send(ClientCommand::new_mode(2).to_bytes());
    }
    if keys.just_pressed(KeyCode::F2) {
        net.send(ClientCommand::new_mode(1).to_bytes());
    }
    if keys.just_pressed(KeyCode::F3) {
        net.send(ClientCommand::new_mode(0).to_bytes());
    }

    // Exit (orig/inter.c button_command case 11: cmd_exit() -> cmd1(CL_CMD_EXIT,0)).
    if keys.just_pressed(KeyCode::F12) {
        cmd_exit(&mut exit_state, &net, &mut player_state);
    }

    // Keyboard shortcuts (orig/main.c): F4=show_proz, F6=hide, F7=show_names.
    if keys.just_pressed(KeyCode::F4) {
        let cur = player_state.player_data().show_proz;
        player_state.player_data_mut().show_proz = 1 - cur;
    }
    if keys.just_pressed(KeyCode::F6) {
        let cur = player_state.player_data().hide;
        player_state.player_data_mut().hide = 1 - cur;
    }
    if keys.just_pressed(KeyCode::F7) {
        let cur = player_state.player_data().show_names;
        player_state.player_data_mut().show_names = 1 - cur;
    }

    // Mouse buttonbox click areas (orig/inter.c::trans_button + mouse_buttonbox).
    if mouse.just_released(MouseButton::Left) {
        if let Some(game) = cursor_game_pos(&windows, &cameras) {
            // F1/F2/F3 mode buttons (nr=0..2): row1 col0..2
            let f1_x = BUTTONBOX_X + 0.0 * BUTTONBOX_STEP_X;
            let f1_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f1_x, f1_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                net.send(ClientCommand::new_mode(2).to_bytes());
            }
            let f2_x = BUTTONBOX_X + 1.0 * BUTTONBOX_STEP_X;
            let f2_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f2_x, f2_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                net.send(ClientCommand::new_mode(1).to_bytes());
            }
            let f3_x = BUTTONBOX_X + 2.0 * BUTTONBOX_STEP_X;
            let f3_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f3_x, f3_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                net.send(ClientCommand::new_mode(0).to_bytes());
            }

            // F4 (nr=3): row1 col3
            let f4_x = BUTTONBOX_X + 3.0 * BUTTONBOX_STEP_X;
            let f4_y = BUTTONBOX_Y_ROW1;
            if in_rect(game, f4_x, f4_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                let cur = player_state.player_data().show_proz;
                player_state.player_data_mut().show_proz = 1 - cur;
            }

            // F6 (nr=5): row2 col1
            let f6_x = BUTTONBOX_X + 1.0 * BUTTONBOX_STEP_X;
            let f6_y = BUTTONBOX_Y_ROW2;
            if in_rect(game, f6_x, f6_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                let cur = player_state.player_data().hide;
                player_state.player_data_mut().hide = 1 - cur;
            }

            // F7 (nr=6): row2 col2
            let f7_x = BUTTONBOX_X + 2.0 * BUTTONBOX_STEP_X;
            let f7_y = BUTTONBOX_Y_ROW2;
            if in_rect(game, f7_x, f7_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                let cur = player_state.player_data().show_names;
                player_state.player_data_mut().show_names = 1 - cur;
            }

            // F12 / EXIT (nr=11): row3 col3 ("bottom right").
            let f12_x = BUTTONBOX_X + 3.0 * BUTTONBOX_STEP_X;
            let f12_y = BUTTONBOX_Y_ROW3;
            if in_rect(game, f12_x, f12_y, BUTTONBOX_BUTTON_W, BUTTONBOX_BUTTON_H) {
                cmd_exit(&mut exit_state, &net, &mut player_state);
            }
        }
    }

    // Update indicator visibility (orig/engine.c dd_showbox calls).
    let pdata = player_state.player_data();
    for (boxc, mut vis) in &mut q_boxes {
        let enabled = match boxc.kind {
            GameplayToggleBoxKind::ShowProz => pdata.show_proz != 0,
            GameplayToggleBoxKind::ShowNames => pdata.show_names != 0,
            GameplayToggleBoxKind::Hide => pdata.hide != 0,
        };
        *vis = if enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let mode = player_state.character_info().mode;
    for (boxc, mut vis) in &mut q_mode_boxes {
        *vis = if mode == boxc.mode {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Updates HUD labels (stats, attributes, skills, hotbar labels, etc.).
pub(crate) fn run_gameplay_update_hud_labels(
    player_state: Res<PlayerState>,
    statbox: Res<GameplayStatboxState>,
    mut last_state_rev: Local<u64>,
    mut q: ParamSet<(
        Query<
            &mut BitmapText,
            (
                With<GameplayUiHitpointsLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiEnduranceLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiManaLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiMoneyLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiUpdateValue>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiWeaponValueLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiArmorValueLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
        Query<
            &mut BitmapText,
            (
                With<GameplayUiExperienceLabel>,
                Without<GameplayUiAttributeLabel>,
                Without<GameplayUiSkillLabel>,
                Without<GameplayUiRaiseStatText>,
                Without<GameplayUiAttributeAuxText>,
                Without<GameplayUiSkillAuxText>,
                Without<GameplayUiXButtonLabel>,
            ),
        >,
    )>,
    mut q_attrib: Query<
        (&GameplayUiAttributeLabel, &mut BitmapText),
        (
            With<GameplayUiAttributeLabel>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_attrib_aux: Query<
        (&GameplayUiAttributeAuxText, &mut BitmapText),
        (
            With<GameplayUiAttributeAuxText>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_skill: Query<
        (&GameplayUiSkillLabel, &mut BitmapText),
        (
            With<GameplayUiSkillLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_skill_aux: Query<
        (&GameplayUiSkillAuxText, &mut BitmapText),
        (
            With<GameplayUiSkillAuxText>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_raise_stats: Query<
        (&GameplayUiRaiseStatText, &mut BitmapText),
        (
            With<GameplayUiRaiseStatText>,
            Without<GameplayUiAttributeLabel>,
            Without<GameplayUiSkillLabel>,
            Without<GameplayUiAttributeAuxText>,
            Without<GameplayUiSkillAuxText>,
            Without<GameplayUiXButtonLabel>,
        ),
    >,
    mut q_xbuttons: Query<(&GameplayUiXButtonLabel, &mut BitmapText)>,
) {
    let rev = player_state.state_revision();
    if *last_state_rev == rev && !statbox.is_changed() {
        return;
    }
    *last_state_rev = rev;

    const HIGH_VAL: i32 = i32::MAX;

    #[inline]
    fn attrib_needed(pl: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        let max_v = pl.attrib[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = pl.attrib[n][3] as i32;
        let v64 = v as i64;
        ((v64 * v64 * v64) * (diff as i64) / 20).clamp(0, i32::MAX as i64) as i32
    }

    #[inline]
    fn skill_needed(pl: &mag_core::types::ClientPlayer, n: usize, v: i32) -> i32 {
        let max_v = pl.skill[n][2] as i32;
        if v >= max_v {
            return HIGH_VAL;
        }
        let diff = pl.skill[n][3] as i32;
        let v64 = v as i64;
        let cubic = ((v64 * v64 * v64) * (diff as i64) / 40).clamp(0, i32::MAX as i64) as i32;
        v.max(cubic)
    }

    let pl = player_state.character_info();

    // Skill hotbar (xbuttons) labels.
    for (slot, mut text) in &mut q_xbuttons {
        let btn = &player_state.player_data().skill_buttons[slot.index];
        let mut name = btn.name_str();
        if name.is_empty() {
            name = "-".to_string();
        }
        if text.text != name {
            text.text = name;
        }
    }

    // Hitpoints (current / max). Server already sends a_* as integer current values.
    if let Some(mut text) = q.p0().iter_mut().next() {
        let cur = pl.a_hp.max(0);
        let desired = format!("Hitpoints         {:3} {:3}", cur, pl.hp[5]);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Endurance (current / max)
    if let Some(mut text) = q.p1().iter_mut().next() {
        let cur = pl.a_end.max(0);
        let desired = format!("Endurance         {:3} {:3}", cur, pl.end[5]);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Mana (current / max)
    if let Some(mut text) = q.p2().iter_mut().next() {
        let cur = pl.a_mana.max(0);
        let desired = format!("Mana              {:3} {:3}", cur, pl.mana[5]);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Money (gold/100 and silver remainder)
    if let Some(mut text) = q.p3().iter_mut().next() {
        let desired = format!("Money  {:8}G {:2}S", pl.gold / 100, pl.gold % 100);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Update points remaining
    if let Some(mut text) = q.p4().iter_mut().next() {
        let desired = format!("{:7}", (pl.points - statbox.stat_points_used).max(0));
        if text.text != desired {
            text.text = desired;
        }
    }

    // Weapon value
    if let Some(mut text) = q.p5().iter_mut().next() {
        let desired = format!("Weapon value   {:10}", pl.weapon);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Armor value
    if let Some(mut text) = q.p6().iter_mut().next() {
        let desired = format!("Armor value    {:10}", pl.armor);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Experience (total points)
    if let Some(mut text) = q.p7().iter_mut().next() {
        let desired = format!("Experience     {:10}", pl.points_tot);
        if text.text != desired {
            text.text = desired;
        }
    }

    // Attributes (5 of them: Braveness, Willpower, Intuition, Agility, Strength)
    // Mirrors engine.c: value uses total[5] + stat_raised[n], and cost uses bare[0] + stat_raised[n].
    let stat_points_used: i32 = statbox.stat_points_used;
    let available_points: i32 = (pl.points - stat_points_used).max(0);
    let stat_raised_attrib: [i32; 5] = [
        statbox.stat_raised[0],
        statbox.stat_raised[1],
        statbox.stat_raised[2],
        statbox.stat_raised[3],
        statbox.stat_raised[4],
    ];

    for (attr_label, mut text) in &mut q_attrib {
        if attr_label.attrib_index < 5 {
            let name = ATTRIBUTE_NAMES[attr_label.attrib_index];
            let v_total = pl.attrib[attr_label.attrib_index][5] as i32
                + stat_raised_attrib[attr_label.attrib_index];
            let desired = format!("{:<16}  {:3}", name, v_total);
            if text.text != desired {
                text.text = desired;
            }
        }
    }

    for (cfg, mut text) in &mut q_attrib_aux {
        if cfg.attrib_index >= 5 {
            if !text.text.is_empty() {
                text.text.clear();
            }
            continue;
        }
        let v_bare = pl.attrib[cfg.attrib_index][0] as i32 + stat_raised_attrib[cfg.attrib_index];
        let need = attrib_needed(pl, cfg.attrib_index, v_bare);
        match cfg.col {
            GameplayUiRaiseStatColumn::Plus => {
                let desired = if need != HIGH_VAL && need <= available_points {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Minus => {
                let desired = if stat_raised_attrib[cfg.attrib_index] > 0 {
                    "-"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Cost => {
                if need != HIGH_VAL {
                    let desired = format!("{:7}", need);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            GameplayUiRaiseStatColumn::Value => {
                if !text.text.is_empty() {
                    text.text.clear();
                }
            }
        }
    }

    // Skills (10 visible at a time)
    let sorted_skills = build_sorted_skills(pl);

    let skill_pos = statbox.skill_pos;
    let mut rows: Vec<String> = Vec::with_capacity(10);
    let mut row_skill_ids: [Option<usize>; 10] = [None; 10];
    let mut stat_raised_skill_rows: [i32; 10] = [0; 10];
    for row in 0..10 {
        let skilltab_index = skill_pos + row;
        let Some(&skill_id) = sorted_skills.get(skilltab_index) else {
            rows.push(String::from("unused"));
            row_skill_ids[row] = None;
            continue;
        };

        let name = get_skill_name(skill_id);
        let is_unused = get_skill_sortkey(skill_id) == 'Z' || name.is_empty();
        if is_unused {
            rows.push(String::from("unused"));
            row_skill_ids[row] = None;
            continue;
        }

        // In engine.c, "unused" also covers unlearned skills (pl.skill[m][0]==0).
        if pl.skill[skill_id][0] == 0 {
            rows.push(String::from("unused"));
            row_skill_ids[row] = None;
            continue;
        }

        let raised = statbox
            .stat_raised
            .get(8 + skilltab_index)
            .copied()
            .unwrap_or(0);
        stat_raised_skill_rows[row] = raised;

        let skill_display = pl.skill[skill_id][5] as i32 + raised;
        rows.push(format!("{:<16}  {:3}", name, skill_display));
        row_skill_ids[row] = Some(skill_id);
    }

    for (skill_label, mut text) in &mut q_skill {
        let row = skill_label.skill_index;
        if row < rows.len() {
            let desired = rows[row].as_str();
            if text.text != desired {
                text.text.clear();
                text.text.push_str(desired);
            }
        } else if !text.text.is_empty() {
            text.text.clear();
        }
    }

    // Skill +/- and cost columns (mirrors engine.c).
    for (cfg, mut text) in &mut q_skill_aux {
        if cfg.row >= 10 {
            if !text.text.is_empty() {
                text.text.clear();
            }
            continue;
        }
        let Some(skill_id) = row_skill_ids[cfg.row] else {
            if !text.text.is_empty() {
                text.text.clear();
            }
            continue;
        };

        let v_bare = pl.skill[skill_id][0] as i32 + stat_raised_skill_rows[cfg.row];
        let need = skill_needed(pl, skill_id, v_bare);
        match cfg.col {
            GameplayUiRaiseStatColumn::Plus => {
                let desired = if need != HIGH_VAL && need <= available_points {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Minus => {
                let desired = if stat_raised_skill_rows[cfg.row] > 0 {
                    "-"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            GameplayUiRaiseStatColumn::Cost => {
                if need != HIGH_VAL {
                    let desired = format!("{:7}", need);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            GameplayUiRaiseStatColumn::Value => {
                if !text.text.is_empty() {
                    text.text.clear();
                }
            }
        }
    }

    // Raise-stat rows (Hitpoints/Endurance/Mana) with +, -, and cost columns.
    let available_points = available_points;
    let stat_raised_hp: i32 = statbox.stat_raised[5];
    let stat_raised_end: i32 = statbox.stat_raised[6];
    let stat_raised_mana: i32 = statbox.stat_raised[7];

    let hp_base = pl.hp[0] as i32 + stat_raised_hp;
    let end_base = pl.end[0] as i32 + stat_raised_end;
    let mana_base = pl.mana[0] as i32 + stat_raised_mana;

    let hp_needed = {
        let v = hp_needed(pl, hp_base);
        if v == HIGH_VAL {
            None
        } else {
            Some(v)
        }
    };
    let end_needed = {
        let v = end_needed(pl, end_base);
        if v == HIGH_VAL {
            None
        } else {
            Some(v)
        }
    };
    let mana_needed = {
        let v = mana_needed(pl, mana_base);
        if v == HIGH_VAL {
            None
        } else {
            Some(v)
        }
    };

    for (cfg, mut text) in &mut q_raise_stats {
        match (cfg.stat, cfg.col) {
            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Value) => {
                let desired = format!("Hitpoints         {:3}", (pl.hp[5] as i32) + stat_raised_hp);
                if text.text != desired {
                    text.text = desired;
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Value) => {
                let desired = format!(
                    "Endurance         {:3}",
                    (pl.end[5] as i32) + stat_raised_end
                );
                if text.text != desired {
                    text.text = desired;
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Value) => {
                let desired = format!(
                    "Mana              {:3}",
                    (pl.mana[5] as i32) + stat_raised_mana
                );
                if text.text != desired {
                    text.text = desired;
                }
            }

            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Plus) => {
                let desired = if hp_needed.is_some_and(|n| n <= available_points) {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Plus) => {
                let desired = if end_needed.is_some_and(|n| n <= available_points) {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Plus) => {
                let desired = if mana_needed.is_some_and(|n| n <= available_points) {
                    "+"
                } else {
                    ""
                };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }

            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Minus) => {
                let desired = if stat_raised_hp > 0 { "-" } else { "" };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Minus) => {
                let desired = if stat_raised_end > 0 { "-" } else { "" };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Minus) => {
                let desired = if stat_raised_mana > 0 { "-" } else { "" };
                if text.text != desired {
                    text.text.clear();
                    text.text.push_str(desired);
                }
            }

            (GameplayUiRaiseStat::Hitpoints, GameplayUiRaiseStatColumn::Cost) => {
                if let Some(n) = hp_needed {
                    let desired = format!("{:7}", n);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            (GameplayUiRaiseStat::Endurance, GameplayUiRaiseStatColumn::Cost) => {
                if let Some(n) = end_needed {
                    let desired = format!("{:7}", n);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
            (GameplayUiRaiseStat::Mana, GameplayUiRaiseStatColumn::Cost) => {
                if let Some(n) = mana_needed {
                    let desired = format!("{:7}", n);
                    if text.text != desired {
                        text.text = desired;
                    }
                } else if !text.text.is_empty() {
                    text.text.clear();
                }
            }
        }
    }
}
