/// "Marble bag" which provides a better player experience
/// for situations when random chance is involved vs. a
/// standard random number check.
///
/// The marble bag provides a way to guarantee that the event
/// will occur at the appropriate frequency within a fixed number
/// of draws.
///
/// For example if we wanted something to happen 25% of
/// the time, we might generate a random float between 0 - 1
/// and program that the event occurs when the value is <= 0.25.
/// Here is the actual outcome of that over 10 samples that I ran
/// when writing this structure (X is happens, O is does not happen):
/// X O X X O X X O O O
/// In this case, the event occurred 5 out of 10 times, or 50%, despite
/// having a 25% probability on each individual roll. This is a perfectly
/// valid outcome with standard random chance, but it may not produce
/// the player experience we want.

/// The marble bag instead allows us to distribute outcomes more evenly,
/// ensuring that a 25% event occurs once within every 4 draws while still
/// randomizing which draw it occurs on.
use rand::seq::SliceRandom;

const fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

const fn precompute_gcds() -> [u32; 100] {
    let mut gcds = [0; 100];
    let mut i = 0;

    while i < 100 {
        gcds[i] = gcd(i as u32, 100);
        i += 1;
    }

    gcds
}

const PRECOMPUTED_GCD: [u32; 100] = precompute_gcds();

pub struct MarbleBag {
    bag: Vec<bool>,
    marble: usize,
}

impl MarbleBag {
    pub fn from_percent(percent: usize) -> Self {
        assert!(
            percent > 0 && percent < 100,
            "Percent chance should be from 1% - 99%"
        );

        let divisor = PRECOMPUTED_GCD[percent];
        let successes = percent / divisor as usize;
        let draw_count = (100 / divisor) as usize;

        // Since we know that we have 100 marbles in the bag,
        // we know that the number of 'true' marbles are equal
        // to the percent passed in here.
        let mut marble_bag = vec![false; draw_count];
        marble_bag[draw_count - successes..].fill(true);
        marble_bag.shuffle(&mut rand::thread_rng());

        MarbleBag {
            bag: marble_bag,
            marble: 0,
        }
    }

    pub fn draw(&mut self) -> bool {
        let result = self.bag[self.marble];
        self.marble += 1;

        if self.marble == self.bag.len() {
            self.reset();
        }

        result
    }

    fn reset(&mut self) {
        self.marble = 0;
        self.bag.shuffle(&mut rand::thread_rng());
    }
}

#[cfg(test)]
mod tests {
    use super::MarbleBag;

    fn count_outcomes(bag: &[bool; MARBLE_BAG_SIZE]) -> (usize, usize) {
        let true_count = bag.iter().filter(|&&marble| marble).count();
        (true_count, MARBLE_BAG_SIZE - true_count)
    }

    #[test]
    fn from_percent_builds_every_valid_percentage_exactly() {
        for percent in 1..100 {
            let bag = MarbleBag::from_percent(percent);

            assert_eq!(count_outcomes(&bag.bag), (percent, 100 - percent));
            assert_eq!(bag.marble, 0);
        }
    }

    #[test]
    #[should_panic(expected = "Percent chance should be from 1% - 99%")]
    fn from_percent_panics_for_zero() {
        let _ = MarbleBag::from_percent(0);
    }

    #[test]
    #[should_panic(expected = "Percent chance should be from 1% - 99%")]
    fn from_percent_panics_for_one_hundred() {
        let _ = MarbleBag::from_percent(100);
    }

    #[test]
    #[should_panic(expected = "Percent chance should be from 1% - 99%")]
    fn from_percent_panics_for_values_above_one_hundred() {
        let _ = MarbleBag::from_percent(101);
    }

    #[test]
    #[should_panic(expected = "Percent chance should be from 1% - 99%")]
    fn from_percent_panics_for_largest_usize() {
        let _ = MarbleBag::from_percent(usize::MAX);
    }

    #[test]
    fn draw_returns_current_marble_and_advances_cursor() {
        let mut values = [false; MARBLE_BAG_SIZE];
        values[0] = true;
        values[2] = true;
        let mut bag = MarbleBag {
            bag: values,
            marble: 0,
        };

        assert!(bag.draw());
        assert_eq!(bag.marble, 1);
        assert!(!bag.draw());
        assert_eq!(bag.marble, 2);
        assert!(bag.draw());
        assert_eq!(bag.marble, 3);
    }

    #[test]
    fn final_draw_returns_final_marble_and_resets_cursor() {
        let mut values = [false; MARBLE_BAG_SIZE];
        values[MARBLE_BAG_SIZE - 1] = true;
        let mut bag = MarbleBag {
            bag: values,
            marble: MARBLE_BAG_SIZE - 1,
        };

        assert!(bag.draw());
        assert_eq!(bag.marble, 0);
        assert_eq!(count_outcomes(&bag.bag), (1, 99));
    }

    #[test]
    fn reset_rewinds_cursor_and_preserves_outcomes() {
        let mut bag = MarbleBag::from_percent(37);
        bag.marble = 63;

        bag.reset();

        assert_eq!(bag.marble, 0);
        assert_eq!(count_outcomes(&bag.bag), (37, 63));
    }

    #[test]
    fn complete_cycle_draws_exact_requested_percentage() {
        for percent in 1..100 {
            let mut bag = MarbleBag::from_percent(percent);
            let true_count = (0..MARBLE_BAG_SIZE).filter(|_| bag.draw()).count();

            assert_eq!(true_count, percent, "failed for {percent}% bag");
            assert_eq!(bag.marble, 0, "cursor did not reset for {percent}% bag");
        }
    }

    #[test]
    fn repeated_cycles_preserve_requested_percentage() {
        const PERCENT: usize = 41;
        const CYCLES: usize = 10;

        let mut bag = MarbleBag::from_percent(PERCENT);

        for cycle in 0..CYCLES {
            let true_count = (0..MARBLE_BAG_SIZE).filter(|_| bag.draw()).count();

            assert_eq!(true_count, PERCENT, "failed on cycle {cycle}");
            assert_eq!(bag.marble, 0, "cursor did not reset on cycle {cycle}");
        }
    }

    #[test]
    fn arbitrary_draw_count_matches_full_cycles_plus_current_prefix() {
        const PERCENT: usize = 23;
        const FULL_CYCLES: usize = 4;
        const PREFIX_LENGTH: usize = 57;

        let mut bag = MarbleBag::from_percent(PERCENT);
        let true_count = (0..FULL_CYCLES * MARBLE_BAG_SIZE + PREFIX_LENGTH)
            .filter(|_| bag.draw())
            .count();
        let remaining_true_count = bag.bag[PREFIX_LENGTH..]
            .iter()
            .filter(|&&marble| marble)
            .count();

        assert_eq!(
            true_count + remaining_true_count,
            (FULL_CYCLES + 1) * PERCENT
        );
        assert_eq!(bag.marble, PREFIX_LENGTH);
    }
}
