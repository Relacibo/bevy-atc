// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use std::{
    ops::{Add, Sub},
    time::Duration,
};

use aircraft::{Aircraft, AircraftPhysics};
use anyhow::anyhow;
use bevy::{
    input::{
        common_conditions::{input_just_pressed, input_just_released, input_pressed},
        mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseButtonInput, MouseWheel},
    },
    prelude::*,
};
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use heading::Heading;
use rand_core::RngCore;

static CAMERA_ZOOM_SPEED: f32 = 0.2;
pub struct GamePlugin;

use crate::{
    APP_CONFIG, AppState,
    dev_gui::{DevGuiInputEvent, DevGuiStructTrait, DevGuiVariableUpdatedEvent},
    util::{
        consts::{FIXED_UPDATE_LENGTH_SECOND, PIXEL_PER_KNOT_SECOND, PIXELS_PER_MILE},
        entities::despawn_all,
        reflect::try_apply_parsed,
    },
};

mod aircraft;
mod heading;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States)]
pub enum GameState {
    BeforeGame,
    Running,
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GameVariables>()
            .insert_resource(GameVariables::default())
            .add_systems(OnEnter(AppState::Game), (setup, spawn_aircraft))
            .add_systems(
                FixedUpdate,
                update_aircraft.run_if(in_state(GameState::Running)),
            )
            .add_systems(
                Update,
                (
                    move_camera.run_if(
                        input_pressed(MouseButton::Right)
                            .or(input_just_released(MouseButton::Right)),
                    ),
                    zoom_camera,
                )
                    .run_if(in_state(GameState::Running)),
            )
            .insert_state(GameState::BeforeGame);

        if APP_CONFIG.dev_gui {
            app.add_systems(OnEnter(AppState::Game), setup_dev_gui)
                .add_systems(
                    Update,
                    (handle_dev_gui_events).run_if(in_state(AppState::Game)),
                );
        }
    }
}

fn setup(
    mut commands: Commands,
    variables: Res<GameVariables>,
    mut game_state: ResMut<NextState<GameState>>,
    camera: Single<Entity, With<Camera2d>>,
) {
    let GameVariables { .. } = *variables;
    commands.insert_resource(GameResources {});
    game_state.set(GameState::Running);
    commands
        .entity(*camera)
        .insert(Transform::from_xyz(0., 0., 0.));
}

fn spawn_aircraft(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Aircraft {
            call_sign: "Mayday321".to_owned(),
            cleared_altitude_feet: None,
            cleared_heading: None,
            cleared_speed_knots: None,
            wanted_speed_knots: 300.,
        },
        AircraftPhysics {
            altitude_feet: 10000.,
            altitude_change_feet_per_second: 10.,
            heading: Heading::from(30.),
            heading_change_degrees_per_second: 1.0,
            speed_knots: 300.,
            acceleration_knots_per_second: 10.,
        },
        Mesh2d(meshes.add(Rectangle {
            half_size: Vec2::new(5., 5.),
        })),
        MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
        Transform::from_xyz(0., 0., 10.),
        children![(
            Mesh2d(meshes.add(Rectangle {
                half_size: Vec2::new(20., 1.),
            })),
            MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
            Transform::from_xyz(20., 0., 11.),
        )],
    ));
}

fn setup_dev_gui(variables: Res<GameVariables>, mut writer: EventWriter<DevGuiInputEvent>) {
    writer.write(DevGuiInputEvent::AddStruct(
        Box::new((*variables).clone()) as Box<dyn DevGuiStructTrait>
    ));
}

fn update_aircraft(
    query: Query<(&Aircraft, &mut AircraftPhysics, &mut Transform)>,
    time: Res<Time>,
    game_variables: Res<GameVariables>,
) {
    let GameVariables {
        heading_accuracy_degrees,
        heading_diff_break_threshold_degrees: heading_change_break_threshold_degrees,
        heading_break_factor: heading_change_break_factor,
        max_delta_heading_degrees_per_second: max_heading_change_degrees_per_second,
        delta_heading_acceleration_degrees_per_second: heading_change_acceleration_degrees_per_second,
        speed_accuracy_knots,
        speed_diff_threshold_knots: speed_accuracy_threshold_knots,
        speed_break_factor,
        max_delta_speed_knots_per_second: max_acceleration_knots_per_second,
        delta_speed_acceleration_knots_per_second: acceleration_change_knots_per_second,
    } = *game_variables;
    let delta_seconds = time.delta_secs_f64();
    for (aircraft, mut physics, mut transform) in query {
        let Aircraft {
            cleared_altitude_feet,
            cleared_heading,
            cleared_speed_knots,
            wanted_speed_knots,
            ..
        } = aircraft;

        if let Some(cleared_heading) = cleared_heading {
            let required_change = physics.heading.required_change(*cleared_heading);
            let required_change_abs = required_change.abs();
            if required_change_abs >= heading_accuracy_degrees {
                if required_change_abs < heading_change_break_threshold_degrees {
                    physics.heading_change_degrees_per_second -=
                        delta_seconds * required_change * heading_change_break_factor;
                } else if physics.heading_change_degrees_per_second
                    < max_heading_change_degrees_per_second
                {
                    physics.heading_change_degrees_per_second +=
                        delta_seconds * heading_change_acceleration_degrees_per_second;
                }
            } else {
                physics.heading_change_degrees_per_second = 0.0;
                physics.heading = *cleared_heading;
                transform.rotation = Quat::from_axis_angle(
                    Vec3 {
                        z: -1.,
                        ..default()
                    },
                    cleared_heading.to_rotation() as f32,
                );
            }
        }
        if physics.heading_change_degrees_per_second != 0. {
            physics.heading = physics
                .heading
                .change(delta_seconds * physics.heading_change_degrees_per_second);
            transform.rotation = Quat::from_axis_angle(
                Vec3 {
                    z: -1.,
                    ..default()
                },
                physics.heading.to_rotation() as f32,
            );
        }

        // speed
        let wanted_speed = cleared_speed_knots.unwrap_or(*wanted_speed_knots);
        let required_change = physics.speed_knots - wanted_speed;
        let required_change_abs = required_change.abs();
        if required_change_abs >= speed_accuracy_knots {
            if required_change_abs < speed_accuracy_threshold_knots {
                physics.acceleration_knots_per_second -=
                    delta_seconds * speed_break_factor * required_change;
            } else if physics.acceleration_knots_per_second < max_acceleration_knots_per_second {
                physics.acceleration_knots_per_second +=
                    delta_seconds * acceleration_change_knots_per_second;
            }
        } else {
            physics.acceleration_knots_per_second = 0.0;
            physics.speed_knots = wanted_speed;
        }

        physics.speed_knots += physics.acceleration_knots_per_second;

        let (Vec3 { z, .. }, angle) = transform.rotation.to_axis_angle();
        let angle = z * angle;
        let Vec2 {
            x: x_part,
            y: y_part,
        } = Vec2::from_angle(angle);
        transform.translation.x +=
            (physics.speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * x_part;
        transform.translation.y +=
            (physics.speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * y_part;
    }
}

fn move_camera(
    mut camera: Single<&mut Transform, With<Camera2d>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
) {
    camera.translation.x -= mouse_motion.delta.x;
    camera.translation.y += mouse_motion.delta.y;
}

fn zoom_camera(
    projection: Single<&mut Projection, With<Camera2d>>,
    mouse_wheel_input: Res<AccumulatedMouseScroll>,
) {
    let Projection::Orthographic(ref mut projection) = *projection.into_inner() else {
        eprintln!("Wrong camera projection. Expected orthographic!");
        return;
    };
    // We want scrolling up to zoom in, decreasing the scale, so we negate the delta.
    let delta_zoom = -mouse_wheel_input.delta.y * CAMERA_ZOOM_SPEED;
    // When changing scales, logarithmic changes are more intuitive.
    // To get this effect, we add 1 to the delta, so that a delta of 0
    // results in no multiplicative effect, positive values result in a multiplicative increase,
    // and negative values result in multiplicative decreases.
    let multiplicative_zoom = 1. + delta_zoom;
    projection.scale = projection.scale * multiplicative_zoom;

}

fn handle_dev_gui_events(
    mut reader: EventReader<DevGuiVariableUpdatedEvent>,
    mut variables: ResMut<GameVariables>,
) {
    for DevGuiVariableUpdatedEvent { key, value } in reader.read() {
        debug!("Updated {key} -> {value}");
        // let old = variables.clone();
        let field = variables
            .reflect_mut()
            .as_struct()
            .unwrap()
            .field_mut(key)
            .unwrap();
        try_apply_parsed(field, value)
            .inspect_err(|err| error!("{err}"))
            .ok();
    }
}

#[derive(Debug, Clone, Resource, Reflect)]
struct GameVariables {
    heading_accuracy_degrees: f64,
    heading_diff_break_threshold_degrees: f64,
    heading_break_factor: f64,
    max_delta_heading_degrees_per_second: f64,
    delta_heading_acceleration_degrees_per_second: f64,
    speed_accuracy_knots: f64,
    speed_diff_threshold_knots: f64,
    speed_break_factor: f64,
    max_delta_speed_knots_per_second: f64,
    delta_speed_acceleration_knots_per_second: f64,
}

impl DevGuiStructTrait for GameVariables {}

impl Default for GameVariables {
    fn default() -> Self {
        Self {
            heading_accuracy_degrees: 0.001,
            heading_diff_break_threshold_degrees: 1.0,
            heading_break_factor: 4. / 5.,
            max_delta_heading_degrees_per_second: 10.0,
            delta_heading_acceleration_degrees_per_second: 0.5,
            speed_accuracy_knots: 0.05,
            speed_diff_threshold_knots: 1.0,
            speed_break_factor: 4. / 5.,
            max_delta_speed_knots_per_second: 1.,
            delta_speed_acceleration_knots_per_second: 0.000005,
        }
    }
}

#[derive(Resource)]
pub struct GameResources {}

const AIRCRAFT_COLOR: Srgba = Srgba {
    red: 0.,
    green: 0.4,
    blue: 0.3,
    alpha: 1.0,
};
