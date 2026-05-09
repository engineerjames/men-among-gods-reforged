//! Shared KeyDB key schema and helpers for externally managed text data.
//!
//! This module centralises the storage format for text content such as
//! badwords so the API, server, and utility binaries agree on keys,
//! validation, and bincode encoding.

/// KeyDB key holding the bincode-encoded badwords list.
pub const BADWORDS_KEY: &str = "game:badwords";

/// KeyDB counter incremented after successful badwords writes.
pub const BADWORDS_VERSION_KEY: &str = "game:meta:badwords:version";

/// KeyDB key the API writes a JSON text-reload payload into.
pub const TEXT_RELOAD_REQUEST_KEY: &str = "game:text:reload_request";

/// Pub/sub channel reserved for text reload notifications.
pub const TEXT_RELOAD_PUBSUB_CHANNEL: &str = "game:text:reload";

/// Maximum number of canonical badword entries accepted in one list.
pub const MAX_BADWORDS: usize = 4096;

/// Maximum byte length of a single canonical badword entry.
pub const MAX_BADWORD_LEN: usize = 64;

/// Minimum byte length of a canonical badword entry.
pub const MIN_BADWORD_LEN: usize = 3;

/// Error returned by text-store helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextStoreError {
    /// The supplied badword entry is empty after canonicalization.
    EmptyEntry,
    /// The supplied badword entry is shorter than the minimum allowed length.
    EntryTooShort {
        /// The canonical entry that failed validation.
        entry: String,
        /// Minimum accepted byte length.
        min_len: usize,
    },
    /// The supplied badword entry exceeds the maximum allowed length.
    EntryTooLong {
        /// The canonical entry that failed validation.
        entry: String,
        /// Maximum accepted byte length.
        max_len: usize,
    },
    /// The supplied badword entry contains unsupported control characters.
    ControlCharacter {
        /// The offending character.
        character: char,
    },
    /// The badwords list has too many entries.
    TooManyEntries {
        /// Number of entries supplied.
        count: usize,
        /// Maximum accepted entry count.
        max: usize,
    },
    /// Encoding badwords to bincode failed.
    Encode(String),
    /// Decoding badwords from bincode failed.
    Decode(String),
}

impl std::fmt::Display for TextStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyEntry => write!(f, "badword entry is empty"),
            Self::EntryTooShort { entry, min_len } => write!(
                f,
                "badword entry \"{}\" is shorter than {} bytes",
                entry, min_len
            ),
            Self::EntryTooLong { entry, max_len } => {
                write!(f, "badword entry \"{}\" exceeds {} bytes", entry, max_len)
            }
            Self::ControlCharacter { character } => {
                write!(
                    f,
                    "badword entry contains control character {:?}",
                    character
                )
            }
            Self::TooManyEntries { count, max } => {
                write!(f, "badword list has {} entries; maximum is {}", count, max)
            }
            Self::Encode(msg) => write!(f, "badwords encode failed: {}", msg),
            Self::Decode(msg) => write!(f, "badwords decode failed: {}", msg),
        }
    }
}

impl std::error::Error for TextStoreError {}

/// Build the KeyDB key for a text-reload status entry.
///
/// # Arguments
///
/// * `request_id` - The request identifier returned by the API.
///
/// # Returns
///
/// * The fully-formatted status key.
pub fn text_reload_status_key(request_id: &str) -> String {
    format!("game:text:reload_status:{}", request_id)
}

/// Canonicalize and validate a single badword entry.
///
/// Whitespace is removed and ASCII letters are lowercased to preserve the
/// legacy matching semantics used by the original server content.
///
/// # Arguments
///
/// * `raw` - Raw badword text supplied by an operator.
///
/// # Returns
///
/// * `Ok(String)` with the canonical entry.
/// * `Err(TextStoreError)` when the entry is invalid.
pub fn normalize_badword(raw: &str) -> Result<String, TextStoreError> {
    for character in raw.chars() {
        if character.is_control() && !character.is_whitespace() {
            return Err(TextStoreError::ControlCharacter { character });
        }
    }

    let normalized = raw
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();

    if normalized.is_empty() {
        return Err(TextStoreError::EmptyEntry);
    }
    if normalized.len() < MIN_BADWORD_LEN {
        return Err(TextStoreError::EntryTooShort {
            entry: normalized,
            min_len: MIN_BADWORD_LEN,
        });
    }
    if normalized.len() > MAX_BADWORD_LEN {
        return Err(TextStoreError::EntryTooLong {
            entry: normalized,
            max_len: MAX_BADWORD_LEN,
        });
    }

    Ok(normalized)
}

/// Canonicalize, validate, and deduplicate a badwords list.
///
/// Stable input order is preserved for the first occurrence of each canonical
/// entry.
///
/// # Arguments
///
/// * `raw_words` - Raw badword entries supplied by an operator or loaded from KeyDB.
///
/// # Returns
///
/// * `Ok(Vec<String>)` containing canonical unique entries.
/// * `Err(TextStoreError)` when any entry or the list size is invalid.
pub fn normalize_badwords(raw_words: &[String]) -> Result<Vec<String>, TextStoreError> {
    if raw_words.len() > MAX_BADWORDS {
        return Err(TextStoreError::TooManyEntries {
            count: raw_words.len(),
            max: MAX_BADWORDS,
        });
    }

    let mut words = Vec::with_capacity(raw_words.len());
    for raw in raw_words {
        let word = normalize_badword(raw)?;
        if !words.iter().any(|existing| existing == &word) {
            words.push(word);
        }
    }

    Ok(words)
}

/// Encode a canonical badwords list to its KeyDB byte representation.
///
/// # Arguments
///
/// * `words` - Canonical badword entries to encode.
///
/// # Returns
///
/// * `Ok(Vec<u8>)` on success.
/// * `Err(TextStoreError::Encode)` when bincode encoding fails.
pub fn encode_badwords(words: &[String]) -> Result<Vec<u8>, TextStoreError> {
    bincode::encode_to_vec(words.to_vec(), bincode::config::standard())
        .map_err(|err| TextStoreError::Encode(err.to_string()))
}

/// Decode badwords from their KeyDB byte representation.
///
/// # Arguments
///
/// * `bytes` - Raw bincode bytes loaded from KeyDB.
///
/// # Returns
///
/// * `Ok(Vec<String>)` containing canonical unique entries.
/// * `Err(TextStoreError)` when decoding or normalization fails.
pub fn decode_badwords(bytes: &[u8]) -> Result<Vec<String>, TextStoreError> {
    let (words, consumed): (Vec<String>, usize) =
        bincode::decode_from_slice(bytes, bincode::config::standard())
            .map_err(|err| TextStoreError::Decode(err.to_string()))?;
    if consumed != bytes.len() {
        return Err(TextStoreError::Decode(format!(
            "trailing bytes after Vec<String> (consumed {} of {})",
            consumed,
            bytes.len()
        )));
    }
    normalize_badwords(&words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_badword_removes_whitespace_and_lowercases() {
        assert_eq!(normalize_badword(" Bad Word ").unwrap(), "badword");
    }

    #[test]
    fn normalize_badword_rejects_short_entries() {
        assert!(matches!(
            normalize_badword("ab"),
            Err(TextStoreError::EntryTooShort { .. })
        ));
    }

    #[test]
    fn normalize_badwords_deduplicates_stably() {
        let raw = vec!["Alpha".to_owned(), "bravo".to_owned(), "al pha".to_owned()];

        let words = normalize_badwords(&raw).unwrap();

        assert_eq!(words, vec!["alpha".to_owned(), "bravo".to_owned()]);
    }

    #[test]
    fn encode_decode_badwords_roundtrips() {
        let words = vec!["alpha".to_owned(), "bravo".to_owned()];

        let bytes = encode_badwords(&words).unwrap();
        let decoded = decode_badwords(&bytes).unwrap();

        assert_eq!(decoded, words);
    }

    #[test]
    fn text_reload_status_key_uses_request_id() {
        assert_eq!(
            text_reload_status_key("req-1"),
            "game:text:reload_status:req-1"
        );
    }
}
