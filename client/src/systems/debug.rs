use bevy::prelude::*;

use crate::GameState;

/// Log all gameplay state transitions for debugging.
pub fn run_on_any_transition(mut transitions: MessageReader<StateTransitionEvent<GameState>>) {
    for ev in transitions.read() {
        log::info!("State Transition from {:?} to {:?}", ev.exited, ev.entered);
    }
}
