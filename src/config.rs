//! Everything M2WGame remembers, saved as one JSON file.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::{bottle::Bottle, game::Game, paths};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub games: Vec<Game>,
    pub bottles: Vec<Bottle>,
    /// `wine64` path of the engine the user picked.
    pub active_runtime: Option<PathBuf>,
    /// Wine binaries the user pointed at by hand.
    pub custom_runtimes: Vec<PathBuf>,
    /// Steam account name last used for downloads (not a secret; the password is never stored).
    pub steam_account: String,
}

impl Config {
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// Never fails: a missing file gives an empty library; an unreadable one is
    /// set aside as `config.json.bad` so nothing the user had is silently lost.
    pub fn load() -> Self {
        let path = paths::config_file();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Config::default();
        };
        match Config::from_json(&text) {
            Ok(cfg) => cfg,
            Err(_) => {
                let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                Config::default()
            }
        }
    }

    /// Writes to a temporary file first so a crash cannot leave half a file.
    pub fn save(&self) -> Result<(), String> {
        let path = paths::config_file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, self.to_json()?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    pub fn bottle(&self, id: &str) -> Option<&Bottle> {
        self.bottles.iter().find(|b| b.id == id)
    }

    pub fn game(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }

    pub fn ensure_default_bottle(&mut self) {
        if self.bottles.is_empty() {
            self.bottles.push(Bottle::new("Mặc định"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let mut cfg = Config::default();
        cfg.ensure_default_bottle();
        let bottle_id = cfg.bottles[0].id.clone();
        cfg.games.push(Game::new(
            "Test".into(),
            PathBuf::from("/tmp/test.exe"),
            bottle_id,
            "-windowed".into(),
        ));
        cfg.active_runtime = Some(PathBuf::from("/Applications/X.app/bin/wine64"));
        let back = Config::from_json(&cfg.to_json().unwrap()).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn old_or_partial_files_still_load() {
        let cfg = Config::from_json(r#"{"games":[]}"#).unwrap();
        assert!(cfg.bottles.is_empty());
        let cfg = Config::from_json(
            r#"{"games":[{"id":"1","name":"A","exe":"/a.exe","bottle_id":"b"}]}"#,
        )
        .unwrap();
        assert!(cfg.games[0].settings.msync);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(Config::from_json("not json").is_err());
    }
}
