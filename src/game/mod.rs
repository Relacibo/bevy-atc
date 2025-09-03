// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use core::f64;
use std::{
    ops::{Add, Sub},
    time::Duration,
};

use aircraft::Aircraft;
use aircraft_card::AircraftCardPlugin;
use anyhow::anyhow;
use bevy::window::PrimaryWindow;
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
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use camera::GameCameraPlugin;
use heading::Heading;
use rand_core::RngCore;
use serde::Deserialize;

pub struct GamePlugin;

use crate::{
    APP_CONFIG, AppState,
    dev_gui::{DevGuiInputEvent, DevGuiStructTrait, DevGuiVariableUpdatedEvent},
    game::control::ControlPlugin,
    menu::LevelMeta,
    util::{consts::PIXEL_PER_KNOT_SECOND, reflect::try_apply_parsed},
};

mod aircraft;
mod aircraft_card;
mod camera;
mod control;
mod heading;
mod run_conditions;

// Z-Index-Konstanten für die Spielobjekte
pub const Z_BACKGROUND: f32 = 0.0;
pub const Z_RUNWAY: f32 = 1.0;
pub const Z_WAYPOINT: f32 = 4.0;
pub const Z_AIRCRAFT: f32 = 8.0;
pub const Z_AIRCRAFT_CARD: f32 = 10.0;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ControlPlugin,
            GameCameraPlugin,
            AircraftCardPlugin,
            MeshPickingPlugin,
            RonAssetPlugin::<LevelFile>::new(&["ron"]),
        ))
        .register_type::<GameVariables>()
        .add_event::<AircraftJustSpawned>()
        .add_systems(OnEnter(AppState::Game), enter_loading_state)
        .add_systems(OnEnter(GameState::Loading), load_level_asset)
        .add_systems(
            Update,
            poll_level_loaded.run_if(in_state(GameState::Loading)),
        )
        .add_systems(
            OnEnter(GameState::Running),
            (setup, spawn_aircraft, spawn_waypoints),
        )
        .add_systems(
            FixedUpdate,
            update_aircrafts.run_if(in_state(GameState::Running)),
        )
        .add_systems(
            Update,
            spawn_aircraft_at_mouse
                .run_if(in_state(GameState::Running).and(input_just_pressed(KeyCode::KeyS))),
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

#[derive(Resource, Default)]
struct PendingLevelHandle(Option<Handle<LevelFile>>);

fn enter_loading_state(mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::Loading);
}

fn load_level_asset(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game_vars: Res<GameVariables>,
) {
    let handle = asset_server.load::<LevelFile>(format!("levels/{}", &game_vars.level.file));
    commands.insert_resource(PendingLevelHandle(Some(handle)));
}

fn poll_level_loaded(
    mut game_state: ResMut<NextState<GameState>>,
    pending: Res<PendingLevelHandle>,
    level_assets: Res<Assets<LevelFile>>,
) {
    let Some(handle) = pending.0.as_ref() else {
        return;
    };
    let Some(_level) = level_assets.get(handle) else {
        return;
    };
    game_state.set(GameState::Running);
}

fn spawn_waypoints(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    pending: Res<PendingLevelHandle>,
    level_assets: Res<Assets<LevelFile>>,
) {
    let Some(handle) = pending.0.as_ref() else {
        return;
    };
    let Some(level) = level_assets.get(handle) else {
        return;
    };
    for wp in &level.waypoints {
        commands.spawn((
            Waypoint {
                name: wp.name.clone(),
            },
            Mesh2d(meshes.add(Circle { radius: 10.0 })),
            MeshMaterial2d(materials.add(Color::srgb(0.5, 0.5, 0.5))),
            Transform::from_xyz(wp.pos.x, wp.pos.y, Z_WAYPOINT),
            Name::new(wp.name.clone()),
            children![(
                Text2d(wp.name.clone()),
                TextFont::from_font_size(32.0),
                Transform::from_xyz(16.0, 16.0, 0.1).with_scale(Vec3 {
                    x: 1.0,
                    y: 1.0,
                    z: 1.0,
                },),
                Visibility::Inherited,
            )],
        ));
    }

    // Spawn runways
    for rw in &level.runways {
        let dir = rw.end - rw.start;
        let length = dir.length();
        let angle = -dir.angle_to(Vec2::X);
        commands.spawn((
            Mesh2d(meshes.add(Rectangle {
                half_size: Vec2::new(length / 2.0, 5.0),
            })),
            MeshMaterial2d(materials.add(Color::srgb(0.3, 0.3, 0.3))),
            Transform::from_xyz(
                (rw.start.x + rw.end.x) / 2.0,
                (rw.start.y + rw.end.y) / 2.0,
                Z_RUNWAY,
            )
            .with_rotation(Quat::from_rotation_z(angle)),
            Name::new(format!("Runway {}", rw.name)),
            Visibility::Visible,
        ));
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
                heading: Heading::from(30.),
                heading_change_degrees_per_second: 1.0,
                speed_knots: 200.,
                acceleration_knots_per_second: 1.,
                altitude_feet: 7000.,
                altitude_change_feet_per_second: 10.,
                cleared_heading_change_direction: None,
            },
            Mesh2d(meshes.add(Rectangle {
                half_size: Vec2::new(5., 5.),
            })),
            MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
            Transform::from_xyz(0., 0., Z_AIRCRAFT),
            children![(
                Mesh2d(meshes.add(Rectangle {
                    half_size: Vec2::new(20., 1.),
                })),
                MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
                Transform::from_xyz(20., 0., 0.5),
            ),],
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
    query: Query<(&mut Aircraft, &mut Transform)>,
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
        ..
    } = *game_variables;
    let delta_seconds = time.delta_secs_f64();
    for (mut aircraft, mut transform) in query {
        let Aircraft {
            cleared_altitude_feet,
            wanted_altitude_feet,
            cleared_heading,
            cleared_heading_change_direction,
            cleared_speed_knots,
            wanted_speed_knots,
            heading,
            heading_change_degrees_per_second,
            speed_knots,
            acceleration_knots_per_second,
            altitude_feet,
            altitude_change_feet_per_second,
            ..
        } = &mut *aircraft;

        // dbg!("Heading");

        // heading
        let wanted = cleared_heading.unwrap_or(*heading);
        let required_change_u = required_heading_change(
            *heading,
            wanted,
            *heading_change_degrees_per_second,
            *cleared_heading_change_direction,
            max_delta_heading_degrees_per_second,
            delta_heading_acceleration_degrees_per_second,
        );

        if *heading_change_degrees_per_second != 0. || required_change_u != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: heading_accuracy_degrees,
                max_delta_val_u_per_second: max_delta_heading_degrees_per_second,
                delta_val_acceleration_u_per_second2: delta_heading_acceleration_degrees_per_second,
                delta_val_u_per_second: *heading_change_degrees_per_second,
            };
            let MoveSmoothReturn {
                finished_moving,
                delta_val_u_per_second,
            } = move_smooth(params);
            *heading_change_degrees_per_second = delta_val_u_per_second;
            if finished_moving {
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
                delta_val_u_per_second: *acceleration_knots_per_second,
            };
            let MoveSmoothReturn {
                finished_moving,
                delta_val_u_per_second,
            } = move_smooth(params);
            *acceleration_knots_per_second = delta_val_u_per_second;
            if finished_moving {
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
                delta_val_u_per_second: *altitude_change_feet_per_second,
            };
            let MoveSmoothReturn {
                finished_moving,
                delta_val_u_per_second,
            } = move_smooth(params);
            *altitude_change_feet_per_second = delta_val_u_per_second;
            if finished_moving {
                *altitude_feet = wanted;
            }
        }
        *altitude_feet += *altitude_change_feet_per_second * delta_seconds;

        // dbg!("---------");
    }
}

struct MoveSmoothParams {
    delta_seconds: f64,
    accuracy_u: f64,
    max_delta_val_u_per_second: f64,
    delta_val_acceleration_u_per_second2: f64,
    delta_val_u_per_second: f64,
    val_remaining_u: f64,
}

struct MoveSmoothReturn {
    delta_val_u_per_second: f64,
    finished_moving: bool,
}

fn move_smooth(params: MoveSmoothParams) -> MoveSmoothReturn {
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
        return MoveSmoothReturn {
            finished_moving: true,
            delta_val_u_per_second: 0.,
        };
    }

    let direction_to_target = val_remaining_u.signum();
    let moving_direction = delta_val_u_per_second.signum();
    // If wrong direction: break
    if direction_to_target != moving_direction {
        let delta_val_x_per_second_new = apply_acceleration(
            direction_to_target,
            delta_seconds,
            delta_val_acceleration_u_per_second2,
            max_delta_val_u_per_second,
            delta_val_u_per_second,
        );
        return MoveSmoothReturn {
            delta_val_u_per_second: delta_val_x_per_second_new,
            finished_moving: false,
        };
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
        + delta_val_u_per_second * braking_time;

    let should_brake = required_change_abs <= braking_distance;
    let delta_val_x_per_second_new =
        if should_brake || delta_val_u_per_second.abs() < max_delta_val_u_per_second {
            let accel_direction = if should_brake { -1.0 } else { 1.0 } * direction_to_target;
            apply_acceleration(
                accel_direction,
                delta_seconds,
                delta_val_acceleration_u_per_second2,
                max_delta_val_u_per_second,
                delta_val_u_per_second,
            )
        } else {
            delta_val_u_per_second
        };
    MoveSmoothReturn {
        delta_val_u_per_second: delta_val_x_per_second_new,
        finished_moving: false,
    }
}

fn apply_acceleration(
    direction: f64,
    delta_seconds: f64,
    delta_val_acceleration_x_per_second2: f64,
    max_delta_val_x_per_second: f64,
    delta_val_x_per_second: f64,
) -> f64 {
    let mut delta_val_x_per_second_new =
        delta_val_x_per_second + direction * (delta_seconds * delta_val_acceleration_x_per_second2);
    delta_val_x_per_second_new =
        delta_val_x_per_second_new.clamp(-max_delta_val_x_per_second, max_delta_val_x_per_second);
    delta_val_x_per_second_new
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
pub struct GameVariables {
    pub level: LevelMeta,
    pub heading_accuracy_degrees: f64,
    pub max_delta_heading_degrees_per_second: f64,
    pub delta_heading_acceleration_degrees_per_second: f64,
    pub speed_accuracy_knots: f64,
    pub max_delta_speed_knots_per_second: f64,
    pub delta_speed_acceleration_knots_per_second: f64,
    pub altitude_accuracy_feet: f64,
    pub max_delta_altitude_feet_per_second: f64,
    pub delta_altitude_acceleration_feet_per_second: f64,
}

impl DevGuiStructTrait for GameVariables {}

impl GameVariables {
    pub fn new(level: LevelMeta) -> Self {
        Self {
            level,
            heading_accuracy_degrees: 0.2,
            max_delta_heading_degrees_per_second: 2.0,
            delta_heading_acceleration_degrees_per_second: 0.5,
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

fn spawn_aircraft_at_mouse(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut rng: GlobalEntropy<WyRand>,
    mut writer: EventWriter<AircraftJustSpawned>,
) {
    let (camera, camera_transform) = &*camera;
    let Some(screen_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, screen_pos) else {
        return;
    };
    let call_signs = ["Alpha1", "Bravo2", "Charlie3", "Delta4", "Echo5"];
    let idx = (rng.next_u32() as usize) % call_signs.len();
    let call_sign = call_signs[idx].to_string();
    let heading = (rng.next_u32() % 360) as f64;
    let altitude_feet = 1000.0 + (rng.next_u32() % 39000) as f64;
    let entity = commands
        .spawn((
            Aircraft {
                call_sign,
                cleared_altitude_feet: None,
                wanted_altitude_feet: 30000.,
                cleared_heading: Some(heading.into()),
                cleared_speed_knots: None,
                wanted_speed_knots: 350.,
                heading: Heading::from(heading),
                heading_change_degrees_per_second: 1.0,
                speed_knots: 200.,
                acceleration_knots_per_second: 1.,
                altitude_feet,
                altitude_change_feet_per_second: 10.,
                cleared_heading_change_direction: None,
            },
            Mesh2d(meshes.add(Rectangle {
                half_size: Vec2::new(5., 5.),
            })),
            MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
            Transform::from_xyz(world_pos.x, world_pos.y, Z_AIRCRAFT),
            children![(
                Mesh2d(meshes.add(Rectangle {
                    half_size: Vec2::new(20., 1.),
                })),
                MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
                Transform::from_xyz(20., 0., 0.5),
            ),],
        ))
        .id();
    writer.write(AircraftJustSpawned(entity));
}

#[derive(Deserialize, Clone, Debug, Asset, Reflect)]
pub struct LevelFile {
    pub waypoints: Vec<WaypointData>,
    pub runways: Vec<RunwayData>,
}

#[derive(Deserialize, Clone, Debug, Reflect)]
pub struct WaypointData {
    pub name: String,
    pub pos: Vec2,
}

#[derive(Deserialize, Clone, Debug, Reflect)]
pub struct RunwayData {
    pub name: String,
    pub start: Vec2,
    pub end: Vec2,
    pub elevation: f32,
}

#[derive(Component, Clone, Debug)]
pub struct Waypoint {
    pub name: String,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States)]
pub enum GameState {
    BeforeGame,
    Loading,
    Running,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TurnDirection {
    Left,
    Right,
}

fn required_heading_change(
    current: Heading,
    wanted: Heading,
    current_rate: f64,
    cleared_direction: Option<TurnDirection>,
    max_rate: f64,
    accel: f64,
) -> f64 {
    let delta = current.required_change(wanted);
    // Wenn Clearance vorhanden, zwinge Richtung
    if let Some(dir) = cleared_direction {
        let delta = match dir {
            TurnDirection::Left if delta > 0.0 => delta - 360.,
            TurnDirection::Right if delta < 0.0 => delta + 360.,
            _ => delta,
        };
        return delta;
    }
    // Bestimme kürzeste Richtung
    let left_delta = if delta < 0.0 { delta } else { delta - 360.0 };
    let right_delta = if delta > 0.0 { delta } else { delta + 360.0 };
    let (shorter, longer) = if left_delta.abs() < right_delta.abs() {
        (left_delta, right_delta)
    } else {
        (right_delta, left_delta)
    };
    // Analytische Zeitabschätzung für beide Richtungen
    fn analytic_time(delta: f64, current_rate: f64, accel: f64) -> f64 {
        // s(t) = current_rate * t + 0.5 * accel * t^2 = delta
        // 0.5 * accel * t^2 + current_rate * t - delta = 0
        let a = 0.5 * accel;
        let b = current_rate;
        let c = -delta;
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 || a.abs() < 1e-8 {
            // Fallback: lineare Lösung
            if b.abs() > 1e-8 {
                return delta / b;
            } else {
                return f64::INFINITY;
            }
        }
        let sqrt_disc = disc.sqrt();
        let t1 = (-b + sqrt_disc) / (2.0 * a);
        let t2 = (-b - sqrt_disc) / (2.0 * a);
        // Wähle die positive, sinnvolle Lösung
        if t1 > 0.0 && t2 > 0.0 {
            t1.min(t2)
        } else if t1 > 0.0 {
            t1
        } else if t2 > 0.0 {
            t2
        } else {
            f64::INFINITY
        }
    }
    let time_shorter = analytic_time(shorter, current_rate, accel * shorter.signum());
    let time_longer = analytic_time(longer, current_rate, accel * longer.signum());
    if time_longer < time_shorter {
        longer
    } else {
        shorter
    }
}

fn time_to_reach_heading(delta: f64, current_rate: f64, accel: f64, max_rate: f64) -> f64 {
    // Annahme: konstante Beschleunigung bis max_rate, dann mit max_rate bis Ziel
    let rate_to_max = max_rate - current_rate;
    let t_accel = rate_to_max.abs() / accel.abs();
    let s_accel = current_rate * t_accel + 0.5 * accel * t_accel * t_accel;
    let s_remain = delta - s_accel;
    let t_remain = if max_rate != 0.0 {
        s_remain / max_rate
    } else {
        0.0
    };
    t_accel.abs() + t_remain.abs()
}

#[cfg(test)]
mod tests {
    use crate::game::{
        Heading, MoveSmoothReturn, TurnDirection, required_heading_change, time_to_reach_heading,
    };

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
            delta_val_u_per_second: *delta_val,
        };
        let MoveSmoothReturn {
            delta_val_u_per_second,
            finished_moving,
        } = move_smooth(params);
        assert!(!finished_moving);
        dbg!(delta_val_u_per_second);
        assert!(delta_val_u_per_second < 2.);
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
            delta_val_u_per_second: *delta_val,
        };
        let MoveSmoothReturn {
            delta_val_u_per_second,
            finished_moving,
        } = move_smooth(params);
        assert!(!finished_moving);
        dbg!(delta_val_u_per_second);
        assert!(delta_val_u_per_second < 2. && delta_val_u_per_second > -2.);
    }

    #[test]
    fn test_heading_change_clearance_left_right() {
        let current = Heading::from(10.0);
        let wanted = Heading::from(350.0);
        let rate = 0.0;
        let max_rate = 2.0;
        let accel = 0.5;
        // Clearance: Left
        let delta_left = required_heading_change(
            current,
            wanted,
            rate,
            Some(TurnDirection::Left),
            max_rate,
            accel,
        );
        assert!(
            delta_left < 0.0,
            "Mit Left-Clearance sollte negative Richtung gewählt werden"
        );
        // Clearance: Right
        let delta_right = required_heading_change(
            current,
            wanted,
            rate,
            Some(TurnDirection::Right),
            max_rate,
            accel,
        );
        assert!(
            delta_right > 0.0,
            "Mit Right-Clearance sollte positive Richtung gewählt werden"
        );
    }

    #[test]
    fn test_heading_change_near_180() {
        let current = Heading::from(0.0);
        let wanted = Heading::from(180.0);
        let rate = 0.0;
        let max_rate = 2.0;
        let accel = 0.5;
        let delta = required_heading_change(current, wanted, rate, None, max_rate, accel);
        assert!(delta.abs() == 180.0, "Delta bei 180° sollte exakt 180 sein");
    }

    #[test]
    fn test_heading_change_current_rate_longer_direction() {
        let current = Heading::from(0.0);
        let wanted = Heading::from(170.);
        let rate = -2.;
        let max_rate = 2.0;
        let accel = 0.5;
        let delta = required_heading_change(current, wanted, rate, None, max_rate, accel);
        assert!(
            delta < 0.0,
            "Soll längeren Weg (links) wählen, wenn Rate und Beschleunigung links sind"
        );
    }

    #[test]
    fn test_heading_change_current_rate_shorter_direction() {
        let current = Heading::from(0.0);
        let wanted = Heading::from(90.0);
        let rate = 2.0; // Dreht nach rechts, kürzeste Richtung
        let max_rate = 2.0;
        let accel = 0.5;
        let delta = required_heading_change(current, wanted, rate, None, max_rate, accel);
        // Sollte kürzesten Weg wählen
        assert!(
            delta > 0.0,
            "Soll kürzesten Weg (rechts) wählen, wenn Rate rechts ist"
        );
    }

    #[test]
    fn test_heading_change_near_zero() {
        let current = Heading::from(359.0);
        let wanted = Heading::from(1.0);
        let rate = 0.0;
        let max_rate = 2.0;
        let accel = 0.5;
        let delta = required_heading_change(current, wanted, rate, None, max_rate, accel);
        assert!(delta.abs() == 2.0, "Delta bei 359->1 sollte 2 sein");
    }

    #[test]
    fn test_time_to_reach_heading_accel_and_maxrate() {
        let delta = 90.0;
        let current_rate = 0.0;
        let accel = 2.0;
        let max_rate = 10.0;
        let t = time_to_reach_heading(delta, current_rate, accel, max_rate);
        assert!(t > 0.0, "Zeit sollte positiv sein");
        dbg!(t);
    }
}
