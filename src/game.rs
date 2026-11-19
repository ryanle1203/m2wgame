//! One entry in the library.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GameSettings {
    /// Faster synchronisation that Wine builds for macOS understand.
    pub msync: bool,
    /// Older alternative to msync.
    pub esync: bool,
    /// Show Apple's Metal performance overlay (FPS, GPU time).
    pub metal_hud: bool,
    /// Tell Rosetta to expose AVX (macOS 15+). Some games refuse to start without it.
    pub advertise_avx: bool,
    /// Print Wine's own error messages into the log (useful when a game will not start).
    pub debug_log: bool,
    /// Extra environment variables, one `KEY=VALUE` per line.
    pub extra_env: String,
}

impl Default for GameSettings {
    fn default() -> Self {
        GameSettings {
            msync: true,
            esync: false,
            metal_hud: false,
            advertise_avx: true,
            debug_log: false,
            extra_env: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Game {
    pub id: String,
    pub name: String,
    /// Path to the Windows program on the Mac's disk.
    pub exe: PathBuf,
    pub bottle_id: String,
    /// Extra command-line arguments handed to the game.
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub settings: GameSettings,
}

impl Game {
    pub fn new(name: String, exe: PathBuf, bottle_id: String, args: String) -> Self {
        Game {
            id: crate::util::new_id(),
            name,
            exe,
            bottle_id,
            args,
            settings: GameSettings::default(),
        }
    }

    pub fn exe_missing(&self) -> bool {
        !self.exe.is_file()
    }
}
