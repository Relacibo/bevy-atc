// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use std::{f32, ops::Add, time::Duration};

use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use rand_core::RngCore;

use crate::AppState;

const VELOCITY: f32 = 500.0;
const PIPE_SPAWN_INTERVAL_MILLIS_MIN: u64 = 2000;
const PIPE_SPAWN_INTERVAL_MILLIS_MAX: u64 = 2500;
const VERTICAL_SPACE_BETWEEN_PIPES_MIN: f32 = 150.0;
const VERTICAL_SPACE_BETWEEN_PIPES_MAX: f32 = 300.0;

pub struct GamePlugin;

#[derive(Component)]
struct RngSource;

#[derive(Resource)]
struct NextPipeSpawn(Duration);

impl NextPipeSpawn {
    fn random(mut rng: GlobalEntropy<WyRand>, time: Time) -> Self {
        let rand = rng.next_u64();
        let now = time.elapsed();
        let add_millis = ((rand as f32) / (u64::MAX as f32)
            * (PIPE_SPAWN_INTERVAL_MILLIS_MAX - PIPE_SPAWN_INTERVAL_MILLIS_MIN) as f32)
            as u64
            + PIPE_SPAWN_INTERVAL_MILLIS_MIN;
        Self(now.add(Duration::from_millis(add_millis)))
    }
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        let elapsed = app
            .world()
            .get_resource::<Time>()
            .expect("Time resource not found!")
            .elapsed();
        app.add_systems(OnEnter(AppState::Game), setup)
            .insert_resource(NextPipeSpawn(elapsed))
            .add_systems(
                Update,
                (
                    spawn_pipes
                        .run_if(should_spawn_pipes)
                        .run_if(in_state(AppState::Game)),
                    transform_pipes.run_if(in_state(AppState::Game)),
                ),
            );
    }
}

fn should_spawn_pipes(time: Res<Time>, next_spawn: Res<NextPipeSpawn>) -> bool {
    time.elapsed() >= next_spawn.0
}

fn spawn_pipes(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut rng: GlobalEntropy<WyRand>,
    mut next_pipe_spawn: ResMut<NextPipeSpawn>,
    time: Res<Time>,
) {
    // let texture = asset_server.load("textures/rpg/chars/gabe/gabe-idle-run.png");
    let texture = asset_server.load("textures/floppy/pipe.png");
    let height_rng = rng.next_u32();
    let space_rng = rng.next_u32();

    let space = (space_rng as f32) / (u32::MAX as f32)
        * (VERTICAL_SPACE_BETWEEN_PIPES_MAX - VERTICAL_SPACE_BETWEEN_PIPES_MIN)
        + VERTICAL_SPACE_BETWEEN_PIPES_MIN;

    let height = (height_rng as f32 / (u32::MAX as f32)) * 300.0 - 500.0;

    let x = 1000.0;
    let spawns = [(height, false), (space + height + 500.0, true)];
    for (y, rotate) in spawns {
        debug!("Spawning Pipe at ({x}, {y})");
        let mut transform =
            Transform::from_scale(Vec3::splat(0.8)).with_translation(Vec3::new(x, y, 0.0));

        if rotate {
            transform.rotate_local_z(f32::consts::PI);
        }
        commands.spawn((
            Sprite {
                image: texture.clone(),
                ..Default::default()
            },
            transform,
            Pipe,
        ));
    }
    *next_pipe_spawn = NextPipeSpawn::random(rng, *time)
}

fn transform_pipes(
    mut commands: Commands,
    time: Res<Time>,
    query: Query<(Entity, &mut Transform), With<Pipe>>,
) {
    let delta = VELOCITY * (time.delta().as_millis() as f32 / 1000.0);
    for (entity, mut transform) in query {
        transform.translation.x -= delta;
        if transform.translation.x < -1000.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Component)]
struct AnimationConfig {
    first_sprite_index: usize,
    last_sprite_index: usize,
    fps: u8,
    frame_timer: Timer,
}

#[derive(Component)]
struct Pipe;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut rng: GlobalEntropy<WyRand>,
) {
    commands.insert_resource(NextPipeSpawn::random(rng, *time));
}
