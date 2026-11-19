//! A "bottle" is one private Windows environment (a Wine prefix) with its own
//! fake C: drive. Keeping games in separate bottles stops them breaking each other.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::paths;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bottle {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub created: u64,
    /// Engine (`wine64` path) this bottle uses. `None` = the default engine.
    #[serde(default)]
    pub runtime: Option<PathBuf>,
}

impl Bottle {
    pub fn new(name: impl Into<String>) -> Self {
        Bottle {
            id: crate::util::new_id(),
            name: name.into(),
            created: crate::util::now_secs(),
            runtime: None,
        }
    }

    pub fn path(&self) -> PathBuf {
        paths::bottles_dir().join(&self.id)
    }

    /// Wine writes `system.reg` once the prefix has been created.
    pub fn is_initialized(&self) -> bool {
        self.path().join("system.reg").is_file()
    }

    /// Where the Windows version of Steam installs itself.
    pub fn steam_exe(&self) -> PathBuf {
        self.path().join("drive_c/Program Files (x86)/Steam/steam.exe")
    }
}
