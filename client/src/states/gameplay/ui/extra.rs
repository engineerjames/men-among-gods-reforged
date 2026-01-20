// Small auxiliary UI update systems live here.

use bevy::prelude::*;

use crate::player_state::PlayerState;
use crate::states::gameplay::components::*;

/// Updates the money label text (gold/silver) in the HUD.
fn update_ui_money_text(
    player_state: &PlayerState,
    mut q: Query<&mut BitmapText, With<GameplayUiMoneyLabel>>,
) {
    // Display gold and silver. This mirrors the money display in run_gameplay_update_hud_labels
    // but can be called separately if needed.
    let pl = player_state.character_info();

    if let Some(mut text) = q.iter_mut().next() {
        let desired = format!("Money  {:8}G {:2}S", pl.gold / 100, pl.gold % 100);
        if text.text != desired {
            text.text = desired;
        }
    }
}

/// Updates smaller auxiliary UI elements that don't fit elsewhere.
///
/// Currently updates the money text (also covered by the main HUD-label system).
pub(crate) fn run_gameplay_update_extra_ui(
    player_state: Res<PlayerState>,
    mut q: ParamSet<(Query<&mut BitmapText, With<GameplayUiMoneyLabel>>,)>,
) {
    // Keep as a thin shim; money is also updated in run_gameplay_update_hud_labels.
    update_ui_money_text(&player_state, q.p0());
}
