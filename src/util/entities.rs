use bevy::prelude::*;

pub fn despawn_all<T: Component>(
    mut commands: Commands,
    to_despawn: Query<(Entity, Option<&Transform>), With<T>>,
) {
    for (entity, transform) in &to_despawn {
        let append_translation = transform
            .map(|t| {
                let Vec3 { x, y, .. } = t.translation;
                format!(" at ({x}, {y})")
            })
            .unwrap_or_default();
        debug!("Despawning entity: {}{}", entity, append_translation);

        commands.entity(entity).despawn();
    }
}
