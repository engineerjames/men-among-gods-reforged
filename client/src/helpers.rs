use bevy::prelude::*;

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
