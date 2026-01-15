use bevy::ecs::resource::Resource;
use mag_core::{
    circular_buffer::CircularBuffer,
    constants::TICKS,
    types::{
        skilltab::{SkillTab, SKILLTAB},
        ClientPlayer,
    },
};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{log_message::LogMessage, look::Look, player_data::PlayerData};
use crate::{
    network::server_commands::{ServerCommand, ServerCommandData},
    types::save_file::SaveFile,
};

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
    character_info: ClientPlayer,
    pending_log: String,
    moa_file_data: SaveFile,
    server_version: u32,
    load_percentage: u32,
    unique1: u32,
    unique2: u32,
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
            character_info: ClientPlayer::default(),

            pending_log: String::new(),

            moa_file_data: SaveFile::default(),
            server_version: 0,
            load_percentage: 0,
            unique1: 0,
            unique2: 0,
        }
    }
}

impl PlayerState {
    fn now_unix_seconds() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    fn log_color_from_font(font: u8) -> crate::types::log_message::LogMessageColor {
        use crate::types::log_message::LogMessageColor;
        match font {
            1 => LogMessageColor::Green,
            2 => LogMessageColor::Blue,
            3 => LogMessageColor::Red,
            _ => LogMessageColor::Yellow,
        }
    }

    fn push_log_message(&mut self, text: String, font: u8) {
        let msg = LogMessage {
            timestamp: Self::now_unix_seconds(),
            message: text,
            color: Self::log_color_from_font(font),
        };
        self.message_log.push(msg);
    }

    fn write_name_chunk(&mut self, offset: usize, max_len: usize, chunk: &str) {
        if offset >= self.moa_file_data.name.len() {
            return;
        }
        let end = std::cmp::min(offset + max_len, self.moa_file_data.name.len());
        self.moa_file_data.name[offset..end].fill(0);

        let bytes = chunk.as_bytes();
        let n = std::cmp::min(bytes.len(), end - offset);
        self.moa_file_data.name[offset..offset + n].copy_from_slice(&bytes[..n]);
    }

    fn handle_log_chunk(&mut self, font: u8, chunk: &str) {
        if self.pending_log.len() > 1024 {
            self.pending_log.clear();
        }

        self.pending_log.push_str(chunk);

        while let Some(idx) = self.pending_log.find('\n') {
            let line = self.pending_log[..idx].to_string();
            self.push_log_message(line, font);
            self.pending_log.drain(..=idx);
        }
    }

    pub fn update_from_server_command(&mut self, command: &ServerCommand) {
        match &command.structured_data {
            ServerCommandData::NewPlayer {
                player_id,
                pass1,
                pass2,
                server_version,
            } => {
                self.moa_file_data.usnr = *player_id;
                self.moa_file_data.pass1 = *pass1;
                self.moa_file_data.pass2 = *pass2;
                self.server_version = *server_version;
            }
            ServerCommandData::LoginOk { server_version } => {
                self.server_version = *server_version;
            }
            ServerCommandData::SetCharName1 { chunk } => {
                self.write_name_chunk(0, 15, chunk);
            }
            ServerCommandData::SetCharName2 { chunk } => {
                self.write_name_chunk(15, 15, chunk);
            }
            ServerCommandData::SetCharName3 { chunk, race } => {
                self.write_name_chunk(30, 10, chunk);
                self.moa_file_data.race = (*race).try_into().unwrap_or(0);
            }
            ServerCommandData::SetCharMode { mode } => {
                self.character_info.mode = *mode as i32;
            }
            ServerCommandData::SetCharAttrib { index, values } => {
                let idx = *index as usize;
                if idx < self.character_info.attrib.len() {
                    self.character_info.attrib[idx] = *values;
                }
            }
            ServerCommandData::SetCharSkill { index, values } => {
                let idx = *index as usize;
                if idx < self.character_info.skill.len() {
                    self.character_info.skill[idx] = *values;
                }
            }
            ServerCommandData::SetCharHp { values } => {
                self.character_info.hp = *values;
            }
            ServerCommandData::SetCharEndur { values } => {
                self.character_info.end = (*values).map(|v| v.max(0) as u16);
            }
            ServerCommandData::SetCharMana { values } => {
                self.character_info.mana = (*values).map(|v| v.max(0) as u16);
            }
            ServerCommandData::SetCharAHP { value } => {
                self.character_info.a_hp = *value as i32;
            }
            ServerCommandData::SetCharAEnd { value } => {
                self.character_info.a_end = *value as i32;
            }
            ServerCommandData::SetCharAMana { value } => {
                self.character_info.a_mana = *value as i32;
            }
            ServerCommandData::SetCharDir { dir } => {
                self.character_info.dir = *dir as i32;
            }
            ServerCommandData::SetCharPts {
                points,
                points_total,
                kindred,
            } => {
                self.character_info.points = *points as i32;
                self.character_info.points_tot = *points_total as i32;
                self.character_info.kindred = *kindred as i32;
            }
            ServerCommandData::SetCharGold {
                gold,
                armor,
                weapon,
            } => {
                self.character_info.gold = *gold as i32;
                self.character_info.armor = *armor as i32;
                self.character_info.weapon = *weapon as i32;
            }
            ServerCommandData::SetCharItem {
                index,
                item,
                item_p,
            } => {
                let idx = *index as usize;
                if idx < self.character_info.item.len() {
                    self.character_info.item[idx] = *item as i32;
                    self.character_info.item_p[idx] = *item_p as i32;
                }
            }
            ServerCommandData::SetCharWorn {
                index,
                worn,
                worn_p,
            } => {
                let idx = *index as usize;
                if idx < self.character_info.worn.len() {
                    self.character_info.worn[idx] = *worn as i32;
                    self.character_info.worn_p[idx] = *worn_p as i32;
                }
            }
            ServerCommandData::SetCharSpell {
                index,
                spell,
                active,
            } => {
                let idx = *index as usize;
                if idx < self.character_info.spell.len() {
                    self.character_info.spell[idx] = *spell as i32;
                    self.character_info.active[idx] = *active as i8;
                }
            }
            ServerCommandData::SetCharObj { citem, citem_p } => {
                self.character_info.citem = *citem as i32;
                self.character_info.citem_p = *citem_p as i32;
            }
            // Ticks do not modify state directly.
            ServerCommandData::Tick { .. } => {}
            ServerCommandData::SetOrigin { x, y } => {
                // TODO: We need a map abstraction to handle and draw sprites and handle
                // animation states, etc.
                // self.character_info.origin_x = *x;
                // self.character_info.origin_y = *y;
            }
            ServerCommandData::SetTarget {
                attack_cn,
                goto_x,
                goto_y,
                misc_action,
                misc_target1,
                misc_target2,
            } => {
                self.character_info.attack_cn = *attack_cn as i32;
                self.character_info.goto_x = *goto_x as i32;
                self.character_info.goto_y = *goto_y as i32;
                self.character_info.misc_action = *misc_action as i32;
                self.character_info.misc_target1 = *misc_target1 as i32;
                self.character_info.misc_target2 = *misc_target2 as i32;
            }
            ServerCommandData::Load { load } => {
                self.load_percentage = *load;
            }
            ServerCommandData::Unique { unique1, unique2 } => {
                self.unique1 = *unique1;
                self.unique2 = *unique2;
            }
            ServerCommandData::Log { font, chunk } => {
                self.handle_log_chunk(*font, chunk);
            }
            ServerCommandData::Mod1 { text }
            | ServerCommandData::Mod2 { text }
            | ServerCommandData::Mod3 { text }
            | ServerCommandData::Mod4 { text }
            | ServerCommandData::Mod5 { text }
            | ServerCommandData::Mod6 { text }
            | ServerCommandData::Mod7 { text }
            | ServerCommandData::Mod8 { text } => {
                // MOTD chunks (15 bytes each in original client)
                self.handle_log_chunk(0, text);
            }
            ServerCommandData::Look1 {
                worn0,
                worn2,
                worn3,
                worn5,
                worn6,
                worn7,
                worn8,
                autoflag,
            } => {
                self.look_target.set_worn(0, *worn0);
                self.look_target.set_worn(2, *worn2);
                self.look_target.set_worn(3, *worn3);
                self.look_target.set_worn(5, *worn5);
                self.look_target.set_worn(6, *worn6);
                self.look_target.set_worn(7, *worn7);
                self.look_target.set_worn(8, *worn8);
                self.look_target.set_autoflag(*autoflag);
            }
            ServerCommandData::Look2 {
                worn9,
                sprite,
                points,
                hp,
                worn10,
            } => {
                self.look_target.set_worn(9, *worn9);
                self.look_target.set_sprite(*sprite);
                self.look_target.set_points(*points);
                self.look_target.set_hp(*hp);
                self.look_target.set_worn(10, *worn10);
            }
            ServerCommandData::Look3 {
                end,
                a_hp,
                a_end,
                nr,
                id,
                mana,
                a_mana,
            } => {
                self.look_target.set_end(*end);
                self.look_target.set_a_hp(*a_hp);
                self.look_target.set_a_end(*a_end);
                self.look_target.set_nr(*nr);
                self.look_target.set_id(*id);
                self.look_target.set_mana(*mana);
                self.look_target.set_a_mana(*a_mana);
            }
            ServerCommandData::Look4 {
                worn1,
                worn4,
                extended,
                pl_price,
                worn11,
                worn12,
                worn13,
            } => {
                self.look_target.set_worn(1, *worn1);
                self.look_target.set_worn(4, *worn4);
                self.look_target.set_extended(*extended);
                self.look_target.set_pl_price(*pl_price);
                self.look_target.set_worn(11, *worn11);
                self.look_target.set_worn(12, *worn12);
                self.look_target.set_worn(13, *worn13);
            }
            ServerCommandData::Look5 { name } => {
                self.look_target.set_name(name);

                if !self.look_target.is_extended() && self.look_target.autoflag() == 0 {
                    self.should_show_look = true;
                    self.look_timer = (10 * TICKS) as f32;
                }
            }
            ServerCommandData::Look6 { start, entries } => {
                for e in entries {
                    self.look_target.set_shop_entry(e.index, e.item, e.price);
                }

                // In the original client this triggers once the final chunk (start==60) arrives.
                if start.saturating_add(2) >= 62 {
                    self.should_show_shop = true;
                    self.shop_target = self.look_target;
                }
            }
            ServerCommandData::Exit { reason } => {
                // TODO: Handle exit reason codes more gracefully.
                self.push_log_message(format!("Server requested exit (reason={reason})"), 3);
            }
            _ => {
                // Keep this quiet for now; there are many SV_* packets not yet wired into gameplay.
                log::debug!("PlayerState ignoring server command: {:?}", command.header);
            }
        }
    }
}
