// Based on:
// https://github.com/bevyengine/bevy/blob/main/examples/ui/flex_layout.rs
//! Demonstrates how the `AlignItems` and `JustifyContent` properties can be composed to layout text.
//!
use bevy::prelude::*;

use crate::AppState;

const MARGIN: Val = Val::Px(12.);

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.25, 0.65, 0.25);
const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

#[derive(Clone, Debug, Component)]
struct MainMenuComponent;

#[derive(Clone, Debug)]
pub struct MenuPlugin;

#[derive(Clone, Debug, Component)]
enum MenuButtonAction {
    Play,
    Exit,
}

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Menu), spawn)
            .add_systems(OnExit(AppState::Menu), despawn_all)
            .add_systems(Update, on_button_pressed.run_if(in_state(AppState::Menu)));
    }
}

fn despawn_all(mut commands: Commands, to_despawn: Query<Entity, With<MainMenuComponent>>) {
    for entity in &to_despawn {
        commands.entity(entity).despawn();
    }
}

#[allow(clippy::type_complexity)]
fn on_button_pressed(
    mut interaction_query: Query<
        (&Interaction, &MenuButtonAction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    mut app_exit_events: EventWriter<AppExit>,
    mut app_state: ResMut<NextState<AppState>>,
) {
    for (interaction, menu_button_action, mut bg_color) in &mut interaction_query {
        let BackgroundColor(color) = &mut *bg_color;
        match *interaction {
            Interaction::Pressed => {
                *color = PRESSED_BUTTON;
                match menu_button_action {
                    MenuButtonAction::Play => {
                        app_state.set(AppState::Game);
                    }
                    MenuButtonAction::Exit => {
                        app_exit_events.write(AppExit::Success);
                    }
                }
            }
            Interaction::Hovered => *color = HOVERED_BUTTON,
            Interaction::None => *color = NORMAL_BUTTON,
        }
    }
}

fn spawn(mut commands: Commands, asset_server: Res<AssetServer>) {
    let right_icon = asset_server.load("textures/Game Icons/right.png");
    let exit_icon = asset_server.load("textures/Game Icons/exitRight.png");
    commands.spawn((
        MainMenuComponent,
        BackgroundColor(Color::BLACK),
        Node {
            // fill the entire window
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: Val::Percent(3.),
            padding: UiRect::all(Val::ZERO)
                .with_top(Val::Px(20.0))
                .with_bottom(Val::Px(20.0)),
            ..Default::default()
        },
        children![
            create_button(right_icon, "Play", MenuButtonAction::Play),
            create_button(exit_icon, "Exit", MenuButtonAction::Exit)
        ],
    ));
}

fn create_button(icon: Handle<Image>, label: &str, action: MenuButtonAction) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(300.),
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
