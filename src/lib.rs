use crate::memory::{Scene, State, Watchers};
use asr::{file_format::pe, future::next_tick, settings::Gui, Address, Process};

pub mod memory;

asr::async_main!(stable);

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

    loop {
        asr::print_message("Connecting to process...");
        let process = Process::wait_attach("Object Impermanence.exe").await;
        let (module_addr, _) = process.wait_module_range("GameAssembly.dll").await;
        let module_size: u64 = pe::read_size_of_image(&process, module_addr)
            .expect("failed to read size of image")
            .into();
        let mut watchers = Watchers::new();
        watchers
            .init(&process, module_addr, module_size)
            .await
            .expect("sig scanning failed");
        process
            .until_closes(async {
                loop {
                    settings.update();
                    let _ = watchers.update(&process, module_addr);
                    let state = State::new(&watchers);
                    state.log_changes();
                    if state.should_start() {
                        asr::print_message("start");
                        asr::timer::start();
                    }
                    if state.is_loading() {
                        asr::timer::pause_game_time();
                    } else {
                        asr::timer::resume_game_time();
                    }
                    if state.should_split(&settings) {
                        asr::print_message("split");
                        asr::timer::split();
                    }
                    next_tick().await;
                }
            })
            .await;
    }
}

impl State {
    fn should_start(&self) -> bool {
        self.respawn_state.changed_to(&4)
            && self.active_checkpoint.current.save_key == "2f4e2d6fd93506b48b9baef53f65c81c"
    }
    fn is_loading(&self) -> bool {
        self.respawn_state.current != 4
    }
    fn should_split(&self, settings: &Settings) -> bool {
        if self.active_checkpoint.changed() {
            if settings.split_all_checkpoints {
                return true;
            }
            if settings.split_main_checkpoints && self.active_checkpoint.current.is_main_checkpoint
            {
                return true;
            }
            if self.active_checkpoint.current.scene != self.active_checkpoint.old.scene {
                match self.active_checkpoint.old.scene {
                    Scene::Landing if settings.split_landing_complete => return true,
                    Scene::Intro if settings.split_intro_complete => return true,
                    Scene::Exterior if settings.split_exterior_complete => return true,
                    Scene::Spatial if settings.split_spatial_complete => return true,
                    _ => {}
                }
            }
        }

        false
    }

    fn log_changes(&self) {
        if self.active_checkpoint.changed() {
            asr::print_message(&format!(
                "active_checkpoint {:?}",
                self.active_checkpoint.current
            ));
        }
        if self.respawn_state.changed() {
            asr::print_message(&format!("respawn_state {:?}", self.respawn_state.current));
        }
    }
}
