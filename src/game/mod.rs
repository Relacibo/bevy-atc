// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/2d/sprite_animation.rs

use crate::game::{
    aircraft::AircraftPlugin,
    aircraft_card::AircraftCardPlugin,
    level::LevelPlugin,
    loading::{LoadingFinishedEvent, LoadingPlugin},
};
use bevy::{dev_tools::states::log_transitions, prelude::*};
use camera::GameCameraPlugin;
pub struct GamePlugin;

use crate::{
    APP_CONFIG, AppState,
    dev_gui::{DevGuiInputEvent, DevGuiStructTrait, DevGuiVariableUpdatedEvent},
    game::control::ControlPlugin,
    menu::LevelMeta,
    util::reflect::try_apply_parsed,
};

mod aircraft;
mod aircraft_card;
mod camera;
mod control;
mod level;
mod loading;
pub mod run_conditions;

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
            LoadingPlugin,
            LevelPlugin,
            AircraftPlugin,
        ))
        .register_type::<GameVariables>()
        .add_systems(OnEnter(AppState::Game), enter_loading_state)
        .add_systems(
            Update,
            handle_loading_finished.run_if(in_state(GameState::Loading)),
        )
        .insert_state(GameState::BeforeGame);

        if APP_CONFIG.dev_gui {
            app.add_systems(OnEnter(AppState::Game), setup_dev_gui)
                .add_systems(
                    Update,
                    (handle_dev_gui_events).run_if(in_state(AppState::Game)),
                );
        }

        if APP_CONFIG.log_state_transitions {
            app.add_systems(Update, (log_transitions::<GameState>,));
        }
    }
}

fn enter_loading_state(mut game_state: ResMut<NextState<GameState>>) {
    game_state.set(GameState::Loading);
}

fn handle_loading_finished(
    mut events: EventReader<LoadingFinishedEvent>,
    mut game_state: ResMut<NextState<GameState>>,
) {
    for LoadingFinishedEvent in events.read() {
        game_state.set(GameState::Running);
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

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States)]
pub enum GameState {
    BeforeGame,
    Loading,
    Running,
}

fn setup_dev_gui(variables: Res<GameVariables>, mut writer: EventWriter<DevGuiInputEvent>) {
    writer.write(DevGuiInputEvent::AddStruct(
        Box::new((*variables).clone()) as Box<dyn DevGuiStructTrait>
    ));
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
