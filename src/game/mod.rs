// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use core::f64;
use std::{
    fs,
    ops::{Add, Sub},
    path::Path,
    time::Duration,
};

use crate::game::aircraft::{AircraftType, AircraftTypeIndexFile};
use aircraft::Aircraft;
use aircraft_card::AircraftCardPlugin;
use anyhow::anyhow;
use bevy::platform::collections::hash_map::HashMap;
use bevy::window::PrimaryWindow;
use bevy::{
    dev_tools::states::log_transitions,
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
            RonAssetPlugin::<AircraftTypeIndexFile>::new(&["ron"]),
            RonAssetPlugin::<AircraftType>::new(&["ron"]),
        ))
        .register_type::<GameVariables>()
        .add_event::<AircraftJustSpawned>()
        .add_systems(OnEnter(AppState::Game), enter_loading_state)
        .add_systems(OnEnter(GameState::Loading), load_level_asset)
        .add_systems(
            Update,
            poll_handles_loaded.run_if(in_state(GameState::Loading)),
        )
        .add_systems(OnEnter(LoadingState::SpawningLevel), spawn_level)
        .add_systems(OnEnter(LoadingState::Finished), finish_loading)
        .add_systems(OnEnter(GameState::Running), (setup, spawn_aircraft))
        .add_systems(
            FixedUpdate,
            update_aircrafts.run_if(in_state(GameState::Running)),
        )
        .add_systems(
            Update,
            spawn_aircraft_at_mouse
                .run_if(in_state(GameState::Running).and(input_just_pressed(KeyCode::KeyS))),
        )
        .insert_state(GameState::BeforeGame)
        .insert_state(LoadingState::LoadingHandles);

        if APP_CONFIG.dev_gui {
            app.add_systems(OnEnter(AppState::Game), setup_dev_gui)
                .add_systems(
                    Update,
                    (handle_dev_gui_events).run_if(in_state(AppState::Game)),
                );
        }

        if APP_CONFIG.log_state_transitions {
            app.add_systems(
                Update,
                (
                    log_transitions::<GameState>,
                    log_transitions::<LoadingState>,
                ),
            );
        }
    }
}

fn enter_loading_state(mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::Loading);
}

#[derive(Resource, Debug, Clone)]
pub struct LevelHandle(pub Handle<LevelFile>);

fn load_level_asset(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    game_vars: Res<GameVariables>,
) {
    let level_handle = asset_server.load::<LevelFile>(format!("levels/{}", &game_vars.level.file));
    let type_index_handle =
        asset_server.load::<AircraftTypeIndexFile>("aircraft_types/index.ron");
    commands.insert_resource(LevelHandle(level_handle.clone()));
    commands.insert_resource(HandleLoadingState {
        level: LevelLoadingState::PendingLevel(level_handle),
        aircraft_types: AircraftTypesLoadingState::PendingIndex(type_index_handle),
    });
    commands.insert_resource(LoadingState::LoadingHandles);
}

fn poll_handles_loaded(
    mut commands: Commands,
    handle_loading: Option<ResMut<HandleLoadingState>>,
    mut next_loading_state: ResMut<NextState<LoadingState>>,
    level_assets: Res<Assets<LevelFile>>,
    type_index_assets: Res<Assets<AircraftTypeIndexFile>>,
    aircraft_type_assets: Res<Assets<AircraftType>>,
    asset_server: Res<AssetServer>,
) {
    let Some(mut handle_loading) = handle_loading else {
        return;
    };
    // Level
    match &handle_loading.level {
        LevelLoadingState::PendingLevel(handle) => {
            if level_assets.get(handle).is_some() {
                handle_loading.level = LevelLoadingState::Finished;
            } else {
                return;
            }
        }
        LevelLoadingState::Finished => {}
        _ => {}
    }
    // Aircraft Types
    match &handle_loading.aircraft_types {
        AircraftTypesLoadingState::PendingIndex(index_handle) => {
            if let Some(index) = type_index_assets.get(index_handle) {
                let type_handles: HashMap<_, _> = index
                    .types
                    .iter()
                    .map(|meta| {
                        (
                            meta.id.clone(),
                            asset_server
                                .load::<AircraftType>(format!("aircraft_types/{}", meta.file)),
                        )
                    })
                    .collect();
                handle_loading.aircraft_types = AircraftTypesLoadingState::PendingAircraftTypes(
                    type_handles.values().cloned().collect(),
                );
                commands.insert_resource(AircraftTypeStore(type_handles));
            } else {
                return;
            }
        }
        AircraftTypesLoadingState::PendingAircraftTypes(handles) => {
            let remaining: Vec<_> = handles
                .iter()
                .filter(|handle| aircraft_type_assets.get(*handle).is_none())
                .cloned()
                .collect();
            if remaining.is_empty() {
                handle_loading.aircraft_types = AircraftTypesLoadingState::Finished;
            } else {
                handle_loading.aircraft_types =
                    AircraftTypesLoadingState::PendingAircraftTypes(remaining);
                return;
            }
        }
        AircraftTypesLoadingState::Finished => {}
    }
    // Wenn alles fertig ist, Stage wechseln
    if matches!(handle_loading.level, LevelLoadingState::Finished)
        && matches!(
            handle_loading.aircraft_types,
            AircraftTypesLoadingState::Finished
        )
    {
        commands.remove_resource::<HandleLoadingState>();
        next_loading_state.set(LoadingState::SpawningLevel);
    }
}

fn spawn_level(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    level_assets: Res<Assets<LevelFile>>,
    level_handle: Res<LevelHandle>,
    mut next_loading_state: ResMut<NextState<LoadingState>>,
) {
    let Some(level) = level_assets.get(&level_handle.0) else {
        unreachable!("Level asset not found!");
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
    commands.remove_resource::<LevelHandle>();
    next_loading_state.set(LoadingState::Finished);
}

fn finish_loading(mut next_game_state: ResMut<NextState<GameState>>) {
    next_game_state.set(GameState::Running);
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
                aircraft_type_id: "a320".to_owned(),
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
    aircraft_types: Res<AircraftTypeStore>,
    aircraft_type_assets: Res<Assets<AircraftType>>,
) {
    let delta_seconds = time.delta_secs_f64();
    for (mut aircraft, mut transform) in query {
        let Some(handle) = aircraft_types.0.get(&aircraft.aircraft_type_id) else {
            continue;
        };
        let Some(aircraft_type) = aircraft_type_assets.get(handle) else {
            continue;
        };
        // Parameter aus AircraftType
        let heading_accuracy_degrees = aircraft_type.heading_accuracy_degrees;
        let max_delta_heading_degrees_per_second =
            aircraft_type.max_delta_heading_degrees_per_second;
        let delta_heading_acceleration_degrees_per_second =
            aircraft_type.delta_heading_acceleration_degrees_per_second;
        let speed_accuracy_knots = aircraft_type.speed_accuracy_knots;
        let max_delta_speed_knots_per_second = aircraft_type.max_delta_speed_knots_per_second;
        let delta_speed_acceleration_knots_per_second =
            aircraft_type.delta_speed_acceleration_knots_per_second;
        let altitude_accuracy_feet = aircraft_type.altitude_accuracy_feet;
        let max_delta_altitude_feet_per_second = aircraft_type.max_delta_altitude_feet_per_second;
        let delta_altitude_acceleration_feet_per_second =
            aircraft_type.delta_altitude_acceleration_feet_per_second;

        // heading
        let wanted = aircraft.cleared_heading.unwrap_or(aircraft.heading);
        let required_change_u = required_heading_change(
            aircraft.heading,
            wanted,
            aircraft.cleared_heading_change_direction,
        );
        if aircraft.heading_change_degrees_per_second != 0. || required_change_u != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: heading_accuracy_degrees,
                max_delta_val_u_per_second: max_delta_heading_degrees_per_second,
                delta_val_acceleration_u_per_second2: delta_heading_acceleration_degrees_per_second,
                delta_val_u_per_second: aircraft.heading_change_degrees_per_second,
            };
            let MoveSmoothReturn {
                finished_moving,
                delta_val_u_per_second,
            } = move_smooth(params);
            aircraft.heading_change_degrees_per_second = delta_val_u_per_second;
            if finished_moving {
                aircraft.heading = wanted;
                transform.rotation = Quat::from_axis_angle(
                    Vec3 {
                        z: -1.,
                        ..default()
                    },
                    wanted.to_rotation() as f32,
                );
            }
        }
        if aircraft.heading_change_degrees_per_second != 0. {
            aircraft.heading = aircraft
                .heading
                .change(delta_seconds * aircraft.heading_change_degrees_per_second);
            transform.rotation = Quat::from_axis_angle(
                Vec3 {
                    z: -1.,
                    ..default()
                },
                aircraft.heading.to_rotation() as f32,
            );
        }

        // speed
        let wanted = aircraft
            .cleared_speed_knots
            .unwrap_or(aircraft.wanted_speed_knots);
        let required_change_u = -aircraft.speed_knots + wanted;
        if required_change_u != 0. || aircraft.acceleration_knots_per_second != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: speed_accuracy_knots,
                max_delta_val_u_per_second: max_delta_speed_knots_per_second,
                delta_val_acceleration_u_per_second2: delta_speed_acceleration_knots_per_second,
                delta_val_u_per_second: aircraft.acceleration_knots_per_second,
            };
            let MoveSmoothReturn {
                finished_moving,
                delta_val_u_per_second,
            } = move_smooth(params);
            aircraft.acceleration_knots_per_second = delta_val_u_per_second;
            if finished_moving {
                aircraft.speed_knots = wanted;
            }
        }
        aircraft.speed_knots += aircraft.acceleration_knots_per_second * delta_seconds;
        let (Vec3 { z, .. }, angle) = transform.rotation.to_axis_angle();
        let angle = z * angle;
        let Vec2 {
            x: x_part,
            y: y_part,
        } = Vec2::from_angle(angle);
        transform.translation.x +=
            (aircraft.speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * x_part;
        transform.translation.y +=
            (aircraft.speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * y_part;

        // altitude
        let wanted = aircraft
            .cleared_altitude_feet
            .unwrap_or(aircraft.wanted_altitude_feet);
        let required_change_u = -aircraft.altitude_feet + wanted;
        if aircraft.altitude_change_feet_per_second != 0. || required_change_u != 0. {
            let params = MoveSmoothParams {
                delta_seconds,
                val_remaining_u: required_change_u,
                accuracy_u: altitude_accuracy_feet,
                max_delta_val_u_per_second: max_delta_altitude_feet_per_second,
                delta_val_acceleration_u_per_second2: delta_altitude_acceleration_feet_per_second,
                delta_val_u_per_second: aircraft.altitude_change_feet_per_second,
            };
            let MoveSmoothReturn {
                finished_moving,
                delta_val_u_per_second,
            } = move_smooth(params);
            aircraft.altitude_change_feet_per_second = delta_val_u_per_second;
            if finished_moving {
                aircraft.altitude_feet = wanted;
            }
        }
        aircraft.altitude_feet += aircraft.altitude_change_feet_per_second * delta_seconds;
    }
}

fn required_heading_change(
    current: Heading,
    wanted: Heading,
    cleared_direction: Option<TurnDirection>,
) -> f64 {
    let delta = current.required_change(wanted);
    if let Some(dir) = cleared_direction {
        match dir {
            TurnDirection::Left if delta > 0.0 => delta - 360.,
            TurnDirection::Right if delta < 0.0 => delta + 360.,
            _ => delta,
        }
    } else {
        delta
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
    delta_val_acceleration_x_per_seconds2: f64,
    max_delta_val_x_per_second: f64,
    delta_val_x_per_second: f64,
) -> f64 {
    let mut delta_val_x_per_second_new = delta_val_x_per_second
        + direction * (delta_seconds * delta_val_acceleration_x_per_seconds2);
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
}

impl DevGuiStructTrait for GameVariables {}

impl GameVariables {
    pub fn new(level: LevelMeta) -> Self {
        Self { level }
    }
}

#[derive(Resource)]
pub struct GameResources {}

#[derive(Resource)]
pub struct AircraftTypeStore(pub HashMap<String, Handle<AircraftType>>);

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

    let aircraft_types = ["a320", "b737", "b747", "cessna172"];
    let idx = (rng.next_u32() as usize) % aircraft_types.len();
    let aircraft_type = aircraft_types[idx].to_string();

    let heading = (rng.next_u32() % 360) as f64;
    let altitude_feet = 1000.0 + (rng.next_u32() % 39000) as f64;
    let entity = commands
        .spawn((
            Aircraft {
                aircraft_type_id: aircraft_type.to_owned(),
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

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum TurnDirection {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub enum AircraftTypesLoadingState {
    PendingIndex(Handle<AircraftTypeIndexFile>),
    PendingAircraftTypes(Vec<Handle<AircraftType>>),
    Finished,
}

#[derive(Debug, Clone)]
pub enum LevelLoadingState {
    PendingLevel(Handle<LevelFile>),
    Finished,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States, Resource)]
pub enum LoadingState {
    LoadingHandles,
    SpawningLevel,
    Finished,
}

#[derive(Resource, Debug, Clone)]
pub struct HandleLoadingState {
    pub level: LevelLoadingState,
    pub aircraft_types: AircraftTypesLoadingState,
}

#[cfg(test)]
mod tests {
    use crate::game::MoveSmoothReturn;

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
}
