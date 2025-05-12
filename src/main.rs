use bevy::{
    DefaultPlugins,
    app::{App, AppExit},
    prelude::*,
    window::WindowPlugin,
};

use bevy_prng::WyRand;
use bevy_rand::prelude::EntropyPlugin;
use rand_core::RngCore;

use game::GamePlugin;
use menu::MenuPlugin;
mod game;
mod menu;

#[derive(Debug, Clone, Hash, PartialEq, Eq, States)]
pub enum AppState {
    Menu,
    Game,
}

fn main() -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    dotenvy::dotenv().ok();

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Floppy".to_owned(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            // Prevents blurry sprites
            .set(ImagePlugin::default_nearest()),
        EntropyPlugin::<WyRand>::default(),
        GamePlugin,
        MenuPlugin,
    ))
    .add_systems(Startup, setup)
    .insert_state(AppState::Menu);

    #[cfg(debug_assertions)]
    {
        app.add_plugins(bevy_egui::EguiPlugin {
            enable_multipass_for_primary_context: true,
        });
    }

    let res = app.run();
    if let AppExit::Error(err) = res {
        eprintln!("Bevy exited with Error: {err}")
    }
    Ok(())
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}
