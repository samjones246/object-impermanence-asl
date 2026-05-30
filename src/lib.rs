#![no_std]

use asr::{future::next_tick, settings::Gui, Process};
use asr::{
    future::retry,
    game_engine::unity::il2cpp::{Module, Version},
    Address, Address64,
};

asr::async_main!(stable);
asr::panic_handler!();

#[derive(Gui)]
struct Settings {
    /// Split on main checkpoints.
    #[default = false]
    split_main_checkpoints: bool,

    /// Split on all checkpoints.
    #[default = false]
    split_all_checkpoints: bool,

    /// Split on level 0 (Landing) complete.
    #[default = true]
    split_landing_complete: bool,

    /// Split on level 1A (Intro) complete.
    #[default = true]
    split_intro_complete: bool,

    /// Split on level 1B (Exterior) complete.
    #[default = true]
    split_exterior_complete: bool,

    /// Split on level 2A (Spatial) complete.
    #[default = true]
    split_spatial_complete: bool,
}

async fn main() {
    let mut settings = Settings::register();

    asr::print_message("Hello, World!");

    loop {
        asr::print_message("Connecting to process...");
        let process = Process::wait_attach("Object Impermanence.exe").await;
        asr::print_message("Connecting to module...");
        let _ = Module::wait_attach(&process, Version::V2020).await;
        asr::print_message("Done!");

        process
            .until_closes(async {
                // TODO: Load some initial information from the process.
                loop {
                    settings.update();

                    // TODO: Do something on every tick.
                    next_tick().await;
                }
            })
            .await;
    }
}
