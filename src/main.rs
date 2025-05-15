use std::{cell::OnceCell, env, sync::LazyLock};

use bevy::{
    DefaultPlugins,
    app::{App, AppExit},
    dev_tools::DevToolsPlugin,
    prelude::*,
    window::WindowPlugin,
    winit::WinitSettings,
};

use bevy_prng::WyRand;
use bevy_rand::prelude::EntropyPlugin;

use bevy_simple_scroll_view::ScrollViewPlugin;
use bevy_ui_text_input::TextInputPlugin;
use dev_gui::DevGuiPlugin;
use game::GamePlugin;
use menu::MenuPlugin;
mod dev_gui;
mod game;
mod menu;
mod util;

pub static APP_CONFIG: LazyLock<AppConfig> = LazyLock::new(AppConfig::from_env);

#[derive(Debug, Clone)]
pub struct AppConfig {
    dev_gui: bool,
    rapier_debug_render: bool,
}

impl AppConfig {
    fn from_env() -> Self {
        let dev_gui = env::var("DEV_GUI").as_deref() == Ok("1");
        let rapier_debug_render = env::var("RAPIER_DEBUG_RENDER").as_deref() == Ok("1");
        Self {
            dev_gui,
            rapier_debug_render,
        }
    }
}

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
                    ..default()
                }),
                ..default()
            })
            // Prevents blurry sprites
            .set(ImagePlugin::default_nearest()),
        EntropyPlugin::<WyRand>::default(),
        GamePlugin,
        MenuPlugin,
        TextInputPlugin,
        ScrollViewPlugin,
    ))
    .add_systems(Startup, setup)
    .insert_state(AppState::Menu);

    if APP_CONFIG.dev_gui {
        app.add_plugins(DevGuiPlugin);
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
