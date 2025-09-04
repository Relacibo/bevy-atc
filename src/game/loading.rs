use crate::game::GameState;
use crate::util::entities::despawn_all;
use bevy::platform::collections::hash_set::HashSet;
use bevy::prelude::*;

pub struct LoadingPlugin;

#[derive(Resource, Debug, Default)]
pub struct PendingLoadingPlugins {
    plugins: HashSet<&'static str>,
}

impl PendingLoadingPlugins {
    pub fn register_plugin(&mut self, plugin: &'static str) {
        self.plugins.insert(plugin);
    }
}

#[derive(Event)]
pub struct PluginLoadingFinishedEvent {
    pub plugin: &'static str,
}

#[derive(Event, Debug, Clone)]
pub struct LoadingFinishedEvent;

#[derive(Component)]
pub struct LoadingScreen;

#[derive(Component)]
pub struct LoadingSpinner;

#[derive(Resource)]
pub struct LoadingScreenTimer(pub Timer);

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingLoadingPlugins>()
            .add_event::<PluginLoadingFinishedEvent>()
            .add_event::<LoadingFinishedEvent>()
            .add_systems(OnEnter(GameState::Loading), setup)
            .add_systems(
                OnExit(GameState::Loading),
                (despawn_all::<LoadingScreen>, destroy),
            )
            .add_systems(
                Update,
                (
                    handle_plugin_finished_events,
                    animate_loading_spinner,
                    update_loading_text,
                )
                    .run_if(in_state(GameState::Loading)),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.insert_resource(LoadingScreenTimer(Timer::from_seconds(
        0.1,
        TimerMode::Repeating,
    )));
    // Root Loading Screen Entity
    commands
        .spawn((
            LoadingScreen,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::Srgba(Srgba::new(0.1, 0.1, 0.1, 1.0))),
        ))
        .with_children(|parent| {
            // Loading Text
            parent.spawn((
                Text::new("Loading..."),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(30.0)),
                    ..default()
                },
            ));

            // Loading Spinner (simple rotating text)
            parent.spawn((
                LoadingSpinner,
                Text::new("◐"),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::Srgba(Srgba::new(0.8, 0.8, 0.8, 1.0))),
            ));

            // Progress Info
            parent.spawn((
                Text::new("Initializing game systems..."),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::Srgba(Srgba::new(0.7, 0.7, 0.7, 1.0))),
                Node {
                    margin: UiRect::top(Val::Px(20.0)),
                    ..default()
                },
            ));
        });
}

fn animate_loading_spinner(
    time: Res<Time>,
    mut timer: ResMut<LoadingScreenTimer>,
    mut query: Query<&mut Transform, With<LoadingSpinner>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        for mut transform in query.iter_mut() {
            transform.rotation *= Quat::from_rotation_z(0.3);
        }
    }
}

fn update_loading_text(
    pending: Option<Res<PendingLoadingPlugins>>,
    mut query: Query<&mut Text, (Without<LoadingSpinner>, With<Text>)>,
) {
    let Some(pending) = pending else {
        return;
    };

    for mut text in query.iter_mut() {
        if text.0.contains("Initializing") {
            if pending.plugins.is_empty() {
                **text = "Almost done...".to_string();
            } else {
                **text = format!("Loading {} plugins...", pending.plugins.len());
            }
        }
    }
}

// Plugins melden sich hier als fertig
fn handle_plugin_finished_events(
    mut events: EventReader<PluginLoadingFinishedEvent>,
    mut pending: ResMut<PendingLoadingPlugins>,
    mut finished_writer: EventWriter<LoadingFinishedEvent>,
) {
    if events.is_empty() {
        return;
    }
    for event in events.read() {
        pending.plugins.remove(event.plugin);
    }
    if pending.plugins.is_empty() {
        finished_writer.write(LoadingFinishedEvent);
    }
}

fn destroy(mut commands: Commands) {
    commands.remove_resource::<PendingLoadingPlugins>();
    commands.remove_resource::<LoadingScreenTimer>();
}
