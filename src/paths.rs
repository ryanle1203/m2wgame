//! Where M2WGame keeps its files. Nothing is hardcoded to a user name:
//! everything is derived from `$HOME` (or `$M2WGAME_HOME`, used by tests).
//!
//! Early builds were called WinBridge and stored their data in `WinBridge` folders. Those
//! folders keep being used when they exist and the new ones do not, so nothing has to be moved
//! (game paths saved in the config would break if it were).

use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// The test/portable override: `$M2WGAME_HOME`, or the old name `$WINBRIDGE_HOME`.
fn custom_home() -> Option<PathBuf> {
    std::env::var_os("M2WGAME_HOME")
        .or_else(|| std::env::var_os("WINBRIDGE_HOME"))
        .map(PathBuf::from)
}

/// `new` unless only the pre-rename folder `old` exists.
fn pick(base: PathBuf, new: &str, old: &str) -> PathBuf {
    let (new, old) = (base.join(new), base.join(old));
    if !new.exists() && old.exists() {
        old
    } else {
        new
    }
}

/// `~/Library/Application Support/M2WGame` (or the older `WinBridge` folder if that is what exists).
pub fn data_dir() -> PathBuf {
    if let Some(custom) = custom_home() {
        return custom;
    }
    pick(home().join("Library/Application Support"), "M2WGame", "WinBridge")
}

/// One folder per game environment ("bottle"), each is a Wine prefix.
pub fn bottles_dir() -> PathBuf {
    data_dir().join("Bottles")
}

/// Wine builds the user dropped here are detected automatically.
pub fn runtimes_dir() -> PathBuf {
    data_dir().join("Runtimes")
}

pub fn downloads_dir() -> PathBuf {
    data_dir().join("Downloads")
}

pub fn logs_dir() -> PathBuf {
    if let Some(custom) = custom_home() {
        return custom.join("Logs");
    }
    pick(home().join("Library/Logs"), "M2WGame", "WinBridge")
}

pub fn config_file() -> PathBuf {
    data_dir().join("config.json")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [
        data_dir(),
        bottles_dir(),
        runtimes_dir(),
        downloads_dir(),
        logs_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_folder_is_kept_only_while_the_new_one_does_not_exist() {
        let tmp = std::env::temp_dir().join(format!("m2wgame-paths-{}", crate::util::new_id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // nothing exists: the new name
        assert_eq!(pick(tmp.clone(), "M2WGame", "WinBridge"), tmp.join("M2WGame"));
        // only the old one: keep using it
        std::fs::create_dir_all(tmp.join("WinBridge")).unwrap();
        assert_eq!(pick(tmp.clone(), "M2WGame", "WinBridge"), tmp.join("WinBridge"));
        // both: the new one wins
        std::fs::create_dir_all(tmp.join("M2WGame")).unwrap();
        assert_eq!(pick(tmp.clone(), "M2WGame", "WinBridge"), tmp.join("M2WGame"));
        std::fs::remove_dir_all(&tmp).ok();
    }
}
