// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/ui/flex_layout.rs
//!
use bevy::asset::{AssetServer, Assets, Handle};
use bevy::ecs::system::command;
use bevy::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use serde::Deserialize;

use crate::game::GameVariables;
use crate::{AppState, util::entities::despawn_all};

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.25, 0.65, 0.25);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

#[derive(Clone, Debug, Component)]
struct MainMenuComponent;

#[derive(Clone, Debug)]
pub struct MenuPlugin;

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
enum MenuState {
    #[default]
    Main,
    LevelSelect,
    Disabled,
}

#[derive(Component)]
struct OnMainMenuScreen;
#[derive(Component)]
struct OnLevelSelectScreen;

#[derive(Clone, Debug, Component)]
enum MenuButtonAction {
    Play,
    Exit,
    LevelSelect,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct MenuData {
    pub selected_level: Option<LevelMeta>,
    pub level_index_handle: Handle<LevelIndexFile>,
}

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<MenuState>()
            .init_resource::<MenuData>()
            .add_plugins(RonAssetPlugin::<LevelIndexFile>::new(&["ron"]))
            .add_systems(OnEnter(AppState::Menu), menu_setup)
            .add_systems(OnExit(AppState::Menu), destroy_menu)
            .add_systems(OnEnter(MenuState::Main), main_menu_setup)
            .add_systems(OnExit(MenuState::Main), despawn_all::<OnMainMenuScreen>)
            .add_systems(OnEnter(MenuState::LevelSelect), level_select_menu_setup)
            .add_systems(
                OnExit(MenuState::LevelSelect),
                despawn_all::<OnLevelSelectScreen>,
            )
            .add_systems(Update, menu_action.run_if(in_state(AppState::Menu)))
            .add_systems(Update, button_system.run_if(in_state(AppState::Menu)))
            .add_systems(
                Update,
                level_button_action.run_if(in_state(MenuState::LevelSelect)),
            );
    }
}

fn menu_setup(
    mut commands: Commands,
    mut menu_state: ResMut<NextState<MenuState>>,
    asset_server: Res<AssetServer>,
) {
    menu_state.set(MenuState::Main);
    let handle = asset_server.load("levels/index.ron");
    commands.insert_resource(MenuData {
        selected_level: None,
        level_index_handle: handle,
    });
}

fn main_menu_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    menu_data: Res<MenuData>,
) {
    let right_icon = asset_server.load("textures/Game Icons/right.png");
    let exit_icon = asset_server.load("textures/Game Icons/exitRight.png");
    let select_icon = asset_server.load("textures/Game Icons/wrench.png");

    let mut parent_commands = commands.spawn((
        OnMainMenuScreen,
        Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Percent(3.),
            ..Default::default()
        },
        children![
            create_button(right_icon, "Play", MenuButtonAction::Play),
            create_button(select_icon, "Select level", MenuButtonAction::LevelSelect),
            create_button(exit_icon, "Exit", MenuButtonAction::Exit),
        ],
    ));
    if let Some(selected) = &menu_data.selected_level {
        parent_commands.with_children(|parent| {
            parent.spawn((
                Text::new(format!("Selected: {}", selected.name)),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0., 1., 1.)),
            ));
        });
    }
}

fn level_select_menu_setup(
    mut commands: Commands,
    menu_data: Res<MenuData>,
    level_assets: Res<Assets<LevelIndexFile>>,
) {
    let handle = &menu_data.level_index_handle;
    let Some(index) = level_assets.get(handle) else {
        bevy::log::error!("Could not load level index (levels/level_index.ron)");
        // Fallback: Zeige nur den Titel
        commands.spawn((
            OnLevelSelectScreen,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Percent(3.),
                ..Default::default()
            },
            children![(
                Text::new("Could not load level index (levels/level_index.ron)"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::srgb(1., 0., 0.)),
            ),],
        ));
        return;
    };
    commands
        .spawn((
            OnLevelSelectScreen,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Percent(3.),
                ..Default::default()
            },
            children![(
                Text::new("Select Level:"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(TEXT_COLOR),
            ),],
        ))
        .with_children(|parent| {
            for level in &index.levels {
                parent.spawn((
                    Button,
                    BackgroundColor(NORMAL_BUTTON),
                    LevelButton {
                        meta: level.clone(),
                    },
                    Node {
                        width: Val::Px(320.),
                        height: Val::Px(40.),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    children![(
                        Text::new(level.name.clone()),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(TEXT_COLOR),
                    ),],
                ));
            }
        });
}

#[derive(Component, Clone, Debug)]
struct LevelButton {
    meta: LevelMeta,
}

#[allow(clippy::type_complexity)]
// System für Level-Button-Auswahl
fn level_button_action(
    mut interaction_query: Query<
        (&Interaction, &LevelButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_data: ResMut<MenuData>,
    mut menu_state: ResMut<NextState<MenuState>>,
) {
    for (interaction, level_button) in &mut interaction_query {
        if *interaction == Interaction::Pressed {
            menu_data.selected_level = Some(level_button.meta.clone());
            menu_state.set(MenuState::Main);
        }
    }
}

fn create_button(icon: Handle<Image>, label: &str, action: MenuButtonAction) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(400.),
            height: Val::Px(50.),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(NORMAL_BUTTON),
        action,
        children![
            (
                ImageNode::new(icon),
                Node {
                    width: Val::Px(30.0),
                    // This takes the icons out of the flexbox flow, to be positioned exactly
                    position_type: PositionType::Absolute,
                    // The icon will be close to the left border of the button
                    left: Val::Px(10.0),
                    ..default()
                }
            ),
            (
                Text::new(label),
                TextFont {
                    font_size: 33.0,
                    ..default()
                },
                TextColor(TEXT_COLOR),
            ),
        ],
    )
}

#[allow(clippy::type_complexity)]
fn menu_action(
    mut commands: Commands,
    interaction_query: Query<
        (&Interaction, &MenuButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut app_exit_events: EventWriter<AppExit>,
    mut app_state: ResMut<NextState<AppState>>,
    menu_data: Res<MenuData>,
) {
    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match menu_button_action {
                MenuButtonAction::Play => {
                    let Some(selected_level) = &menu_data.selected_level else {
                        continue;
                    };
                    commands.insert_resource(GameVariables::new(selected_level.clone()));
                    app_state.set(AppState::Game);
                }
                MenuButtonAction::Exit => {
                    app_exit_events.write(AppExit::Success);
                }
                MenuButtonAction::LevelSelect => {
                    menu_state.set(MenuState::LevelSelect);
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn button_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut background_color) in &mut interaction_query {
        *background_color = match *interaction {
            Interaction::Pressed => BackgroundColor(PRESSED_BUTTON),
            Interaction::Hovered => BackgroundColor(HOVERED_BUTTON),
            Interaction::None => BackgroundColor(NORMAL_BUTTON),
        }
    }
}

fn destroy_menu(mut commands: Commands, mut menu_state: ResMut<NextState<MenuState>>) {
    commands.remove_resource::<MenuData>();
    menu_state.set(MenuState::Disabled);
}

#[derive(Deserialize, Clone, Debug, Asset, Reflect)]
pub struct LevelIndexFile {
    pub levels: Vec<LevelMeta>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Reflect)]
pub struct LevelMeta {
    pub file: String,
    pub name: String,
    // Optional: pub description: Option<String>,
}
