// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use core::f64;
use std::{
    ops::{Add, Sub},
    time::Duration,
};

use aircraft::{Aircraft, AircraftPhysics};
use aircraft_card::AircraftCardPlugin;
use anyhow::anyhow;
use bevy::{
    input::{
        common_conditions::{input_just_pressed, input_just_released, input_pressed},
        mouse::{
            AccumulatedMouseMotion, AccumulatedMouseScroll, MouseButtonInput, MouseScrollUnit,
            MouseWheel,
        },
    },
    prelude::*,
};
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use camera::GameCameraPlugin;
use heading::Heading;
use rand_core::RngCore;

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
mod aircraft_card;
mod camera;
mod heading;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States)]
pub enum GameState {
    BeforeGame,
    Running,
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((GameCameraPlugin, AircraftCardPlugin))
            .register_type::<GameVariables>()
            .insert_resource(GameVariables::default())
            .add_event::<AircraftJustSpawned>()
            .add_systems(OnEnter(AppState::Game), (setup, spawn_aircraft))
            .add_systems(
                FixedUpdate,
                update_aircrafts.run_if(in_state(GameState::Running)),
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
) {
    let GameVariables { .. } = *variables;
    commands.insert_resource(GameResources {});
    game_state.set(GameState::Running);
}

fn spawn_aircraft(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut writer: EventWriter<AircraftJustSpawned>,
) {
    let entity = commands
        .spawn((
            Aircraft {
                call_sign: "Mayday321".to_owned(),
                cleared_altitude_feet: None,
                wanted_altitude_feet: 30000.,
                cleared_heading: Some(200.0.into()),
                cleared_speed_knots: None,
                wanted_speed_knots: 350.,
            },
            AircraftPhysics {
                altitude_feet: 7000.,
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
        ))
        .id();
    writer.write(AircraftJustSpawned(entity));
}

fn setup_dev_gui(variables: Res<GameVariables>, mut writer: EventWriter<DevGuiInputEvent>) {
    writer.write(DevGuiInputEvent::AddStruct(
        Box::new((*variables).clone()) as Box<dyn DevGuiStructTrait>
    ));
}

fn update_aircrafts(
    query: Query<(&Aircraft, &mut AircraftPhysics, &mut Transform)>,
    time: Res<Time>,
    game_variables: Res<GameVariables>,
) {
    let GameVariables {
        heading_accuracy_degrees,
        heading_diff_break_threshold_degrees,
        heading_break_factor,
        max_delta_heading_degrees_per_second,
        delta_heading_acceleration_degrees_per_second,
        speed_accuracy_knots,
        speed_diff_threshold_knots,
        speed_break_factor,
        max_delta_speed_knots_per_second,
        delta_speed_acceleration_knots_per_second,
        altitude_accuracy_feet,
        altitude_change_break_factor,
        altitude_diff_threshold_feet,
        max_delta_altitude_feet_per_second,
    } = *game_variables;
    let delta_seconds = time.delta_secs_f64();
    for (aircraft, physics, mut transform) in query {
        let Aircraft {
            cleared_altitude_feet,
            wanted_altitude_feet,
            cleared_heading,
            cleared_speed_knots,
            wanted_speed_knots,
            ..
        } = aircraft;

        let AircraftPhysics {
            heading,
            heading_change_degrees_per_second,
            speed_knots,
            acceleration_knots_per_second,
            altitude_feet,
            altitude_change_feet_per_second,
        } = physics.into_inner();

        let wanted = cleared_heading.unwrap_or(*heading);
        let required_change = heading.required_change(wanted);

        if *heading_change_degrees_per_second != 0. || required_change != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                required_change,
                accuracy: heading_accuracy_degrees,
                diff_threshold: heading_diff_break_threshold_degrees,
                break_factor: heading_break_factor,
                max_delta_val: max_delta_heading_degrees_per_second,
                delta_val_acceleration: delta_heading_acceleration_degrees_per_second,
                delta_val: heading_change_degrees_per_second,
            };
            if move_smooth(params) {
                *heading_change_degrees_per_second = 0.0;
                *heading = wanted;
                transform.rotation = Quat::from_axis_angle(
                    Vec3 {
                        z: -1.,
                        ..default()
                    },
                    wanted.to_rotation() as f32,
                );
            }
        }
        if *heading_change_degrees_per_second != 0. {
            *heading = heading.change(delta_seconds * *heading_change_degrees_per_second);
            transform.rotation = Quat::from_axis_angle(
                Vec3 {
                    z: -1.,
                    ..default()
                },
                heading.to_rotation() as f32,
            );
        }

        // speed
        let wanted = cleared_speed_knots.unwrap_or(*wanted_speed_knots);
        let required_change = -*speed_knots + wanted;
        if required_change != 0. || *acceleration_knots_per_second != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                required_change,
                accuracy: speed_accuracy_knots,
                diff_threshold: speed_diff_threshold_knots,
                break_factor: speed_break_factor,
                max_delta_val: max_delta_speed_knots_per_second,
                delta_val_acceleration: delta_speed_acceleration_knots_per_second,
                delta_val: acceleration_knots_per_second,
            };
            if move_smooth(params) {
                *acceleration_knots_per_second = 0.0;
                *speed_knots = wanted;
            }
        }
        *speed_knots += *acceleration_knots_per_second;

        let (Vec3 { z, .. }, angle) = transform.rotation.to_axis_angle();
        let angle = z * angle;
        let Vec2 {
            x: x_part,
            y: y_part,
        } = Vec2::from_angle(angle);
        transform.translation.x +=
            (*speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * x_part;
        transform.translation.y +=
            (*speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * y_part;

        // altitude

        let wanted = cleared_altitude_feet.unwrap_or(*wanted_altitude_feet);
        let required_change = -*altitude_feet + wanted;
        if *altitude_change_feet_per_second != 0. || required_change != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                required_change,
                accuracy: altitude_accuracy_feet,
                diff_threshold: altitude_diff_threshold_feet,
                break_factor: altitude_change_break_factor,
                max_delta_val: max_delta_speed_knots_per_second,
                delta_val_acceleration: max_delta_altitude_feet_per_second,
                delta_val: altitude_change_feet_per_second,
            };
            if move_smooth(params) {
                *altitude_change_feet_per_second = 0.0;
                *altitude_feet = wanted;
            }
        }
        *altitude_feet += *altitude_change_feet_per_second;
    }
}

struct MoveSmoothParams<'a> {
    delta_seconds: f64,
    required_change: f64,
    accuracy: f64,
    diff_threshold: f64,
    break_factor: f64,
    max_delta_val: f64,
    delta_val_acceleration: f64,
    delta_val: &'a mut f64,
}

fn move_smooth(params: MoveSmoothParams) -> bool {
    let MoveSmoothParams {
        delta_seconds,
        required_change,
        accuracy,
        diff_threshold,
        break_factor,
        max_delta_val,
        delta_val_acceleration,
        delta_val,
    } = params;
    let required_change_abs = required_change.abs();
    if required_change_abs < accuracy {
        return true;
    }

    // FIXME: airplane just falls from the sky, after reaching its cleared altitude,
    // Also doesn't reach cleared heading.
    if required_change_abs < diff_threshold {
        *delta_val += delta_seconds * break_factor * required_change * *delta_val;
    } else {
        let delta_val_abs = delta_val.abs();
        if delta_val_abs < max_delta_val {
            *delta_val -= required_change.signum()
                * ((delta_seconds * delta_val_acceleration).min(max_delta_val));
        }
    }
    false
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
    altitude_accuracy_feet: f64,
    altitude_diff_threshold_feet: f64,
    altitude_change_break_factor: f64,
    max_delta_altitude_feet_per_second: f64,
}

impl DevGuiStructTrait for GameVariables {}

impl Default for GameVariables {
    fn default() -> Self {
        Self {
            heading_accuracy_degrees: 0.001,
            heading_diff_break_threshold_degrees: 10.0,
            heading_break_factor: 0.99,
            max_delta_heading_degrees_per_second: 2.0,
            delta_heading_acceleration_degrees_per_second: 0.5,
            speed_accuracy_knots: 0.05,
            speed_diff_threshold_knots: 1.0,
            speed_break_factor: 4. / 5.,
            max_delta_speed_knots_per_second: 1.,
            delta_speed_acceleration_knots_per_second: 0.000005,
            altitude_accuracy_feet: 3.,
            altitude_diff_threshold_feet: 300.,
            altitude_change_break_factor: 4. / 5.,
            max_delta_altitude_feet_per_second: 100.0,
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

#[derive(Clone, Debug, Event)]
struct AircraftJustSpawned(Entity);
