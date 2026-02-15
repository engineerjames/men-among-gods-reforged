use crate::constants;
use bincode::{Decode, Encode};

/// Global server state structure
#[derive(Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Global {
    pub mdtime: i32,
    pub mdday: i32,
    pub mdyear: i32,
    pub dlight: i32,

    pub players_created: i32,
    pub npcs_created: i32,
    pub players_died: i32,
    pub npcs_died: i32,

    pub character_cnt: i32,
    pub item_cnt: i32,
    pub effect_cnt: i32,

    pub expire_cnt: i32,
    pub expire_run: i32,

    pub gc_cnt: i32,
    pub gc_run: i32,

    pub lost_cnt: i32,
    pub lost_run: i32,

    pub reset_char: i32,
    pub reset_item: i32,

    pub ticker: i32,

    pub total_online_time: i64,
    pub online_per_hour: [i64; 24],

    pub flags: i32,

    pub uptime: i64,
    pub uptime_per_hour: [i64; 24],

    pub awake: i32,
    pub body: i32,

    pub players_online: i32,
    pub queuesize: i32,

    pub recv: i64,
    pub send: i64,

    pub transfer_reset_time: i32,
    pub load_avg: i32,

    pub load: i64,

    pub max_online: i32,
    pub max_online_per_hour: [i32; 24],

    pub fullmoon: i8,
    pub newmoon: i8,

    pub unique: u64,

    pub cap: i32,
}

impl Global {
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::encode_to_vec(self, bincode::config::standard()).expect("Global::to_bytes failed")
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (value, consumed): (Self, usize) =
            bincode::decode_from_slice(bytes, bincode::config::standard()).ok()?;
        if consumed == bytes.len() {
            Some(value)
        } else {
            None
        }
    }

    pub fn is_dirty(&self) -> bool {
        (self.flags & constants::GF_DIRTY) != 0
    }

    pub fn set_dirty(&mut self, value: bool) {
        if value {
            self.flags |= constants::GF_DIRTY;
        } else {
            self.flags &= !constants::GF_DIRTY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_to_bytes_size() {
        let global = Global::default();
        let bytes = global.to_bytes();
        assert!(!bytes.is_empty(), "Serialized Global should not be empty");
    }

    #[test]
    fn test_global_roundtrip() {
        let mut original = Global::default();
        original.mdtime = 12345;
        original.mdday = 100;
        original.mdyear = 2026;
        original.dlight = 50;
        original.players_created = 1000;
        original.npcs_created = 5000;
        original.ticker = 99999;
        original.total_online_time = 1234567890;
        original.online_per_hour = [100; 24];
        original.uptime = 9876543210;
        original.uptime_per_hour = [200; 24];
        original.max_online = 150;
        original.max_online_per_hour = [50; 24];
        original.fullmoon = 1;
        original.newmoon = 0;
        original.unique = 0xDEADBEEFCAFEBABE;
        original.cap = 250;

        let bytes = original.to_bytes();
        let deserialized = Global::from_bytes(&bytes).expect("Failed to deserialize Global");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_global_from_bytes_insufficient_data() {
        let mut bytes = Global::default().to_bytes();
        bytes.pop();
        assert!(
            Global::from_bytes(&bytes).is_none(),
            "Should fail with insufficient data"
        );
    }

    #[test]
    fn test_global_dirty_flag() {
        let mut global = Global::default();
        assert!(!global.is_dirty(), "Should not be dirty by default");

        global.set_dirty(true);
        assert!(global.is_dirty(), "Should be dirty after setting");

        global.set_dirty(false);
        assert!(!global.is_dirty(), "Should not be dirty after clearing");
    }

    #[test]
    fn diagnose_struct_layout() {
        use std::mem::{offset_of, size_of};

        println!("\n=== Global Struct Layout Diagnostics ===");
        println!("Total struct size: {}", size_of::<Global>());
        println!("\nField offsets and sizes:");

        macro_rules! print_field {
            ($field:ident, $type:ty) => {
                println!(
                    "{:30} offset: {:4}, size: {:2}",
                    stringify!($field),
                    offset_of!(Global, $field),
                    size_of::<$type>()
                );
            };
        }

        print_field!(mdtime, i32);
        print_field!(mdday, i32);
        print_field!(mdyear, i32);
        print_field!(dlight, i32);
        print_field!(players_created, i32);
        print_field!(npcs_created, i32);
        print_field!(players_died, i32);
        print_field!(npcs_died, i32);
        print_field!(character_cnt, i32);
        print_field!(item_cnt, i32);
        print_field!(effect_cnt, i32);
        print_field!(expire_cnt, i32);
        print_field!(expire_run, i32);
        print_field!(gc_cnt, i32);
        print_field!(gc_run, i32);
        print_field!(lost_cnt, i32);
        print_field!(lost_run, i32);
        print_field!(reset_char, i32);
        print_field!(reset_item, i32);
        print_field!(ticker, i32);
        print_field!(total_online_time, i64);
        print_field!(online_per_hour, [i64; 24]);
        print_field!(flags, i32);
        print_field!(uptime, i64);
        print_field!(uptime_per_hour, [i64; 24]);
        print_field!(awake, i32);
        print_field!(body, i32);
        print_field!(players_online, i32);
        print_field!(queuesize, i32);
        print_field!(recv, i64);
        print_field!(send, i64);
        print_field!(transfer_reset_time, i32);
        print_field!(load_avg, i32);
        print_field!(load, i64);
        print_field!(max_online, i32);
        print_field!(max_online_per_hour, [i32; 24]);
        print_field!(fullmoon, i8);
        print_field!(newmoon, i8);
        print_field!(unique, u64);
        print_field!(cap, i32);

        println!("\n=== Padding Detection ===");

        // Check for padding after ticker (i32) before total_online_time (i64)
        let ticker_end = offset_of!(Global, ticker) + size_of::<i32>();
        let total_online_time_start = offset_of!(Global, total_online_time);
        if total_online_time_start > ticker_end {
            println!(
                "Padding after ticker: {} bytes",
                total_online_time_start - ticker_end
            );
        }

        // Check for padding after flags (i32) before uptime (i64)
        let flags_end = offset_of!(Global, flags) + size_of::<i32>();
        let uptime_start = offset_of!(Global, uptime);
        if uptime_start > flags_end {
            println!("Padding after flags: {} bytes", uptime_start - flags_end);
        }

        // Check for padding after queuesize (i32) before recv (i64)
        let queuesize_end = offset_of!(Global, queuesize) + size_of::<i32>();
        let recv_start = offset_of!(Global, recv);
        if recv_start > queuesize_end {
            println!(
                "Padding after queuesize: {} bytes",
                recv_start - queuesize_end
            );
        }

        // Check for padding after load_avg (i32) before load (i64)
        let load_avg_end = offset_of!(Global, load_avg) + size_of::<i32>();
        let load_start = offset_of!(Global, load);
        if load_start > load_avg_end {
            println!(
                "Padding after load_avg: {} bytes",
                load_start - load_avg_end
            );
        }

        // Check for padding after max_online (i32) before max_online_per_hour
        let max_online_end = offset_of!(Global, max_online) + size_of::<i32>();
        let max_online_per_hour_start = offset_of!(Global, max_online_per_hour);
        if max_online_per_hour_start > max_online_end {
            println!(
                "Padding after max_online: {} bytes",
                max_online_per_hour_start - max_online_end
            );
        }

        // Already checked newmoon -> unique padding
        let newmoon_end = offset_of!(Global, newmoon) + size_of::<i8>();
        let unique_start = offset_of!(Global, unique);
        if unique_start > newmoon_end {
            println!(
                "Padding after newmoon: {} bytes",
                unique_start - newmoon_end
            );
        } else {
            println!("No trailing padding after newmoon (or field reordering under repr(Rust))");
        }

        // Already checked cap -> end padding
        let cap_end = offset_of!(Global, cap) + size_of::<i32>();
        let struct_size = size_of::<Global>();
        println!(
            "Padding after cap: {} bytes",
            struct_size.saturating_sub(cap_end)
        );
    }
}
