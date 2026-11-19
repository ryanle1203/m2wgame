//! Turns raw Wine / Steam log text into plain-Vietnamese hints.
//!
//! The patterns below are lines we really saw in logs, plus a few classic Wine
//! errors. The first hint is the most specific one.

use crate::system::Level;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hint {
    pub level: Level,
    pub text: String,
}

fn hint(level: Level, text: impl Into<String>) -> Hint {
    Hint {
        level,
        text: text.into(),
    }
}

/// `err:module:import_dll:Library MSVCP140.dll (which is needed by ...) not found`
fn missing_library(log: &str) -> Option<String> {
    let key = "import_dll:Library ";
    let start = log.find(key)? + key.len();
    let name: String = log[start..]
        .chars()
        .take_while(|c| !c.is_whitespace())
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// `failed` = the program already ended with an error (non-zero exit code).
/// `is_steam` = the program is the Windows Steam client.
pub fn diagnose(log: &str, failed: bool, is_steam: bool) -> Vec<Hint> {
    let lower = log.to_lowercase();
    let has = |needle: &str| lower.contains(&needle.to_lowercase());
    let mut out: Vec<Hint> = Vec::new();

    // --- Steam updating ---
    if has("Failed to determine download location") {
        out.push(hint(
            Level::Warn,
            "Steam bị gián đoạn khi tải gói cập nhật. Bấm Dừng rồi Chơi lại, Steam sẽ làm tiếp phần còn thiếu.",
        ));
    } else if has("needs to be online") || has("http error 0") {
        out.push(hint(
            Level::Bad,
            "Steam không tải được bản cập nhật. Thường do mạng chặn máy chủ của Steam: vào Kiểm tra máy, bấm Kiểm tra mạng. Nếu bị chặn, bật VPN rồi chạy lại Steam.",
        ));
    }

    // --- Crash inside the program ---
    if has("Unhandled page fault") || has("Unhandled exception") {
        if is_steam {
            out.push(hint(
                Level::Bad,
                "Steam bị lỗi bộ nhớ khi chạy trên động cơ này, thường vì động cơ Wine quá cũ (xem phiên bản ở mục Động cơ). Hãy thử một động cơ Wine mới hơn cho môi trường này.",
            ));
        } else {
            out.push(hint(
                Level::Bad,
                "Chương trình bị lỗi bộ nhớ (crash). Thử bật \"Ghi chi tiết lỗi vào log\" trong Cài đặt của game rồi chạy lại, hoặc thử động cơ khác.",
            ));
        }
    }

    // --- Missing pieces ---
    if let Some(dll) = missing_library(log) {
        out.push(hint(
            Level::Bad,
            format!("Thiếu thư viện Windows {dll}. Game thường kèm bộ cài bổ sung (tìm thư mục vcredist hoặc _CommonRedist), hãy chạy nó trong cùng môi trường."),
        ));
    }
    if has("bad CPU type") {
        out.push(hint(
            Level::Bad,
            "Máy chưa cài Rosetta 2. Vào Kiểm tra máy để xem cách cài.",
        ));
    }
    if has("could not load kernel32.dll") || has("failed to load ntdll") || has("wine: failed to initialize") {
        out.push(hint(
            Level::Bad,
            "Môi trường game bị hỏng. Hãy tạo một môi trường mới ở mục Môi trường và thử lại.",
        ));
    }
    if has("no driver could be loaded") {
        out.push(hint(
            Level::Bad,
            "Wine không mở được cửa sổ trên macOS. Thử một động cơ khác.",
        ));
    }
    if has("out of memory") {
        out.push(hint(
            Level::Warn,
            "Máy hết bộ nhớ. Đóng bớt ứng dụng khác rồi thử lại.",
        ));
    }
    if has("err:d3d") {
        out.push(hint(
            Level::Warn,
            "Game báo lỗi đồ họa DirectX. Kiểm tra động cơ đang dùng có D3DMetal (mục Động cơ).",
        ));
    }

    // --- Nothing recognised ---
    if out.is_empty() && failed {
        out.push(hint(
            Level::Warn,
            "Chưa nhận ra lỗi cụ thể. Bật \"Ghi chi tiết lỗi vào log\" trong Cài đặt của game, chạy lại, rồi mở Log để có thêm thông tin.",
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Lines copied from real logs (macOS, Apple Silicon).
    const PAGE_FAULT: &str = "wine: Unhandled page fault on read access to 0000000000000081 at address 00000002D4D4390A (thread 01c4), starting debugger...";
    const NO_LOCATION: &str = "[2026-11-18 16:11:26] Error: Failed to determine download location for universe 1";
    const OFFLINE: &str = "[2026-11-18 16:04:43] Error: Steam needs to be online to update. Please confirm your network connection and try again.";

    #[test]
    fn steam_crash_points_to_an_older_engine() {
        let hints = diagnose(PAGE_FAULT, true, true);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].level, Level::Bad);
        assert!(hints[0].text.contains("Wine quá cũ"));
    }

    #[test]
    fn other_programs_crash_gets_a_different_message() {
        let hints = diagnose(PAGE_FAULT, true, false);
        assert!(hints[0].text.contains("crash"));
        assert!(!hints[0].text.contains("Steam"));
    }

    #[test]
    fn interrupted_steam_update_says_play_again() {
        let hints = diagnose(NO_LOCATION, true, true);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].text.contains("Chơi lại"));
    }

    #[test]
    fn steam_offline_error_points_to_the_network_check() {
        let hints = diagnose(OFFLINE, true, true);
        assert!(hints[0].text.contains("Kiểm tra mạng"));
        let hints = diagnose("Download failed: http error 0 (cdn.steamstatic.com/client/steam_client_win32)", true, true);
        assert!(hints[0].text.contains("VPN"));
    }

    #[test]
    fn the_more_specific_steam_hint_wins_over_the_generic_one() {
        let both = format!("{OFFLINE}\n{NO_LOCATION}");
        let hints = diagnose(&both, true, true);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].text.contains("Chơi lại"));
    }

    #[test]
    fn names_the_missing_library() {
        let log = r#"err:module:import_dll:Library MSVCP140.dll (which is needed by L"C:\\Game\\game.exe") not found"#;
        let hints = diagnose(log, true, false);
        assert!(hints[0].text.contains("MSVCP140.dll"));
    }

    #[test]
    fn rosetta_and_broken_prefix_are_recognised() {
        assert!(diagnose("zsh: bad CPU type in executable: wine64", true, false)[0]
            .text
            .contains("Rosetta"));
        assert!(diagnose("wine: could not load kernel32.dll, status c0000135", true, false)[0]
            .text
            .contains("tạo một môi trường mới"));
    }

    #[test]
    fn unknown_failure_asks_for_detailed_logging() {
        let hints = diagnose("nothing useful here", true, false);
        assert_eq!(hints.len(), 1);
        assert!(hints[0].text.contains("Ghi chi tiết lỗi"));
    }

    #[test]
    fn clean_run_has_no_hints() {
        assert!(diagnose("WINBRIDGE_OK", false, false).is_empty());
        assert!(diagnose("anything", false, true).is_empty());
    }
}
