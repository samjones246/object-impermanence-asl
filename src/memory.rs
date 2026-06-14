use asr::signature::Signature;
use asr::watcher::{Pair, Watcher};
use asr::{Address, Process};

const SIG_STATIC_BASE: Signature<21> =
    Signature::new("48 8B 05 ?? ?? ?? ?? 48 8B 88 B8 00 00 00 48 89 59 18 0F B6 15");

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Checkpoint {
    pub save_key: String,
    pub scene: Scene,
    pub is_main_checkpoint: bool,
}

impl Default for Checkpoint {
    fn default() -> Self {
        Self {
            save_key: String::new(),
            scene: Scene::Unknown,
            is_main_checkpoint: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scene {
    Landing,
    Intro,
    Exterior,
    Spatial,
    Unknown,
}

impl Scene {
    fn from_scene_index(scene_index: &str) -> Self {
        match scene_index {
            "Assets/Scenes/0_Landing.unity" => Scene::Landing,
            "Assets/Scenes/1A_Intro.unity" => Scene::Intro,
            "Assets/Scenes/1B_Exterior.unity" => Scene::Exterior,
            "Assets/Scenes/2A_Spatial.unity" => Scene::Spatial,
            _ => Scene::Unknown,
        }
    }
}

pub struct Watchers {
    active_checkpoint: Watcher<Checkpoint>,
    respawn_state: Watcher<u8>,
    base_addr: Option<u64>,
}

impl Watchers {
    pub fn new() -> Self {
        Self {
            active_checkpoint: Watcher::new(),
            respawn_state: Watcher::new(),
            base_addr: None,
        }
    }

    pub async fn init(
        &mut self,
        process: &Process,
        module_addr: Address,
        module_size: u64,
    ) -> Result<(), asr::Error> {
        let match_addr = SIG_STATIC_BASE
            .wait_scan_process_range(process, (module_addr, module_size))
            .await;
        let offset = process.read::<u32>(match_addr + 3)?;
        asr::print_message(&format!(
            "sig found at: {}, offset: {:#010x}",
            match_addr, offset
        ));
        let base_addr = match_addr.value() + 7 + offset as u64 - module_addr.value();
        self.base_addr = Some(base_addr);
        asr::print_message(&format!("base addr: {:#10x}", base_addr));
        Result::Ok(())
    }

    pub fn update(&mut self, process: &Process, module_addr: Address) -> Result<(), asr::Error> {
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
            scene: Scene::from_scene_index(&scene_index),
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

pub struct State {
    pub active_checkpoint: Pair<Checkpoint>,
    pub respawn_state: Pair<u8>,
}

impl State {
    pub fn new(watchers: &Watchers) -> Self {
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
