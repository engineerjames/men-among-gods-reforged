//! BLOB schema: array fields stored as fixed-size BLOBs in flat tables.
//!
//! This is the simpler schema — each entity is a single row with scalar columns
//! for frequently-accessed fields and BLOB columns for array data (attribs,
//! skills, inventory slots, driver data, etc.).

use super::{BenchSchema, CharacterRow, CharacterSlots, ItemRow, ViewportTile};
use anyhow::Result;
use mag_core::types::{Character, Effect, Global, Item, Map};
use rusqlite::{params, Connection, Transaction};

pub struct BlobSchema;

impl BlobSchema {
    pub fn new() -> Self {
        Self
    }

    // ── Helpers for serializing fixed-size arrays to BLOB bytes ──────

    fn u8_array_to_blob(arr: &[u8]) -> Vec<u8> {
        arr.to_vec()
    }

    fn u8_2d_to_blob<const R: usize, const C: usize>(arr: &[[u8; C]; R]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(R * C);
        for row in arr {
            buf.extend_from_slice(row);
        }
        buf
    }

    fn u16_array_to_blob(arr: &[u16]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(arr.len() * 2);
        for v in arr {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    fn i16_array_to_blob(arr: &[i16]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(arr.len() * 2);
        for v in arr {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    fn u32_array_to_blob(arr: &[u32]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(arr.len() * 4);
        for v in arr {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    fn i32_array_to_blob(arr: &[i32]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(arr.len() * 4);
        for v in arr {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    fn i8_array_to_blob(arr: &[i8]) -> Vec<u8> {
        arr.iter().map(|&v| v as u8).collect()
    }

    fn i8_2d_to_blob<const R: usize, const C: usize>(arr: &[[i8; C]; R]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(R * C);
        for row in arr {
            for &v in row {
                buf.push(v as u8);
            }
        }
        buf
    }

    fn i64_array_to_blob(arr: &[i64]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(arr.len() * 8);
        for v in arr {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    fn item_attrib_to_blob(arr: &[[i8; 3]; 5]) -> Vec<u8> {
        Self::i8_2d_to_blob(arr)
    }

    fn item_i16_3_to_blob(arr: &[i16; 3]) -> Vec<u8> {
        Self::i16_array_to_blob(arr)
    }

    fn item_i8_2_to_blob(arr: &[i8; 2]) -> Vec<u8> {
        Self::i8_array_to_blob(arr)
    }

    fn item_skill_to_blob(arr: &[[i8; 3]; 50]) -> Vec<u8> {
        Self::i8_2d_to_blob(arr)
    }

    fn populate_characters(tx: &Transaction, characters: &[Character]) -> Result<()> {
        let mut stmt = tx.prepare(
            "INSERT INTO characters (
                id, used, name, reference, description, kindred, player,
                sprite, sound, flags, alignment,
                temple_x, temple_y, tavern_x, tavern_y, temp,
                attrib, hp, end_, mana, skill,
                weapon_bonus, armor_bonus,
                a_hp, a_end, a_mana,
                light, mode, speed,
                points, points_tot, armor, weapon,
                x, y, tox, toy, frx, fry, status, status2, dir,
                gold, item_slots, worn_slots, spell_slots, citem,
                creation_date, login_date, current_online_time, total_online_time,
                attack_cn, skill_nr, goto_x, goto_y, use_nr,
                misc_action, misc_target1, misc_target2,
                stunned, speed_mod, depot, depot_cost, luck,
                data, text
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16,
                ?17, ?18, ?19, ?20, ?21,
                ?22, ?23,
                ?24, ?25, ?26,
                ?27, ?28, ?29,
                ?30, ?31, ?32, ?33,
                ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42,
                ?43, ?44, ?45, ?46, ?47,
                ?48, ?49, ?50, ?51,
                ?52, ?53, ?54, ?55, ?56,
                ?57, ?58, ?59,
                ?60, ?61, ?62, ?63, ?64,
                ?65, ?66
            )",
        )?;

        for (i, ch) in characters.iter().enumerate() {
            stmt.execute(params![
                i as u32,
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
                Self::u8_2d_to_blob(&ch.attrib),
                Self::u16_array_to_blob(&ch.hp),
                Self::u16_array_to_blob(&ch.end),
                Self::u16_array_to_blob(&ch.mana),
                Self::u8_2d_to_blob(&ch.skill),
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
                Self::u32_array_to_blob(&ch.item),
                Self::u32_array_to_blob(&ch.worn),
                Self::u32_array_to_blob(&ch.spell),
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
                Self::u32_array_to_blob(&ch.depot),
                ch.depot_cost,
                ch.luck,
                Self::i32_array_to_blob(&ch.data),
                Self::u8_2d_to_blob(&ch.text),
            ])?;
        }
        Ok(())
    }

    fn populate_items(tx: &Transaction, items: &[Item]) -> Result<()> {
        let mut stmt = tx.prepare(
            "INSERT INTO items (
                id, used, name, reference, description,
                flags, value, placement, temp, damage_state,
                max_age, current_age, max_damage, current_damage,
                attrib, hp, end_, mana, skill,
                armor, weapon, light,
                duration, cost, power, active,
                x, y, carried, sprite_override,
                sprite, status,
                gethit_dam, min_rank, driver, data
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19,
                ?20, ?21, ?22,
                ?23, ?24, ?25, ?26,
                ?27, ?28, ?29, ?30,
                ?31, ?32,
                ?33, ?34, ?35, ?36
            )",
        )?;

        for (i, it) in items.iter().enumerate() {
            stmt.execute(params![
                i as u32,
                it.used,
                &it.name[..],
                &it.reference[..],
                &it.description[..],
                it.flags as i64,
                it.value,
                it.placement,
                it.temp,
                it.damage_state,
                Self::u32_array_to_blob(&it.max_age),
                Self::u32_array_to_blob(&it.current_age),
                it.max_damage,
                it.current_damage,
                Self::item_attrib_to_blob(&it.attrib),
                Self::item_i16_3_to_blob(&it.hp),
                Self::item_i16_3_to_blob(&it.end),
                Self::item_i16_3_to_blob(&it.mana),
                Self::item_skill_to_blob(&it.skill),
                Self::item_i8_2_to_blob(&it.armor),
                Self::item_i8_2_to_blob(&it.weapon),
                Self::i16_array_to_blob(&it.light),
                it.duration,
                it.cost,
                it.power,
                it.active,
                it.x,
                it.y,
                it.carried,
                it.sprite_override,
                Self::i16_array_to_blob(&it.sprite),
                Self::u8_array_to_blob(&it.status),
                Self::i8_2_to_blob(&it.gethit_dam),
                it.min_rank,
                it.driver,
                Self::u32_array_to_blob(&it.data),
            ])?;
        }
        Ok(())
    }

    fn i8_2_to_blob(arr: &[i8; 2]) -> Vec<u8> {
        arr.iter().map(|&v| v as u8).collect()
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
            "INSERT INTO effects (id, used, flags, effect_type, duration, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;

        for (i, eff) in effects.iter().enumerate() {
            stmt.execute(params![
                i as u32,
                eff.used,
                eff.flags,
                eff.effect_type,
                eff.duration,
                Self::u32_array_to_blob(&eff.data),
            ])?;
        }
        Ok(())
    }

    fn populate_globals(tx: &Transaction, g: &Global) -> Result<()> {
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
                Self::i64_array_to_blob(&g.online_per_hour),
                g.flags,
                g.uptime,
                Self::i64_array_to_blob(&g.uptime_per_hour),
                g.awake,
                g.body,
                g.players_online,
                g.queuesize,
                g.recv,
                g.send,
                g.load,
                g.max_online,
                Self::i32_array_to_blob(&g.max_online_per_hour),
                g.fullmoon,
                g.newmoon,
                g.cap,
            ],
        )?;
        Ok(())
    }
}

impl BenchSchema for BlobSchema {
    fn name(&self) -> &'static str {
        "blob"
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
                attrib          BLOB,
                hp              BLOB,
                end_            BLOB,
                mana            BLOB,
                skill           BLOB,
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
                item_slots      BLOB,
                worn_slots      BLOB,
                spell_slots     BLOB,
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
                depot           BLOB,
                depot_cost      INTEGER,
                luck            INTEGER,
                data            BLOB,
                text            BLOB
            );

            CREATE INDEX idx_characters_active ON characters(used) WHERE used != 0;
            CREATE INDEX idx_characters_xy ON characters(x, y) WHERE used != 0;

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
                max_age         BLOB,
                current_age     BLOB,
                max_damage      INTEGER,
                current_damage  INTEGER,
                attrib          BLOB,
                hp              BLOB,
                end_            BLOB,
                mana            BLOB,
                skill           BLOB,
                armor           BLOB,
                weapon          BLOB,
                light           BLOB,
                duration        INTEGER,
                cost            INTEGER,
                power           INTEGER,
                active          INTEGER,
                x               INTEGER,
                y               INTEGER,
                carried         INTEGER,
                sprite_override INTEGER,
                sprite          BLOB,
                status          BLOB,
                gethit_dam      BLOB,
                min_rank        INTEGER,
                driver          INTEGER,
                data            BLOB
            );

            CREATE INDEX idx_items_active ON items(used) WHERE used != 0;

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
                data        BLOB
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
        // Use a transaction for bulk insert performance.
        let tx = unsafe {
            // SAFETY: We need a mutable reference but only have &Connection from
            // the trait. The caller guarantees single-threaded access during populate.
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
        // The viewport is 34×34 tiles. In the linearized map (id = x + y*1024),
        // each row of the viewport is a contiguous range of 34 ids, but rows
        // are 1024 apart. We query row by row.
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
        let mut stmt = conn.prepare_cached(
            "SELECT id, used, flags, x, y, sprite, a_hp, a_end, a_mana, status,
                    speed, light, attack_cn, name, attrib, hp, mana, end_, skill
             FROM characters WHERE id = ?1",
        )?;
        let row = stmt.query_row(params![id], |row| {
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
                attrib: row.get::<_, Vec<u8>>(14)?,
                hp: row.get::<_, Vec<u8>>(15)?,
                mana: row.get::<_, Vec<u8>>(16)?,
                end: row.get::<_, Vec<u8>>(17)?,
                skill: row.get::<_, Vec<u8>>(18)?,
            })
        })?;
        Ok(row)
    }

    fn read_item(&self, conn: &Connection, id: u32) -> Result<ItemRow> {
        let mut stmt = conn.prepare_cached(
            "SELECT id, used, flags, sprite, status, value, name,
                    attrib, hp, mana, end_, skill, armor, weapon, light
             FROM items WHERE id = ?1",
        )?;
        let row = stmt.query_row(params![id], |row| {
            Ok(ItemRow {
                id: row.get(0)?,
                used: row.get(1)?,
                flags: row.get::<_, i64>(2)? as u64,
                sprite_0: {
                    let blob: Vec<u8> = row.get(3)?;
                    if blob.len() >= 2 {
                        i16::from_le_bytes([blob[0], blob[1]])
                    } else {
                        0
                    }
                },
                status_0: {
                    let blob: Vec<u8> = row.get(4)?;
                    if !blob.is_empty() {
                        blob[0]
                    } else {
                        0
                    }
                },
                value: row.get(5)?,
                name: row.get(6)?,
                attrib: row.get(7)?,
                hp: row.get(8)?,
                mana: row.get(9)?,
                end: row.get(10)?,
                skill: row.get(11)?,
                armor: row.get(12)?,
                weapon: row.get(13)?,
                light: row.get(14)?,
            })
        })?;
        Ok(row)
    }

    fn read_items_batch(&self, conn: &Connection, ids: &[u32]) -> Result<Vec<ItemRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Build dynamic IN clause. For benchmark purposes this is acceptable.
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT id, used, flags, sprite, status, value, name,
                    attrib, hp, mana, end_, skill, armor, weapon, light
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
                sprite_0: {
                    let blob: Vec<u8> = row.get(3)?;
                    if blob.len() >= 2 {
                        i16::from_le_bytes([blob[0], blob[1]])
                    } else {
                        0
                    }
                },
                status_0: {
                    let blob: Vec<u8> = row.get(4)?;
                    if !blob.is_empty() {
                        blob[0]
                    } else {
                        0
                    }
                },
                value: row.get(5)?,
                name: row.get(6)?,
                attrib: row.get(7)?,
                hp: row.get(8)?,
                mana: row.get(9)?,
                end: row.get(10)?,
                skill: row.get(11)?,
                armor: row.get(12)?,
                weapon: row.get(13)?,
                light: row.get(14)?,
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
        // Update a square area of map tiles. Each row is a contiguous range.
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
            "SELECT id, used, flags, sprite, status, value, name,
                    attrib, hp, mana, end_, skill, armor, weapon, light
             FROM items WHERE id BETWEEN ?1 AND ?2",
        )?;
        let rows = stmt.query_map(params![start_id, start_id + count - 1], |row| {
            Ok(ItemRow {
                id: row.get(0)?,
                used: row.get(1)?,
                flags: row.get::<_, i64>(2)? as u64,
                sprite_0: {
                    let blob: Vec<u8> = row.get(3)?;
                    if blob.len() >= 2 {
                        i16::from_le_bytes([blob[0], blob[1]])
                    } else {
                        0
                    }
                },
                status_0: {
                    let blob: Vec<u8> = row.get(4)?;
                    if !blob.is_empty() {
                        blob[0]
                    } else {
                        0
                    }
                },
                value: row.get(5)?,
                name: row.get(6)?,
                attrib: row.get(7)?,
                hp: row.get(8)?,
                mana: row.get(9)?,
                end: row.get(10)?,
                skill: row.get(11)?,
                armor: row.get(12)?,
                weapon: row.get(13)?,
                light: row.get(14)?,
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
        let mut stmt = conn.prepare_cached(
            "SELECT item_slots, worn_slots, spell_slots, depot FROM characters WHERE id = ?1",
        )?;
        let slots = stmt.query_row(params![id], |row| {
            Ok(CharacterSlots {
                item: row.get(0)?,
                worn: row.get(1)?,
                spell: row.get(2)?,
                depot: row.get(3)?,
            })
        })?;
        Ok(slots)
    }
}
