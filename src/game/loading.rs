use crate::game::GameState;
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

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PluginLoadingFinishedEvent>()
            .add_event::<LoadingFinishedEvent>()
            .insert_resource(PendingLoadingPlugins::default())
            .add_systems(
                Update,
                handle_plugin_finished_events.run_if(in_state(GameState::Loading)),
            );
    }
}

// Plugins melden sich hier als fertig
fn handle_plugin_finished_events(
    mut commands: Commands,
    mut events: EventReader<PluginLoadingFinishedEvent>,
    pending: Option<ResMut<PendingLoadingPlugins>>,
    mut finished_writer: EventWriter<LoadingFinishedEvent>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    for event in events.read() {
        pending.plugins.remove(event.plugin);
    }
    if pending.plugins.is_empty() {
        commands.remove_resource::<PendingLoadingPlugins>();
        finished_writer.write(LoadingFinishedEvent);
    }
}
