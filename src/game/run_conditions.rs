use bevy::{ecs::system::Res, input::mouse::AccumulatedMouseScroll};

pub fn was_mouse_wheel_used(mouse_wheel_input: Res<AccumulatedMouseScroll>) -> bool {
    mouse_wheel_input.delta.y != 0.
}
