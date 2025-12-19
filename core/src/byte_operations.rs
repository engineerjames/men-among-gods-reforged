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

macro_rules! read_array {
    ($bytes:expr, $offset:expr, $size:expr) => {{
        let mut arr = [0u8; $size];
        arr.copy_from_slice(&$bytes[$offset..$offset + $size]);
        $offset += $size;
        arr
    }};
}
