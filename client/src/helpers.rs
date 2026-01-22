use bevy::prelude::*;
use mag_core::constants::{
    LO_CHALLENGE, LO_EXIT, LO_FAILURE, LO_IDLE, LO_KICKED, LO_NONACTIVE, LO_NOROOM, LO_PARAMS,
    LO_PASSWORD, LO_SHUTDOWN, LO_SLOW, LO_TAVERN, LO_USURP, LO_VERSION,
};

pub fn despawn_tree(entity: Entity, children_q: &Query<&Children>, commands: &mut Commands) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_tree(child, children_q, commands);
        }
    }
    commands.entity(entity).queue_silenced(|e: EntityWorldMut| {
        e.despawn();
    });
}

pub fn exit_reason_string(code: u32) -> &'static str {
    match code as u8 {
        LO_CHALLENGE => "[LO_CHALLENGE] Challenge failure",
        LO_IDLE => "[LO_IDLE] Player idle too long",
        LO_NOROOM => "[LO_NOROOM] No room left on server",
        LO_PARAMS => "[LO_PARAMS] Invalid parameters",
        LO_NONACTIVE => "[LO_NONACTIVE] Player not active",
        LO_PASSWORD => "[LO_PASSWORD] Invalid password",
        LO_SLOW => "[LO_SLOW] Connection too slow",
        LO_FAILURE => "[LO_FAILURE] Login failure",
        LO_SHUTDOWN => "[LO_SHUTDOWN] Server shutting down",
        LO_TAVERN => "[LO_TAVERN] Returned to tavern",
        LO_VERSION => "[LO_VERSION] Version mismatch",
        LO_EXIT => "[LO_EXIT] Client exit",
        LO_USURP => "[LO_USURP] Logged in elsewhere",
        LO_KICKED => "[LO_KICKED] Kicked from server",
        _ => "[UNKNOWN] Unrecognized reason code",
    }
}
