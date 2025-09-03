use crate::{
    APP_CONFIG,
    game::{
        GameState, Z_WAYPOINT,
        loading::{PendingLoadingPlugins, PluginLoadingFinishedEvent},
    },
};
use bevy::{dev_tools::states::log_transitions, prelude::*};
use bevy_common_assets::ron::RonAssetPlugin;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States, Default)]
pub enum LoadingState {
    #[default]
    Uninit,
    LoadingHandles,
    SpawningLevel,
    Finished,
}

const LEVEL_PLUGIN: &str = "LevelPlugin";

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RonAssetPlugin::<LevelFile>::new(&["ron"]))
            .add_systems(Startup, on_startup)
            .add_systems(OnEnter(GameState::Loading), setup)
            .add_systems(
                Update,
                poll_level_handles_loaded.run_if(in_state(LoadingState::LoadingHandles)),
            )
            .add_systems(OnEnter(LoadingState::SpawningLevel), spawn_level)
            .init_state::<LoadingState>();

        if APP_CONFIG.log_state_transitions {
            app.add_systems(Update, (log_transitions::<LoadingState>,));
        }
    }
}

fn on_startup(mut pending: ResMut<PendingLoadingPlugins>) {
    pending.register_plugin(LEVEL_PLUGIN);
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let level_handle = asset_server.load::<LevelFile>("levels/example_level.ron");
    commands.insert_resource(LevelHandle(level_handle));
    commands.set_state(LoadingState::LoadingHandles);
}

fn poll_level_handles_loaded(
    level: ResMut<LevelHandle>,
    level_assets: Res<Assets<LevelFile>>,
    mut next_loading_state: ResMut<NextState<LoadingState>>,
) {
    let LevelHandle(handle) = &*level;
    if level_assets.get(handle).is_some() {
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
    mut event_writer: EventWriter<PluginLoadingFinishedEvent>,
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
                }),
                Visibility::Inherited,
            )],
        ));
    }
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
                crate::game::Z_RUNWAY,
            )
            .with_rotation(Quat::from_rotation_z(angle)),
            Name::new(format!("Runway {}", rw.name)),
            Visibility::Visible,
        ));
    }
    commands.remove_resource::<LevelHandle>();
    next_loading_state.set(LoadingState::Finished);
    event_writer.write(PluginLoadingFinishedEvent {
        plugin: LEVEL_PLUGIN,
    });
}

#[derive(Resource, Debug, Clone)]
struct LevelHandle(Handle<LevelFile>);

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
