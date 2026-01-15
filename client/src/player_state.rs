use bevy::ecs::resource::Resource;
use mag_core::{
    circular_buffer::CircularBuffer,
    types::skilltab::{SkillTab, SKILLTAB},
};

use crate::types::{log_message::LogMessage, look::Look, player_data::PlayerData};

#[allow(dead_code)]
#[derive(Resource)]
pub struct PlayerState {
    look_target: Look,
    shop_target: Look,
    player_info: PlayerData,
    skills_list: &'static [SkillTab],
    message_log: CircularBuffer<LogMessage>,
    player_sprite_index: usize,
    should_show_look: bool,
    should_show_shop: bool,
    look_timer: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            look_target: Look::default(),
            shop_target: Look::default(),
            player_info: PlayerData::default(),
            skills_list: &SKILLTAB,
            message_log: CircularBuffer::new(300), // TODO: Customize this?
            player_sprite_index: 0,
            should_show_look: false,
            should_show_shop: false,
            look_timer: 0.0,
        }
    }
}
