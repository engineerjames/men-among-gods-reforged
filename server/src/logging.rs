/*************************************************************************

This file is part of 'Mercenaries of Astonia v2'
Copyright (c) 1997-2001 Daniel Brockhaus (joker@astonia.com)
All rights reserved.

Rust port maintains original logic and comments.

**************************************************************************/

//! Logging module - server logging functionality

use chrono::Local;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::sync::Mutex;

use crate::constants::*;
use crate::types::*;

/// Global log file handle wrapped in a Mutex for thread safety
pub struct Logger {
    log_file: Mutex<Box<dyn Write + Send>>,
    use_stdout: bool,
}

impl Logger {
    /// Create a new logger that writes to stdout
    pub fn new_stdout() -> Self {
        Self {
            log_file: Mutex::new(Box::new(io::stdout())),
            use_stdout: true,
        }
    }
    
    /// Create a new logger that writes to a file
    pub fn new_file(filename: &str) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)?;
        Ok(Self {
            log_file: Mutex::new(Box::new(file)),
            use_stdout: false,
        })
    }
    
    /// Rotate the log file (close and reopen)
    pub fn rotate(&self, filename: &str) -> io::Result<()> {
        if self.use_stdout {
            return Ok(());
        }
        
        let mut guard = self.log_file.lock().unwrap();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(filename)?;
        *guard = Box::new(file);
        Ok(())
    }
    
    /// Format timestamp in the original format: DD.MM.YY HH:MM:SS
    fn format_timestamp() -> String {
        let now = Local::now();
        format!(
            "{:02}.{:02}.{:02} {:02}:{:02}:{:02}",
            now.format("%d"),
            now.format("%m"),
            now.format("%y"),
            now.format("%H"),
            now.format("%M"),
            now.format("%S")
        )
    }
    
    /// disembodied server log
    pub fn xlog(&self, message: &str) {
        let timestamp = Self::format_timestamp();
        let mut guard = self.log_file.lock().unwrap();
        let _ = writeln!(&mut *guard, "{}: {}", timestamp, message);
        let _ = guard.flush();
    }
    
    /// server log message about a character
    pub fn chlog(
        &self,
        cn: usize,
        ch: &[Character],
        players: &[crate::player::Player],
        message: &str,
    ) {
        let timestamp = Self::format_timestamp();
        
        // Get player info
        let mut nr = ch[cn].player as usize;
        if nr < 1 || nr > MAXPLAYER {
            nr = 0;
        }
        if nr > 0 && players[nr].usnr != cn {
            nr = 0;
        }
        
        let (addr, unique) = if nr > 0 {
            (players[nr].addr, players[nr].unique)
        } else {
            (0, 0)
        };
        
        let name = ch[cn].get_name();
        
        // Check for usurp
        let usurp_data = ch[cn].data[97];
        let usurp_name = if usurp_data > 0 
            && is_sane_char(usurp_data as usize) 
            && ch[usurp_data as usize].is_player() 
        {
            Some(ch[usurp_data as usize].get_name())
        } else {
            None
        };
        
        let x = ch[cn].x;
        let y = ch[cn].y;
        
        // Format IP address from u32 (little endian)
        let ip = format!(
            "{}.{}.{}.{}",
            addr & 255,
            (addr >> 8) & 255,
            (addr >> 16) & 255,
            addr >> 24
        );
        
        let mut guard = self.log_file.lock().unwrap();
        
        if let Some(usurp) = usurp_name {
            let _ = writeln!(
                &mut *guard,
                "{}: {} [{}] ({} at {},{} from {}, ID={}): {}",
                timestamp, name, usurp, cn, x, y, ip, unique, message
            );
        } else {
            let _ = writeln!(
                &mut *guard,
                "{}: {} ({} at {},{} from {}, ID={}): {}",
                timestamp, name, cn, x, y, ip, unique, message
            );
        }
        let _ = guard.flush();
    }
    
    /// server log about a player
    pub fn plog(
        &self,
        nr: usize,
        ch: &[Character],
        players: &[crate::player::Player],
        message: &str,
    ) {
        let timestamp = Self::format_timestamp();
        
        let mut cn = players[nr].usnr;
        if cn == 0 || cn >= MAXCHARS {
            cn = 0;
        }
        if cn > 0 && ch[cn].player as usize != nr {
            cn = 0;
        }
        
        let (name, x, y) = if cn > 0 {
            (ch[cn].get_name(), ch[cn].x as i32, ch[cn].y as i32)
        } else {
            ("*unknown*", 0, 0)
        };
        
        let addr = players[nr].addr;
        let unique = players[nr].unique;
        
        // Format IP address from u32 (little endian)
        let ip = format!(
            "{}.{}.{}.{}",
            addr & 255,
            (addr >> 8) & 255,
            (addr >> 16) & 255,
            addr >> 24
        );
        
        let mut guard = self.log_file.lock().unwrap();
        let _ = writeln!(
            &mut *guard,
            "{}: {} ({} at {},{} from {}, ID={}): {}",
            timestamp, name, cn, x, y, ip, unique, message
        );
        let _ = guard.flush();
    }
}

/// Helper macro for xlog (disembodied server log)
#[macro_export]
macro_rules! xlog {
    ($logger:expr, $($arg:tt)*) => {
        $logger.xlog(&format!($($arg)*))
    };
}

/// Helper macro for chlog (character log)
#[macro_export]
macro_rules! chlog {
    ($logger:expr, $cn:expr, $ch:expr, $players:expr, $($arg:tt)*) => {
        $logger.chlog($cn, $ch, $players, &format!($($arg)*))
    };
}

/// Helper macro for plog (player log)
#[macro_export]
macro_rules! plog {
    ($logger:expr, $nr:expr, $ch:expr, $players:expr, $($arg:tt)*) => {
        $logger.plog($nr, $ch, $players, &format!($($arg)*))
    };
}
