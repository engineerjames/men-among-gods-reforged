pub fn c_string_to_str(c_string: &[u8]) -> &str {
    let end = c_string
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(c_string.len());
    std::str::from_utf8(&c_string[..end]).unwrap_or("*UNKNOWN*")
}

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
}
