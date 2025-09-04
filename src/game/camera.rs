use bevy::{
    input::{
        common_conditions::{input_just_released, input_pressed},
        mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    },
    prelude::*,
};

use crate::game::control::control_mode_is_normal;
use crate::{AppState, game::run_conditions::was_mouse_wheel_used};

static CAMERA_ZOOM_SPEED: f32 = 0.2;

#[derive(Clone, Debug)]
pub struct GameCameraPlugin;

impl Plugin for GameCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Game), setup).add_systems(
            Update,
            (
                move_camera.run_if(
                    input_pressed(MouseButton::Right).or(input_just_released(MouseButton::Right)),
                ),
                zoom_camera.run_if(control_mode_is_normal.and(was_mouse_wheel_used)),
            )
                .run_if(in_state(AppState::Game)),
        );
    }
}

fn setup(mut commands: Commands, camera: Single<Entity, With<Camera2d>>) {
    commands
        .entity(*camera)
        .insert(Transform::from_xyz(0., 0., 0.));
}

fn move_camera(
    camera: Single<(&mut Transform, &Projection), With<Camera2d>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
) {
    let (ref mut transform, projection) = camera.into_inner();
    let Projection::Orthographic(projection) = projection else {
        bevy::log::error!("Wrong camera projection. Expected orthographic!");
        return;
    };
    let factor = projection.scale;
    transform.translation.x -= factor * mouse_motion.delta.x;
    transform.translation.y += factor * mouse_motion.delta.y;
}

fn zoom_camera(
    projection: Single<&mut Projection, With<Camera2d>>,
    mouse_wheel_input: Res<AccumulatedMouseScroll>,
) {
    // https://bevyengine.org/examples/camera/projection-zoom/
    let Projection::Orthographic(ref mut projection) = *projection.into_inner() else {
        bevy::log::error!("Wrong camera projection. Expected orthographic!");
        return;
    };

    // We want scrolling up to zoom in, decreasing the scale, so we negate the delta.
    let delta_y = if mouse_wheel_input.unit == MouseScrollUnit::Line {
        -mouse_wheel_input.delta.y
    } else {
        // When unit is Pixel, then the value is always 132 with my browser,
        // but it probably depends on the configured sensitivity.
        -mouse_wheel_input.delta.y / 100.
    };
    // When changing scales, logarithmic changes are more intuitive.
    // To get this effect, we add 1 to the delta, so that a delta of 0
    // results in no multiplicative effect, positive values result in a multiplicative increase,
    // and negative values result in multiplicative decreases.
    projection.scale *= 1. + delta_y * CAMERA_ZOOM_SPEED;
}
