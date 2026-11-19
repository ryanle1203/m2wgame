//! "Engines": the Wine builds that actually translate Windows to macOS.
//! M2WGame does not ship one. It finds the ones already on this Mac,
//! and prefers a build that contains Apple's D3DMetal (DirectX -> Metal).

use std::path::{Path, PathBuf};

use crate::paths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    pub name: String,
    /// The `wine64` (or `wine`) executable.
    pub wine_bin: PathBuf,
    /// True when Apple's D3DMetal is bundled (best DirectX 11/12 support).
    pub d3dmetal: bool,
    /// Output of `wine --version`. Filled in later from a background thread
    /// because asking takes a moment (Rosetta).
    pub version: Option<String>,
}

/// Wine releases older than this major version are "old" (2022-2023 or earlier).
/// Recent Windows clients such as Steam may not start on them.
pub const OLD_WINE_MAJOR: u32 = 9;

/// Ask the engine for its version, e.g. `wine-7.7 (Game Porting Toolkit 1.1)`.
pub fn detect_version(wine_bin: &Path) -> Option<String> {
    use std::process::{Command, Stdio};
    let out = Command::new(wine_bin)
        .arg("--version")
        // Never let a version check create or touch a real prefix.
        .env("WINEPREFIX", std::env::temp_dir().join("m2wgame-version-check"))
        .env("WINEDEBUG", "-all")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// `wine-7.7 (Game Porting Toolkit 1.1)` -> (7, 7); `wine-10.0-rc1` -> (10, 0).
pub fn parse_version(text: &str) -> Option<(u32, u32)> {
    let rest = text.trim().strip_prefix("wine-")?;
    let mut parts = rest.split(|c: char| !c.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

impl Runtime {
    pub fn from_bin(name: impl Into<String>, wine_bin: PathBuf) -> Option<Self> {
        if !wine_bin.is_file() {
            return None;
        }
        let root = wine_bin.parent()?.parent()?;
        Some(Runtime {
            name: name.into(),
            d3dmetal: has_d3dmetal(root),
            wine_bin,
            version: None,
        })
    }

    /// True when the version is known and older than `OLD_WINE_MAJOR`.
    pub fn is_old(&self) -> bool {
        self.version
            .as_deref()
            .and_then(parse_version)
            .map(|(major, _)| major < OLD_WINE_MAJOR)
            .unwrap_or(false)
    }

    pub fn bin_dir(&self) -> &Path {
        self.wine_bin.parent().unwrap_or_else(|| Path::new("/"))
    }

    pub fn wineserver(&self) -> PathBuf {
        self.wine_bin.with_file_name("wineserver")
    }
}

fn has_d3dmetal(root: &Path) -> bool {
    root.join("lib/external/D3DMetal.framework").exists()
        || root.join("lib/external/libd3dshared.dylib").exists()
        || root.join("lib64/apple_gptk/external/D3DMetal.framework").exists()
}

/// `wine64` if present, otherwise `wine` (new WoW64 builds only ship `wine`).
fn pick_bin(bin_dir: &Path) -> Option<PathBuf> {
    ["wine64", "wine"]
        .iter()
        .map(|n| bin_dir.join(n))
        .find(|p| p.is_file())
}

fn app_bin_dirs(apps_root: &Path) -> Vec<(String, PathBuf)> {
    [
        "Game Porting Toolkit",
        "Wine Crossover",
        "Wine Stable",
        "Wine Staging",
        "Wine Devel",
    ]
    .iter()
    .map(|app| {
        (
            app.to_string(),
            apps_root.join(format!("{app}.app/Contents/Resources/wine/bin")),
        )
    })
    .collect()
}

/// Folders inside the M2WGame "Runtimes" directory that look like a Wine build.
fn own_runtimes() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(paths::runtimes_dir()) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        for rel in [
            "bin",
            "wine/bin",
            "Contents/Resources/wine/bin",
            "Resources/wine/bin",
        ] {
            out.push((name.clone(), dir.join(rel)));
        }
        // A downloaded engine usually unpacks to "<folder>/Something.app".
        if let Ok(children) = std::fs::read_dir(&dir) {
            for child in children.flatten() {
                let p = child.path();
                if p.extension().map(|e| e == "app").unwrap_or(false) {
                    out.push((name.clone(), p.join("Contents/Resources/wine/bin")));
                }
            }
        }
    }
    out
}

/// Look in every place a Wine build normally lives.
pub fn detect_all(extra_wine_bins: &[PathBuf]) -> Vec<Runtime> {
    let mut dirs: Vec<(String, PathBuf)> = Vec::new();
    dirs.extend(app_bin_dirs(Path::new("/Applications")));
    dirs.extend(app_bin_dirs(&paths::home().join("Applications")));
    dirs.push(("Wine (Homebrew)".into(), PathBuf::from("/opt/homebrew/bin")));
    dirs.push(("Wine (Homebrew Intel)".into(), PathBuf::from("/usr/local/bin")));
    dirs.push((
        t!("Wine từ Whisky").into(),
        paths::home().join("Library/Application Support/com.isaacmarovitz.Whisky/Libraries/Wine/bin"),
    ));
    dirs.extend(own_runtimes());

    let mut found: Vec<Runtime> = Vec::new();
    let mut push = |rt: Runtime| {
        let key = std::fs::canonicalize(&rt.wine_bin).unwrap_or_else(|_| rt.wine_bin.clone());
        if !found.iter().any(|r| {
            std::fs::canonicalize(&r.wine_bin).unwrap_or_else(|_| r.wine_bin.clone()) == key
        }) {
            found.push(rt);
        }
    };

    for (name, dir) in dirs {
        if let Some(bin) = pick_bin(&dir) {
            if let Some(rt) = Runtime::from_bin(name, bin) {
                push(rt);
            }
        }
    }
    for bin in extra_wine_bins {
        let name = bin
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!("Wine (tự chọn)").into());
        if let Some(rt) = Runtime::from_bin(name, bin.clone()) {
            push(rt);
        }
    }

    // D3DMetal builds first, then by name.
    found.sort_by(|a, b| b.d3dmetal.cmp(&a.d3dmetal).then(a.name.cmp(&b.name)));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fake_wine(root: &Path, with_d3dmetal: bool) -> PathBuf {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let wine = bin.join("wine64");
        fs::write(&wine, "#!/bin/sh\n").unwrap();
        if with_d3dmetal {
            fs::create_dir_all(root.join("lib/external/D3DMetal.framework")).unwrap();
        }
        wine
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("m2wgame-test-{tag}-{}", crate::util::new_id()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn detects_d3dmetal_next_to_the_binary() {
        let a = temp_dir("a");
        let b = temp_dir("b");
        let with = Runtime::from_bin("x", fake_wine(&a, true)).unwrap();
        let without = Runtime::from_bin("y", fake_wine(&b, false)).unwrap();
        assert!(with.d3dmetal);
        assert!(!without.d3dmetal);
        fs::remove_dir_all(a).ok();
        fs::remove_dir_all(b).ok();
    }

    #[test]
    fn parses_wine_versions() {
        assert_eq!(parse_version("wine-7.7 (Game Porting Toolkit 1.1)"), Some((7, 7)));
        assert_eq!(parse_version("wine-10.0-rc1"), Some((10, 0)));
        assert_eq!(parse_version("wine-11"), Some((11, 0)));
        assert_eq!(parse_version("wine-9.0 (Staging)\n"), Some((9, 0)));
        assert_eq!(parse_version("not wine"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn old_wine_is_flagged_but_unknown_is_not() {
        let mk = |v: Option<&str>| Runtime {
            name: "x".into(),
            wine_bin: PathBuf::from("/x/bin/wine64"),
            d3dmetal: false,
            version: v.map(String::from),
        };
        assert!(mk(Some("wine-7.7 (Game Porting Toolkit 1.1)")).is_old());
        assert!(mk(Some("wine-8.21")).is_old());
        assert!(!mk(Some("wine-9.0")).is_old());
        assert!(!mk(Some("wine-11.0")).is_old());
        assert!(!mk(None).is_old());
    }

    #[test]
    fn missing_binary_is_not_a_runtime() {
        assert!(Runtime::from_bin("x", PathBuf::from("/definitely/not/here/wine64")).is_none());
    }

    #[test]
    fn prefers_wine64_but_accepts_wine() {
        let d = temp_dir("pick");
        let bin = d.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("wine"), "").unwrap();
        assert_eq!(pick_bin(&bin).unwrap(), bin.join("wine"));
        fs::write(bin.join("wine64"), "").unwrap();
        assert_eq!(pick_bin(&bin).unwrap(), bin.join("wine64"));
        fs::remove_dir_all(d).ok();
    }

    #[test]
    fn manual_runtime_is_listed_once() {
        let d = temp_dir("extra");
        let wine = fake_wine(&d, true);
        let list = detect_all(&[wine.clone(), wine]);
        assert_eq!(list.iter().filter(|r| r.d3dmetal && r.wine_bin.starts_with(&d)).count(), 1);
        fs::remove_dir_all(d).ok();
    }
}
