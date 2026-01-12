/// Secret key for xcrypt function (from svr_tick.cpp)
/// C version: static char secret[256] = "..." (string + null terminator = 256 bytes)
const SECRET: &[u8] = b"Ifhjf64hH8sa,-#39ddj843tvxcv0434dvsdc40G#34Trefc349534Y5#34trecerr943\
5#erZt#eA534#5erFtw#Trwec,9345mwrxm gerte-534lMIZDN(/dn8sfn8&DBDB/D&s\
8efnsd897)DDzD'D'D''Dofs,t0943-rg-gdfg-gdf.t,e95.34u.5retfrh.wretv.56\
9v4#asf.59m(D)/ND/DDLD;gd+dsa,fw9r,x  OD(98snfsf\0";

/// Port of `xcrypt` from `svr_tick.cpp`
/// Encryption function for challenge verification
pub fn xcrypt(val: u32) -> u32 {
    let mut res: u32 = 0;

    // Direct port of C implementation - SECRET now has 256 bytes (string + null terminator)
    res = res.wrapping_add(SECRET[(val & 255) as usize] as u32);
    res = res.wrapping_add((SECRET[((val >> 8) & 255) as usize] as u32) << 8);
    res = res.wrapping_add((SECRET[((val >> 16) & 255) as usize] as u32) << 16);
    res = res.wrapping_add((SECRET[((val >> 24) & 255) as usize] as u32) << 24);

    res ^= 0x5a7ce52e;

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xcrypt_basic_functionality() {
        // Test basic functionality with known values
        let result1 = xcrypt(0);
        let result2 = xcrypt(1);
        let result3 = xcrypt(0xFFFFFFFF);

        // Results should be deterministic
        assert_eq!(xcrypt(0), result1);
        assert_eq!(xcrypt(1), result2);
        assert_eq!(xcrypt(0xFFFFFFFF), result3);
    }

    #[test]
    fn test_xcrypt_boundary_values() {
        // Test the problematic case that was causing the crash
        // When any byte of val is 255, it should not panic
        xcrypt(0xFF); // Last byte is 255
        xcrypt(0xFF00); // Second byte is 255
        xcrypt(0xFF0000); // Third byte is 255
        xcrypt(0xFF000000); // First byte is 255
        xcrypt(0xFFFFFFFF); // All bytes are 255
    }

    #[test]
    fn test_xcrypt_edge_cases() {
        // Test various edge cases
        xcrypt(0x00000000);
        xcrypt(0x12345678);
        xcrypt(0xABCDEF01);
        xcrypt(0x80808080);
        xcrypt(0x7F7F7F7F);
    }

    #[test]
    fn test_xcrypt_consistency() {
        // Test that the same input always produces the same output
        let test_values = [0, 1, 255, 256, 65535, 65536, 0xFFFFFFFF];

        for &val in &test_values {
            let result1 = xcrypt(val);
            let result2 = xcrypt(val);
            assert_eq!(result1, result2, "xcrypt({}) should be deterministic", val);
        }
    }

    #[test]
    fn test_xcrypt_specific_crash_case() {
        // Test the specific case from the log that was causing crashes
        // The challenge response 391DC658 from the log
        let crash_value = 0x391DC658;
        let result = xcrypt(crash_value);

        // Should not panic and should return a valid u32
        assert!(result != crash_value); // Should be transformed
    }

    #[test]
    fn test_xcrypt_all_byte_combinations() {
        // Test all possible single byte values to ensure no panics
        for i in 0..=255u32 {
            xcrypt(i);
            xcrypt(i << 8);
            xcrypt(i << 16);
            xcrypt(i << 24);
        }
    }

    #[test]
    fn test_secret_array_bounds() {
        // Verify our SECRET array has the expected length (C string + null terminator)
        assert_eq!(SECRET.len(), 256);

        // Verify that direct indexing with val & 255 works correctly (0-255 are all valid)
        for i in 0..=255u32 {
            let index = (i & 255) as usize;
            assert!(
                index < SECRET.len(),
                "Index {} should be less than SECRET.len() {}",
                index,
                SECRET.len()
            );
            // This should not panic
            let _byte = SECRET[index];
        }
    }
}
