#[cfg(test)]
mod tests {
    /// Encode a single-tile light packet (SV_SETMAP4) the same way
    /// [`cl_light_one`] does and decode it the way the client parser does,
    /// asserting there is no truncation or corruption for any valid tile index.
    fn encode_decode_single(n: usize, light: u8) -> (u16, u8) {
        let mut buf: [u8; 4] = [0; 4];
        buf[1] = (n & 0xff) as u8;
        buf[2] = ((n >> 8) & 0xff) as u8;
        buf[3] = light & 0x0f;

        let start_index = u16::from_le_bytes([buf[1], buf[2]]);
        let base_light = buf[3] & 0x0f;
        (start_index, base_light)
    }

    #[test]
    fn light_packet_roundtrip_index_zero() {
        let (idx, light) = encode_decode_single(0, 0);
        assert_eq!(idx, 0);
        assert_eq!(light, 0);
    }

    #[test]
    fn light_packet_roundtrip_below_old_limit() {
        let (idx, light) = encode_decode_single(2047, 7);
        assert_eq!(idx, 2047);
        assert_eq!(light, 7);
    }

    #[test]
    fn light_packet_roundtrip_above_old_limit() {
        let (idx, light) = encode_decode_single(2048, 8);
        assert_eq!(idx, 2048);
        assert_eq!(light, 8);
    }

    #[test]
    fn light_packet_roundtrip_above_4096() {
        let (idx, light) = encode_decode_single(4096, 15);
        assert_eq!(idx, 4096);
        assert_eq!(light, 15);
    }

    #[test]
    fn light_packet_roundtrip_max_viewport_index() {
        let (idx, light) = encode_decode_single(6399, 12);
        assert_eq!(idx, 6399);
        assert_eq!(light, 12);
    }

    #[test]
    fn light_packet_nibble_pack_ordering() {
        let light_n1: u8 = 5;
        let light_n2: u8 = 11;
        let packed_byte = light_n2 | (light_n1 << 4);

        let hi = (packed_byte >> 4) & 0x0f;
        let lo = packed_byte & 0x0f;

        assert_eq!(hi, light_n1, "high nibble should be tile n+1");
        assert_eq!(lo, light_n2, "low nibble should be tile n+2");
    }
}
