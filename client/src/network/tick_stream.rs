use flate2::{Decompress, FlushDecompress, Status};
use mag_core::server_commands::ServerCommandType;

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

/// Splits a server tick payload into individual raw server command byte slices.
pub(super) fn split_tick_payload(payload: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::<Vec<u8>>::new();
    let mut idx = 0usize;
    let mut last_setmap_n: i32 = -1;

    while idx < payload.len() {
        let len = ServerCommandType::get_expected_length(&payload[idx..], &mut last_setmap_n)?;
        if len == 0 {
            return Err("sv_cmd_len returned 0".to_string());
        }
        if idx + len > payload.len() {
            let opcode = ServerCommandType::from(payload[idx]);
            let remaining = payload.len() - idx;

            if opcode == ServerCommandType::Exit && remaining < 5 {
                let mut cmd = vec![0u8; 5];
                cmd[0] = ServerCommandType::Exit as u8;
                cmd[1..1 + remaining.saturating_sub(1)]
                    .copy_from_slice(&payload[idx + 1..payload.len()]);
                out.push(cmd);
                break;
            }

            return Err(format!(
                "Truncated server command opcode={opcode:?} at offset={idx}: need {len} bytes, have {remaining}"
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

    /// `split_tick_payload` correctly splits a payload that mixes a SV_TICK (2
    /// bytes) with one of each light command, all using the new 4-byte header.
    #[test]
    fn split_tick_payload_light_packets_new_format() {
        let mut payload: Vec<u8> = Vec::new();

        // SV_TICK (2 bytes)
        payload.push(ServerCommandType::Tick as u8);
        payload.push(0x05);

        // SV_SETMAP4 / cl_light_one (4 bytes): [op, idx_lo, idx_hi, light]
        payload.push(ServerCommandType::SetMap4 as u8);
        payload.push(0x01); // idx = 1
        payload.push(0x00);
        payload.push(0x07); // light = 7

        // SV_SETMAP5 / cl_light_three (5 bytes): [op, idx_lo, idx_hi, light, nibble]
        payload.push(ServerCommandType::SetMap5 as u8);
        payload.push(0x04);
        payload.push(0x00);
        payload.push(0x05);
        payload.push(0x23); // nibble pair for tiles 5,6

        // SV_SETMAP6 / cl_light_seven (7 bytes): [op, idx_lo, idx_hi, light, 3 nibbles]
        payload.push(ServerCommandType::SetMap6 as u8);
        payload.push(0x0A);
        payload.push(0x00);
        payload.push(0x03);
        payload.push(0x45);
        payload.push(0x67);
        payload.push(0x89);

        // SV_SETMAP3 / cl_light_26 (17 bytes): [op, idx_lo, idx_hi, light, 13 nibbles]
        payload.push(ServerCommandType::SetMap3 as u8);
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
        let payload = vec![ServerCommandType::SetMap4 as u8, 0x01, 0x00]; // only 3 bytes — old format
        let result = split_tick_payload(&payload);
        assert!(result.is_err(), "3-byte SV_SETMAP4 should be rejected");
    }
}
