use std::sync::OnceLock;

use crate::single_thread_cell::SingleThreadCell;

mod admin;
mod combat;
mod commands;
mod commerce;
mod communication;
mod death;
mod economy;
mod inventory;
mod logging;
mod player_actions;
mod stats;
mod visibility;

static STATE: OnceLock<SingleThreadCell<State>> = OnceLock::new();

pub struct State {
    _visi: [i8; 40 * 40],
    visi: [i8; 40 * 40],
    vis_is_global: bool,
    see_miss: u64,
    see_hit: u64,
    ox: i32,
    oy: i32,
    is_monster: bool,
    pub penta_needed: usize,
}

impl State {
    fn new() -> Self {
        State {
            _visi: [0; 40 * 40],
            visi: [0; 40 * 40],
            vis_is_global: true,
            see_miss: 0,
            see_hit: 0,
            ox: 0,
            oy: 0,
            is_monster: false,
            penta_needed: 5,
        }
    }

    pub fn initialize() -> Result<(), String> {
        let state = State::new();
        STATE
            .set(SingleThreadCell::new(state))
            .map_err(|_| "State already initialized".to_string())?;
        Ok(())
    }

    // Internal helpers that lock and create references from the UnsafeCell
    fn with_state<F, R>(f: F) -> R
    where
        F: FnOnce(&State) -> R,
    {
        let state = STATE.get().expect("State not initialized");
        state.with(f)
    }

    fn with_state_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        let state = STATE.get().expect("State not initialized");
        state.with_mut(f)
    }

    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&State) -> R,
    {
        Self::with_state(f)
    }

    pub fn with_mut<F, R>(f: F) -> R
    where
        F: FnOnce(&mut State) -> R,
    {
        Self::with_state_mut(f)
    }
}
