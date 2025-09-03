use aviation_helper_rs::clearance::turn_direction::TurnDirection;
use bevy::asset::Asset;
use bevy::dev_tools::states::log_transitions;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::window::PrimaryWindow;
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use rand_core::RngCore;
use serde::Deserialize;

use crate::APP_CONFIG;
use crate::game::loading::{PendingLoadingPlugins, PluginLoadingFinishedEvent};
use crate::game::{GameState, Z_AIRCRAFT};
use crate::util::consts::PIXEL_PER_KNOT_SECOND;
use aviation_helper_rs::heading::Heading;

const AIRCRAFT_PLUGIN: &str = "AircraftPlugin";

pub struct AircraftPlugin;

impl Plugin for AircraftPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            RonAssetPlugin::<AircraftTypeIndexFile>::new(&["ron"]),
            RonAssetPlugin::<AircraftType>::new(&["ron"]),
        ))
        .add_event::<AircraftJustSpawned>()
        .add_systems(Startup, on_startup)
        .add_systems(OnEnter(GameState::Loading), setup)
        .add_systems(
            Update,
            spawn_aircraft_at_mouse
                .run_if(in_state(GameState::Running).and(input_just_pressed(KeyCode::KeyS))),
        )
        .add_systems(OnEnter(GameState::Running), spawn_aircraft)
        .add_systems(
            FixedUpdate,
            update_aircrafts.run_if(in_state(GameState::Running)),
        )
        .add_systems(
            Update,
            poll_aircraft_types_loaded.run_if(in_state(LoadingState::LoadingHandles)),
        )
        .init_state::<LoadingState>();

        if APP_CONFIG.log_state_transitions {
            app.add_systems(Update, (log_transitions::<LoadingState>,));
        }
    }
}

fn on_startup(mut pending: ResMut<PendingLoadingPlugins>) {
    pending.register_plugin(AIRCRAFT_PLUGIN);
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let index_handle = asset_server.load::<AircraftTypeIndexFile>("aircraft_types/index.ron");
    commands.insert_resource(AircraftTypeDataLoadingState::PendingIndex(index_handle));
    commands.set_state(LoadingState::LoadingHandles);
}

fn poll_aircraft_types_loaded(
    mut commands: Commands,
    aircraft_loading: Option<ResMut<AircraftTypeDataLoadingState>>,
    type_index_assets: Res<Assets<AircraftTypeIndexFile>>,
    aircraft_type_assets: Res<Assets<AircraftType>>,
    asset_server: Res<AssetServer>,
    mut loading_state: ResMut<NextState<LoadingState>>,
    mut event_writer: EventWriter<PluginLoadingFinishedEvent>,
) {
    let Some(mut aircraft_loading) = aircraft_loading else {
        return;
    };
    match &*aircraft_loading {
        AircraftTypeDataLoadingState::PendingIndex(index_handle) => {
            if let Some(index) = type_index_assets.get(index_handle) {
                let type_handles: std::collections::HashMap<_, _> = index
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
                *aircraft_loading = AircraftTypeDataLoadingState::PendingAircraftTypes(
                    type_handles.values().cloned().collect(),
                );
                commands.insert_resource(AircraftTypeStore(type_handles));
            }
        }
        AircraftTypeDataLoadingState::PendingAircraftTypes(handles) => {
            let remaining: Vec<_> = handles
                .iter()
                .filter(|handle| aircraft_type_assets.get(*handle).is_none())
                .cloned()
                .collect();
            if !remaining.is_empty() {
                *aircraft_loading = AircraftTypeDataLoadingState::PendingAircraftTypes(remaining);
                return;
            }
            commands.remove_resource::<AircraftTypeDataLoadingState>();
            loading_state.set(LoadingState::Finished);
            event_writer.write(PluginLoadingFinishedEvent {
                plugin: AIRCRAFT_PLUGIN,
            });
        }
    }
}

pub fn spawn_aircraft(
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
                cleared_heading: Some(Heading::from(30.)),
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
            MeshMaterial2d(materials.add(Color::Srgba(Srgba {
                red: 0.,
                green: 0.4,
                blue: 0.3,
                alpha: 1.0,
            }))),
            Transform::from_xyz(0., 0., 8.0),
            children![(
                Mesh2d(meshes.add(Rectangle {
                    half_size: Vec2::new(20., 1.),
                })),
                MeshMaterial2d(materials.add(Color::Srgba(Srgba {
                    red: 0.,
                    green: 0.4,
                    blue: 0.3,
                    alpha: 1.0,
                }))),
                Transform::from_xyz(20., 0., 0.5),
            ),],
        ))
        .id();
    writer.write(AircraftJustSpawned(entity));
}

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

pub fn update_aircrafts(
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
        transform.translation.x += (aircraft.speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * x_part;
        transform.translation.y += (aircraft.speed_knots * delta_seconds * PIXEL_PER_KNOT_SECOND) as f32 * y_part;

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

pub fn move_smooth(params: MoveSmoothParams) -> MoveSmoothReturn {
    let MoveSmoothParams {
        delta_seconds,
        val_remaining_u,
        accuracy_u,
        max_delta_val_u_per_second,
        delta_val_acceleration_u_per_second2,
        delta_val_u_per_second,
    } = params;

    let finished_moving = val_remaining_u.abs() <= accuracy_u;

    if finished_moving {
        return MoveSmoothReturn {
            finished_moving: true,
            delta_val_u_per_second: 0.0,
        };
    }

    let target_speed = if val_remaining_u > 0.0 {
        max_delta_val_u_per_second.min(val_remaining_u / delta_seconds)
    } else {
        (-max_delta_val_u_per_second).max(val_remaining_u / delta_seconds)
    };

    let speed_diff = target_speed - delta_val_u_per_second;
    let max_speed_change = delta_val_acceleration_u_per_second2 * delta_seconds;

    let new_speed = if speed_diff.abs() <= max_speed_change {
        target_speed
    } else if speed_diff > 0.0 {
        delta_val_u_per_second + max_speed_change
    } else {
        delta_val_u_per_second - max_speed_change
    };

    MoveSmoothReturn {
        finished_moving: false,
        delta_val_u_per_second: new_speed,
    }
}

pub fn required_heading_change(
    current: Heading,
    target: Heading,
    direction: Option<TurnDirection>,
) -> f64 {
    let current_degrees = current.get();
    let target_degrees = target.get();

    let mut diff = target_degrees - current_degrees;

    // Normalize to [-180, 180]
    while diff > 180.0 {
        diff -= 360.0;
    }
    while diff < -180.0 {
        diff += 360.0;
    }

    match direction {
        Some(TurnDirection::Left) => {
            if diff > 0.0 {
                diff - 360.0
            } else {
                diff
            }
        }
        Some(TurnDirection::Right) => {
            if diff < 0.0 {
                diff + 360.0
            } else {
                diff
            }
        }
        None => diff,
    }
}

#[derive(Debug, Clone, Resource)]
pub enum AircraftTypeDataLoadingState {
    PendingIndex(Handle<AircraftTypeIndexFile>),
    PendingAircraftTypes(Vec<Handle<AircraftType>>),
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States, Default)]
pub enum LoadingState {
    #[default]
    Uninit,
    LoadingHandles,
    Finished,
}

#[derive(Clone, Debug, Component)]
pub struct Aircraft {
    pub aircraft_type_id: String,
    pub call_sign: String,
    pub cleared_altitude_feet: Option<f64>,
    pub wanted_altitude_feet: f64,
    pub cleared_heading: Option<Heading>,
    pub cleared_heading_change_direction: Option<TurnDirection>,
    pub cleared_speed_knots: Option<f64>,
    pub wanted_speed_knots: f64,
    pub altitude_feet: f64,
    pub altitude_change_feet_per_second: f64,
    pub heading: Heading,
    pub heading_change_degrees_per_second: f64,
    pub speed_knots: f64,
    pub acceleration_knots_per_second: f64,
}

#[derive(Debug, Clone, Deserialize, Asset, TypePath)]
pub struct AircraftTypeMeta {
    pub id: String,
    pub file: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Asset, TypePath)]
pub struct AircraftTypeIndexFile {
    pub types: Vec<AircraftTypeMeta>,
}

#[derive(Debug, Clone, Deserialize, Asset, TypePath)]
pub struct AircraftType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub characteristics: Vec<AircraftCharacteristic>,
    pub heading_accuracy_degrees: f64,
    pub max_delta_heading_degrees_per_second: f64,
    pub delta_heading_acceleration_degrees_per_second: f64,
    pub speed_accuracy_knots: f64,
    pub max_delta_speed_knots_per_second: f64,
    pub delta_speed_acceleration_knots_per_second: f64,
    pub altitude_accuracy_feet: f64,
    pub max_delta_altitude_feet_per_second: f64,
    pub delta_altitude_acceleration_feet_per_second: f64,
    pub optimal_cruising_altitude_feet: f64,
}

const AIRCRAFT_COLOR: Srgba = Srgba {
    red: 0.,
    green: 0.4,
    blue: 0.3,
    alpha: 1.0,
};

#[derive(Debug, Clone, Deserialize, Reflect)]
pub enum AircraftCharacteristic {
    Heavy,
}

#[derive(Resource)]
pub struct AircraftTypeStore(pub std::collections::HashMap<String, Handle<AircraftType>>);

#[derive(Clone, Debug, Event)]
pub struct AircraftJustSpawned(pub Entity);

#[derive(Debug)]
pub struct MoveSmoothParams {
    pub delta_seconds: f64,
    pub val_remaining_u: f64,
    pub accuracy_u: f64,
    pub max_delta_val_u_per_second: f64,
    pub delta_val_acceleration_u_per_second2: f64,
    pub delta_val_u_per_second: f64,
}

#[derive(Debug)]
pub struct MoveSmoothReturn {
    pub finished_moving: bool,
    pub delta_val_u_per_second: f64,
}

#[cfg(test)]
mod tests {
    use super::{MoveSmoothParams, MoveSmoothReturn, move_smooth};

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
