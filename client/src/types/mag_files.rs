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
    // Keep compatibility with the original C client: relative to current working directory.
    PathBuf::from(MAG_DAT_FILENAME)
}

pub fn load_mag_dat() -> io::Result<MagDatV1> {
    let path = mag_dat_path();
    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(MagDatV1::default()),
        Err(e) => return Err(e),
    };

    let data: MagDatV1 = read_struct(&mut file)?;
    if &data.magic != b"MAGD" || data.version != 1 {
        // Unknown format; don't fail hard.
        return Ok(MagDatV1::default());
    }
    Ok(data)
}

pub fn save_mag_dat(data: &MagDatV1) -> io::Result<()> {
    let path = mag_dat_path();
    let mut file = File::create(&path)?;
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
