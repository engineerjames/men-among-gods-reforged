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

use crate::{
    map::GameMap,
    network::server_commands::{ServerCommand, ServerCommandData},
    types::save_file::SaveFile,
};
use crate::{
    network::server_commands::ServerCommandType,
    types::{log_message::LogMessage, look::Look, player_data::PlayerData},
};

#[allow(dead_code)]
#[derive(Resource)]
pub struct PlayerState {
    map: GameMap,
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

    // Mirrors engine.c's `looks[]` name cache (nr -> {id,name}). Used for show_names/show_proz.
    look_names: Vec<Option<LookNameEntry>>,
    pending_log: String,
    moa_file_data: SaveFile,
    server_version: u32,
    load_percentage: u32,
    unique1: u32,
    unique2: u32,

    // Server-provided ctick (0..19), sent in ServerCommandData::Tick.
    // Using this keeps SPEEDTAB-based animations perfectly in-phase with the server.
    server_ctick: u8,
    server_ctick_pending: bool,
    local_ctick: u8,
}

#[derive(Clone, Debug)]
struct LookNameEntry {
    id: u16,
    name: String,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            map: GameMap::default(),
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

            look_names: Vec::new(),

            pending_log: String::new(),

            moa_file_data: SaveFile::default(),
            server_version: 0,
            load_percentage: 0,
            unique1: 0,
            unique2: 0,

            server_ctick: 0,
            server_ctick_pending: false,
            local_ctick: 0,
        }
    }
}

impl PlayerState {
    #[allow(dead_code)]
    pub fn map(&self) -> &GameMap {
        &self.map
    }

    pub fn map_mut(&mut self) -> &mut GameMap {
        &mut self.map
    }

    pub fn character_info(&self) -> &ClientPlayer {
        &self.character_info
    }

    pub fn should_show_shop(&self) -> bool {
        self.should_show_shop
    }

    pub fn shop_target(&self) -> &Look {
        &self.shop_target
    }

    pub fn player_data(&self) -> &PlayerData {
        &self.player_info
    }

    pub fn player_data_mut(&mut self) -> &mut PlayerData {
        &mut self.player_info
    }

    pub fn lookup_name(&self, nr: u16, id: u16) -> Option<&str> {
        self.look_names
            .get(nr as usize)
            .and_then(|e| e.as_ref())
            .filter(|e| e.id == id)
            .map(|e| e.name.as_str())
    }

    fn set_known_name(&mut self, nr: u16, id: u16, name: &str) {
        let idx = nr as usize;
        if self.look_names.len() <= idx {
            self.look_names.resize_with(idx + 1, || None);
        }
        self.look_names[idx] = Some(LookNameEntry {
            id,
            name: name.to_string(),
        });
    }

    pub fn local_ctick(&self) -> u8 {
        self.local_ctick
    }

    pub fn on_tick_packet(&mut self, client_ticker: u32) {
        let _ = client_ticker;
        if self.server_ctick_pending {
            self.local_ctick = self.server_ctick.min(19);
            self.server_ctick_pending = false;
        } else {
            self.local_ctick = (self.local_ctick + 1) % 20;
        }
    }

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

    /// Inserts extra `\n` so log text won't run off the end of the screen.
    ///
    /// This matches the original client's behavior (wrap width `XS=49`) by preferring
    /// breaking on spaces, and falling back to a hard cut when needed.
    pub fn wrap_log_text(text: &str, max_cols: usize) -> String {
        let max_cols = max_cols.max(2);
        let wrap_at = max_cols.saturating_sub(1);

        let mut out = String::with_capacity(text.len() + text.len() / wrap_at.max(1));

        for raw in text.split('\n') {
            let mut line = raw.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }

            while line.len() > wrap_at {
                let mut cut = wrap_at;
                if cut >= line.len() {
                    break;
                }

                if let Some(space) = line[..cut].rfind(' ') {
                    if space > 0 {
                        cut = space;
                    }
                }

                let head = line[..cut].trim_end();
                if !head.is_empty() {
                    out.push_str(head);
                    out.push('\n');
                }

                line = line[cut..].trim_start();
            }

            if !line.is_empty() {
                out.push_str(line);
                out.push('\n');
            }
        }

        out.pop();
        out
    }

    pub fn log_message(&self, index: usize) -> Option<&LogMessage> {
        self.message_log.get(index)
    }

    pub fn tlog(&mut self, font: u8, text: impl AsRef<str>) {
        const XS: usize = 49; // matches orig engine.c "XS" (line wrap width)

        let wrapped = Self::wrap_log_text(text.as_ref(), XS);
        for line in wrapped.split('\n') {
            let line = line.trim_end_matches('\r');
            if !line.is_empty() {
                self.push_log_message(line.to_string(), font);
            }
        }
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
            self.tlog(font, line);
            self.pending_log.drain(..=idx);
        }
    }

    pub fn update_from_server_command(&mut self, command: &ServerCommand) {
        // Check for command specific processing, then update state based on structured data.
        match command.header {
            ServerCommandType::ScrollDown => {
                self.map.scroll_down();
            }
            ServerCommandType::ScrollUp => {
                self.map.scroll_up();
            }
            ServerCommandType::ScrollLeft => {
                self.map.scroll_left();
            }
            ServerCommandType::ScrollRight => {
                self.map.scroll_right();
            }
            ServerCommandType::ScrollLeftDown => {
                self.map.scroll_left_down();
            }
            ServerCommandType::ScrollLeftUp => {
                self.map.scroll_left_up();
            }
            ServerCommandType::ScrollRightDown => {
                self.map.scroll_right_down();
            }
            ServerCommandType::ScrollRightUp => {
                self.map.scroll_right_up();
            }
            _ => {}
        }

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
            ServerCommandData::Tick { ctick } => {
                self.server_ctick = *ctick;
                self.server_ctick_pending = true;
            }
            ServerCommandData::SetOrigin { x, y } => {
                self.map.set_origin(*x, *y);
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

                // engine.c: add_look(tmplook.nr, tmplook.name, tmplook.id)
                // Our Look3 packet sets nr/id; Look5 provides the name.
                let nr = self.look_target.nr();
                let id = self.look_target.id();
                if !name.is_empty() {
                    self.set_known_name(nr, id, name);
                }

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
            ServerCommandData::SetMap {
                off,
                absolute_tile_index,
                flags: _flags,
                ba_sprite,
                flags1,
                flags2,
                it_sprite,
                it_status,
                ch_sprite,
                ch_status,
                ch_stat_off,
                ch_nr,
                ch_id,
                ch_speed,
                ch_proz,
            } => {
                self.map.apply_set_map(
                    *off,
                    *absolute_tile_index,
                    *ba_sprite,
                    *flags1,
                    *flags2,
                    *it_sprite,
                    *it_status,
                    *ch_sprite,
                    *ch_status,
                    *ch_stat_off,
                    *ch_nr,
                    *ch_id,
                    *ch_speed,
                    *ch_proz,
                );
            }

            ServerCommandData::SetMap3 {
                start_index,
                base_light,
                packed,
            } => {
                self.map.apply_set_map3(*start_index, *base_light, packed);
            }

            ServerCommandData::Exit { reason } => {
                // TODO: Handle exit reason codes more gracefully.
                self.tlog(3, format!("Server requested exit (reason={reason})"));
            }
            _ => {
                // Keep this quiet for now; there are many SV_* packets not yet wired into gameplay.
                log::debug!("PlayerState ignoring server command: {:?}", command.header);
            }
        }
    }
}
