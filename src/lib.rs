use asr::signature::Signature;
use asr::watcher::{Pair, Watcher};
use asr::Address;
use asr::{future::next_tick, settings::Gui, Process};

asr::async_main!(stable);

const SIG_STATIC_BASE: Signature<21> = Signature::new("48 8B 05 ?? ?? ?? ?? 48 8B 88 B8 00 00 00 48 89 59 18 0F B6 15");

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
    base_addr: Option<u64>,
}

struct State {
    active_checkpoint: Pair<Checkpoint>,
    respawn_state: Pair<u8>
}

impl Watchers {
    fn new() -> Self {
        Self {
            active_checkpoint: Watcher::new(),
            respawn_state: Watcher::new(),
            base_addr: None,
        }
    }

    async fn init(&mut self, process: &Process, module_addr: Address, module_size: u64) -> Result<(), asr::Error> {
        let match_addr = SIG_STATIC_BASE.wait_scan_process_range(process, (module_addr, module_size)).await;
        let offset = process.read::<u32>(match_addr + 3)?;
        asr::print_message(&format!("sig found at: {}, offset: {:#010x}", match_addr, offset));
        let base_addr = match_addr.value() + 7 + offset as u64 - module_addr.value();
        self.base_addr = Some(base_addr);
        asr::print_message(&format!("base addr: {:#10x}", base_addr));
        Result::Ok(())
    }

    fn update(&mut self, process: &Process, module_addr: Address) -> Result<(), asr::Error> {
        let base_addr = self.base_addr.expect("Uninitialized");
        let active_checkpoint_addr = process.read_pointer_path::<u64>(
            module_addr,
            asr::PointerSize::Bit64,
            &[base_addr, 0xB8, 0x18],
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
            &[base_addr, 0xB8, 0x28, 0x38],
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

    loop {
        asr::print_message("Connecting to process...");
        let process = Process::wait_attach("Object Impermanence.exe").await;
        let (module_addr, module_size) = process.wait_module_range("GameAssembly.dll").await;
        let mut watchers = Watchers::new();
        watchers.init(&process, module_addr, module_size).await.expect("sig scanning failed");
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
