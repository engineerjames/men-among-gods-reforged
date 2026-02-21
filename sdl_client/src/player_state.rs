use mag_core::{
    circular_buffer::CircularBuffer,
    constants::{MAX_SPEEDTAB_INDEX, TICKS},
    types::ClientPlayer,
};

use crate::{
    game_map::GameMap,
    helpers::exit_reason_string,
    network::server_commands::{ServerCommand, ServerCommandData, ServerCommandType},
    types::{log_message::LogMessage, look::Look, player_data::PlayerData, save_file::SaveFile},
};

pub struct PlayerState {
    map: GameMap,
    look_target: Look,
    shop_target: Look,
    player_info: PlayerData,
    message_log: CircularBuffer<LogMessage>,
    should_show_look: bool,
    should_show_shop: bool,
    shop_refresh_requested: bool,
    look_timer: f32,
    character_info: ClientPlayer,

    selected_char: u16,
    selected_char_id: u16,

    look_names: Vec<Option<LookNameEntry>>,
    pending_log: String,
    moa_file_data: SaveFile,
    server_version: u32,
    load_percentage: u32,
    unique1: u32,
    unique2: u32,

    server_ctick: u8,
    server_ctick_pending: bool,
    local_ctick: u8,

    state_revision: u64,

    exit_requested_reason: Option<u32>,
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
            message_log: CircularBuffer::new(300),
            should_show_look: false,
            should_show_shop: false,
            shop_refresh_requested: false,
            look_timer: 0.0,
            character_info: ClientPlayer::default(),

            selected_char: 0,
            selected_char_id: 0,

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

            state_revision: 0,

            exit_requested_reason: None,
        }
    }
}

impl PlayerState {
    pub fn log_len(&self) -> usize {
        self.message_log.len()
    }

    pub fn take_exit_requested_reason(&mut self) -> Option<u32> {
        self.exit_requested_reason.take()
    }

    pub fn state_revision(&self) -> u64 {
        self.state_revision
    }

    pub fn mark_dirty(&mut self) {
        self.state_revision = self.state_revision.wrapping_add(1);
    }

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

    pub fn close_shop(&mut self) {
        if self.should_show_shop {
            self.should_show_shop = false;
            self.shop_refresh_requested = false;
            self.mark_dirty();
        }
    }

    pub fn request_shop_refresh(&mut self) {
        self.shop_refresh_requested = true;
    }

    pub fn take_shop_refresh_requested(&mut self) -> bool {
        let was = self.shop_refresh_requested;
        self.shop_refresh_requested = false;
        was
    }

    pub fn should_show_look(&self) -> bool {
        self.should_show_look
    }

    pub fn close_look(&mut self) {
        if self.should_show_look {
            self.should_show_look = false;
            self.look_timer = 0.0;
            self.mark_dirty();
        }
    }

    pub fn look_target(&self) -> &Look {
        &self.look_target
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

    pub fn save_file(&self) -> &SaveFile {
        &self.moa_file_data
    }

    pub fn save_file_mut(&mut self) -> &mut SaveFile {
        &mut self.moa_file_data
    }

    pub fn set_character_from_file(&mut self, save_file: SaveFile, player_data: PlayerData) {
        self.moa_file_data = save_file;
        self.player_info = player_data;
        self.mark_dirty();
    }

    pub fn lookup_name(&self, nr: u16, id: u16) -> Option<&str> {
        self.look_names
            .get(nr as usize)
            .and_then(|e| e.as_ref())
            .filter(|e| id == 0 || e.id == id)
            .map(|e| e.name.as_str())
    }

    pub fn selected_char(&self) -> u16 {
        self.selected_char
    }

    pub fn selected_char_id(&self) -> u16 {
        self.selected_char_id
    }

    pub fn set_selected_char_with_id(&mut self, selected_char: u16, selected_char_id: u16) {
        self.selected_char = selected_char;
        self.selected_char_id = selected_char_id;
    }

    pub fn clear_selected_char(&mut self) {
        self.selected_char = 0;
        self.selected_char_id = 0;
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

        if self.should_show_look {
            if self.look_timer > 0.0 {
                self.look_timer -= 1.0;
            }
            if self.look_timer <= 0.0 {
                self.close_look();
            }
        }

        if self.server_ctick_pending {
            self.local_ctick = self.server_ctick.min(MAX_SPEEDTAB_INDEX as u8);
            self.server_ctick_pending = false;
        } else {
            self.local_ctick = (self.local_ctick + 1) % (MAX_SPEEDTAB_INDEX as u8 + 1);
        }

        crate::legacy_engine::engine_tick(self, client_ticker, self.local_ctick as usize);
    }

    fn log_color_from_font(font: u8) -> crate::types::log_message::LogMessageColor {
        use crate::types::log_message::LogMessageColor;
        match font {
            0 => LogMessageColor::Red,
            1 => LogMessageColor::Yellow,
            2 => LogMessageColor::Green,
            3 => LogMessageColor::Blue,
            _ => LogMessageColor::Yellow,
        }
    }

    fn push_log_message(&mut self, text: String, font: u8) {
        let msg = LogMessage {
            message: text,
            color: Self::log_color_from_font(font),
        };
        self.message_log.push(msg);
    }

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
        const XS: usize = 49;

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
        self.character_info.name[offset..end].fill(0);

        let bytes = chunk.as_bytes();
        let n = std::cmp::min(bytes.len(), end - offset);
        self.moa_file_data.name[offset..offset + n].copy_from_slice(&bytes[..n]);
        self.character_info.name[offset..offset + n].copy_from_slice(&bytes[..n]);
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
        match command.header {
            ServerCommandType::ScrollDown => {
                self.map.scroll_down();
                return;
            }
            ServerCommandType::ScrollUp => {
                self.map.scroll_up();
                return;
            }
            ServerCommandType::ScrollLeft => {
                self.map.scroll_left();
                return;
            }
            ServerCommandType::ScrollRight => {
                self.map.scroll_right();
                return;
            }
            ServerCommandType::ScrollLeftDown => {
                self.map.scroll_left_down();
                return;
            }
            ServerCommandType::ScrollLeftUp => {
                self.map.scroll_left_up();
                return;
            }
            ServerCommandType::ScrollRightDown => {
                self.map.scroll_right_down();
                return;
            }
            ServerCommandType::ScrollRightUp => {
                self.map.scroll_right_up();
                return;
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
                self.tlog(
                    3,
                    format!(
                        "Server requested exit (reason={})",
                        exit_reason_string(*reason)
                    ),
                );
                self.exit_requested_reason = Some(*reason);
            }
            _ => {
                log::debug!("PlayerState ignoring server command: {:?}", command.header);
            }
        }

        self.state_revision = self.state_revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_log_text_breaks_on_space() {
        let wrapped = PlayerState::wrap_log_text("hello world", 10);
        assert_eq!(wrapped, "hello\nworld");
    }

    #[test]
    fn wrap_log_text_hard_cuts_long_words() {
        let wrapped = PlayerState::wrap_log_text("abcdefghijk", 6);
        assert_eq!(wrapped, "abcde\nfghij\nk");
    }

    #[test]
    fn log_color_from_font_matches_expected_mapping() {
        use crate::types::log_message::LogMessageColor;
        assert!(matches!(
            PlayerState::log_color_from_font(0),
            LogMessageColor::Red
        ));
        assert!(matches!(
            PlayerState::log_color_from_font(1),
            LogMessageColor::Yellow
        ));
        assert!(matches!(
            PlayerState::log_color_from_font(2),
            LogMessageColor::Green
        ));
        assert!(matches!(
            PlayerState::log_color_from_font(3),
            LogMessageColor::Blue
        ));
        assert!(matches!(
            PlayerState::log_color_from_font(99),
            LogMessageColor::Yellow
        ));
    }

    #[test]
    fn lookup_name_requires_matching_id() {
        let mut ps = PlayerState::default();
        ps.set_known_name(5, 42, "Bob");
        assert_eq!(ps.lookup_name(5, 42), Some("Bob"));
        assert_eq!(ps.lookup_name(5, 43), None);
        assert_eq!(ps.lookup_name(6, 42), None);
    }

    #[test]
    fn tlog_adds_message_lines() {
        let mut ps = PlayerState::default();
        ps.tlog(0, "hello world");
        let msg = ps.log_message(0).expect("expected first log message");
        assert_eq!(msg.message, "hello world");
    }
}
