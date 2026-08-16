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

const MARBLE_BAG_SIZE: usize = 100;

pub struct MarbleBag {
    // TODO: This isn't really space optimized; but I know I can represent
    // all percentages provided with this. There are certainly a lot of
    // optimizations that could be done here.
    bag: [bool; MARBLE_BAG_SIZE],
    marble: usize,
}

impl MarbleBag {
    pub fn from_percent(percent: usize) -> Self {
        assert!(
            percent > 0 && percent < 100,
            "Percent chance should be from 1% - 99%"
        );

        // Since we know that we have 100 marbles in the bag,
        // we know that the number of 'true' marbles are equal
        // to the percent passed in here.
        let mut marble_bag = [false; MARBLE_BAG_SIZE];

        marble_bag[MARBLE_BAG_SIZE - percent..MARBLE_BAG_SIZE].fill(true);

        let mut rng = rand::thread_rng();
        marble_bag.shuffle(&mut rng);

        MarbleBag {
            bag: marble_bag,
            marble: 0,
        }
    }

    pub fn draw(&mut self) -> bool {
        let return_value = self.bag[self.marble];

        if self.marble == MARBLE_BAG_SIZE - 1 {
            self.reset();
        } else {
            self.marble += 1;
        }

        return_value
    }

    fn reset(&mut self) {
        self.marble = 0;
        let mut rng = rand::thread_rng();
        self.bag.shuffle(&mut rng);
    }
}

mod tests {}
