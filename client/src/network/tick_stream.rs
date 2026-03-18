use flate2::{Decompress, FlushDecompress, Status};
use mag_core::constants::{
    SV_EXIT, SV_IGNORE, SV_LOAD, SV_PLAYSOUND, SV_SCROLL_DOWN, SV_SCROLL_LEFT, SV_SCROLL_LEFTDOWN,
    SV_SCROLL_LEFTUP, SV_SCROLL_RIGHT, SV_SCROLL_RIGHTDOWN, SV_SCROLL_RIGHTUP, SV_SCROLL_UP,
    SV_SETCHAR_AEND, SV_SETCHAR_AHP, SV_SETCHAR_AMANA, SV_SETCHAR_ATTRIB, SV_SETCHAR_DIR,
    SV_SETCHAR_ENDUR, SV_SETCHAR_GOLD, SV_SETCHAR_HP, SV_SETCHAR_ITEM, SV_SETCHAR_MANA,
    SV_SETCHAR_MODE, SV_SETCHAR_OBJ, SV_SETCHAR_PTS, SV_SETCHAR_SKILL, SV_SETCHAR_SPELL,
    SV_SETCHAR_WORN, SV_SETMAP, SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6, SV_SETORIGIN,
    SV_SETTARGET, SV_TICK, SV_UNIQUE,
};

/// Decode one zlib-compressed chunk from a continuous zlib stream.
pub fn inflate_chunk(z: &mut Decompress, input: &[u8]) -> Result<Vec<u8>, String> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::<u8>::new();
    let mut in_pos = 0usize;
    let mut scratch = [0u8; 8192];

    while in_pos <= input.len() {
        let before_in = z.total_in() as usize;
        let before_out = z.total_out() as usize;

        let status = z
            .decompress(&input[in_pos..], &mut scratch, FlushDecompress::Sync)
            .map_err(|e| format!("zlib inflate failed: {e}"))?;

        let after_in = z.total_in() as usize;
        let after_out = z.total_out() as usize;

        let consumed = after_in.saturating_sub(before_in);
        let produced = after_out.saturating_sub(before_out);

        if produced > 0 {
            out.extend_from_slice(&scratch[..produced]);
        }

        if consumed > 0 {
            in_pos += consumed;
            continue;
        }

        if produced > 0 {
            continue;
        }

        if in_pos < input.len() && status == Status::Ok {
            return Err("zlib inflate made no progress (truncated input?)".to_string());
        }
        break;
    }

    Ok(out)
}

/// Computes the total byte length of a variable-length `SV_SETMAP` command
/// given its flags byte and delta offset.
///
/// # Arguments
/// * `bytes` - The raw command bytes (starting at the opcode).
/// * `off` - The delta offset from the opcode (0 = absolute index follows).
/// * `lastn` - Mutable tracker for the last absolute tile index.
///
/// # Returns
/// * `Ok(length)` — the number of bytes this command occupies.
/// * `Err(msg)` on truncated input.
fn sv_setmap_len(bytes: &[u8], off: u8, lastn: &mut i32) -> Result<usize, String> {
    if bytes.len() < 2 {
        return Err("SV_SETMAP truncated (need at least 2 bytes)".to_string());
    }

    let mut p: usize;
    let n: i32;
    if off != 0 {
        n = *lastn + off as i32;
        p = 2;
    } else {
        if bytes.len() < 4 {
            return Err("SV_SETMAP truncated (need 4 bytes for index)".to_string());
        }
        n = u16::from_le_bytes([bytes[2], bytes[3]]) as i32;
        p = 4;
    }

    *lastn = n;

    let flags = bytes[1];
    if flags == 0 {
        return Err("SV_SETMAP has zero flags".to_string());
    }

    if flags & 1 != 0 {
        p += 2;
    }
    if flags & 2 != 0 {
        p += 4;
    }
    if flags & 4 != 0 {
        p += 4;
    }
    if flags & 8 != 0 {
        p += 2;
    }
    if flags & 16 != 0 {
        p += 1;
    }
    if flags & 32 != 0 {
        p += 4;
    }
    if flags & 64 != 0 {
        p += 5;
    }
    if flags & 128 != 0 {
        p += 1;
    }

    Ok(p)
}

/// Returns the byte length of a `SV_SETMAP3`-style lighting command with the
/// given tile count.
///
/// The new light packet format is: `[opcode, idx_lo, idx_hi, base_light, nibble_pairs...]`
/// — a 4-byte header followed by `cnt / 2` nibble-pair bytes.
///
/// Opcode-->cnt mapping (matching server-side `cl_light_*` functions):
/// * `SV_SETMAP4` (`cl_light_one`, 1 tile)  --> `sv_setmap3_len(0)` = 4
/// * `SV_SETMAP5` (`cl_light_three`, 3 tiles) --> `sv_setmap3_len(2)` = 5
/// * `SV_SETMAP6` (`cl_light_seven`, 7 tiles) --> `sv_setmap3_len(6)` = 7
/// * `SV_SETMAP3` (`cl_light_26`, 26 tiles) --> `sv_setmap3_len(26)` = 17
fn sv_setmap3_len(cnt: usize) -> usize {
    4 + (cnt / 2)
}

/// Returns the total byte length of the server command starting at
/// `bytes[0]`, advancing `last_setmap_n` for SV_SETMAP delta tracking.
///
/// # Arguments
/// * `bytes` - Slice starting at the command opcode.
/// * `last_setmap_n` - Running delta index for SV_SETMAP commands.
///
/// # Returns
/// * `Ok(length)` on success.
/// * `Err(msg)` on truncated or malformed input.
fn sv_cmd_len(bytes: &[u8], last_setmap_n: &mut i32) -> Result<usize, String> {
    if bytes.is_empty() {
        return Err("sv_cmd_len called with empty buffer".to_string());
    }

    let op = bytes[0];

    if (op & SV_SETMAP) != 0 {
        let off = op & !SV_SETMAP;
        return sv_setmap_len(bytes, off, last_setmap_n);
    }

    let len = match op {
        SV_SETCHAR_MODE => 2,
        SV_SETCHAR_ATTRIB => 8,
        SV_SETCHAR_SKILL => 8,
        SV_SETCHAR_HP => 13,
        SV_SETCHAR_ENDUR => 13,
        SV_SETCHAR_MANA => 13,
        SV_SETCHAR_AHP => 3,
        SV_SETCHAR_AEND => 3,
        SV_SETCHAR_AMANA => 3,
        SV_SETCHAR_DIR => 2,
        SV_SETCHAR_PTS => 13,
        SV_SETCHAR_GOLD => 13,
        SV_SETCHAR_ITEM => 9,
        SV_SETCHAR_WORN => 9,
        SV_SETCHAR_SPELL => 9,
        SV_SETCHAR_OBJ => 5,
        SV_SETMAP3 => sv_setmap3_len(26),
        SV_SETMAP4 => sv_setmap3_len(0),
        SV_SETMAP5 => sv_setmap3_len(2),
        SV_SETMAP6 => sv_setmap3_len(6),
        SV_SETORIGIN => 5,
        SV_TICK => 2,
        SV_SCROLL_RIGHT => 1,
        SV_SCROLL_LEFT => 1,
        SV_SCROLL_DOWN => 1,
        SV_SCROLL_UP => 1,
        SV_SCROLL_RIGHTDOWN => 1,
        SV_SCROLL_RIGHTUP => 1,
        SV_SCROLL_LEFTDOWN => 1,
        SV_SCROLL_LEFTUP => 1,
        SV_SETTARGET => 13,
        SV_PLAYSOUND => 13,
        SV_LOAD => 5,
        SV_UNIQUE => 9,
        SV_EXIT => {
            if bytes.len() >= 16 {
                16
            } else {
                5
            }
        }
        SV_IGNORE => {
            if bytes.len() < 5 {
                return Err("SV_IGNORE truncated (need 5 bytes for size)".to_string());
            }
            u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize
        }
        _ => 16,
    };

    Ok(len)
}

/// Splits a server tick payload into individual raw server command byte slices.
pub(super) fn split_tick_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::<Vec<u8>>::new();
    let mut idx = 0usize;
    let mut last_setmap_n: i32 = -1;

    while idx < payload.len() {
        let len = sv_cmd_len(&payload[idx..], &mut last_setmap_n)?;
        if len == 0 {
            return Err("sv_cmd_len returned 0".to_string());
        }
        if idx + len > payload.len() {
            let opcode = payload[idx];
            let remaining = payload.len() - idx;

            if opcode == SV_EXIT && remaining < 5 {
                let mut cmd = vec![0u8; 5];
                cmd[0] = SV_EXIT;
                cmd[1..1 + remaining.saturating_sub(1)]
                    .copy_from_slice(&payload[idx + 1..payload.len()]);
                out.push(cmd);
                break;
            }

            return Err(format!(
                "Truncated server command opcode={opcode} at offset={idx}: need {len} bytes, have {remaining}"
            ));
        }
        out.push(payload[idx..idx + len].to_vec());
        idx += len;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mag_core::constants::{SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6, SV_TICK};

    /// Verify the new light-packet header lengths match what the server encodes.
    /// Server: [opcode, idx_lo, idx_hi, base_light, nibble_pairs...]
    #[test]
    fn sv_setmap3_len_matches_server_buffers() {
        // SV_SETMAP4 = cl_light_one (1 tile): 4-byte packet, no nibble pairs
        assert_eq!(sv_setmap3_len(0), 4);
        // SV_SETMAP5 = cl_light_three (3 tiles): 4 header + 1 nibble byte = 5
        assert_eq!(sv_setmap3_len(2), 5);
        // SV_SETMAP6 = cl_light_seven (7 tiles): 4 header + 3 nibble bytes = 7
        assert_eq!(sv_setmap3_len(6), 7);
        // SV_SETMAP3 = cl_light_26 (26 tiles): 4 header + 13 nibble bytes = 17
        assert_eq!(sv_setmap3_len(26), 17);
    }

    /// `split_tick_payload` correctly splits a payload that mixes a SV_TICK (2
    /// bytes) with one of each light command, all using the new 4-byte header.
    #[test]
    fn split_tick_payload_light_packets_new_format() {
        let mut payload: Vec<u8> = Vec::new();

        // SV_TICK (2 bytes)
        payload.push(SV_TICK);
        payload.push(0x05);

        // SV_SETMAP4 / cl_light_one (4 bytes): [op, idx_lo, idx_hi, light]
        payload.push(SV_SETMAP4);
        payload.push(0x01); // idx = 1
        payload.push(0x00);
        payload.push(0x07); // light = 7

        // SV_SETMAP5 / cl_light_three (5 bytes): [op, idx_lo, idx_hi, light, nibble]
        payload.push(SV_SETMAP5);
        payload.push(0x04);
        payload.push(0x00);
        payload.push(0x05);
        payload.push(0x23); // nibble pair for tiles 5,6

        // SV_SETMAP6 / cl_light_seven (7 bytes): [op, idx_lo, idx_hi, light, 3 nibbles]
        payload.push(SV_SETMAP6);
        payload.push(0x0A);
        payload.push(0x00);
        payload.push(0x03);
        payload.push(0x45);
        payload.push(0x67);
        payload.push(0x89);

        // SV_SETMAP3 / cl_light_26 (17 bytes): [op, idx_lo, idx_hi, light, 13 nibbles]
        payload.push(SV_SETMAP3);
        payload.push(0x10);
        payload.push(0x00);
        payload.push(0x0F);
        for _ in 0..13 {
            payload.push(0xAB);
        }

        let cmds = split_tick_payload(&payload).expect("should parse without error");
        assert_eq!(cmds.len(), 5);
        assert_eq!(cmds[0].len(), 2); // SV_TICK
        assert_eq!(cmds[1].len(), 4); // SV_SETMAP4
        assert_eq!(cmds[2].len(), 5); // SV_SETMAP5
        assert_eq!(cmds[3].len(), 7); // SV_SETMAP6
        assert_eq!(cmds[4].len(), 17); // SV_SETMAP3
    }

    /// Ensure a payload containing ONLY an old-format 3-byte SV_SETMAP4 produces
    /// a truncation error (guards against regression to the old length).
    #[test]
    fn split_tick_payload_rejects_old_3byte_light_packet() {
        let payload = vec![SV_SETMAP4, 0x01, 0x00]; // only 3 bytes — old format
        let result = split_tick_payload(&payload);
        assert!(result.is_err(), "3-byte SV_SETMAP4 should be rejected");
    }
}
