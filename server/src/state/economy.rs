use core::constants::TICKS;

use crate::game_state::GameState;
use crate::god::God;

impl GameState {
    /// Port of `do_balance(int cn)` from `svr_do.cpp`
    ///
    /// Display character's bank balance.
    ///
    /// Shows the player's current bank balance and any depot-related messages
    /// such as items sold to cover costs or rent deductions.
    ///
    /// # Arguments
    /// * `cn` - Character id requesting their balance
    pub(crate) fn do_balance(&mut self, cn: usize) {
        let m = self.characters[cn].x as usize
            + (self.characters[cn].y as usize * core::constants::SERVER_MAPX as usize);
        let is_bank = (self.map[m].flags & core::constants::MF_BANK as u64) != 0;
        if !is_bank {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, balance works only in banks.\n",
            );
            return;
        }

        let balance = self.characters[cn].data[13];
        let depot_sold = self.characters[cn].depot_sold as i32;
        let depot_cost = self.characters[cn].depot_cost;
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("Your balance is {}G {}S.\n", balance / 100, balance % 100),
        );

        // get_depot_cost placeholder
        let tmp = 0;
        if tmp != 0 {
            self.do_character_log(cn, core::types::FontColor::Yellow, &format!("The rent for your depot is {}G {}S per Astonian day or {}G {}S per Earth day.\n", tmp / 100, tmp % 100, (tmp * TICKS) / 100, (tmp * TICKS) % 100));
        }

        if depot_sold != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "The bank sold {} items from your depot to cover the costs.\n",
                    depot_sold
                ),
            );
            self.characters[cn].depot_sold = 0;
        }

        if depot_cost != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "{}G {}S were deducted from your bank account as rent for your depot.\n",
                    depot_cost / 100,
                    depot_cost % 100
                ),
            );
            self.characters[cn].depot_cost = 0;
        }
    }

    /// Port of `do_withdraw(int cn, int g, int s)` from `svr_do.cpp`
    ///
    /// Withdraw gold/silver from bank.
    ///
    /// Validates that the caller is in a bank, that the requested amount is
    /// non-negative and available in the account, then transfers funds from
    /// the character's bank balance to their carried gold.
    ///
    /// # Arguments
    /// * `cn` - Character id performing the withdrawal
    /// * `g` - Gold portion to withdraw
    /// * `s` - Silver portion to withdraw
    pub(crate) fn do_withdraw(&mut self, cn: usize, g: i32, s: i32) {
        let m = self.characters[cn].x as usize
            + (self.characters[cn].y as usize * core::constants::SERVER_MAPX as usize);
        if (self.map[m].flags & core::constants::MF_BANK as u64) == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, withdraw works only in banks.\n",
            );
            return;
        }
        // Match C semantics: signed 32-bit overflow wraps.
        // This avoids debug-mode panics and keeps behavior consistent.
        let v = g.wrapping_mul(100).wrapping_add(s);
        if v < 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "If you want to deposit money, then say so!\n",
            );
            return;
        }
        let bank = self.characters[cn].data[13];
        if v > bank {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, you don't have that much money in the bank.\n",
            );
            return;
        }
        self.characters[cn].gold += v;
        self.characters[cn].data[13] -= v;
        self.do_update_char(cn);
        let newbal = self.characters[cn].data[13];
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "You withdraw {}G {}S; your new balance is {}G {}S.\n",
                v / 100,
                v % 100,
                newbal / 100,
                newbal % 100
            ),
        );
    }

    /// Port of `do_deposit(int cn, int g, int s)` from `svr_do.cpp`
    ///
    /// Deposit gold/silver into bank.
    ///
    /// Validates the caller is in a bank and that they have the specified
    /// funds, then moves the specified gold/silver from carried gold into
    /// their bank account.
    ///
    /// # Arguments
    /// * `cn` - Character id performing the deposit
    /// * `g` - Gold portion to deposit
    /// * `s` - Silver portion to deposit
    pub(crate) fn do_deposit(&mut self, cn: usize, g: i32, s: i32) {
        let m = self.characters[cn].x as usize
            + (self.characters[cn].y as usize * core::constants::SERVER_MAPX as usize);
        if (self.map[m].flags & core::constants::MF_BANK as u64) == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, deposit works only in banks.\n",
            );
            return;
        }
        // Match C semantics: signed 32-bit overflow wraps.
        // This avoids debug-mode panics and keeps behavior consistent.
        let v = g.wrapping_mul(100).wrapping_add(s);
        if v < 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "If you want to withdraw money, then say so!\n",
            );
            return;
        }
        let have = self.characters[cn].gold;
        if v > have {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Sorry, you don't have that much money.\n",
            );
            return;
        }
        self.characters[cn].gold -= v;
        self.characters[cn].data[13] += v;
        self.do_update_char(cn);
        let newbal = self.characters[cn].data[13];
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "You deposited {}G {}S; your new balance is {}G {}S.\n",
                v / 100,
                v % 100,
                newbal / 100,
                newbal % 100
            ),
        );
    }

    /// Port of `do_gold(int cn, int val)` from `svr_do.cpp`
    ///
    /// Admin command to take gold from the character's purse and prepare it
    /// as a cursor item for transfer.
    ///
    /// Ensures there is no item on the cursor, validates the value, and then
    /// sets `citem` to a special encoded value representing the gold taken.
    ///
    /// # Arguments
    /// * `cn` - Character id executing the command
    /// * `val` - Amount of gold (in full gold units) to take from the purse
    pub(crate) fn do_gold(&mut self, cn: usize, val: i32) {
        let citem = self.characters[cn].citem;
        if citem != 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "Please remove the item from your mouse cursor first.\n",
            );
            return;
        }
        if val < 1 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "That's not very much, is it?\n",
            );
            return;
        }
        let v = val * 100;
        let have = self.characters[cn].gold;
        if v > have || v < 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You don't have that much gold!\n",
            );
            return;
        }

        self.characters[cn].gold -= v;
        self.characters[cn].citem = 0x8000_0000u32 | (v as u32);
        self.characters[cn].set_do_update_flags();

        self.do_update_char(cn);
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("You take {}G from your purse.\n", val),
        );
    }

    /// Port of `do_god_give(cn, co)` from `svr_do.cpp`
    ///
    /// Give the item currently on the caller's cursor to the target character
    /// using the `God::give_character_item` helper. Clears the caller's cursor
    /// and logs the transfer on success.
    ///
    /// # Arguments
    /// * `cn` - Character id giving the item
    /// * `co` - Target character id receiving the item
    pub fn do_god_give(&mut self, cn: usize, co: usize) {
        let in_id = self.characters[cn].citem as usize;
        if in_id == 0 {
            self.do_character_log(
                cn,
                core::types::FontColor::Red,
                "You have nothing under your mouse cursor!\n",
            );
            return;
        }
        if !God::give_character_item(co, in_id) {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "god_give_char() returned error.\n",
            );
            return;
        }
        let iname = self.items[in_id].get_name().to_string();
        let cname = self.characters[co].get_name().to_string();
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("{} given to {}.\n", iname, cname),
        );
        log::info!("IMP: Gave {} (t={}) to {} ({})", iname, in_id, cname, co);
        self.characters[cn].citem = 0;
        self.characters[cn].set_do_update_flags();
    }

    /// Port of `do_lag(cn, lag)` from `svr_do.cpp`
    ///
    /// Sets or clears an automated lag-control timer for a player. When set,
    /// the server will (in game logic elsewhere) take action if the player's
    /// lag exceeds the configured threshold.
    ///
    /// # Arguments
    /// * `cn` - Character id to modify lag control for
    /// * `lag` - Seconds threshold (0 to disable)
    pub(crate) fn do_lag(&mut self, cn: usize, lag: i32) {
        if lag == 0 {
            let prev = self.characters[cn].data[19];
            self.characters[cn].data[19] = 0;
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                &format!(
                    "Lag control turned off (was at {}).\n",
                    prev / core::constants::TICKS
                ),
            );
            return;
        }
        if !(3..=20).contains(&lag) {
            self.do_character_log(
                cn,
                core::types::FontColor::Yellow,
                "Lag control needs a value between 3 and 20. Use 0 to turn it off.\n",
            );
            return;
        }
        self.characters[cn].data[19] = lag * core::constants::TICKS;
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "Lag control will turn you to stone if lag exceeds {} seconds.\n",
                lag
            ),
        );
    }

    /// Port of `rank2points(int rank)` from `svr_do.cpp`
    ///
    /// Calculate points needed for a given rank.
    ///
    /// Returns the points threshold for the provided rank or -1 for invalid
    /// ranks not handled by the mapping.
    ///
    /// # Arguments
    /// * `rank` - Rank index to lookup
    pub(crate) fn rank2points(&mut self, rank: i32) -> i32 {
        match rank {
            0 => 50,
            1 => 850,
            2 => 4900,
            3 => 17700,
            4 => 48950,
            5 => 113750,
            6 => 233800,
            7 => 438600,
            8 => 766650,
            9 => 1266650,
            10 => 1998700,
            11 => 3035500,
            12 => 4463550,
            13 => 6384350,
            14 => 8915600,
            15 => 12192400,
            16 => 16368450,
            17 => 21617250,
            18 => 28133300,
            19 => 36133300,
            20 => 49014500,
            21 => 63000600,
            22 => 80977100,
            _ => -1,
        }
    }

    /// Port of `do_view_exp_to_rank(int cn)` from `svr_do.cpp`
    ///
    /// Display experience needed to next rank.
    ///
    /// Calculates the player's current rank and the experience required for
    /// the next rank, then sends a message to the player with the amount and
    /// the name of the next rank.
    ///
    /// # Arguments
    /// * `cn` - Character id to view requirements for
    pub(crate) fn do_view_exp_to_rank(&mut self, cn: usize) {
        let current_rank = core::ranks::points2rank(self.characters[cn].points_tot as u32) as usize;
        let exp_to_next = self.rank2points(current_rank as i32);
        let exp_needed = exp_to_next - self.characters[cn].points_tot;
        let next_name = core::ranks::RANK_NAMES
            .get(current_rank + 1)
            .unwrap_or(&"Unknown");
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!("You need {} exp for {}.\n", exp_needed, next_name),
        );
    }

    /// Port of `do_check_pent_count(int cn)` from `svr_do.cpp`
    ///
    /// Check pentagram item count for character.
    ///
    /// Scans the global item list for active pentagram drivers and reports
    /// how many are active versus how many are required to solve the puzzle.
    ///
    /// # Arguments
    /// * `cn` - Character id to receive the report
    pub(crate) fn do_check_pent_count(&mut self, cn: usize) {
        let mut active = 0;
        for it in self.items.iter() {
            if it.used == core::constants::USE_EMPTY {
                continue;
            }
            if it.driver != 33 {
                continue;
            }
            if it.active != u32::MAX {
                // active == -1 in C
                continue;
            }
            active += 1;
        }

        let penta_needed: usize = self.penta_needed;
        self.do_character_log(
            cn,
            core::types::FontColor::Yellow,
            &format!(
                "There are {} pentagrams active. {} needed to solve.\n",
                active, penta_needed
            ),
        );
    }
}
