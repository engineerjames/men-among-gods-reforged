pub fn c_string_to_str(c_string: &[u8]) -> &str {
    let end = c_string
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(c_string.len());
    std::str::from_utf8(&c_string[..end]).unwrap_or("*UNKNOWN*")
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::c_string_to_str;

    #[test]
    fn test_c_string_to_str() {
        let c_string = b"Hello, World!\0Extra data";
        let result = c_string_to_str(c_string);
        assert_eq!(result, "Hello, World!");

        let c_string_no_null = b"Hello, World!";
        let result_no_null = c_string_to_str(c_string_no_null);
        assert_eq!(result_no_null, "Hello, World!");

        let c_string_invalid_utf8 = b"Hello, \xFFWorld!\0";
        let result_invalid_utf8 = c_string_to_str(c_string_invalid_utf8);
        assert_eq!(result_invalid_utf8, "*UNKNOWN*");
    }

    #[test]
    fn test_empty_string() {
        let c_string = b"\0";
        let result = c_string_to_str(c_string);
        assert_eq!(result, "");
    }

    #[test]
    fn test_empty_buffer() {
        let c_string = b"";
        let result = c_string_to_str(c_string);
        assert_eq!(result, "");
    }

    #[test]
    fn test_null_in_middle() {
        let c_string = b"First\0Second\0Third";
        let result = c_string_to_str(c_string);
        assert_eq!(result, "First");
    }

    #[test]
    fn test_special_characters() {
        let c_string = b"Test\t\n\r\0";
        let result = c_string_to_str(c_string);
        assert_eq!(result, "Test\t\n\r");
    }

    #[test]
    fn test_unicode_valid() {
        let c_string = "Hello 世界\0Extra".as_bytes();
        let result = c_string_to_str(c_string);
        assert_eq!(result, "Hello 世界");
    }

    #[test]
    fn test_max_length_string() {
        let mut long_string = vec![b'a'; 100];
        long_string.push(0);
        long_string.extend_from_slice(b"extra");
        let result = c_string_to_str(&long_string);
        assert_eq!(result.len(), 100);
        assert!(result.chars().all(|c| c == 'a'));
    }

    #[test]
    fn test_invalid_utf8_sequences() {
        // Invalid UTF-8 sequences
        let sequences = [
            &b"\x80\0"[..],             // Continuation byte without start
            &b"\xC0\x80\0"[..],         // Overlong encoding
            &b"\xED\xA0\x80\0"[..],     // Surrogate half
            &b"\xF4\x90\x80\x80\0"[..], // Out of range
        ];

        for seq in &sequences {
            let result = c_string_to_str(seq);
            assert_eq!(result, "*UNKNOWN*", "Failed for sequence: {:?}", seq);
        }
    }
}
