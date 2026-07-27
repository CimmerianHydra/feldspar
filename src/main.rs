use avian3d::PhysicsPlugins;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::RenderPlugin;

use feldspar::command::console_log_layer;
use feldspar::GamePlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                // Block-icon rendering needs the pipeline ready on the frame
                // it is asked for; see `render::icons`.
                .set(RenderPlugin {
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                // Routes `chat!` lines into the in-game console.
                .set(LogPlugin {
                    custom_layer: console_log_layer,
                    ..default()
                }),
        )
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(GamePlugin)
        .add_plugins(DevPlugins)
        .run();
}

/// Dev-only fixtures, compiled out when the `dev` feature is off.
struct DevPlugins;

impl Plugin for DevPlugins {
    fn build(&self, _app: &mut App) {
        #[cfg(feature = "dev")]
        _app.add_plugins(feldspar::dev::DevPlugin);
    }
}
