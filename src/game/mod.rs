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
                heading: Heading::from(30.),
                heading_change_degrees_per_second: 1.0,
                speed_knots: 200.,
                acceleration_knots_per_second: 1.,
                altitude_feet: 7000.,
                altitude_change_feet_per_second: 10.,
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
        max_delta_heading_degrees_per_second,
        delta_heading_acceleration_degrees_per_second,
        speed_accuracy_knots,
        max_delta_speed_knots_per_second,
        delta_speed_acceleration_knots_per_second,
        altitude_accuracy_feet,
        max_delta_altitude_feet_per_second,
        delta_altitude_acceleration_feet_per_second,
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

        // dbg!("Heading");

        // heading
        let wanted = cleared_heading.unwrap_or(*heading);
        // FIXME: With heading, maybe not always correct, in proximity of
        // required_change = 180 degrees, moving away from wanted heading
        let required_change_u = heading.required_change(wanted);

        if *heading_change_degrees_per_second != 0. || required_change_u != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: heading_accuracy_degrees,
                max_delta_val_u_per_second: max_delta_heading_degrees_per_second,
                delta_val_acceleration_u_per_second2: delta_heading_acceleration_degrees_per_second,
                delta_val_u_per_second: heading_change_degrees_per_second,
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

        // dbg!("Speed");
        // speed
        let wanted = cleared_speed_knots.unwrap_or(*wanted_speed_knots);
        let required_change_u = -*speed_knots + wanted;
        if required_change_u != 0. || *acceleration_knots_per_second != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: speed_accuracy_knots,
                max_delta_val_u_per_second: max_delta_speed_knots_per_second,
                delta_val_acceleration_u_per_second2: delta_speed_acceleration_knots_per_second,
                delta_val_u_per_second: acceleration_knots_per_second,
            };
            if move_smooth(params) {
                *acceleration_knots_per_second = 0.0;
                *speed_knots = wanted;
            }
        }
        *speed_knots += *acceleration_knots_per_second * delta_seconds;

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
        // dbg!("Altitude");

        let wanted = cleared_altitude_feet.unwrap_or(*wanted_altitude_feet);
        let required_change_u = -*altitude_feet + wanted;
        if *altitude_change_feet_per_second != 0. || required_change_u != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: altitude_accuracy_feet,
                max_delta_val_u_per_second: max_delta_altitude_feet_per_second,
                delta_val_acceleration_u_per_second2: delta_altitude_acceleration_feet_per_second,
                delta_val_u_per_second: altitude_change_feet_per_second,
            };
            if move_smooth(params) {
                *altitude_change_feet_per_second = 0.0;
                *altitude_feet = wanted;
            }
        }
        *altitude_feet += *altitude_change_feet_per_second * delta_seconds;

        // dbg!("---------");
    }
}

struct MoveSmoothParams<'a> {
    delta_seconds: f64,
    accuracy_u: f64,
    max_delta_val_u_per_second: f64,
    delta_val_acceleration_u_per_second2: f64,
    delta_val_u_per_second: &'a mut f64,
    val_remaining_u: f64,
}

fn move_smooth(params: MoveSmoothParams) -> bool {
    let MoveSmoothParams {
        delta_seconds,
        accuracy_u,
        max_delta_val_u_per_second,
        delta_val_acceleration_u_per_second2,
        delta_val_u_per_second,
        val_remaining_u,
    } = params;
    let required_change_abs = val_remaining_u.abs();

    if required_change_abs < accuracy_u {
        return true;
    }

    let direction_to_target = val_remaining_u.signum();
    let moving_direction = delta_val_u_per_second.signum();
    // If wrong direction: break
    if direction_to_target != moving_direction {
        apply_acceleration(
            direction_to_target,
            delta_seconds,
            delta_val_acceleration_u_per_second2,
            max_delta_val_u_per_second,
            delta_val_u_per_second,
        );
        return false;
    }

    let delta_val_abs = delta_val_u_per_second.abs();
    // Required break time:
    // (+-) == required_signum
    // f(x) = (-+)delta_val_acceleration * x + delta_val
    // solve: f(x1) = 0
    // x = -delta_val / (-+)delta_val_acceleration
    // x = (+-) (+-)|delta_val| / delta_val_acceleration [because delta_val.signum() == required_signum]
    let braking_time = delta_val_abs / delta_val_acceleration_u_per_second2;

    // required breaking distance
    // f1_int(x) = (-+)delta_val_acceleration/2 * x^2 + delta_val * x
    // f1_int(x1)
    let braking_distance = -0.5
        * direction_to_target
        * delta_val_acceleration_u_per_second2
        * braking_time
        * braking_time
        + *delta_val_u_per_second * braking_time;

    let should_brake = required_change_abs <= braking_distance;

    if should_brake || delta_val_u_per_second.abs() < max_delta_val_u_per_second {
        let accel_direction = if should_brake { -1.0 } else { 1.0 } * direction_to_target;
        apply_acceleration(
            accel_direction,
            delta_seconds,
            delta_val_acceleration_u_per_second2,
            max_delta_val_u_per_second,
            delta_val_u_per_second,
        );
    }
    false
}

fn apply_acceleration(
    direction: f64,
    delta_seconds: f64,
    delta_val_acceleration_x_per_second2: f64,
    max_delta_val_x_per_second: f64,
    delta_val_x_per_second: &mut f64,
) {
    *delta_val_x_per_second += direction * (delta_seconds * delta_val_acceleration_x_per_second2);
    *delta_val_x_per_second =
        delta_val_x_per_second.clamp(-max_delta_val_x_per_second, max_delta_val_x_per_second);
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
    max_delta_heading_degrees_per_second: f64,
    delta_heading_acceleration_degrees_per_second: f64,
    speed_accuracy_knots: f64,
    max_delta_speed_knots_per_second: f64,
    delta_speed_acceleration_knots_per_second: f64,
    altitude_accuracy_feet: f64,
    max_delta_altitude_feet_per_second: f64,
    delta_altitude_acceleration_feet_per_second: f64,
}

impl DevGuiStructTrait for GameVariables {}

impl Default for GameVariables {
    fn default() -> Self {
        Self {
            heading_accuracy_degrees: 0.2,
            max_delta_heading_degrees_per_second: 2.0,
            delta_heading_acceleration_degrees_per_second: 0.1,
            speed_accuracy_knots: 0.2,
            max_delta_speed_knots_per_second: 2.,
            delta_speed_acceleration_knots_per_second: 0.1,
            altitude_accuracy_feet: 10.,
            max_delta_altitude_feet_per_second: 100.0,
            delta_altitude_acceleration_feet_per_second: 5.,
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

#[cfg(test)]
mod tests {
    use super::{MoveSmoothParams, move_smooth};

    #[test]
    fn test_move_ascend_over() {
        let delta_val = &mut 2.;
        let params = MoveSmoothParams {
            delta_seconds: 0.02,
            val_remaining_u: 1.,
            accuracy_u: 0.1,
            max_delta_val_u_per_second: 2.,
            delta_val_acceleration_u_per_second2: 0.1,
            delta_val_u_per_second: delta_val,
        };
        let res = move_smooth(params);
        assert!(!res);
        dbg!(&delta_val);
        assert!(*delta_val < 2.);
    }

    #[test]
    fn test_move_ascend_over_below_theshold() {
        let delta_val = &mut 2.;
        let params = MoveSmoothParams {
            delta_seconds: 0.02,
            val_remaining_u: 0.9,
            accuracy_u: 0.1,
            max_delta_val_u_per_second: 2.,
            delta_val_acceleration_u_per_second2: 0.1,
            delta_val_u_per_second: delta_val,
        };
        let res = move_smooth(params);
        assert!(!res);
        dbg!(&delta_val);
        assert!(*delta_val < 2. && *delta_val > -2.);
    }
}
