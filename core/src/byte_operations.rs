// Helper macros for reading different types
macro_rules! read_u8 {
    ($bytes:expr, $offset:expr) => {{
        let val = $bytes[$offset];
        $offset += 1;
        val
    }};
}

macro_rules! read_i8 {
    ($bytes:expr, $offset:expr) => {{
        let val = $bytes[$offset] as i8;
        $offset += 1;
        val
    }};
}

macro_rules! read_u16 {
    ($bytes:expr, $offset:expr) => {{
        let val = u16::from_le_bytes([$bytes[$offset], $bytes[$offset + 1]]);
        $offset += 2;
        val
    }};
}

macro_rules! read_i16 {
    ($bytes:expr, $offset:expr) => {{
        let val = i16::from_le_bytes([$bytes[$offset], $bytes[$offset + 1]]);
        $offset += 2;
        val
    }};
}

macro_rules! read_u32 {
    ($bytes:expr, $offset:expr) => {{
        let val = u32::from_le_bytes([
            $bytes[$offset],
            $bytes[$offset + 1],
            $bytes[$offset + 2],
            $bytes[$offset + 3],
        ]);
        $offset += 4;
        val
    }};
}

macro_rules! read_i32 {
    ($bytes:expr, $offset:expr) => {{
        let val = i32::from_le_bytes([
            $bytes[$offset],
            $bytes[$offset + 1],
            $bytes[$offset + 2],
            $bytes[$offset + 3],
        ]);
        $offset += 4;
        val
    }};
}

macro_rules! read_i64 {
    ($bytes:expr, $offset:expr) => {{
        let val = i64::from_le_bytes([
            $bytes[$offset],
            $bytes[$offset + 1],
            $bytes[$offset + 2],
            $bytes[$offset + 3],
            $bytes[$offset + 4],
            $bytes[$offset + 5],
            $bytes[$offset + 6],
            $bytes[$offset + 7],
        ]);
        $offset += 8;
        val
    }};
}

macro_rules! read_u64 {
    ($bytes:expr, $offset:expr) => {{
        let val = u64::from_le_bytes([
            $bytes[$offset],
            $bytes[$offset + 1],
            $bytes[$offset + 2],
            $bytes[$offset + 3],
            $bytes[$offset + 4],
            $bytes[$offset + 5],
            $bytes[$offset + 6],
            $bytes[$offset + 7],
        ]);
        $offset += 8;
        val
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_read_u8() {
        let bytes = [42u8, 100, 200];
        let mut offset = 0;

        assert_eq!(read_u8!(bytes, offset), 42);
        assert_eq!(offset, 1);
        assert_eq!(read_u8!(bytes, offset), 100);
        assert_eq!(offset, 2);
        assert_eq!(read_u8!(bytes, offset), 200);
        assert_eq!(offset, 3);
    }

    #[test]
    fn test_read_i8() {
        let bytes = [127u8, 255, 128, 0];
        let mut offset = 0;

        assert_eq!(read_i8!(bytes, offset), 127);
        assert_eq!(offset, 1);
        assert_eq!(read_i8!(bytes, offset), -1);
        assert_eq!(offset, 2);
        assert_eq!(read_i8!(bytes, offset), -128);
        assert_eq!(offset, 3);
        assert_eq!(read_i8!(bytes, offset), 0);
        assert_eq!(offset, 4);
    }

    #[test]
    fn test_read_u16() {
        // Little endian: 0x0201
        let bytes = [0x01, 0x02, 0xFF, 0xFF];
        let mut offset = 0;

        assert_eq!(read_u16!(bytes, offset), 0x0201);
        assert_eq!(offset, 2);
        assert_eq!(read_u16!(bytes, offset), 0xFFFF);
        assert_eq!(offset, 4);
    }

    #[test]
    fn test_read_i16() {
        // Little endian
        let bytes = [0x01, 0x02, 0xFF, 0xFF, 0x00, 0x80];
        let mut offset = 0;

        assert_eq!(read_i16!(bytes, offset), 0x0201);
        assert_eq!(offset, 2);
        assert_eq!(read_i16!(bytes, offset), -1);
        assert_eq!(offset, 4);
        assert_eq!(read_i16!(bytes, offset), -32768);
        assert_eq!(offset, 6);
    }

    #[test]
    fn test_read_u32() {
        // Little endian: 0x04030201
        let bytes = [0x01, 0x02, 0x03, 0x04, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut offset = 0;

        assert_eq!(read_u32!(bytes, offset), 0x04030201);
        assert_eq!(offset, 4);
        assert_eq!(read_u32!(bytes, offset), 0xFFFFFFFF);
        assert_eq!(offset, 8);
    }

    #[test]
    fn test_read_i32() {
        // Little endian
        let bytes = [0x01, 0x02, 0x03, 0x04, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut offset = 0;

        assert_eq!(read_i32!(bytes, offset), 0x04030201);
        assert_eq!(offset, 4);
        assert_eq!(read_i32!(bytes, offset), -1);
        assert_eq!(offset, 8);
    }

    #[test]
    fn test_read_i64() {
        // Little endian
        let bytes = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ];
        let mut offset = 0;

        assert_eq!(read_i64!(bytes, offset), 0x0807060504030201);
        assert_eq!(offset, 8);
        assert_eq!(read_i64!(bytes, offset), -1);
        assert_eq!(offset, 16);
    }

    #[test]
    fn test_read_u64() {
        // Little endian
        let bytes = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ];
        let mut offset = 0;

        assert_eq!(read_u64!(bytes, offset), 0x0807060504030201);
        assert_eq!(offset, 8);
        assert_eq!(read_u64!(bytes, offset), 0xFFFFFFFFFFFFFFFF);
        assert_eq!(offset, 16);
    }

    #[test]
    fn test_mixed_reads() {
        let bytes = [
            0x42, // u8: 66
            0xFF, // i8: -1
            0x34, 0x12, // u16: 0x1234
            0xFF, 0xFF, // i16: -1
            0x78, 0x56, 0x34, 0x12, // u32: 0x12345678
        ];
        let mut offset = 0;

        assert_eq!(read_u8!(bytes, offset), 66);
        assert_eq!(offset, 1);
        assert_eq!(read_i8!(bytes, offset), -1);
        assert_eq!(offset, 2);
        assert_eq!(read_u16!(bytes, offset), 0x1234);
        assert_eq!(offset, 4);
        assert_eq!(read_i16!(bytes, offset), -1);
        assert_eq!(offset, 6);
        assert_eq!(read_u32!(bytes, offset), 0x12345678);
        assert_eq!(offset, 10);
    }
}
