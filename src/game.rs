// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use std::{
    any::{Any, TypeId},
    f32::{self, consts::PI},
    ops::Add,
    time::Duration,
};

use bevy::{
    input::common_conditions::{input_just_pressed, input_just_released},
    prelude::*,
};
use bevy_prng::WyRand;
use bevy_rand::global::GlobalEntropy;
use bevy_rapier2d::{
    plugin::{NoUserData, RapierPhysicsPlugin},
    prelude::*,
    render::RapierDebugRenderPlugin,
};
use rand_core::RngCore;

use crate::{
    APP_CONFIG, AppState,
    dev_gui::DevGuiEvent,
    util::{entities::despawn_all, reflect::try_apply_parsed},
};

const PIPE_SPAWN_DESPAWN_X: f32 = 1000.0;

const PIXELS_PER_METER: f32 = 100.0;

const PIPE_UP_TEXTURE_PATH: &str = "textures/pipe_up.png";
const PIPE_DOWN_TEXTURE_PATH: &str = "textures/pipe_down.png";
const FLOPPY_TEXTURE_PATH: &str = "textures/floppy.png";

const BACKGROUND_COLOR: Srgba = Srgba {
    red: 0.,
    green: 0.4,
    blue: 0.3,
    alpha: 0.3,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States)]
pub enum GameState {
    BeforeGame,
    Running,
    FloppyDead,
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(
            PIXELS_PER_METER,
        ),))
            .register_type::<GameVariables>()
            .add_event::<GameVariablesEvent>()
            .insert_resource(GameVariables::default())
            .add_systems(
                OnEnter(AppState::Game),
                (setup, start_game, setup_score_gui),
            )
            .add_systems(
                OnEnter(GameState::Running),
                (
                    despawn_all::<DeathScreenGui>,
                    despawn_all::<Pipe>,
                    despawn_all::<BetweenPipes>,
                    reset_score,
                    spawn_floppy,
                    reset_camera.after(spawn_floppy),
                ),
            )
            .add_systems(
                OnEnter(GameState::FloppyDead),
                (despawn_all::<Floppy>, show_death_screen),
            )
            .add_systems(
                Update,
                (
                    (
                        spawn_pipes.run_if(should_spawn_pipes),
                        update_pipes,
                        update_floppy,
                        update_camera_transform,
                        handle_jump.run_if(input_just_pressed(KeyCode::ArrowUp)),
                        handle_move.run_if(horizontal_arrow_key_changed),
                        despawn_all::<Pipe>.run_if(input_just_pressed(KeyCode::KeyD)),
                        (toggle_cam_mode, reset_camera.after(toggle_cam_mode))
                            .run_if(input_just_pressed(KeyCode::KeyC)),
                        kill_floppy
                            .run_if(is_floppy_out_of_bounds.or(input_just_pressed(KeyCode::KeyR))),
                        handle_collision_events,
                        handle_game_variables_event,
                        handle_score_change.run_if(state_changed::<Score>),
                    )
                        .run_if(in_state(GameState::Running)),
                    (start_game.run_if(input_just_pressed(KeyCode::KeyR)))
                        .run_if(in_state(GameState::FloppyDead)),
                )
                    .run_if(in_state(AppState::Game)),
            )
            .insert_state(GameState::BeforeGame)
            .insert_state(Score::default());

        if APP_CONFIG.dev_gui {
            app.add_systems(OnEnter(AppState::Game), setup_debug_gui)
                .add_systems(
                    Update,
                    (handle_debug_gui_events).run_if(in_state(AppState::Game)),
                );
        }

        if APP_CONFIG.rapier_debug_render {
            app.add_plugins(RapierDebugRenderPlugin::default());
        }
    }
}

fn handle_score_change(query: Query<&mut Text, With<ScoreGui>>, score: Res<State<Score>>) {
    for mut text in query {
        text.0 = score.0.to_string();
    }
}

fn reset_score(mut score: ResMut<NextState<Score>>) {
    score.set(Score::default())
}

fn toggle_cam_mode(q_camera: Query<&mut FollowStateComponent, With<Camera2d>>) {
    for mut follow in q_camera {
        *follow = match *follow {
            FollowStateComponent::Centered => FollowStateComponent::WantsToFollowFloppy,
            _ => FollowStateComponent::Centered,
        }
    }
}

fn start_game(mut game_state: ResMut<NextState<GameState>>) {
    debug!("Game started");
    game_state.set(GameState::Running)
}

fn kill_floppy(mut game_state: ResMut<NextState<GameState>>) {
    debug!("Floppy died");
    game_state.set(GameState::FloppyDead)
}

fn is_floppy_out_of_bounds(
    variables: Res<GameVariables>,
    q_floppy: Query<&Transform, With<Floppy>>,
) -> bool {
    let GameVariables {
        floppy_alive_zone_x,
        floppy_alive_zone_y,
        ..
    } = *variables;
    for transform in q_floppy {
        let Vec3 { x, y, .. } = transform.translation;
        if x < -floppy_alive_zone_x
            || x > floppy_alive_zone_x
            || y < -floppy_alive_zone_y
            || y > floppy_alive_zone_y
        {
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn handle_collision_events(
    mut commands: Commands,
    q_floppy: Query<(), With<Floppy>>,
    q_pipe: Query<(), With<Pipe>>,
    q_between_pipes: Query<Entity, With<BetweenPipes>>,
    mut collision_events: EventReader<CollisionEvent>,
    mut game_state: ResMut<NextState<GameState>>,
    score: Res<State<Score>>,
    mut next_score: ResMut<NextState<Score>>,
) {
    for collision_event in collision_events.read() {
        match collision_event {
            CollisionEvent::Started(entity, entity1, ..) => {
                let other = get_other_helper(q_floppy, entity, entity1);
                if q_pipe.contains(*other) {
                    game_state.set(GameState::FloppyDead);
                };
            }
            CollisionEvent::Stopped(entity, entity1, ..) => {
                let other = get_other_helper(q_floppy, entity, entity1);
                if q_between_pipes.contains(*other) {
                    next_score.set(Score(score.0 + 1));
                    commands.entity(*other).despawn()
                }
            }
        }
        if let CollisionEvent::Started(entity, entity1, ..) = collision_event {
            let floppy_and_pipe_collided = q_floppy.get(*entity).is_ok()
                && q_pipe.get(*entity1).is_ok()
                || q_floppy.get(*entity1).is_ok() && q_pipe.get(*entity).is_ok();
            if floppy_and_pipe_collided {
                game_state.set(GameState::FloppyDead);
            }
        }
    }
}

fn get_other_helper<'a>(
    q_floppy: Query<(), With<Floppy>>,
    entity: &'a Entity,
    entity1: &'a Entity,
) -> &'a Entity {
    if q_floppy.contains(*entity) {
        entity1
    } else {
        entity
    }
}

#[derive(Debug, Clone, Event)]
enum GameVariablesEvent {
    Initialized {
        new: GameVariables,
    },
    Changed {
        old: GameVariables,
        new: GameVariables,
    },
}

#[derive(Clone, Debug, Component)]
struct ScoreGuiRoot;

#[derive(Clone, Debug, Component)]
struct ScoreGui;

#[derive(Clone, Default, Hash, PartialEq, Eq, Debug, States)]
struct Score(u32);

fn setup_score_gui(mut commands: Commands) {
    commands.spawn((
        ScoreGuiRoot,
        Node {
            height: Val::Px(100.),
            width: Val::Px(300.),
            align_self: AlignSelf::Start,
            justify_self: JustifySelf::End,
            flex_direction: FlexDirection::Column,
            align_content: AlignContent::End,
            ..default()
        },
        Transform::from_xyz(0., 0., -3.),
        children![(
            ScoreGui,
            Node {
                height: Val::Percent(100.),
                width: Val::Percent(100.),
                margin: UiRect {
                    left: Val::Auto,
                    ..default()
                },
                justify_self: JustifySelf::End,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Srgba::new(0.2, 0., 0.2, 0.4).into()),
            Text("0".to_owned()),
            TextFont {
                font_size: 100.0,
                ..default()
            },
            BorderColor(Color::BLACK),
            Outline {
                width: Val::Px(6.),
                offset: Val::Px(6.),
                color: Color::WHITE,
            },
        ),],
    ));
}

fn handle_game_variables_event(
    mut events: EventReader<GameVariablesEvent>,
    mut q_camera_follow_state: Query<&mut FollowStateComponent, With<Camera>>,
) {
    for event in events.read() {
        let (old, new) = match event {
            GameVariablesEvent::Initialized { new } => (None, new),
            GameVariablesEvent::Changed { old, new } => (Some(old), new),
        };

        let old_is_none = old.is_none();
        if old_is_none || old.unwrap().camera_follow_floppy != new.camera_follow_floppy {
            for mut camera in &mut q_camera_follow_state {
                *camera = if new.camera_follow_floppy {
                    FollowStateComponent::WantsToFollowFloppy
                } else {
                    FollowStateComponent::Centered
                };
            }
        }
    }
}

fn horizontal_arrow_key_changed(key_codes: Res<ButtonInput<KeyCode>>) -> bool {
    let keys = [KeyCode::ArrowLeft, KeyCode::ArrowRight];
    key_codes.get_just_pressed().any(|k| keys.contains(k))
        || key_codes.get_just_released().any(|k| keys.contains(k))
}

fn handle_move(
    key_codes: Res<ButtonInput<KeyCode>>,
    ext_forces: Query<&mut ExternalForce, With<Floppy>>,
    variables: Res<GameVariables>,
) {
    let GameVariables {
        floppy_horizontal_force,
        ..
    } = *variables;
    let mut force = 0.;
    for key_code in key_codes.get_pressed() {
        match key_code {
            KeyCode::ArrowLeft => force -= floppy_horizontal_force,
            KeyCode::ArrowRight => force += floppy_horizontal_force,
            _ => {}
        }
    }
    for mut ext_forces in ext_forces {
        ext_forces.force = Vec2::new(force, 0.);
    }
}

#[derive(Clone, Debug, Component)]
struct DeathScreenGui;

fn show_death_screen(mut commands: Commands) {
    commands.spawn((
        DeathScreenGui,
        Node {
            align_self: AlignSelf::Center,
            justify_self: JustifySelf::Center,
            ..default()
        },
        Text("Floppy died!".to_owned()),
    ));
}

fn update_floppy(
    q_floppy: Query<(&mut ExternalForce, &Transform, &Velocity), With<Floppy>>,
    variables: Res<GameVariables>,
) {
    let GameVariables {
        floppy_torque_spring_strength,
        floppy_rotation_friction,
        ..
    } = *variables;
    for (mut force, transform, velocity) in q_floppy {
        let Velocity { angvel, .. } = velocity;
        let quat = transform.rotation;
        let (Vec3 { z, .. }, angle) = quat.to_axis_angle();
        force.torque =
            -angvel * floppy_rotation_friction + -z * angle * floppy_torque_spring_strength;
    }
}

fn reset_camera(
    q_floppy: Query<Entity, With<Floppy>>,
    mut q_camera: Query<(&mut Transform, &mut FollowStateComponent), With<Camera2d>>,
) {
    let Some(floppy) = q_floppy.iter().next() else {
        error!("Wanted to follow floppy, but didn't find him");
        return;
    };
    for (mut transform, mut follow) in &mut q_camera {
        match *follow {
            FollowStateComponent::Centered => {
                *transform = Transform::default();
            }
            _ => *follow = FollowStateComponent::Follow(floppy),
        }
    }
}

fn update_camera_transform(
    q_entity: Query<&Transform, Without<FollowStateComponent>>,
    q_camera: Query<(&mut Transform, &FollowStateComponent), With<Camera2d>>,
) {
    for (mut cam_transform, follow) in q_camera {
        let FollowStateComponent::Follow(entity) = follow else {
            continue;
        };

        let Ok(e) = q_entity.get(*entity) else {
            continue;
        };
        *cam_transform = *e;
    }
}

fn handle_jump(
    ext_impulses: Query<&mut ExternalImpulse, With<Floppy>>,
    variables: Res<GameVariables>,
) {
    let GameVariables {
        floppy_jump_torque_impulse,
        floppy_jump_vertical_impulse,
        ..
    } = *variables;
    for mut ext_impulse in ext_impulses {
        *ext_impulse = ExternalImpulse {
            impulse: Vec2::new(0., floppy_jump_vertical_impulse),
            torque_impulse: floppy_jump_torque_impulse,
        };
    }
}

fn spawn_floppy(
    mut commands: Commands,
    game_resources: Res<GameResources>,
    variables: Res<GameVariables>,
) {
    let GameVariables {
        floppy_spawn_x,
        floppy_spawn_y,
        floppy_radius,
        floppy_mass,
        ..
    } = *variables;
    let GameResources { floppy, .. } = &*game_resources;
    debug!("Game started");
    debug!("Spawning Floppy at ({floppy_spawn_x}, {floppy_spawn_y})");
    debug!("Entity: ");

    /* Create the bouncing ball. */
    commands.spawn((
        Floppy,
        Sprite {
            image: floppy.clone(),
            custom_size: Some(Vec2::new(floppy_radius * 2., floppy_radius * 2.)),
            image_mode: SpriteImageMode::Scale(ScalingMode::FillCenter),
            ..Default::default()
        },
        RigidBody::Dynamic,
        Collider::ball(floppy_radius),
        Sensor,
        ColliderMassProperties::Mass(floppy_mass),
        // AdditionalMassProperties::Mass(floppy_mass),
        Transform::from_xyz(floppy_spawn_x, floppy_spawn_y, 0.0),
        Velocity::default(),
        ExternalForce::default(),
        ExternalImpulse::default(),
        ActiveCollisionTypes::DYNAMIC_KINEMATIC,
        ActiveEvents::COLLISION_EVENTS,
    ));
}

fn should_spawn_pipes(time: Res<Time>, next_spawn: Option<Res<PipeSpawnTimer>>) -> bool {
    let Some(PipeSpawnTimer(next_spawn)) = next_spawn.as_deref() else {
        return false;
    };
    time.elapsed() >= *next_spawn
}

#[derive(Clone, Debug, Component)]
struct PipeMovement;

#[derive(Clone, Debug, Component)]
struct BetweenPipes;

fn spawn_pipes(
    mut commands: Commands,
    mut rng: GlobalEntropy<WyRand>,
    mut next_pipe_spawn: ResMut<PipeSpawnTimer>,
    time: Res<Time>,
    game_resources: Res<GameResources>,
    variables: Res<GameVariables>,
) {
    let GameResources {
        pipe_up_texture,
        pipe_down_texture,
        ..
    } = &*game_resources;

    let GameVariables {
        vertical_space_between_pipes_min_px,
        vertical_space_between_pipes_max_px,
        pipe_height_px,
        pipe_width_px,
        pipe_spawn_height_modifier_px,
        pipe_max_deviation,
        ..
    } = *variables;
    let spawn_height_rng = rng.next_u32();
    let space_rng = rng.next_u32();

    let space = (space_rng as f32) / (u32::MAX as f32)
        * (vertical_space_between_pipes_max_px - vertical_space_between_pipes_min_px)
        + vertical_space_between_pipes_min_px;

    let spawn_height =
        ((spawn_height_rng as f32 / (u32::MAX as f32)) - 0.5) * 2. * pipe_max_deviation
            + pipe_spawn_height_modifier_px
            + (pipe_height_px + space) / 2.0;

    let x = PIPE_SPAWN_DESPAWN_X;
    let pipes = [
        (spawn_height, pipe_down_texture),
        (spawn_height - pipe_height_px - space, pipe_up_texture),
    ]
    .map(|(y, texture)| {
        (
            Pipe,
            PipeMovement,
            Sprite {
                image: texture.clone(),
                custom_size: Some(Vec2::new(pipe_width_px, pipe_height_px)),
                image_mode: SpriteImageMode::Scale(ScalingMode::FillCenter),
                ..Default::default()
            },
            RigidBody::KinematicVelocityBased,
            Transform::from_xyz(x, y, 0.0),
            Collider::cuboid(pipe_width_px / 2.0, pipe_height_px / 2.0),
            Sensor,
            ActiveCollisionTypes::DYNAMIC_KINEMATIC,
        )
    });
    commands.spawn_batch(pipes);

    commands.spawn((
        BetweenPipes,
        PipeMovement,
        RigidBody::KinematicVelocityBased,
        Transform::from_xyz(x, spawn_height - (pipe_height_px + space) / 2., 0.0),
        Collider::cuboid(pipe_width_px / 2.0, space / 2.0),
        Sensor,
        ActiveCollisionTypes::DYNAMIC_KINEMATIC,
    ));
    *next_pipe_spawn = PipeSpawnTimer::random(&mut rng, &time, &variables)
}

fn update_pipes(
    mut commands: Commands,
    time: Res<Time>,
    query: Query<(Entity, &mut Transform), With<PipeMovement>>,
    variables: Res<GameVariables>,
) {
    let GameVariables {
        pipe_velocity_meter_per_secs,
        ..
    } = *variables;
    let delta = pipe_velocity_meter_per_secs
        * PIXELS_PER_METER
        * (time.delta().as_millis() as f32 / 1000.0);
    for (entity, mut transform) in query {
        transform.translation.x -= delta;
        if transform.translation.x < -PIPE_SPAWN_DESPAWN_X {
            commands.entity(entity).despawn();
            debug!(
                "Despawned Pipe at ({}, {})",
                transform.translation.x, transform.translation.y
            );
        }
    }
}

#[derive(Component)]
struct Pipe;

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    variables: Res<GameVariables>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut events: EventWriter<GameVariablesEvent>,
    q_camera: Query<Entity, With<Camera2d>>,
) {
    let GameVariables {
        floppy_alive_zone_x,
        floppy_alive_zone_y,
        ..
    } = *variables;
    let pipe_up_texture = asset_server.load::<Image>(PIPE_UP_TEXTURE_PATH);
    let pipe_down_texture = asset_server.load::<Image>(PIPE_DOWN_TEXTURE_PATH);
    let floppy = asset_server.load::<Image>(FLOPPY_TEXTURE_PATH);
    commands.insert_resource(GameResources {
        floppy,
        pipe_up_texture,
        pipe_down_texture,
    });
    commands.insert_resource(PipeSpawnTimer(time.elapsed()));
    commands.spawn((
        Mesh2d(meshes.add(Rectangle {
            half_size: Vec2::new(floppy_alive_zone_x, floppy_alive_zone_y),
        })),
        MeshMaterial2d(materials.add(Color::Srgba(BACKGROUND_COLOR))),
        Transform::from_xyz(0., 0., -20.),
    ));
    for camera in q_camera {
        commands
            .entity(camera)
            .insert(FollowStateComponent::Centered);
    }
    events.write(GameVariablesEvent::Initialized {
        new: variables.clone(),
    });
}

fn setup_debug_gui(variables: Res<GameVariables>, mut writer: EventWriter<DevGuiEvent>) {
    let vars = variables
        .iter_fields()
        .enumerate()
        .map(|(i, v)| {
            (
                variables.name_at(i).unwrap().to_owned(),
                format!("{v:?}").trim_matches('\"').to_owned(),
            )
        })
        .collect();
    writer.write(DevGuiEvent::AddVariables { vars });
}

fn handle_debug_gui_events(
    mut reader: EventReader<DevGuiEvent>,
    mut variables: ResMut<GameVariables>,
    mut events: EventWriter<GameVariablesEvent>,
) {
    for event in reader.read() {
        if let DevGuiEvent::VariableUpdated { key, value } = event {
            debug!("Updated {key} -> {value}");
            let old = variables.clone();
            let field = variables
                .reflect_mut()
                .as_struct()
                .unwrap()
                .field_mut(key)
                .unwrap();
            try_apply_parsed(field, value)
                .inspect_err(|err| error!("{err}"))
                .ok();
            events.write(GameVariablesEvent::Changed {
                old,
                new: variables.clone(),
            });
        }
    }
}

#[derive(Clone, Debug, Resource, Reflect)]
struct GameVariables {
    pipe_velocity_meter_per_secs: f32,
    pipe_spawn_distance_min_meters: f32,
    pipe_spawn_distance_max_meters: f32,
    vertical_space_between_pipes_min_px: f32,
    vertical_space_between_pipes_max_px: f32,
    pipe_max_deviation: f32,
    pipe_height_px: f32,
    pipe_width_px: f32,
    pipe_spawn_height_modifier_px: f32,
    floppy_jump_vertical_impulse: f32,
    floppy_jump_torque_impulse: f32,
    floppy_torque_spring_strength: f32,
    floppy_rotation_friction: f32,
    floppy_spawn_x: f32,
    floppy_spawn_y: f32,
    floppy_radius: f32,
    floppy_mass: f32,
    floppy_horizontal_force: f32,
    floppy_alive_zone_x: f32,
    floppy_alive_zone_y: f32,
    camera_follow_floppy: bool,
}

impl Default for GameVariables {
    fn default() -> Self {
        Self {
            pipe_velocity_meter_per_secs: 2.0,
            pipe_spawn_distance_min_meters: 3.0,
            pipe_spawn_distance_max_meters: 4.0,
            vertical_space_between_pipes_min_px: 150.0,
            vertical_space_between_pipes_max_px: 300.0,
            pipe_max_deviation: 150.0,
            pipe_height_px: 700.0,
            pipe_width_px: 50.0,
            pipe_spawn_height_modifier_px: 0.,
            floppy_jump_vertical_impulse: 1200.,
            floppy_jump_torque_impulse: 8000.,
            floppy_torque_spring_strength: 50000.,
            floppy_rotation_friction: 4000.,
            floppy_spawn_x: -300.,
            floppy_spawn_y: 400.,
            floppy_radius: 20.,
            floppy_mass: 2.,
            floppy_horizontal_force: 1000.,
            floppy_alive_zone_x: 700.,
            floppy_alive_zone_y: 400.,
            camera_follow_floppy: false,
        }
    }
}

pub struct GamePlugin;

#[derive(Clone, Debug, Component)]
struct Floppy;

#[derive(Clone, Debug, Resource)]
struct PipeSpawnTimer(Duration);

#[derive(Resource)]
pub struct GameResources {
    floppy: Handle<Image>,
    pipe_up_texture: Handle<Image>,
    pipe_down_texture: Handle<Image>,
}

impl PipeSpawnTimer {
    fn random(rng: &mut GlobalEntropy<WyRand>, time: &Time, variables: &GameVariables) -> Self {
        let GameVariables {
            pipe_spawn_distance_min_meters,
            pipe_spawn_distance_max_meters,
            pipe_velocity_meter_per_secs,
            ..
        } = variables;
        let rand = rng.next_u64();
        let now = time.elapsed();
        let meters = ((rand as f32) / (u64::MAX as f32)
            * (*pipe_spawn_distance_max_meters - *pipe_spawn_distance_min_meters))
            + *pipe_spawn_distance_min_meters;
        Self(now.add(Duration::from_millis(
            (meters * 1000. / pipe_velocity_meter_per_secs) as u64,
        )))
    }
}

#[derive(Clone, Debug, Copy, Component)]
enum FollowStateComponent {
    Follow(Entity),
    WantsToFollowFloppy,
    Centered,
}
