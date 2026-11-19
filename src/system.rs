//! Is this Mac ready to run Windows games? Plain-language checks.

use std::process::Command;

use crate::paths;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Bad,
}

#[derive(Clone, Debug)]
pub struct Check {
    pub label: String,
    pub detail: String,
    pub level: Level,
}

#[derive(Clone, Debug, Default)]
pub struct SystemInfo {
    pub chip: String,
    pub arch: String,
    pub macos: String,
    pub ram_gb: u64,
    pub rosetta: bool,
    pub free_disk_gb: Option<u64>,
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl SystemInfo {
    pub fn detect() -> Self {
        let arch = run("uname", &["-m"]).unwrap_or_default();
        let chip = run("sysctl", &["-n", "machdep.cpu.brand_string"])
            .unwrap_or_else(|| "Không rõ".to_string());
        let macos = run("sw_vers", &["-productVersion"]).unwrap_or_default();
        let ram_bytes: u64 = run("sysctl", &["-n", "hw.memsize"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let rosetta = Command::new("arch")
            .args(["-x86_64", "/usr/bin/true"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        SystemInfo {
            chip,
            arch,
            macos,
            ram_gb: ram_bytes / 1024 / 1024 / 1024,
            rosetta,
            free_disk_gb: free_disk_gb(),
        }
    }

    pub fn macos_major(&self) -> u32 {
        self.macos
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    pub fn checks(&self, has_runtime: bool) -> Vec<Check> {
        let mut out = Vec::new();

        let apple_silicon = self.arch == "arm64";
        out.push(Check {
            label: "Chip Apple Silicon".into(),
            detail: if apple_silicon {
                self.chip.clone()
            } else {
                "Máy này không phải chip Apple Silicon (M1/M2/M3/M4...). Công cụ chạy game cần chip này.".into()
            },
            level: if apple_silicon { Level::Ok } else { Level::Bad },
        });

        let major = self.macos_major();
        out.push(Check {
            label: "Phiên bản macOS".into(),
            detail: format!("macOS {} (cần macOS 14 trở lên)", self.macos),
            level: if major >= 14 { Level::Ok } else { Level::Bad },
        });

        let (ram_level, ram_note) = if self.ram_gb >= 16 {
            (Level::Ok, "đủ cho hầu hết game")
        } else if self.ram_gb >= 8 {
            (Level::Warn, "game nặng có thể bị giật, nên có 16 GB")
        } else {
            (Level::Bad, "quá ít để chạy game Windows")
        };
        out.push(Check {
            label: "Bộ nhớ RAM".into(),
            detail: format!("{} GB, {}", self.ram_gb, ram_note),
            level: ram_level,
        });

        out.push(Check {
            label: "Rosetta 2".into(),
            detail: if self.rosetta {
                "Đã cài".into()
            } else {
                "Chưa cài. Mở Terminal và gõ: softwareupdate --install-rosetta --agree-to-license".into()
            },
            level: if self.rosetta { Level::Ok } else { Level::Bad },
        });

        match self.free_disk_gb {
            Some(gb) => out.push(Check {
                label: "Dung lượng trống".into(),
                detail: format!("{} GB còn trống", gb),
                level: if gb >= 30 {
                    Level::Ok
                } else if gb >= 10 {
                    Level::Warn
                } else {
                    Level::Bad
                },
            }),
            None => out.push(Check {
                label: "Dung lượng trống".into(),
                detail: "Không đọc được".into(),
                level: Level::Warn,
            }),
        }

        out.push(Check {
            label: "Động cơ chạy game (Wine)".into(),
            detail: if has_runtime {
                "Đã tìm thấy. Xem ở mục Động cơ.".into()
            } else {
                "Chưa có. Vào mục Động cơ để xem cách cài.".into()
            },
            level: if has_runtime { Level::Ok } else { Level::Bad },
        });

        out
    }

    /// True when nothing blocks running games (warnings are fine).
    pub fn ready(&self, has_runtime: bool) -> bool {
        self.checks(has_runtime).iter().all(|c| c.level != Level::Bad)
    }
}

fn free_disk_gb() -> Option<u64> {
    let home = paths::home();
    let out = run("df", &["-k", home.to_str()?])?;
    let kb: u64 = out
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()?;
    Some(kb / 1024 / 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(arch: &str, macos: &str, ram: u64, rosetta: bool) -> SystemInfo {
        SystemInfo {
            chip: "Apple M1 Pro".into(),
            arch: arch.into(),
            macos: macos.into(),
            ram_gb: ram,
            rosetta,
            free_disk_gb: Some(100),
        }
    }

    #[test]
    fn healthy_mac_is_ready() {
        assert!(info("arm64", "15.2", 16, true).ready(true));
    }

    #[test]
    fn missing_pieces_block_readiness() {
        assert!(!info("x86_64", "15.2", 16, true).ready(true));
        assert!(!info("arm64", "13.6", 16, true).ready(true));
        assert!(!info("arm64", "15.2", 16, false).ready(true));
        assert!(!info("arm64", "15.2", 16, true).ready(false));
    }

    #[test]
    fn low_ram_is_only_a_warning_at_8gb() {
        let checks = info("arm64", "15.2", 8, true).checks(true);
        let ram = checks.iter().find(|c| c.label == "Bộ nhớ RAM").unwrap();
        assert_eq!(ram.level, Level::Warn);
    }
}
