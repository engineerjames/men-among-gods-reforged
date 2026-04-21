use core::{
    constants::{CharacterFlags, ST_NORMAL, USE_ACTIVE},
    string_operations::write_ascii_into_fixed,
};

use crate::game_state::GameState;

/// Run a test closure with a freshly initialized in-memory `GameState`.
///
/// `GameState` owns large fixed-size visibility buffers, so tests create it on
/// a dedicated 8 MiB stack to avoid stack overflows on the default test stack.
///
/// # Arguments
///
/// * `f` - Closure that receives mutable access to a fresh `GameState`.
///
/// # Returns
///
/// * The closure's return value.
pub(crate) fn with_test_gs<F, R>(f: F) -> R
where
    F: FnOnce(&mut GameState) -> R + Send + 'static,
    R: Send + 'static,
{
    std::thread::Builder::new()
        .name("server-test-gamestate".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let mut gs = GameState::new();
            f(&mut gs)
        })
        .expect("failed to spawn GameState test thread")
        .join()
        .expect("GameState test thread panicked")
}

/// Create a linked player slot and character slot for command-handler tests.
///
/// # Arguments
///
/// * `gs` - Active test game state.
///
/// # Returns
///
/// * `(character_id, player_id)` for the linked test entities.
pub(crate) fn add_test_player(gs: &mut GameState) -> (usize, usize) {
    let cn = 1;
    let nr = 1;

    gs.players[nr].state = ST_NORMAL;
    gs.players[nr].usnr = cn;
    gs.players[nr].lasttick = 0;
    gs.players[nr].lasttick2 = 0;
    gs.players[nr].ltick = 0;
    gs.players[nr].rtick = 0;

    let ch = &mut gs.characters[cn];
    *ch = core::types::Character::default();
    ch.used = USE_ACTIVE;
    ch.flags = CharacterFlags::Player.bits();
    ch.player = nr as i32;
    ch.x = 10;
    ch.y = 10;
    ch.tox = 10;
    ch.toy = 10;
    ch.frx = 10;
    ch.fry = 10;
    write_ascii_into_fixed(&mut ch.name, "Tester");
    write_ascii_into_fixed(&mut ch.reference, "Tester");

    (cn, nr)
}

/// Overwrite a player's incoming packet buffer for a direct handler call.
///
/// # Arguments
///
/// * `gs` - Active test game state.
/// * `nr` - Player slot receiving the packet bytes.
/// * `data` - Raw command payload to copy into `inbuf`.
pub(crate) fn write_inbuf(gs: &mut GameState, nr: usize, data: &[u8]) {
    gs.players[nr].inbuf.fill(0);
    let len = data.len().min(gs.players[nr].inbuf.len());
    gs.players[nr].inbuf[..len].copy_from_slice(&data[..len]);
    gs.players[nr].in_len = len;
}
