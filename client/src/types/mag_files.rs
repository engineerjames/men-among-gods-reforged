use std::{
    fs::File,
    io::{self, Read, Write},
    mem::MaybeUninit,
    path::Path,
};

use crate::types::{player_data::PlayerData, save_file::SaveFile};

/// Decode a fixed-size NUL-terminated ASCII buffer into a trimmed string.
pub fn fixed_ascii_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    String::from_utf8_lossy(slice).trim().to_string()
}

/// Read a plain-old-data struct from a reader.
fn read_struct<T>(mut reader: impl Read) -> io::Result<T> {
    let mut value = MaybeUninit::<T>::uninit();
    let value_bytes = unsafe {
        std::slice::from_raw_parts_mut(value.as_mut_ptr() as *mut u8, std::mem::size_of::<T>())
    };
    reader.read_exact(value_bytes)?;
    Ok(unsafe { value.assume_init() })
}

/// Write a plain-old-data struct to a writer.
fn write_struct<T>(mut writer: impl Write, value: &T) -> io::Result<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
    };
    writer.write_all(bytes)
}

/// Load a character file (our `.mag`) maintaining the original `.moa` binary layout:
/// `SaveFile` followed by `PlayerData`.
/// Load a character file (`.mag`) and return its save and player data.
pub fn load_character_file(path: &Path) -> io::Result<(SaveFile, PlayerData)> {
    let mut file = File::open(path)?;
    let save_file: SaveFile = read_struct(&mut file)?;

    // Our character `.mag` format is a simple binary dump of:
    // `SaveFile` followed by `PlayerData`.
    let player_data: PlayerData = read_struct(&mut file)?;

    // Critical validation: a character save must include a character name.
    // The authoritative name is stored in `pdata.cname`.
    if fixed_ascii_to_string(&player_data.cname).is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid .mag file: missing character name (pdata.cname)",
        ));
    }

    // Be strict: refuse trailing bytes so silent layout mismatches don't go unnoticed.
    let mut trailing = [0u8; 1];
    if file.read(&mut trailing)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unexpected trailing bytes after PlayerData in .mag",
        ));
    }

    Ok((save_file, player_data))
}

/// Save a character file (our `.mag`) maintaining the original `.moa` binary layout:
/// `SaveFile` followed by `PlayerData`.
/// Save a character file (`.mag`) with save and player data.
pub fn save_character_file(
    path: &Path,
    save_file: &SaveFile,
    player_data: &PlayerData,
) -> io::Result<()> {
    let mut file = File::create(path)?;
    write_struct(&mut file, save_file)?;
    write_struct(&mut file, player_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    /// Create a unique temp directory path for tests.
    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{pid}_{nanos}"))
    }

    /// View any value as its raw byte representation.
    fn as_bytes<T>(value: &T) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
        }
    }

    #[test]
    /// Verify fixed ASCII decoding trims at NUL and whitespace.
    fn fixed_ascii_to_string_stops_at_nul_and_trims() {
        let bytes = b" hello\0world";
        assert_eq!(fixed_ascii_to_string(bytes), "hello");
    }

    #[test]
    /// Ensure character file save/load preserves raw bytes.
    fn mag_character_file_roundtrip_preserves_bytes() {
        let dir = unique_temp_dir("mag_char");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.mag");

        let mut save_file = SaveFile::default();
        save_file.usnr = 123;
        save_file.pass1 = 456;
        save_file.pass2 = 789;
        save_file.race = 76;
        save_file.name[0..4].copy_from_slice(b"Test");

        let mut player_data = PlayerData::default();
        player_data.changed = 1;
        player_data.hide = 1;
        player_data.show_names = 1;
        player_data.cname[0..4].copy_from_slice(b"Test");
        player_data.desc[0..11].copy_from_slice(b"Hello world");

        // Ensure xbuttons are part of the roundtrip.
        player_data.skill_buttons[0].set_skill_nr(123);
        player_data.skill_buttons[0].set_name("Fire");
        player_data.skill_buttons[7].set_skill_nr(999);
        player_data.skill_buttons[7].set_name("Heal");

        save_character_file(&path, &save_file, &player_data).unwrap();
        let (loaded_save, loaded_pdata) = load_character_file(&path).unwrap();

        assert_eq!(as_bytes(&loaded_save), as_bytes(&save_file));
        assert_eq!(as_bytes(&loaded_pdata), as_bytes(&player_data));

        let _ = fs::remove_dir_all(&dir);
    }
}
