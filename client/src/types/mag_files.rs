use std::{
    fs::File,
    io::{self, Read, Write},
    mem::MaybeUninit,
    path::{Path, PathBuf},
};

use crate::types::{player_data::PlayerData, save_file::SaveFile};

const MAG_DAT_FILENAME: &str = "mag.dat";

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MagDatV1 {
    /// Magic/version marker for format detection.
    pub magic: [u8; 4],
    /// Format version (little-endian on disk).
    pub version: u32,

    /// NUL-terminated ASCII string.
    pub server_ip: [u8; 64],
    /// Server port.
    pub server_port: u16,
    /// Reserved/padding.
    pub _reserved0: [u8; 2],

    /// Matches original `.moa` (key) binary layout.
    pub save_file: SaveFile,
    /// Matches original `pdata` binary layout.
    pub player_data: PlayerData,
}

const _: () = {
    // 4 + 4 + 64 + 2 + 2 + 56 + 484
    assert!(std::mem::size_of::<MagDatV1>() == 616);
};

impl Default for MagDatV1 {
    fn default() -> Self {
        Self {
            magic: *b"MAGD",
            version: 1,
            server_ip: [0; 64],
            server_port: 0,
            _reserved0: [0; 2],
            save_file: SaveFile::default(),
            player_data: PlayerData::default(),
        }
    }
}

fn ascii_to_fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    if N == 0 {
        return out;
    }

    let mut i = 0usize;
    for &b in s.as_bytes() {
        if i >= N.saturating_sub(1) {
            break;
        }
        out[i] = if (32..=126).contains(&b) { b } else { b' ' };
        i += 1;
    }
    out
}

pub fn fixed_ascii_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let slice = &bytes[..end];
    String::from_utf8_lossy(slice).trim().to_string()
}

fn read_struct<T>(mut reader: impl Read) -> io::Result<T> {
    let mut value = MaybeUninit::<T>::uninit();
    let value_bytes = unsafe {
        std::slice::from_raw_parts_mut(value.as_mut_ptr() as *mut u8, std::mem::size_of::<T>())
    };
    reader.read_exact(value_bytes)?;
    Ok(unsafe { value.assume_init() })
}

fn write_struct<T>(mut writer: impl Write, value: &T) -> io::Result<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
    };
    writer.write_all(bytes)
}

pub fn mag_dat_path() -> PathBuf {
    // Place `mag.dat` next to the client executable, regardless of where it was launched from.
    // This avoids surprising behavior when running from different working directories.
    match std::env::current_exe() {
        Ok(exe) => exe
            .parent()
            .map(|dir| dir.join(MAG_DAT_FILENAME))
            .unwrap_or_else(|| PathBuf::from(MAG_DAT_FILENAME)),
        Err(e) => {
            log::warn!("Failed to resolve current_exe for mag.dat path: {e}");
            PathBuf::from(MAG_DAT_FILENAME)
        }
    }
}

pub fn load_mag_dat() -> io::Result<MagDatV1> {
    let path = mag_dat_path();
    load_mag_dat_at(&path)
}

pub fn save_mag_dat(data: &MagDatV1) -> io::Result<()> {
    let path = mag_dat_path();
    save_mag_dat_at(&path, data)
}

pub fn load_mag_dat_at(path: &Path) -> io::Result<MagDatV1> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(MagDatV1::default()),
        Err(e) => {
            log::error!("Failed to open mag.dat at {:?}: {e}", path);
            return Err(e);
        }
    };

    let data: MagDatV1 = read_struct(&mut file)?;
    if &data.magic != b"MAGD" || data.version != 1 {
        // Unknown format; don't fail hard.
        log::warn!(
            "mag.dat had unexpected header (magic={:?}, version={}); using defaults",
            data.magic,
            data.version
        );
        return Ok(MagDatV1::default());
    }
    Ok(data)
}

pub fn save_mag_dat_at(path: &Path, data: &MagDatV1) -> io::Result<()> {
    let mut file = File::create(path)?;
    write_struct(&mut file, data)
}

pub fn build_mag_dat(
    server_ip: &str,
    server_port: u16,
    save_file: &SaveFile,
    player_data: &PlayerData,
) -> MagDatV1 {
    let mut out = MagDatV1::default();
    out.server_ip = ascii_to_fixed(server_ip);
    out.server_port = server_port;
    out.save_file = *save_file;
    out.player_data = *player_data;
    out
}

/// Load a character file (our `.mag`) maintaining the original `.moa` binary layout:
/// `SaveFile` followed by `PlayerData`.
pub fn load_character_file(path: &Path) -> io::Result<(SaveFile, PlayerData)> {
    let mut file = File::open(path)?;
    let save_file: SaveFile = read_struct(&mut file)?;
    let player_data: PlayerData = read_struct(&mut file)?;
    Ok((save_file, player_data))
}

/// Save a character file (our `.mag`) maintaining the original `.moa` binary layout:
/// `SaveFile` followed by `PlayerData`.
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
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{pid}_{nanos}"))
    }

    fn as_bytes<T>(value: &T) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
        }
    }

    #[test]
    fn fixed_ascii_to_string_stops_at_nul_and_trims() {
        let bytes = b" hello\0world";
        assert_eq!(fixed_ascii_to_string(bytes), "hello");
    }

    #[test]
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
        player_data.desc[0..11].copy_from_slice(b"Hello world");

        save_character_file(&path, &save_file, &player_data).unwrap();
        let (loaded_save, loaded_pdata) = load_character_file(&path).unwrap();

        assert_eq!(as_bytes(&loaded_save), as_bytes(&save_file));
        assert_eq!(as_bytes(&loaded_pdata), as_bytes(&player_data));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mag_dat_missing_file_returns_default() {
        let dir = unique_temp_dir("mag_dat_missing");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mag.dat");

        let loaded = load_mag_dat_at(&path).unwrap();
        assert_eq!(&loaded.magic, b"MAGD");
        assert_eq!(loaded.version, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mag_dat_invalid_magic_returns_default() {
        let dir = unique_temp_dir("mag_dat_invalid_magic");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mag.dat");

        let mut bad = MagDatV1::default();
        bad.magic = *b"NOPE";
        save_mag_dat_at(&path, &bad).unwrap();

        let loaded = load_mag_dat_at(&path).unwrap();
        assert_eq!(&loaded.magic, b"MAGD");
        assert_eq!(loaded.version, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mag_dat_roundtrip_preserves_bytes() {
        let dir = unique_temp_dir("mag_dat_roundtrip");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mag.dat");

        let mut save_file = SaveFile::default();
        save_file.usnr = 1;
        save_file.pass1 = 2;
        save_file.pass2 = 3;
        save_file.race = 3;
        save_file.name[0..5].copy_from_slice(b"Alice");

        let mut player_data = PlayerData::default();
        player_data.show_proz = 1;
        player_data.desc[0..4].copy_from_slice(b"Desc");

        let mag = build_mag_dat("127.0.0.1", 5555, &save_file, &player_data);
        save_mag_dat_at(&path, &mag).unwrap();
        let loaded = load_mag_dat_at(&path).unwrap();

        assert_eq!(as_bytes(&loaded), as_bytes(&mag));

        let _ = fs::remove_dir_all(&dir);
    }
}
