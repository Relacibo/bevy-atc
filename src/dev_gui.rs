use std::{collections::BTreeMap, ops::Deref};

use bevy::{
    ecs::schedule::ScheduleLabel,
    input::{common_conditions::input_just_pressed, keyboard::Key},
    prelude::*,
};
use bevy_simple_scroll_view::{ScrollView, ScrollableContent};
use bevy_ui_text_input::{
    TextInputBuffer, TextInputContents, TextInputNode, TextInputPrompt, TextSubmissionEvent,
};

#[derive(Debug, Clone)]
pub struct DevGuiPlugin;

#[derive(Debug, Clone, Component)]
pub struct DevGuiRootComponent;

#[derive(Debug, Clone, Component)]
pub struct DevGuiScrollComponent;

#[derive(Debug, Clone, Component)]
pub struct DevGuiVariableInputContainer {
    key: String,
}

#[derive(Debug, Clone, Component)]
pub struct DevGuiVariableInput;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Event)]
pub enum DevGuiEvent {
    ClearAllVariables,
    AddVariables { vars: Vec<(String, String)> },
    VariableAdded { key: String },
    RemoveVariables { keys: Vec<String> },
    VariableRemoved { key: String },
    VariableUpdated { key: String, value: String },
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, States)]
pub enum DevGuiVisibilityState {
    Visible,
    Hidden,
}

impl DevGuiVisibilityState {
    fn to_visibility(self) -> Visibility {
        match self {
            DevGuiVisibilityState::Visible => Visibility::Visible,
            DevGuiVisibilityState::Hidden => Visibility::Hidden,
        }
    }
    fn from_visibility(val: Visibility) -> Self {
        match val {
            Visibility::Visible => DevGuiVisibilityState::Visible,
            Visibility::Hidden => DevGuiVisibilityState::Hidden,
            _ => DevGuiVisibilityState::Hidden,
        }
    }

    fn toggle(self) -> DevGuiVisibilityState {
        match self {
            DevGuiVisibilityState::Visible => DevGuiVisibilityState::Hidden,
            DevGuiVisibilityState::Hidden => DevGuiVisibilityState::Visible,
        }
    }
}

impl Plugin for DevGuiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_state(DevGuiVisibilityState::Hidden)
            .add_event::<DevGuiEvent>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    handle_toggle_visibility.run_if(input_just_pressed(KeyCode::KeyL)),
                    handle_visibility_state_changed.run_if(state_changed::<DevGuiVisibilityState>),
                    handle_events,
                    handle_ui_events.run_if(in_state(DevGuiVisibilityState::Visible)),
                ),
            );
    }
}

fn handle_visibility_state_changed(
    visibility_state: Res<State<DevGuiVisibilityState>>,
    visibilities: Query<&mut Visibility, With<DevGuiRootComponent>>,
) {
    let new_v = visibility_state.to_visibility();
    for mut v in visibilities {
        *v = new_v;
    }
}

fn handle_toggle_visibility(
    visibility_state: Res<State<DevGuiVisibilityState>>,
    mut new_visibility_state: ResMut<NextState<DevGuiVisibilityState>>,
) {
    let new_v = visibility_state.toggle();
    new_visibility_state.set(new_v);
}

fn setup(mut commands: Commands, visibility_state: Res<State<DevGuiVisibilityState>>) {
    commands.spawn((
        DevGuiRootComponent,
        Node {
            ..default()
        },
        Transform::from_xyz(0., 0., -2.),
        visibility_state.to_visibility(),
        children![(
            Node {
                height: Val::Px(300.0),
                width: Val::Px(600.0),
                flex_direction: FlexDirection::Row,
                overflow: Overflow::clip(),
                align_items: AlignItems::Start,
                ..default()
            },
            ScrollView {
                scroll_speed: 2000.0,
            },
            Visibility::Inherited,
            BackgroundColor(Srgba::new(0., 0., 0., 0.3).into()),
            children![(
                DevGuiScrollComponent,
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Visibility::Inherited,
                ScrollableContent::default()
            )]
        ),],
    ));
}

pub fn handle_ui_events(
    mut dev_gui_event_writer: EventWriter<DevGuiEvent>,
    q_variable_input_containers: Query<&DevGuiVariableInputContainer>,
    q_variable_inputs: Query<&ChildOf, With<DevGuiVariableInput>>,
    mut text_input_submit_event_reader: EventReader<TextSubmissionEvent>,
) {
    for event in text_input_submit_event_reader.read() {
        let TextSubmissionEvent { entity, text } = event;
        debug!("{entity}, {text}");
        let Ok(ChildOf(parent)) = q_variable_inputs.get(*entity) else {
            unreachable!()
        };

        let Ok(DevGuiVariableInputContainer { key }) = q_variable_input_containers.get(*parent)
        else {
            unreachable!()
        };

        dev_gui_event_writer.write(DevGuiEvent::VariableUpdated {
            key: key.to_owned(),
            value: text.clone(),
        });
    }
}

pub fn handle_events(
    mut commands: Commands,
    q_root: Query<(Entity, Option<&Children>), With<DevGuiScrollComponent>>,
    q_variable_input_containers: Query<&DevGuiVariableInputContainer>,
    mut dev_gui_event_param_set: ParamSet<(EventReader<DevGuiEvent>, EventWriter<DevGuiEvent>)>,
) {
    let Some((ui_root, children)) = q_root.iter().next() else {
        error!("Unexpected: DevGuiComponent not found!");
        return;
    };

    let mut new_events = Vec::new();

    for event in dev_gui_event_param_set.p0().read() {
        match event {
            DevGuiEvent::ClearAllVariables => {
                for child in children.iter().cloned().flatten() {
                    commands.entity(*child).despawn();
                }
            }
            DevGuiEvent::AddVariables { vars } => {
                for (key, value) in vars {
                    let node_bundle = create_variable_input(ui_root, key, value);
                    commands.spawn(node_bundle);
                    new_events.push(DevGuiEvent::VariableAdded {
                        key: key.to_owned(),
                    })
                }
            }
            DevGuiEvent::RemoveVariables { keys } => {
                for child in children.iter().cloned().flatten() {
                    let Ok(DevGuiVariableInputContainer { key }) =
                        q_variable_input_containers.get(*child)
                    else {
                        continue;
                    };

                    if !keys.contains(key) {
                        continue;
                    }

                    commands.entity(*child).despawn();
                    new_events.push(DevGuiEvent::VariableRemoved {
                        key: key.to_owned(),
                    });
                }
            }
            _ => {}
        }
    }
    for new_event in new_events {
        dev_gui_event_param_set.p1().write(new_event);
    }
}

fn create_variable_input(
    parent: Entity,
    key: impl Into<String>,
    initial_value: impl Into<String>,
) -> impl Bundle {
    let key = key.into();
    (
        DevGuiVariableInputContainer { key: key.clone() },
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        Visibility::Inherited,
        ChildOf(parent),
        children![
            (Node::default(), Text(key), Visibility::Inherited,),
            (
                DevGuiVariableInput,
                Node {
                    width: Val::Px(100.),
                    height: Val::Px(40.),
                    ..default()
                },
                TextInputNode {
                    clear_on_submit: false,
                    mode: bevy_ui_text_input::TextInputMode::TextSingleLine,
                    ..default()
                },
                TextInputPrompt {
                    text: initial_value.into(),
                    ..default()
                },
                Visibility::Inherited,
            ),
        ],
    )
}
