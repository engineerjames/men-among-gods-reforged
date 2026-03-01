//! Normalized schema: array fields broken out into relational sub-tables.
//!
//! This is the more relational schema — each array-typed field on a character
//! or item becomes a separate child table with a foreign key and slot/index
//! column. This eliminates BLOBs in favor of proper columns at the cost of
//! additional JOINs and row counts.

use super::{BenchSchema, CharacterRow, CharacterSlots, ItemRow, ViewportTile};
use anyhow::Result;
use mag_core::types::{Character, Effect, Global, Item, Map};
use rusqlite::{params, Connection, Transaction};

pub struct NormalizedSchema;

impl NormalizedSchema {
    pub fn new() -> Self {
        Self
    }

    fn populate_characters(tx: &Transaction, characters: &[Character]) -> Result<()> {
        let mut char_stmt = tx.prepare(
            "INSERT INTO characters (
                id, used, name, reference, description, kindred, player,
                sprite, sound, flags, alignment,
                temple_x, temple_y, tavern_x, tavern_y, temp,
                weapon_bonus, armor_bonus,
                a_hp, a_end, a_mana,
                light, mode, speed,
                points, points_tot, armor, weapon,
                x, y, tox, toy, frx, fry, status, status2, dir,
                gold, citem,
                creation_date, login_date, current_online_time, total_online_time,
                attack_cn, skill_nr, goto_x, goto_y, use_nr,
                misc_action, misc_target1, misc_target2,
                stunned, speed_mod, depot_cost, luck
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16,
                ?17, ?18,
                ?19, ?20, ?21,
                ?22, ?23, ?24,
                ?25, ?26, ?27, ?28,
                ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37,
                ?38, ?39,
                ?40, ?41, ?42, ?43,
                ?44, ?45, ?46, ?47, ?48,
                ?49, ?50, ?51,
                ?52, ?53, ?54, ?55
            )",
        )?;

        let mut attrib_stmt = tx.prepare(
            "INSERT INTO character_attribs (char_id, attr_idx, v0, v1, v2, v3, v4, v5)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        let mut hp_stmt =
            tx.prepare("INSERT INTO character_hp (char_id, idx, value) VALUES (?1, ?2, ?3)")?;
        let mut end_stmt =
            tx.prepare("INSERT INTO character_end (char_id, idx, value) VALUES (?1, ?2, ?3)")?;
        let mut mana_stmt =
            tx.prepare("INSERT INTO character_mana (char_id, idx, value) VALUES (?1, ?2, ?3)")?;
        let mut skill_stmt = tx.prepare(
            "INSERT INTO character_skills (char_id, skill_id, v0, v1, v2, v3, v4, v5)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        let mut item_slot_stmt =
            tx.prepare("INSERT INTO character_items (char_id, slot, item_id) VALUES (?1, ?2, ?3)")?;
        let mut worn_stmt =
            tx.prepare("INSERT INTO character_worn (char_id, slot, item_id) VALUES (?1, ?2, ?3)")?;
        let mut spell_stmt = tx
            .prepare("INSERT INTO character_spells (char_id, slot, item_id) VALUES (?1, ?2, ?3)")?;
        let mut depot_stmt =
            tx.prepare("INSERT INTO character_depot (char_id, slot, item_id) VALUES (?1, ?2, ?3)")?;
        let mut data_stmt =
            tx.prepare("INSERT INTO character_data (char_id, idx, value) VALUES (?1, ?2, ?3)")?;

        for (i, ch) in characters.iter().enumerate() {
            let id = i as u32;
            char_stmt.execute(params![
                id,
                ch.used,
                &ch.name[..],
                &ch.reference[..],
                &ch.description[..],
                ch.kindred,
                ch.player,
                ch.sprite,
                ch.sound,
                ch.flags as i64,
                ch.alignment,
                ch.temple_x,
                ch.temple_y,
                ch.tavern_x,
                ch.tavern_y,
                ch.temp,
                ch.weapon_bonus,
                ch.armor_bonus,
                ch.a_hp,
                ch.a_end,
                ch.a_mana,
                ch.light,
                ch.mode,
                ch.speed,
                ch.points,
                ch.points_tot,
                ch.armor,
                ch.weapon,
                ch.x,
                ch.y,
                ch.tox,
                ch.toy,
                ch.frx,
                ch.fry,
                ch.status,
                ch.status2,
                ch.dir,
                ch.gold,
                ch.citem,
                ch.creation_date,
                ch.login_date,
                ch.current_online_time,
                ch.total_online_time,
                ch.attack_cn,
                ch.skill_nr,
                ch.goto_x,
                ch.goto_y,
                ch.use_nr,
                ch.misc_action,
                ch.misc_target1,
                ch.misc_target2,
                ch.stunned,
                ch.speed_mod,
                ch.depot_cost,
                ch.luck,
            ])?;

            // Only populate sub-tables for non-empty characters to save time
            if ch.used == 0 {
                continue;
            }

            for (ai, attr) in ch.attrib.iter().enumerate() {
                attrib_stmt.execute(params![
                    id, ai as u32, attr[0], attr[1], attr[2], attr[3], attr[4], attr[5]
                ])?;
            }
            for (idx, &v) in ch.hp.iter().enumerate() {
                hp_stmt.execute(params![id, idx as u32, v])?;
            }
            for (idx, &v) in ch.end.iter().enumerate() {
                end_stmt.execute(params![id, idx as u32, v])?;
            }
            for (idx, &v) in ch.mana.iter().enumerate() {
                mana_stmt.execute(params![id, idx as u32, v])?;
            }
            for (si, sk) in ch.skill.iter().enumerate() {
                skill_stmt.execute(params![
                    id, si as u32, sk[0], sk[1], sk[2], sk[3], sk[4], sk[5]
                ])?;
            }
            for (slot, &item_id) in ch.item.iter().enumerate() {
                if item_id != 0 {
                    item_slot_stmt.execute(params![id, slot as u32, item_id])?;
                }
            }
            for (slot, &item_id) in ch.worn.iter().enumerate() {
                if item_id != 0 {
                    worn_stmt.execute(params![id, slot as u32, item_id])?;
                }
            }
            for (slot, &item_id) in ch.spell.iter().enumerate() {
                if item_id != 0 {
                    spell_stmt.execute(params![id, slot as u32, item_id])?;
                }
            }
            for (slot, &item_id) in ch.depot.iter().enumerate() {
                if item_id != 0 {
                    depot_stmt.execute(params![id, slot as u32, item_id])?;
                }
            }
            for (idx, &v) in ch.data.iter().enumerate() {
                if v != 0 {
                    data_stmt.execute(params![id, idx as u32, v])?;
                }
            }
        }
        Ok(())
    }

    fn populate_items(tx: &Transaction, items: &[Item]) -> Result<()> {
        let mut item_stmt = tx.prepare(
            "INSERT INTO items (
                id, used, name, reference, description,
                flags, value, placement, temp, damage_state,
                max_damage, current_damage,
                duration, cost, power, active,
                x, y, carried, sprite_override,
                sprite_0, sprite_1, status_0, status_1,
                gethit_dam_0, gethit_dam_1,
                min_rank, driver
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12,
                ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24,
                ?25, ?26,
                ?27, ?28
            )",
        )?;

        let mut attrib_stmt = tx.prepare(
            "INSERT INTO item_attribs (item_id, attr_idx, v0, v1, v2)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut skill_stmt = tx.prepare(
            "INSERT INTO item_skills (item_id, skill_id, v0, v1, v2)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut data_stmt =
            tx.prepare("INSERT INTO item_data (item_id, idx, value) VALUES (?1, ?2, ?3)")?;

        for (i, it) in items.iter().enumerate() {
            let id = i as u32;
            item_stmt.execute(params![
                id,
                it.used,
                &it.name[..],
                &it.reference[..],
                &it.description[..],
                it.flags as i64,
                it.value,
                it.placement,
                it.temp,
                it.damage_state,
                it.max_damage,
                it.current_damage,
                it.duration,
                it.cost,
                it.power,
                it.active,
                it.x,
                it.y,
                it.carried,
                it.sprite_override,
                it.sprite[0],
                it.sprite[1],
                it.status[0],
                it.status[1],
                it.gethit_dam[0],
                it.gethit_dam[1],
                it.min_rank,
                it.driver,
            ])?;

            if it.used == 0 {
                continue;
            }

            for (ai, attr) in it.attrib.iter().enumerate() {
                attrib_stmt.execute(params![id, ai as u32, attr[0], attr[1], attr[2]])?;
            }
            for (si, sk) in it.skill.iter().enumerate() {
                if sk[0] != 0 || sk[1] != 0 || sk[2] != 0 {
                    skill_stmt.execute(params![id, si as u32, sk[0], sk[1], sk[2]])?;
                }
            }
            for (idx, &v) in it.data.iter().enumerate() {
                if v != 0 {
                    data_stmt.execute(params![id, idx as u32, v])?;
                }
            }
        }
        Ok(())
    }

    fn populate_map(tx: &Transaction, map: &[Map]) -> Result<()> {
        let mut stmt = tx.prepare(
            "INSERT INTO map (id, sprite, fsprite, ch, to_ch, it, dlight, light, flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?;
        for (i, tile) in map.iter().enumerate() {
            stmt.execute(params![
                i as u32,
                tile.sprite,
                tile.fsprite,
                tile.ch,
                tile.to_ch,
                tile.it,
                tile.dlight,
                tile.light,
                tile.flags as i64,
            ])?;
        }
        Ok(())
    }

    fn populate_effects(tx: &Transaction, effects: &[Effect]) -> Result<()> {
        let mut stmt = tx.prepare(
            "INSERT INTO effects (id, used, flags, effect_type, duration, data_blob)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;
        for (i, eff) in effects.iter().enumerate() {
            let data_blob: Vec<u8> = eff.data.iter().flat_map(|v| v.to_le_bytes()).collect();
            stmt.execute(params![
                i as u32,
                eff.used,
                eff.flags,
                eff.effect_type,
                eff.duration,
                data_blob,
            ])?;
        }
        Ok(())
    }

    fn populate_globals(tx: &Transaction, g: &Global) -> Result<()> {
        let online_blob: Vec<u8> = g
            .online_per_hour
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let uptime_blob: Vec<u8> = g
            .uptime_per_hour
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let max_online_blob: Vec<u8> = g
            .max_online_per_hour
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();

        tx.execute(
            "INSERT INTO globals (
                id, mdtime, mdday, mdyear, dlight,
                players_created, npcs_created, players_died, npcs_died,
                character_cnt, item_cnt, effect_cnt,
                expire_cnt, gc_cnt, lost_cnt,
                reset_char, reset_item, ticker,
                total_online_time, online_per_hour,
                flags, uptime, uptime_per_hour,
                awake, body, players_online, queuesize,
                recv, send_, load, max_online,
                max_online_per_hour, fullmoon, newmoon, cap
            ) VALUES (
                0, ?1, ?2, ?3, ?4,
                ?5, ?6, ?7, ?8,
                ?9, ?10, ?11,
                ?12, ?13, ?14,
                ?15, ?16, ?17,
                ?18, ?19,
                ?20, ?21, ?22,
                ?23, ?24, ?25, ?26,
                ?27, ?28, ?29, ?30,
                ?31, ?32, ?33, ?34
            )",
            params![
                g.mdtime,
                g.mdday,
                g.mdyear,
                g.dlight,
                g.players_created,
                g.npcs_created,
                g.players_died,
                g.npcs_died,
                g.character_cnt,
                g.item_cnt,
                g.effect_cnt,
                g.expire_cnt,
                g.gc_cnt,
                g.lost_cnt,
                g.reset_char,
                g.reset_item,
                g.ticker,
                g.total_online_time,
                online_blob,
                g.flags,
                g.uptime,
                uptime_blob,
                g.awake,
                g.body,
                g.players_online,
                g.queuesize,
                g.recv,
                g.send,
                g.load,
                g.max_online,
                max_online_blob,
                g.fullmoon,
                g.newmoon,
                g.cap,
            ],
        )?;
        Ok(())
    }
}

impl BenchSchema for NormalizedSchema {
    fn name(&self) -> &'static str {
        "normalized"
    }

    fn create_tables(&self, conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE characters (
                id              INTEGER PRIMARY KEY,
                used            INTEGER NOT NULL,
                name            BLOB,
                reference       BLOB,
                description     BLOB,
                kindred         INTEGER,
                player          INTEGER,
                sprite          INTEGER,
                sound           INTEGER,
                flags           INTEGER,
                alignment       INTEGER,
                temple_x        INTEGER,
                temple_y        INTEGER,
                tavern_x        INTEGER,
                tavern_y        INTEGER,
                temp            INTEGER,
                weapon_bonus    INTEGER,
                armor_bonus     INTEGER,
                a_hp            INTEGER,
                a_end           INTEGER,
                a_mana          INTEGER,
                light           INTEGER,
                mode            INTEGER,
                speed           INTEGER,
                points          INTEGER,
                points_tot      INTEGER,
                armor           INTEGER,
                weapon          INTEGER,
                x               INTEGER,
                y               INTEGER,
                tox             INTEGER,
                toy             INTEGER,
                frx             INTEGER,
                fry             INTEGER,
                status          INTEGER,
                status2         INTEGER,
                dir             INTEGER,
                gold            INTEGER,
                citem           INTEGER,
                creation_date   INTEGER,
                login_date      INTEGER,
                current_online_time INTEGER,
                total_online_time   INTEGER,
                attack_cn       INTEGER,
                skill_nr        INTEGER,
                goto_x          INTEGER,
                goto_y          INTEGER,
                use_nr          INTEGER,
                misc_action     INTEGER,
                misc_target1    INTEGER,
                misc_target2    INTEGER,
                stunned         INTEGER,
                speed_mod       INTEGER,
                depot_cost      INTEGER,
                luck            INTEGER
            );

            CREATE INDEX idx_characters_active ON characters(used) WHERE used != 0;
            CREATE INDEX idx_characters_xy ON characters(x, y) WHERE used != 0;

            -- Sub-tables for character array fields
            CREATE TABLE character_attribs (
                char_id     INTEGER NOT NULL,
                attr_idx    INTEGER NOT NULL,
                v0 INTEGER, v1 INTEGER, v2 INTEGER, v3 INTEGER, v4 INTEGER, v5 INTEGER,
                PRIMARY KEY (char_id, attr_idx)
            );

            CREATE TABLE character_hp (
                char_id INTEGER NOT NULL, idx INTEGER NOT NULL, value INTEGER,
                PRIMARY KEY (char_id, idx)
            );

            CREATE TABLE character_end (
                char_id INTEGER NOT NULL, idx INTEGER NOT NULL, value INTEGER,
                PRIMARY KEY (char_id, idx)
            );

            CREATE TABLE character_mana (
                char_id INTEGER NOT NULL, idx INTEGER NOT NULL, value INTEGER,
                PRIMARY KEY (char_id, idx)
            );

            CREATE TABLE character_skills (
                char_id     INTEGER NOT NULL,
                skill_id    INTEGER NOT NULL,
                v0 INTEGER, v1 INTEGER, v2 INTEGER, v3 INTEGER, v4 INTEGER, v5 INTEGER,
                PRIMARY KEY (char_id, skill_id)
            );

            CREATE TABLE character_items (
                char_id INTEGER NOT NULL, slot INTEGER NOT NULL, item_id INTEGER NOT NULL,
                PRIMARY KEY (char_id, slot)
            );

            CREATE TABLE character_worn (
                char_id INTEGER NOT NULL, slot INTEGER NOT NULL, item_id INTEGER NOT NULL,
                PRIMARY KEY (char_id, slot)
            );

            CREATE TABLE character_spells (
                char_id INTEGER NOT NULL, slot INTEGER NOT NULL, item_id INTEGER NOT NULL,
                PRIMARY KEY (char_id, slot)
            );

            CREATE TABLE character_depot (
                char_id INTEGER NOT NULL, slot INTEGER NOT NULL, item_id INTEGER NOT NULL,
                PRIMARY KEY (char_id, slot)
            );

            CREATE TABLE character_data (
                char_id INTEGER NOT NULL, idx INTEGER NOT NULL, value INTEGER NOT NULL,
                PRIMARY KEY (char_id, idx)
            );

            -- Items table (scalar fields only)
            CREATE TABLE items (
                id              INTEGER PRIMARY KEY,
                used            INTEGER NOT NULL,
                name            BLOB,
                reference       BLOB,
                description     BLOB,
                flags           INTEGER,
                value           INTEGER,
                placement       INTEGER,
                temp            INTEGER,
                damage_state    INTEGER,
                max_damage      INTEGER,
                current_damage  INTEGER,
                duration        INTEGER,
                cost            INTEGER,
                power           INTEGER,
                active          INTEGER,
                x               INTEGER,
                y               INTEGER,
                carried         INTEGER,
                sprite_override INTEGER,
                sprite_0        INTEGER,
                sprite_1        INTEGER,
                status_0        INTEGER,
                status_1        INTEGER,
                gethit_dam_0    INTEGER,
                gethit_dam_1    INTEGER,
                min_rank        INTEGER,
                driver          INTEGER
            );

            CREATE INDEX idx_items_active ON items(used) WHERE used != 0;

            CREATE TABLE item_attribs (
                item_id     INTEGER NOT NULL,
                attr_idx    INTEGER NOT NULL,
                v0 INTEGER, v1 INTEGER, v2 INTEGER,
                PRIMARY KEY (item_id, attr_idx)
            );

            CREATE TABLE item_skills (
                item_id     INTEGER NOT NULL,
                skill_id    INTEGER NOT NULL,
                v0 INTEGER, v1 INTEGER, v2 INTEGER,
                PRIMARY KEY (item_id, skill_id)
            );

            CREATE TABLE item_data (
                item_id INTEGER NOT NULL, idx INTEGER NOT NULL, value INTEGER NOT NULL,
                PRIMARY KEY (item_id, idx)
            );

            -- Map & effects are the same in both schemas (no arrays)
            CREATE TABLE map (
                id      INTEGER PRIMARY KEY,
                sprite  INTEGER NOT NULL,
                fsprite INTEGER NOT NULL,
                ch      INTEGER NOT NULL,
                to_ch   INTEGER NOT NULL,
                it      INTEGER NOT NULL,
                dlight  INTEGER NOT NULL,
                light   INTEGER NOT NULL,
                flags   INTEGER NOT NULL
            );

            CREATE INDEX idx_map_ch ON map(ch) WHERE ch != 0;
            CREATE INDEX idx_map_it ON map(it) WHERE it != 0;

            CREATE TABLE effects (
                id          INTEGER PRIMARY KEY,
                used        INTEGER NOT NULL,
                flags       INTEGER NOT NULL,
                effect_type INTEGER NOT NULL,
                duration    INTEGER NOT NULL,
                data_blob   BLOB
            );

            CREATE INDEX idx_effects_active ON effects(used) WHERE used != 0;

            CREATE TABLE globals (
                id                  INTEGER PRIMARY KEY,
                mdtime              INTEGER,
                mdday               INTEGER,
                mdyear              INTEGER,
                dlight              INTEGER,
                players_created     INTEGER,
                npcs_created        INTEGER,
                players_died        INTEGER,
                npcs_died           INTEGER,
                character_cnt       INTEGER,
                item_cnt            INTEGER,
                effect_cnt          INTEGER,
                expire_cnt          INTEGER,
                gc_cnt              INTEGER,
                lost_cnt            INTEGER,
                reset_char          INTEGER,
                reset_item          INTEGER,
                ticker              INTEGER,
                total_online_time   INTEGER,
                online_per_hour     BLOB,
                flags               INTEGER,
                uptime              INTEGER,
                uptime_per_hour     BLOB,
                awake               INTEGER,
                body                INTEGER,
                players_online      INTEGER,
                queuesize           INTEGER,
                recv                INTEGER,
                send_               INTEGER,
                load                INTEGER,
                max_online          INTEGER,
                max_online_per_hour BLOB,
                fullmoon            INTEGER,
                newmoon             INTEGER,
                cap                 INTEGER
            );
            ",
        )?;
        Ok(())
    }

    fn populate(
        &self,
        conn: &Connection,
        characters: &[Character],
        items: &[Item],
        map: &[Map],
        effects: &[Effect],
        globals: &Global,
    ) -> Result<()> {
        let tx = unsafe {
            let conn_ptr = conn as *const Connection as *mut Connection;
            (*conn_ptr).transaction()?
        };

        Self::populate_characters(&tx, characters)?;
        Self::populate_items(&tx, items)?;
        Self::populate_map(&tx, map)?;
        Self::populate_effects(&tx, effects)?;
        Self::populate_globals(&tx, globals)?;

        tx.commit()?;
        Ok(())
    }

    fn character_triage(&self, conn: &Connection) -> Result<Vec<(u32, u8, u64)>> {
        let mut stmt =
            conn.prepare_cached("SELECT id, used, flags FROM characters WHERE used != 0")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u8>(1)?,
                row.get::<_, i64>(2)? as u64,
            ))
        })?;
        let mut result = Vec::with_capacity(1024);
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    fn read_viewport(
        &self,
        conn: &Connection,
        base_x: u16,
        base_y: u16,
    ) -> Result<Vec<ViewportTile>> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, sprite, fsprite, ch, to_ch, it, dlight, light, flags
             FROM map WHERE id BETWEEN ?1 AND ?2",
        )?;

        let mut tiles = Vec::with_capacity(34 * 34);
        let mapx = mag_core::constants::SERVER_MAPX as u32;

        for dy in 0u16..34 {
            let y = base_y.wrapping_add(dy);
            if y >= mag_core::constants::SERVER_MAPY as u16 {
                continue;
            }
            let row_start = base_x as u32 + y as u32 * mapx;
            let row_end = row_start + 33;

            let rows = stmt.query_map(params![row_start, row_end], |row| {
                Ok(ViewportTile {
                    id: row.get(0)?,
                    sprite: row.get::<_, u32>(1)? as u16,
                    fsprite: row.get::<_, u32>(2)? as u16,
                    ch: row.get(3)?,
                    to_ch: row.get(4)?,
                    it: row.get(5)?,
                    dlight: row.get::<_, u32>(6)? as u16,
                    light: row.get::<_, i32>(7)? as i16,
                    flags: row.get::<_, i64>(8)? as u64,
                })
            })?;

            for r in rows {
                tiles.push(r?);
            }
        }
        Ok(tiles)
    }

    fn read_character(&self, conn: &Connection, id: u32) -> Result<CharacterRow> {
        // Read main character row + sub-table data via separate queries.
        let mut stmt = conn.prepare_cached(
            "SELECT id, used, flags, x, y, sprite, a_hp, a_end, a_mana, status,
                    speed, light, attack_cn, name
             FROM characters WHERE id = ?1",
        )?;
        let mut row = stmt.query_row(params![id], |row| {
            Ok(CharacterRow {
                id: row.get(0)?,
                used: row.get(1)?,
                flags: row.get::<_, i64>(2)? as u64,
                x: row.get::<_, i32>(3)? as i16,
                y: row.get::<_, i32>(4)? as i16,
                sprite: row.get::<_, u32>(5)? as u16,
                a_hp: row.get(6)?,
                a_end: row.get(7)?,
                a_mana: row.get(8)?,
                status: row.get::<_, i32>(9)? as i16,
                speed: row.get::<_, i32>(10)? as i16,
                light: row.get::<_, u32>(11)? as u8,
                attack_cn: row.get::<_, u32>(12)? as u16,
                name: row.get::<_, Vec<u8>>(13)?,
                ..Default::default()
            })
        })?;

        // Fetch attribs
        {
            let mut attrib_stmt = conn.prepare_cached(
                "SELECT v0, v1, v2, v3, v4, v5 FROM character_attribs WHERE char_id = ?1 ORDER BY attr_idx",
            )?;
            let attrib_rows = attrib_stmt.query_map(params![id], |r| {
                Ok([
                    r.get::<_, u8>(0)?,
                    r.get::<_, u8>(1)?,
                    r.get::<_, u8>(2)?,
                    r.get::<_, u8>(3)?,
                    r.get::<_, u8>(4)?,
                    r.get::<_, u8>(5)?,
                ])
            })?;
            let mut buf = Vec::with_capacity(30);
            for r in attrib_rows {
                buf.extend_from_slice(&r?);
            }
            row.attrib = buf;
        }

        // Fetch hp, end, mana
        for (field, table) in [
            (&mut row.hp, "character_hp"),
            (&mut row.end, "character_end"),
            (&mut row.mana, "character_mana"),
        ] {
            let sql = format!(
                "SELECT value FROM {} WHERE char_id = ?1 ORDER BY idx",
                table
            );
            let mut stmt = conn.prepare_cached(&sql)?;
            let vals = stmt.query_map(params![id], |r| r.get::<_, u16>(0))?;
            let mut buf = Vec::with_capacity(12);
            for v in vals {
                buf.extend_from_slice(&v?.to_le_bytes());
            }
            *field = buf;
        }

        // Fetch skills
        {
            let mut skill_stmt = conn.prepare_cached(
                "SELECT v0, v1, v2, v3, v4, v5 FROM character_skills WHERE char_id = ?1 ORDER BY skill_id",
            )?;
            let skill_rows = skill_stmt.query_map(params![id], |r| {
                Ok([
                    r.get::<_, u8>(0)?,
                    r.get::<_, u8>(1)?,
                    r.get::<_, u8>(2)?,
                    r.get::<_, u8>(3)?,
                    r.get::<_, u8>(4)?,
                    r.get::<_, u8>(5)?,
                ])
            })?;
            let mut buf = Vec::with_capacity(300);
            for r in skill_rows {
                buf.extend_from_slice(&r?);
            }
            row.skill = buf;
        }

        Ok(row)
    }

    fn read_item(&self, conn: &Connection, id: u32) -> Result<ItemRow> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, used, flags, sprite_0, status_0, value, name
             FROM items WHERE id = ?1",
        )?;
        let mut item = stmt.query_row(params![id], |row| {
            Ok(ItemRow {
                id: row.get(0)?,
                used: row.get(1)?,
                flags: row.get::<_, i64>(2)? as u64,
                sprite_0: row.get::<_, i32>(3)? as i16,
                status_0: row.get::<_, u32>(4)? as u8,
                value: row.get(5)?,
                name: row.get(6)?,
                ..Default::default()
            })
        })?;

        // Fetch attribs
        {
            let mut attrib_stmt = conn.prepare_cached(
                "SELECT v0, v1, v2 FROM item_attribs WHERE item_id = ?1 ORDER BY attr_idx",
            )?;
            let rows = attrib_stmt.query_map(params![id], |r| {
                Ok([
                    r.get::<_, i8>(0)? as u8,
                    r.get::<_, i8>(1)? as u8,
                    r.get::<_, i8>(2)? as u8,
                ])
            })?;
            let mut buf = Vec::with_capacity(15);
            for r in rows {
                buf.extend_from_slice(&r?);
            }
            item.attrib = buf;
        }

        // Fetch skills
        {
            let mut skill_stmt = conn.prepare_cached(
                "SELECT v0, v1, v2 FROM item_skills WHERE item_id = ?1 ORDER BY skill_id",
            )?;
            let rows = skill_stmt.query_map(params![id], |r| {
                Ok([
                    r.get::<_, i8>(0)? as u8,
                    r.get::<_, i8>(1)? as u8,
                    r.get::<_, i8>(2)? as u8,
                ])
            })?;
            let mut buf = Vec::with_capacity(150);
            for r in rows {
                buf.extend_from_slice(&r?);
            }
            item.skill = buf;
        }

        Ok(item)
    }

    fn read_items_batch(&self, conn: &Connection, ids: &[u32]) -> Result<Vec<ItemRow>> {
        // For normalized schema, batch reads require N queries for sub-tables too.
        // We read only the main item rows in batch; sub-tables are skipped for
        // the batch case since the tick simulation primarily needs scalar fields
        // (sprite, status, flags) for batch viewport lookups.
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT id, used, flags, sprite_0, status_0, value, name
             FROM items WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::types::ToSql>> = ids
            .iter()
            .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
            .collect();
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| &**p).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(ItemRow {
                id: row.get(0)?,
                used: row.get(1)?,
                flags: row.get::<_, i64>(2)? as u64,
                sprite_0: row.get::<_, i32>(3)? as i16,
                status_0: row.get::<_, u32>(4)? as u8,
                value: row.get(5)?,
                name: row.get(6)?,
                ..Default::default()
            })
        })?;
        let mut result = Vec::with_capacity(ids.len());
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    fn read_map_tile(&self, conn: &Connection, idx: u32) -> Result<ViewportTile> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, sprite, fsprite, ch, to_ch, it, dlight, light, flags
             FROM map WHERE id = ?1",
        )?;
        let tile = stmt.query_row(params![idx], |row| {
            Ok(ViewportTile {
                id: row.get(0)?,
                sprite: row.get::<_, u32>(1)? as u16,
                fsprite: row.get::<_, u32>(2)? as u16,
                ch: row.get(3)?,
                to_ch: row.get(4)?,
                it: row.get(5)?,
                dlight: row.get::<_, u32>(6)? as u16,
                light: row.get::<_, i32>(7)? as i16,
                flags: row.get::<_, i64>(8)? as u64,
            })
        })?;
        Ok(tile)
    }

    fn update_character_stats(
        &self,
        conn: &Connection,
        id: u32,
        a_hp: i32,
        a_end: i32,
        a_mana: i32,
        status: i16,
    ) -> Result<()> {
        let mut stmt = conn.prepare_cached(
            "UPDATE characters SET a_hp = ?1, a_end = ?2, a_mana = ?3, status = ?4 WHERE id = ?5",
        )?;
        stmt.execute(params![a_hp, a_end, a_mana, status as i32, id])?;
        Ok(())
    }

    fn update_map_ch(&self, conn: &Connection, idx: u32, ch: u32) -> Result<()> {
        let mut stmt = conn.prepare_cached("UPDATE map SET ch = ?1 WHERE id = ?2")?;
        stmt.execute(params![ch, idx])?;
        Ok(())
    }

    fn update_map_light(&self, conn: &Connection, idx: u32, light: i16) -> Result<()> {
        let mut stmt = conn.prepare_cached("UPDATE map SET light = light + ?1 WHERE id = ?2")?;
        stmt.execute(params![light as i32, idx])?;
        Ok(())
    }

    fn update_map_light_area(
        &self,
        conn: &Connection,
        center_x: u16,
        center_y: u16,
        radius: u16,
        amount: i16,
    ) -> Result<()> {
        let mapx = mag_core::constants::SERVER_MAPX as u32;
        let mapy = mag_core::constants::SERVER_MAPY as u16;

        let mut stmt =
            conn.prepare_cached("UPDATE map SET light = light + ?1 WHERE id BETWEEN ?2 AND ?3")?;

        let min_y = center_y.saturating_sub(radius);
        let max_y = (center_y + radius).min(mapy - 1);

        for y in min_y..=max_y {
            let min_x = center_x.saturating_sub(radius) as u32;
            let max_x = (center_x + radius).min(mag_core::constants::SERVER_MAPX as u16 - 1) as u32;
            let row_start = min_x + y as u32 * mapx;
            let row_end = max_x + y as u32 * mapx;
            stmt.execute(params![amount as i32, row_start, row_end])?;
        }
        Ok(())
    }

    fn effect_triage(&self, conn: &Connection) -> Result<Vec<(u32, u8, u32)>> {
        let mut stmt =
            conn.prepare_cached("SELECT id, effect_type, duration FROM effects WHERE used != 0")?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u8>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;
        let mut result = Vec::with_capacity(256);
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    fn update_effect(&self, conn: &Connection, id: u32, duration: u32) -> Result<()> {
        let mut stmt = conn.prepare_cached("UPDATE effects SET duration = ?1 WHERE id = ?2")?;
        stmt.execute(params![duration, id])?;
        Ok(())
    }

    fn read_map_row(&self, conn: &Connection, row_y: u16) -> Result<Vec<ViewportTile>> {
        let mapx = mag_core::constants::SERVER_MAPX as u32;
        let row_start = row_y as u32 * mapx;
        let row_end = row_start + mapx - 1;

        let mut stmt = conn.prepare_cached(
            "SELECT id, sprite, fsprite, ch, to_ch, it, dlight, light, flags
             FROM map WHERE id BETWEEN ?1 AND ?2",
        )?;
        let rows = stmt.query_map(params![row_start, row_end], |row| {
            Ok(ViewportTile {
                id: row.get(0)?,
                sprite: row.get::<_, u32>(1)? as u16,
                fsprite: row.get::<_, u32>(2)? as u16,
                ch: row.get(3)?,
                to_ch: row.get(4)?,
                it: row.get(5)?,
                dlight: row.get::<_, u32>(6)? as u16,
                light: row.get::<_, i32>(7)? as i16,
                flags: row.get::<_, i64>(8)? as u64,
            })
        })?;
        let mut result = Vec::with_capacity(mapx as usize);
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    fn read_items_range(
        &self,
        conn: &Connection,
        start_id: u32,
        count: u32,
    ) -> Result<Vec<ItemRow>> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, used, flags, sprite_0, status_0, value, name
             FROM items WHERE id BETWEEN ?1 AND ?2",
        )?;
        let rows = stmt.query_map(params![start_id, start_id + count - 1], |row| {
            Ok(ItemRow {
                id: row.get(0)?,
                used: row.get(1)?,
                flags: row.get::<_, i64>(2)? as u64,
                sprite_0: row.get::<_, i32>(3)? as i16,
                status_0: row.get::<_, u32>(4)? as u8,
                value: row.get(5)?,
                name: row.get(6)?,
                ..Default::default()
            })
        })?;
        let mut result = Vec::with_capacity(count as usize);
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    fn update_globals_tick(&self, conn: &Connection, ticker: i32, uptime: i64) -> Result<()> {
        let mut stmt =
            conn.prepare_cached("UPDATE globals SET ticker = ?1, uptime = ?2 WHERE id = 0")?;
        stmt.execute(params![ticker, uptime])?;
        Ok(())
    }

    fn read_character_slots(&self, conn: &Connection, id: u32) -> Result<CharacterSlots> {
        // Read from sub-tables and reconstruct as blobs for comparison parity.
        let mut slots = CharacterSlots::default();

        // Items (40 slots)
        {
            let mut stmt = conn.prepare_cached(
                "SELECT slot, item_id FROM character_items WHERE char_id = ?1 ORDER BY slot",
            )?;
            let mut buf = vec![0u8; 40 * 4];
            let rows = stmt.query_map(params![id], |r| {
                Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?))
            })?;
            for r in rows {
                let (slot, item_id) = r?;
                if (slot as usize) < 40 {
                    let off = slot as usize * 4;
                    buf[off..off + 4].copy_from_slice(&item_id.to_le_bytes());
                }
            }
            slots.item = buf;
        }

        // Worn (20 slots)
        {
            let mut stmt = conn.prepare_cached(
                "SELECT slot, item_id FROM character_worn WHERE char_id = ?1 ORDER BY slot",
            )?;
            let mut buf = vec![0u8; 20 * 4];
            let rows = stmt.query_map(params![id], |r| {
                Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?))
            })?;
            for r in rows {
                let (slot, item_id) = r?;
                if (slot as usize) < 20 {
                    let off = slot as usize * 4;
                    buf[off..off + 4].copy_from_slice(&item_id.to_le_bytes());
                }
            }
            slots.worn = buf;
        }

        // Spells (20 slots)
        {
            let mut stmt = conn.prepare_cached(
                "SELECT slot, item_id FROM character_spells WHERE char_id = ?1 ORDER BY slot",
            )?;
            let mut buf = vec![0u8; 20 * 4];
            let rows = stmt.query_map(params![id], |r| {
                Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?))
            })?;
            for r in rows {
                let (slot, item_id) = r?;
                if (slot as usize) < 20 {
                    let off = slot as usize * 4;
                    buf[off..off + 4].copy_from_slice(&item_id.to_le_bytes());
                }
            }
            slots.spell = buf;
        }

        // Depot (62 slots)
        {
            let mut stmt = conn.prepare_cached(
                "SELECT slot, item_id FROM character_depot WHERE char_id = ?1 ORDER BY slot",
            )?;
            let mut buf = vec![0u8; 62 * 4];
            let rows = stmt.query_map(params![id], |r| {
                Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?))
            })?;
            for r in rows {
                let (slot, item_id) = r?;
                if (slot as usize) < 62 {
                    let off = slot as usize * 4;
                    buf[off..off + 4].copy_from_slice(&item_id.to_le_bytes());
                }
            }
            slots.depot = buf;
        }

        Ok(slots)
    }
}
