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
pub struct MarbleBag {
    p: f32,
}

impl MarbleBag {
    pub fn new(draws: usize, probability: f32) -> Self {
        assert!(draws > 0, "draws must be > 0");
        assert!(
            probability >= 0.0 && probability <= 1.0,
            "probability must be between 0.0 and 1.0"
        );
        MarbleBag {
            p: probability / draws as f32,
        }
    }

    pub fn draw(&self) -> bool {
        let roll = rand::random::<f32>();
        roll <= self.p
    }

    fn reset(&mut self) {}
}
