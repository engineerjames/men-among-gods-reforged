use flate2::{Decompress, FlushDecompress, Status};

/// Decode one zlib-compressed chunk from a continuous zlib stream.
///
/// The server uses a per-connection `ZlibEncoder` and sends only the newly
/// produced bytes each tick (i.e. it's a single streaming zlib payload split
/// into chunks). Therefore we must keep a persistent `Decompress` state.
pub fn inflate_chunk(z: &mut Decompress, input: &[u8]) -> Result<Vec<u8>, String> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::<u8>::new();
    let mut in_pos = 0usize;
    let mut scratch = [0u8; 8192];

    while in_pos < input.len() {
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

        if consumed == 0 {
            // Avoid an infinite loop if the inflater can't make forward progress.
            // This can happen if the input is truncated mid-stream.
            if status == Status::Ok && produced == 0 {
                return Err("zlib inflate made no progress (truncated input?)".to_string());
            }
            break;
        }

        in_pos += consumed;
    }

    Ok(out)
}
use mag_core::constants::{
    SV_IGNORE, SV_LOAD, SV_PLAYSOUND, SV_SCROLL_DOWN, SV_SCROLL_LEFT, SV_SCROLL_LEFTDOWN,
    SV_SCROLL_LEFTUP, SV_SCROLL_RIGHT, SV_SCROLL_RIGHTDOWN, SV_SCROLL_RIGHTUP, SV_SCROLL_UP,
    SV_SETCHAR_AEND, SV_SETCHAR_AHP, SV_SETCHAR_AMANA, SV_SETCHAR_ATTRIB, SV_SETCHAR_DIR,
    SV_SETCHAR_ENDUR, SV_SETCHAR_GOLD, SV_SETCHAR_HP, SV_SETCHAR_ITEM, SV_SETCHAR_MANA,
    SV_SETCHAR_MODE, SV_SETCHAR_OBJ, SV_SETCHAR_PTS, SV_SETCHAR_SKILL, SV_SETCHAR_SPELL,
    SV_SETCHAR_WORN, SV_SETMAP, SV_SETMAP3, SV_SETMAP4, SV_SETMAP5, SV_SETMAP6, SV_SETORIGIN,
    SV_SETTARGET, SV_TICK, SV_UNIQUE,
};

fn sv_setmap_len(bytes: &[u8], off: u8, lastn: &mut i32) -> Result<usize, String> {
    if bytes.len() < 2 {
        return Err("SV_SETMAP truncated (need at least 2 bytes)".to_string());
    }

    // Mirrors `socket.c::sv_setmap`.
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

    // Size accounting only.
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

fn sv_setmap3_len(cnt: usize) -> usize {
    // Mirrors `socket.c::sv_setmap3`: returns p where p starts at 3 and increments once per two
    // tiles covered by `cnt`.
    3 + (cnt / 2)
}

fn sv_cmd_len(bytes: &[u8], last_setmap_n: &mut i32) -> Result<usize, String> {
    if bytes.is_empty() {
        return Err("sv_cmd_len called with empty buffer".to_string());
    }

    let op = bytes[0];

    // Special case: any opcode with the SV_SETMAP (0x80) bit set is a setmap packet.
    // The lower 7 bits carry the delta offset.
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
        SV_IGNORE => {
            if bytes.len() < 5 {
                return Err("SV_IGNORE truncated (need 5 bytes for size)".to_string());
            }
            u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize
        }

        // Most remaining commands are fixed 16 bytes in the original client.
        // Unknown opcodes should be treated as errors (the original client exits).
        _ => 16,
    };

    Ok(len)
}

pub(super) fn split_tick_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::<Vec<u8>>::new();
    let mut idx = 0usize;

    // Mirrors `tick_do`: reset lastn before scanning each tick payload.
    let mut last_setmap_n: i32 = -1;

    while idx < payload.len() {
        let len = sv_cmd_len(&payload[idx..], &mut last_setmap_n)?;
        if len == 0 {
            return Err("sv_cmd_len returned 0".to_string());
        }
        if idx + len > payload.len() {
            return Err(format!(
                "Truncated server command: need {len} bytes, have {}",
                payload.len() - idx
            ));
        }
        out.push(payload[idx..idx + len].to_vec());
        idx += len;
    }

    Ok(out)
}
