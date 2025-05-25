// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use std::{
    ops::{Add, Sub},
    time::Duration,
};

use aircraft::{Aircraft, AircraftPhysics};
use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use heading::Heading;
use rand_core::RngCore;

pub struct GamePlugin;

use crate::{
    APP_CONFIG, AppState,
    dev_gui::DevGuiEvent,
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
            .insert_state(GameState::BeforeGame);
    }
}

fn setup(
    mut commands: Commands,
    variables: Res<GameVariables>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    let GameVariables { .. } = *variables;
    commands.insert_resource(GameResources {});
    game_state.set(GameState::Running);
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

fn update_aircraft(
    mut commands: Commands,
    mut query: Query<(&Aircraft, &mut AircraftPhysics, &mut Transform)>,
    time: Res<Time>,
    game_variables: Res<GameVariables>,
) {
    let GameVariables {
        heading_accuracy_degrees,
        heading_change_break_threshold_degrees,
        heading_change_break_factor,
        max_heading_change_degrees_per_second,
        heading_change_acceleration_degrees_per_second,
        speed_accuracy_knots,
        speed_accuracy_threshold_knots,
        speed_break_factor,
        max_acceleration_knots_per_second,
        acceleration_change_knots_per_second,
    } = *game_variables;
    let delta = time.delta();
    let delta_seconds = delta.as_micros() as f64 / 1000000.;
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

#[derive(Clone, Debug, Resource, Reflect)]
struct GameVariables {
    heading_accuracy_degrees: f64,
    heading_change_break_threshold_degrees: f64,
    heading_change_break_factor: f64,
    max_heading_change_degrees_per_second: f64,
    heading_change_acceleration_degrees_per_second: f64,
    speed_accuracy_knots: f64,
    speed_accuracy_threshold_knots: f64,
    speed_break_factor: f64,
    max_acceleration_knots_per_second: f64,
    acceleration_change_knots_per_second: f64,
}

impl Default for GameVariables {
    fn default() -> Self {
        Self {
            heading_accuracy_degrees: 0.001,
            heading_change_break_threshold_degrees: 1.0,
            heading_change_break_factor: 4. / 5.,
            max_heading_change_degrees_per_second: 10.0,
            heading_change_acceleration_degrees_per_second: 0.5,
            speed_accuracy_knots: 0.05,
            speed_accuracy_threshold_knots: 1.0,
            speed_break_factor: 4. / 5.,
            max_acceleration_knots_per_second: 1.,
            acceleration_change_knots_per_second: 0.000005,
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
