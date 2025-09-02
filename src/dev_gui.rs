use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use bevy_simple_scroll_view::{ScrollView, ScrollableContent};
use bevy_ui_text_input::{TextInputNode, TextInputPrompt, TextSubmitEvent};

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
pub struct DevGuiVariableUpdatedEvent {
    pub key: String,
    pub value: String,
}

pub trait DevGuiStructTrait: Struct + std::fmt::Debug {}

#[derive(Debug, Event)]
pub enum DevGuiInputEvent {
    AddStruct(Box<dyn DevGuiStructTrait>),
    RemoveAll,
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
            .add_event::<DevGuiVariableUpdatedEvent>()
            .add_event::<DevGuiInputEvent>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    handle_toggle_visibility.run_if(input_just_pressed(KeyCode::KeyL)),
                    handle_visibility_state_changed.run_if(state_changed::<DevGuiVisibilityState>),
                    handle_input_events,
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
        Node { ..default() },
        Transform::from_xyz(0., 0., -2.),
        visibility_state.to_visibility(),
        children![(
            Node {
                height: Val::Px(300.0),
                width: Val::Px(800.0),
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
                    width: Val::Percent(100.),
                    ..default()
                },
                Visibility::Inherited,
                ScrollableContent::default()
            )]
        ),],
    ));
}

pub fn handle_input_events(
    mut commands: Commands,
    mut events: EventReader<DevGuiInputEvent>,
    q_root: Query<(Entity, Option<&Children>), With<DevGuiScrollComponent>>,
) {
    let Some((ui_root, children)) = q_root.iter().next() else {
        error!("Unexpected: DevGuiComponent not found!");
        return;
    };
    for event in events.read() {
        match event {
            DevGuiInputEvent::AddStruct(dev_gui_struct_trait) => {
                let vars: Vec<(String, String)> = dev_gui_struct_trait
                    .iter_fields()
                    .enumerate()
                    .map(|(i, v)| {
                        (
                            dev_gui_struct_trait.name_at(i).unwrap().to_owned(),
                            format!("{v:?}").trim_matches('\"').to_owned(),
                        )
                    })
                    .collect();
                for (key, value) in vars {
                    let node_bundle = create_variable_input(ui_root, key, value);
                    commands.spawn(node_bundle);
                }
            }
            DevGuiInputEvent::RemoveAll => {
                for child in children.iter().cloned().flatten() {
                    commands.entity(*child).despawn();
                }
            }
        }
    }
}

pub fn handle_ui_events(
    mut dev_gui_event_writer: EventWriter<DevGuiVariableUpdatedEvent>,
    q_variable_input_containers: Query<&DevGuiVariableInputContainer>,
    q_variable_inputs: Query<&ChildOf, With<DevGuiVariableInput>>,
    mut text_input_submit_event_reader: EventReader<TextSubmitEvent>,
) {
    for event in text_input_submit_event_reader.read() {
        let TextSubmitEvent { entity, text } = event;
        debug!("{entity}, {text}");
        let Ok(ChildOf(parent)) = q_variable_inputs.get(*entity) else {
            unreachable!()
        };

        let Ok(DevGuiVariableInputContainer { key }) = q_variable_input_containers.get(*parent)
        else {
            unreachable!()
        };

        dev_gui_event_writer.write(DevGuiVariableUpdatedEvent {
            key: key.to_owned(),
            value: text.clone(),
        });
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
            width: Val::Percent(100.),
            ..default()
        },
        Visibility::Inherited,
        ChildOf(parent),
        children![
            (
                Node {
                    overflow: Overflow::hidden(),
                    max_width: Val::Px(350.),
                    ..default()
                },
                Text(key),
                Visibility::Inherited,
            ),
            (
                DevGuiVariableInput,
                Node {
                    width: Val::Px(200.),
                    height: Val::Px(40.),
                    ..default()
                },
                TextInputNode {
                    clear_on_submit: false,
                    mode: bevy_ui_text_input::TextInputMode::SingleLine,
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
