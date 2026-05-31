use asr::watcher::{Pair, Watcher};
use asr::Address;
use asr::{future::next_tick, settings::Gui, Process};

asr::async_main!(stable);
// asr::panic_handler!();

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Checkpoint {
    save_key: String,
    scene_index: String,
    is_main_checkpoint: bool,
}

impl Default for Checkpoint {
    fn default() -> Self {
        Self {
            save_key: String::new(),
            scene_index: String::new(),
            is_main_checkpoint: false,
        }
    }
}

struct Watchers {
    active_checkpoint: Watcher<Checkpoint>,
    respawn_state: Watcher<u8>,
}

struct State {
    active_checkpoint: Pair<Checkpoint>,
    respawn_state: Pair<u8>,
}

impl Watchers {
    fn new() -> Self {
        Self {
            active_checkpoint: Watcher::new(),
            respawn_state: Watcher::new(),
        }
    }

    fn update(&mut self, process: &Process, module_addr: Address) -> Result<(), asr::Error> {
        let active_checkpoint_addr = process.read_pointer_path::<u64>(
            module_addr,
            asr::PointerSize::Bit64,
            &[0x020A8978, 0xB8, 0x18],
        )?;
        let save_key = process.read_string(active_checkpoint_addr + 0x20, 64)?;
        let scene_index = process.read_string(active_checkpoint_addr + 0x18, 64)?;
        let is_main_checkpoint = process.read::<bool>(active_checkpoint_addr + 0x28)?;
        let active_checkpoint = Checkpoint {
            save_key,
            scene_index,
            is_main_checkpoint,
        };
        let respawn_state = process.read_pointer_path::<u8>(
            module_addr,
            asr::PointerSize::Bit64,
            &[0x020AB398, 0xB8, 0x68, 0x38],
        )?;
        let _ = self.respawn_state.update(Some(respawn_state));
        let _ = self.active_checkpoint.update(Some(active_checkpoint));
        Ok(())
    }
}

impl State {
    fn new(watchers: &Watchers) -> Self {
        Self {
            active_checkpoint: watchers.active_checkpoint.pair.clone().unwrap_or_default(),
            respawn_state: watchers.respawn_state.pair.unwrap_or_default(),
        }
    }
}

trait StringReader {
    // Read a System.String (unicode) at the given address.
    fn read_string(
        &self,
        addr: impl Into<Address>,
        max_length: usize,
    ) -> Result<String, asr::Error>;
}

impl StringReader for Process {
    fn read_string(
        &self,
        addr: impl Into<Address>,
        max_length: usize,
    ) -> Result<String, asr::Error> {
        let mut buf = vec![0; max_length];
        let pointer = self.read_pointer(addr, asr::PointerSize::Bit64)?;
        self.read_into_buf(pointer + 0x14, &mut buf)?;
        // group u8s to u16s, since this is a unicode string, and trim at the first nul character
        let bytes: Vec<u16> = buf
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let nul_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        let string_bytes = &bytes[..nul_pos];
        Ok(String::from_utf16_lossy(string_bytes))
    }
}

async fn main() {
    let mut settings = Settings::register();

    asr::print_message("Hello, World!");

    loop {
        asr::print_message("Connecting to process...");
        let process = Process::wait_attach("Object Impermanence.exe").await;
        let (module_addr, _) = process.wait_module_range("GameAssembly.dll").await;
        let mut watchers = Watchers::new();
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
            if self.active_checkpoint.current.scene_index != self.active_checkpoint.old.scene_index
            {
                match self.active_checkpoint.old.scene_index.as_str() {
                    "Assets/Scenes/0_Landing.unity" if settings.split_landing_complete => {
                        return true
                    }
                    "Assets/Scenes/1A_Intro.unity" if settings.split_intro_complete => return true,
                    "Assets/Scenes/1B_Exterior.unity" if settings.split_exterior_complete => {
                        return true
                    }
                    "Assets/Scenes/2A_Spatial.unity" if settings.split_spatial_complete => {
                        return true
                    }
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
