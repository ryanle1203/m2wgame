//! Is the path to the servers M2WGame and Steam need actually open?
//!
//! Born from a real failure: Windows Steam said "needs to be online" while the
//! Internet worked, because the network reset connections to one Steam server
//! (cdn.steamstatic.com) and nothing else.

use std::process::{Command, Stdio};

use crate::system::Level;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetStatus {
    Ok,
    /// Connection reset or cut: typical of a network filter.
    Blocked,
    Slow,
    NoDns,
    Failed,
}

pub struct Probe {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
    pub why: &'static str,
}

pub static PROBES: [Probe; 5] = [
    Probe {
        id: "steam-store",
        label: "Cửa hàng Steam",
        url: "https://store.steampowered.com/",
        why: "đăng nhập và mua game",
    },
    Probe {
        id: "steam-update",
        label: "Máy chủ cập nhật Steam",
        url: "https://cdn.steamstatic.com/client/steam_client_win32",
        why: "Steam cho Windows tải bản cập nhật ở đây",
    },
    Probe {
        id: "steam-alt",
        label: "Máy chủ Steam dự phòng",
        url: "https://cdn.akamai.steamstatic.com/",
        why: "đường thay thế của Steam",
    },
    Probe {
        id: "steamcmd",
        label: "Máy chủ SteamCMD của Valve",
        url: "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_osx.tar.gz",
        why: "tải SteamCMD",
    },
    Probe {
        id: "github",
        label: "GitHub",
        url: "https://github.com/Gcenx/game-porting-toolkit/releases",
        why: "tải động cơ chạy game",
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeResult {
    pub id: &'static str,
    pub label: &'static str,
    pub why: &'static str,
    pub status: NetStatus,
    pub detail: String,
}

/// Turn curl's exit code and HTTP status into a plain-language verdict.
/// Any HTTP answer at all (even 404) proves the server can be reached.
pub fn classify(exit: Option<i32>, http: &str) -> (NetStatus, String) {
    let http = http.trim();
    match exit {
        Some(0) if !http.is_empty() && http != "000" => {
            (NetStatus::Ok, format!("Vào được (HTTP {http})"))
        }
        Some(0) => (NetStatus::Failed, "Không nhận được phản hồi".to_string()),
        // 35 = TLS/recv failure ("connection reset by peer"), 56 = recv failure,
        // 52 = empty reply, 55 = send failure.
        Some(35) | Some(56) | Some(52) | Some(55) => (
            NetStatus::Blocked,
            "Bị cắt kết nối giữa chừng, có thể mạng đang chặn".to_string(),
        ),
        Some(28) => (NetStatus::Slow, "Không phản hồi trong 12 giây".to_string()),
        Some(6) => (
            NetStatus::NoDns,
            "Không tìm được địa chỉ máy chủ (lỗi DNS)".to_string(),
        ),
        Some(7) => (NetStatus::Failed, "Không kết nối được".to_string()),
        Some(60) | Some(51) | Some(77) => (
            NetStatus::Failed,
            "Chứng chỉ bảo mật không hợp lệ, có thể có phần mềm đang can thiệp".to_string(),
        ),
        Some(c) => (NetStatus::Failed, format!("Lỗi mạng (mã {c})")),
        None => (NetStatus::Failed, "Bị ngắt đột ngột".to_string()),
    }
}

pub fn check(probe: &'static Probe) -> ProbeResult {
    let out = Command::new("/usr/bin/curl")
        .args(["-sS", "-I", "-L", "-o", "/dev/null", "-m", "12", "-w", "%{http_code}"])
        .arg(probe.url)
        .stdin(Stdio::null())
        .output();
    let (status, detail) = match out {
        Ok(o) => classify(o.status.code(), &String::from_utf8_lossy(&o.stdout)),
        Err(e) => (NetStatus::Failed, format!("Không chạy được curl: {e}")),
    };
    ProbeResult {
        id: probe.id,
        label: probe.label,
        why: probe.why,
        status,
        detail,
    }
}

/// Check every server at the same time (about 12 seconds in the worst case).
pub fn run_all() -> Vec<ProbeResult> {
    let handles: Vec<_> = PROBES
        .iter()
        .map(|p| std::thread::spawn(move || check(p)))
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

/// What the user should do, in plain Vietnamese.
pub fn advice(results: &[ProbeResult]) -> Vec<(Level, String)> {
    let ok = |id: &str| results.iter().any(|r| r.id == id && r.status == NetStatus::Ok);
    let mut out: Vec<(Level, String)> = Vec::new();

    if !ok("steam-store") {
        out.push((
            Level::Bad,
            "Không vào được cửa hàng Steam. Hãy kiểm tra kết nối Internet.".to_string(),
        ));
    } else if !ok("steam-update") {
        out.push((
            Level::Bad,
            "Mạng của bạn đang chặn máy chủ cập nhật của Steam (cdn.steamstatic.com). Steam cho Windows sẽ báo lỗi \"needs to be online\". Hãy bật VPN khi cập nhật Steam lần đầu, tải xong có thể tắt.".to_string(),
        ));
    }
    if ok("steam-store") && !ok("steamcmd") {
        out.push((
            Level::Warn,
            "Không tải được SteamCMD từ máy chủ của Valve. Thử lại sau hoặc bật VPN.".to_string(),
        ));
    }
    if !ok("github") {
        out.push((
            Level::Warn,
            "Không vào được GitHub nên khó tải động cơ chạy game. Bạn có thể tải tay bằng trình duyệt (có thể cần VPN).".to_string(),
        ));
    }
    if out.is_empty() {
        out.push((
            Level::Ok,
            "Mạng ổn: không có máy chủ cần thiết nào bị chặn.".to_string(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(id: &'static str, status: NetStatus) -> ProbeResult {
        ProbeResult {
            id,
            label: "x",
            why: "y",
            status,
            detail: String::new(),
        }
    }

    fn all_ok() -> Vec<ProbeResult> {
        PROBES.iter().map(|p| result(p.id, NetStatus::Ok)).collect()
    }

    /// Uses the real network. Run with: `cargo test -- --ignored --nocapture real_network_check`
    #[test]
    #[ignore]
    fn real_network_check() {
        let results = run_all();
        for r in &results {
            println!("{:<30} {:?}: {}", r.label, r.status, r.detail);
        }
        for (level, text) in advice(&results) {
            println!("[{level:?}] {text}");
        }
        assert_eq!(results.len(), PROBES.len());
    }

    #[test]
    fn any_http_answer_counts_as_reachable() {
        assert_eq!(classify(Some(0), "200").0, NetStatus::Ok);
        assert_eq!(classify(Some(0), "404").0, NetStatus::Ok);
        assert_eq!(classify(Some(0), "000").0, NetStatus::Failed);
    }

    #[test]
    fn connection_reset_is_reported_as_blocked() {
        // Exactly what curl printed for cdn.steamstatic.com: "(35) Recv failure: Connection reset by peer".
        assert_eq!(classify(Some(35), "000").0, NetStatus::Blocked);
        assert_eq!(classify(Some(56), "000").0, NetStatus::Blocked);
    }

    #[test]
    fn other_failures_are_told_apart() {
        assert_eq!(classify(Some(28), "").0, NetStatus::Slow);
        assert_eq!(classify(Some(6), "").0, NetStatus::NoDns);
        assert_eq!(classify(Some(7), "").0, NetStatus::Failed);
        assert_eq!(classify(None, "").0, NetStatus::Failed);
    }

    #[test]
    fn healthy_network_gets_one_good_line() {
        let advice = advice(&all_ok());
        assert_eq!(advice.len(), 1);
        assert_eq!(advice[0].0, Level::Ok);
    }

    #[test]
    fn the_real_steam_cdn_block_suggests_a_vpn() {
        let mut r = all_ok();
        r.iter_mut().find(|x| x.id == "steam-update").unwrap().status = NetStatus::Blocked;
        let advice = advice(&r);
        assert_eq!(advice.len(), 1);
        assert_eq!(advice[0].0, Level::Bad);
        assert!(advice[0].1.contains("VPN"));
        assert!(advice[0].1.contains("cdn.steamstatic.com"));
    }

    #[test]
    fn offline_machine_says_check_the_internet_not_vpn() {
        let r: Vec<ProbeResult> = PROBES.iter().map(|p| result(p.id, NetStatus::NoDns)).collect();
        let advice = advice(&r);
        assert!(advice[0].1.contains("kết nối Internet"));
        assert!(!advice.iter().any(|(_, t)| t.contains("chặn máy chủ cập nhật")));
    }
}
