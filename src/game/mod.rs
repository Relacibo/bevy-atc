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
            altitude_change: 10.,
            heading: Heading::from(30.),
            heading_change: 2.0,
            speed_knots: 300.,
            acceleration_knots_per_second: 0.,
        },
        Mesh2d(meshes.add(Rectangle {
            half_size: Vec2::new(5., 5.),
        })),
        MeshMaterial2d(materials.add(Color::Srgba(AIRCRAFT_COLOR))),
        Transform::from_xyz(0., 0., 10.),
    ));
}

fn update_aircraft(
    mut commands: Commands,
    mut query: Query<(&Aircraft, &mut AircraftPhysics, &mut Transform)>,
) {
    for (aircraft, mut physics, mut transform) in query {
        let Aircraft {
            cleared_altitude_feet,
            cleared_heading,
            cleared_speed_knots,
            wanted_speed_knots,
            ..
        } = aircraft;

        dbg!("------------");

        // heading
        // if let Some(cleared_heading) = cleared_heading {
        //     let required_change = physics.heading.required_change(*cleared_heading);
        //     let required_change_abs = required_change.abs();
        //     if required_change_abs >= 0.05 {
        //         if required_change_abs < 30.0 {
        //             physics.heading_change -= 4. * required_change / 5.;
        //         } else if physics.heading_change < 1.0 {
        //             physics.heading_change += 0.005;
        //         }
        //     } else {
        //         physics.heading_change = 0.0;
        //         physics.heading = *cleared_heading;
        //         transform.rotation = Quat::from_axis_angle(
        //             Vec3 {
        //                 z: -1.,
        //                 ..default()
        //             },
        //             cleared_heading.to_rotation() as f32,
        //         );
        //     }
        // }
        // if physics.heading_change != 0. {
        //     physics.heading = physics.heading.change(physics.heading_change);
        // }

        // transform.rotation = Quat::from_axis_angle(
        //     Vec3 {
        //         z: -1.,
        //         ..default()
        //     },
        //     physics.heading.to_rotation() as f32,
        // );

        // speed
        let wanted_speed = cleared_speed_knots.unwrap_or(*wanted_speed_knots);
        let required_change = physics.speed_knots - wanted_speed;
        dbg!(physics.speed_knots);
        dbg!(wanted_speed);
        dbg!(required_change);
        let required_change_abs = required_change.abs();
        if required_change_abs >= 0.05 {
            if required_change_abs < 30.0 {
                physics.acceleration_knots_per_second -= 4. * required_change / 5.;
            } else if physics.acceleration_knots_per_second < 1.0 {
                physics.acceleration_knots_per_second += 0.000005;
            }
        } else {
            physics.acceleration_knots_per_second = 0.0;
            physics.speed_knots = wanted_speed;
        }
        dbg!(physics.acceleration_knots_per_second);

        physics.speed_knots += physics.acceleration_knots_per_second;

        let (Vec3 { z, .. }, angle) = transform.rotation.to_axis_angle();
        let angle = z * angle;
        dbg!(angle);
        let debug_v @ Vec2 {
            x: x_part,
            y: y_part,
        } = Vec2::from_angle(angle);
        dbg!(debug_v);
        transform.translation.x += x_part
            * physics.speed_knots
            * PIXEL_PER_KNOT_SECOND as f32
            * FIXED_UPDATE_LENGTH_SECOND;
        transform.translation.y += y_part
            * physics.speed_knots
            * PIXEL_PER_KNOT_SECOND as f32
            * FIXED_UPDATE_LENGTH_SECOND;
        dbg!(transform);
    }
}

#[derive(Clone, Debug, Resource, Reflect)]
struct GameVariables {}

impl Default for GameVariables {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Resource)]
pub struct GameResources {}

const AIRCRAFT_COLOR: Srgba = Srgba {
    red: 0.,
    green: 0.4,
    blue: 0.3,
    alpha: 0.3,
};
