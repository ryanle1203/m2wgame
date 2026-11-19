//! Windows Steam inside a bottle: which games it already holds, and how to get new ones.
//!
//! Steam's own window is drawn black under Wine, so M2WGame never drives Steam's UI. Games
//! that are already in the bottle's Steam library are found by reading `appmanifest_*.acf`;
//! new games are downloaded with SteamCMD in a Terminal window where the user types the
//! password (M2WGame never sees it).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::bottle::Bottle;
use crate::paths;

/// Arguments that make Steam start without any of its (black) windows.
pub const STEAM_QUIET_ARGS: &str =
    "-no-cef-sandbox -cef-disable-gpu -cef-single-process -silent -nofriendsui -nochatui";

/// The arguments of a Steam launch rewritten so Steam keeps its windows hidden but opens a
/// debugging port on `port` (only reachable from this Mac). Quiet flags are dropped.
pub fn assist_args(args: &str, port: u16) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut skip_next = false;
    for tok in args.split_whitespace() {
        if skip_next {
            skip_next = false;
            continue;
        }
        match tok {
            "-silent" | "-nofriendsui" | "-nochatui" | "-cef-enable-debugging" => {}
            "-devtools-port" => skip_next = true,
            other => out.push(other.to_string()),
        }
    }
    out.push("-cef-enable-debugging".into());
    out.push("-devtools-port".into());
    out.push(port.to_string());
    out.join(" ")
}

/// Valve's shared runtimes show up as "apps" but are not games.
const NOT_GAMES: [&str; 3] = ["228980", "1007", "1493710"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SteamApp {
    pub appid: String,
    pub name: String,
    pub installdir: String,
    /// Bytes the game occupies (Steam's `SizeOnDisk`).
    pub size: u64,
    /// Fully installed (Steam's StateFlags has the "installed" bit).
    pub installed: bool,
}

/// Read the top-level `"key" "value"` pairs of a Valve KeyValues file (enough for appmanifests).
pub fn parse_acf(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        // `\t"key"\t\t"value"` splits into ["\t", key, "\t\t", value, ""].
        if parts.len() >= 4 && parts[0].trim().is_empty() && !parts[1].is_empty() {
            out.entry(parts[1].to_string()).or_insert_with(|| parts[3].to_string());
        }
    }
    out
}

pub fn parse_app(text: &str) -> Option<SteamApp> {
    let kv = parse_acf(text);
    let appid = kv.get("appid")?.clone();
    if NOT_GAMES.contains(&appid.as_str()) || !appid.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let flags: u32 = kv.get("StateFlags").and_then(|v| v.parse().ok()).unwrap_or(0);
    Some(SteamApp {
        name: kv.get("name").cloned().unwrap_or_else(|| format!("Game {appid}")),
        installdir: kv.get("installdir").cloned().unwrap_or_default(),
        size: kv.get("SizeOnDisk").and_then(|v| v.parse().ok()).unwrap_or(0),
        installed: flags & 4 != 0,
        appid,
    })
}

pub fn steamapps_dir(bottle: &Bottle) -> PathBuf {
    bottle
        .steam_exe()
        .parent()
        .map(|p| p.join("steamapps"))
        .unwrap_or_else(|| bottle.path().join("steamapps"))
}

/// Games found in the bottle's Steam library, sorted by name.
pub fn installed_games(bottle: &Bottle) -> Vec<SteamApp> {
    let Ok(rd) = std::fs::read_dir(steamapps_dir(bottle)) else {
        return Vec::new();
    };
    let mut apps: Vec<SteamApp> = rd
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.starts_with("appmanifest_") && n.ends_with(".acf")
        })
        .filter_map(|e| parse_app(&std::fs::read_to_string(e.path()).ok()?))
        .collect();
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// One game's manifest.
pub fn read_app(bottle: &Bottle, appid: &str) -> Option<SteamApp> {
    if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let text = std::fs::read_to_string(steamapps_dir(bottle).join(format!("appmanifest_{appid}.acf"))).ok()?;
    parse_app(&text)
}

/// Delete a game's files from the bottle's Steam library (its folder in `steamapps/common`, its
/// manifest and its shader cache) to free disk space. Saves are not touched: they live in the
/// Windows profile of the bottle and in Steam Cloud. Returns the bytes Steam said it occupied.
pub fn uninstall(bottle: &Bottle, app: &SteamApp) -> Result<u64, String> {
    let dir_name = app.installdir.as_str();
    if dir_name.is_empty() || dir_name == "." || dir_name == ".." || dir_name.contains('/') || dir_name.contains('\\') {
        return Err(t!("Tên thư mục của game không hợp lệ, đã hủy để an toàn.").into());
    }
    let steamapps = steamapps_dir(bottle);
    let common = steamapps.join("common");
    let dir = common.join(dir_name);
    // Belt and braces: whatever happens, never delete outside this bottle's own library folder.
    if !dir.starts_with(&common) || dir == common || !common.starts_with(bottle.path()) {
        return Err(t!("Đường dẫn game nằm ngoài thư viện Steam của môi trường, đã hủy để an toàn.").into());
    }
    if dir.is_symlink() {
        return Err(t!("Thư mục game là một liên kết (symlink), đã hủy để an toàn.").into());
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| t!("Không xóa được thư mục game: {}", e))?;
    }
    let manifest = steamapps.join(format!("appmanifest_{}.acf", app.appid));
    if manifest.is_file() {
        std::fs::remove_file(&manifest).map_err(|e| t!("Không xóa được file xác nhận: {}", e))?;
    }
    let shaders = steamapps.join("shadercache").join(&app.appid);
    if shaders.is_dir() && !shaders.is_symlink() {
        let _ = std::fs::remove_dir_all(&shaders);
    }
    Ok(app.size)
}

/// Pull the app id out of what a person pastes: a store link, or just the number.
pub fn parse_app_id(input: &str) -> Option<String> {
    let t = input.trim();
    if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        return Some(t.to_string());
    }
    let after = t.split("/app/").nth(1)?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    (!digits.is_empty()).then_some(digits)
}

/// Name and platforms of a store page, from Steam's public store API.
pub fn lookup(appid: &str) -> Result<(String, bool), String> {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={appid}&filters=basic,platforms&l=english");
    let out = Command::new("/usr/bin/curl")
        .args(["-sfL", "--max-time", "20", &url])
        .output()
        .map_err(|e| t!("Không chạy được curl: {}", e))?;
    if !out.status.success() {
        return Err(t!("Không vào được cửa hàng Steam. Kiểm tra mạng (mục Hệ thống).").into());
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| t!("Steam trả về dữ liệu lạ, thử lại sau.").to_string())?;
    let entry = &v[appid];
    if entry["success"] != true {
        return Err(t!("Không tìm thấy game có mã này trên cửa hàng Steam.").into());
    }
    let name = entry["data"]["name"].as_str().unwrap_or("").to_string();
    let windows = entry["data"]["platforms"]["windows"].as_bool().unwrap_or(true);
    if name.is_empty() {
        return Err(t!("Không đọc được tên game.").into());
    }
    Ok((name, windows))
}

pub fn steamcmd_path() -> PathBuf {
    paths::data_dir().join("SteamCMD").join("steamcmd.sh")
}

fn sh_quote(p: &Path) -> String {
    format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
}

/// A shell script that downloads `appid` with SteamCMD and files it into the bottle's Steam
/// library (the way Steam itself would). Meant to run in Terminal: SteamCMD asks for the
/// account password there. `steamcmd` is a parameter so tests can use a stand-in.
pub fn install_script(bottle: &Bottle, appid: &str, steamcmd: &Path) -> String {
    let steamapps = steamapps_dir(bottle);
    let tmp = paths::downloads_dir().join(format!("steamcmd-{appid}"));
    // The messages the script prints (shell variables inside them are expanded by bash).
    let m_no_steamcmd = t!("Chưa có SteamCMD tại: $STEAMCMD");
    let m_enter = t!("Nhấn Enter để đóng...");
    let m_no_steam = t!("Môi trường này chưa cài Steam (không thấy $STEAMAPPS).");
    let m_intro = t!("Tải game {}. SteamCMD sẽ hỏi mật khẩu và mã Steam Guard: hãy nhập tại đây.", appid);
    let m_tip = t!("Mẹo: tắt VPN khi đăng nhập, và duyệt yêu cầu trên điện thoại bằng 'Steam Client'.");
    let m_account = t!("Tên tài khoản Steam: ");
    let m_incomplete = t!("Tải chưa xong hoặc bị lỗi (không thấy file xác nhận). Các file đã tải vẫn ở: $TMP");
    let m_cannot_file = t!("Không thể xếp game vào thư viện (thiếu tên thư mục hoặc đã có sẵn): $STEAMAPPS/common/$DIR");
    let m_kept = t!("Game đã tải vẫn ở: $TMP");
    let m_done = t!("Xong. Quay lại M2WGame, bấm Quét lại trong tab Từ Steam để thêm game.");
    format!(
        r#"#!/bin/bash
# M2WGame: download Steam game {appid} (Windows version) into the Steam library of "{bottle_name}".
set -u
STEAMCMD={steamcmd}
STEAMAPPS={steamapps}
TMP={tmp}
APPID={appid}

if [ ! -x "$STEAMCMD" ]; then
  echo "{m_no_steamcmd}"
  read -r -p "{m_enter}" _; exit 1
fi
if [ ! -d "$STEAMAPPS" ]; then
  echo "{m_no_steam}"
  read -r -p "{m_enter}" _; exit 1
fi

echo "{m_intro}"
echo "{m_tip}"
read -r -p "{m_account}" ACCOUNT
[ -z "$ACCOUNT" ] && exit 1
mkdir -p "$TMP"
"$STEAMCMD" +@sSteamCmdForcePlatformType windows +force_install_dir "$TMP" +login "$ACCOUNT" +app_update "$APPID" validate +quit

ACF="$TMP/steamapps/appmanifest_$APPID.acf"
if [ ! -f "$ACF" ]; then
  echo "{m_incomplete}"
  read -r -p "{m_enter}" _; exit 1
fi
DIR=$(sed -n 's/^[[:space:]]*"installdir"[[:space:]]*"\(.*\)".*/\1/p' "$ACF" | head -1)
if [ -z "$DIR" ] || [ -e "$STEAMAPPS/common/$DIR" ]; then
  echo "{m_cannot_file}"
  echo "{m_kept}"
  read -r -p "{m_enter}" _; exit 1
fi
mkdir -p "$STEAMAPPS/common"
mv "$ACF" "$STEAMAPPS/appmanifest_$APPID.acf" || exit 1
rm -rf "$TMP/steamapps"
mv "$TMP" "$STEAMAPPS/common/$DIR" || exit 1
echo
echo "{m_done}"
read -r -p "{m_enter}" _
"#,
        appid = appid,
        bottle_name = bottle.name.replace('"', ""),
        steamcmd = sh_quote(steamcmd),
        steamapps = sh_quote(&steamapps),
        tmp = sh_quote(&tmp),
        m_no_steamcmd = m_no_steamcmd,
        m_enter = m_enter,
        m_no_steam = m_no_steam,
        m_intro = m_intro,
        m_tip = m_tip,
        m_account = m_account,
        m_incomplete = m_incomplete,
        m_cannot_file = m_cannot_file,
        m_kept = m_kept,
        m_done = m_done,
    )
}

/// Write the install script to the downloads folder (executable) and return its path.
pub fn write_install_script(bottle: &Bottle, appid: &str) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    if !appid.chars().all(|c| c.is_ascii_digit()) || appid.is_empty() {
        return Err(t!("Mã game không hợp lệ.").into());
    }
    if !steamcmd_path().is_file() {
        return Err(t!("Chưa có SteamCMD. Cần file: {}", steamcmd_path().display()));
    }
    if !steamapps_dir(bottle).is_dir() {
        return Err(t!("Môi trường này chưa cài Steam. Vào mục Môi trường để cài Steam trước.").into());
    }
    let dir = paths::downloads_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("install-steam-{appid}.command"));
    std::fs::write(&path, install_script(bottle, appid, &steamcmd_path())).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Move a finished SteamCMD download (`tmp`) into the Steam library the way Steam itself keeps it:
/// game files in `steamapps/common/<installdir>`, manifest in `steamapps/`. Returns the game's name.
pub fn file_into_library(tmp: &Path, steamapps: &Path, appid: &str) -> Result<(String, PathBuf), String> {
    let acf = tmp.join("steamapps").join(format!("appmanifest_{appid}.acf"));
    let text = std::fs::read_to_string(&acf)
        .map_err(|_| t!("SteamCMD báo xong nhưng không thấy file xác nhận của game.").to_string())?;
    let kv = parse_acf(&text);
    let dir = kv.get("installdir").cloned().unwrap_or_default();
    if dir.is_empty() || dir == "." || dir == ".." || dir.contains('/') || dir.contains('\\') {
        return Err(t!("Tên thư mục game trong file xác nhận không hợp lệ.").into());
    }
    let name = kv.get("name").cloned().unwrap_or_else(|| format!("Game {appid}"));
    let dest = steamapps.join("common").join(&dir);
    if dest.exists() {
        return Err(t!("Thư viện Steam đã có thư mục {}. Game đã tải vẫn nằm ở {}.", dir, tmp.display()));
    }
    std::fs::create_dir_all(steamapps.join("common")).map_err(|e| e.to_string())?;
    std::fs::rename(&acf, steamapps.join(format!("appmanifest_{appid}.acf"))).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(tmp.join("steamapps"));
    std::fs::rename(tmp, &dest).map_err(|e| t!("Không chuyển được game vào thư viện: {}", e))?;
    Ok((name, dest))
}

/// The CPU type a Windows program was built for, from its PE header (0x14c = 32-bit x86,
/// 0x8664 = 64-bit x86).
pub fn pe_machine(path: &Path) -> Option<u16> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let mut dos = [0u8; 64];
    f.read_exact(&mut dos).ok()?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    let off = u32::from_le_bytes([dos[0x3c], dos[0x3d], dos[0x3e], dos[0x3f]]) as u64;
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut pe = [0u8; 6];
    f.read_exact(&mut pe).ok()?;
    if &pe[..4] != b"PE\0\0" {
        return None;
    }
    Some(u16::from_le_bytes([pe[4], pe[5]]))
}

/// 32 or 64 for a game folder, judged from its main program (or Unity's crash handler, which
/// comes in a 32- or 64-bit flavour). `None` when it cannot tell.
pub fn game_bits(dir: &Path) -> Option<u32> {
    let mut best: Option<(u64, PathBuf)> = None;
    let (mut crash32, mut crash64) = (false, false);
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if !name.ends_with(".exe") {
            continue;
        }
        if name == "unitycrashhandler32.exe" {
            crash32 = true;
        } else if name == "unitycrashhandler64.exe" {
            crash64 = true;
        }
        const SKIP: [&str; 6] = ["crash", "unins", "redist", "setup", "dotnet", "dxsetup"];
        if SKIP.iter().any(|s| name.contains(s)) {
            continue;
        }
        let size = e.metadata().map(|m| m.len()).unwrap_or(0);
        if best.as_ref().map(|(s, _)| size > *s).unwrap_or(true) {
            best = Some((size, e.path()));
        }
    }
    match (crash32, crash64) {
        (true, false) => return Some(32),
        (false, true) => return Some(64),
        _ => {}
    }
    match pe_machine(&best?.1)? {
        0x014c => Some(32),
        0x8664 | 0xaa64 => Some(64),
        _ => None,
    }
}

/// Where a game's files live inside the bottle's Steam library.
pub fn game_dir(bottle: &Bottle, app: &SteamApp) -> PathBuf {
    steamapps_dir(bottle).join("common").join(&app.installdir)
}

/// Said when a game is 32-bit: M2WGame's engines run those badly on Apple Silicon.
pub fn bits32_warning() -> String {
    t!("Game 32-bit: trên Mac chip Apple, Wine miễn phí thường không chạy được game 32-bit (hay sập ngay lúc mở). Cân nhắc CrossOver hoặc chọn game 64-bit")
}

/// Steam's own log (`logs/console_log.txt`) of the bottle.
pub fn console_log(bottle: &Bottle) -> PathBuf {
    bottle
        .steam_exe()
        .parent()
        .map(|p| p.join("logs").join("console_log.txt"))
        .unwrap_or_else(|| bottle.path().join("console_log.txt"))
}

/// When Steam is stuck on a question that appears in its hidden window, its log says
/// `GameAction [AppID n, ...] : LaunchApp waiting for user response to <step> "<detail>"`.
/// Returns that line and a plain explanation, if it is the latest thing said about `appid`.
pub fn blocked_reason(log: &str, appid: &str) -> Option<(String, String)> {
    let marker = format!("[AppID {appid},");
    let last = log.lines().rev().find(|l| l.contains(&marker) && l.contains("LaunchApp"))?;
    let rest = last.split("waiting for user response to ").nth(1)?;
    let step = rest.split_whitespace().next().unwrap_or("");
    let why = match step {
        "SynchronizingCloud" => {
            t!("đồng bộ dữ liệu đám mây của Steam bị lỗi và Steam đang chờ trả lời trong cửa sổ ẩn. Thường chỉ là lỗi nhất thời. Bấm Dừng rồi Chơi lại").to_string()
        }
        "ShowEula" => {
            t!("Steam đang chờ bạn đồng ý điều khoản của nhà phát hành game. Chơi lại cũng sẽ hỏi lại; hãy bấm \"Mở hộp thoại Steam\" để đọc và xác nhận").to_string()
        }
        other => t!("Steam đang chờ trả lời một câu hỏi ở bước {} trong cửa sổ ẩn. Thử Dừng rồi Chơi lại; nếu vẫn vậy, bấm \"Mở hộp thoại Steam\" để xem Steam đang hỏi gì", other),
    };
    Some((last.to_string(), why))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACF: &str = "\"AppState\"\n{\n\t\"appid\"\t\t\"100200\"\n\t\"name\"\t\t\"Example Game: Deluxe\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"Example Game Deluxe\"\n\t\"InstalledDepots\"\n\t{\n\t\t\"1351081\"\n\t\t{\n\t\t\t\"manifest\"\t\t\"123\"\n\t\t}\n\t}\n}\n";

    #[test]
    fn reads_an_appmanifest() {
        let app = parse_app(ACF).unwrap();
        assert_eq!(app.appid, "100200");
        assert_eq!(app.name, "Example Game: Deluxe");
        assert_eq!(app.installdir, "Example Game Deluxe");
        assert!(app.installed);
        // nested keys do not leak into the top level
        assert!(!parse_acf(ACF).contains_key("manifest") || parse_acf(ACF)["manifest"] == "123");
    }

    #[test]
    fn half_downloaded_and_runtime_apps_are_handled() {
        let partial = ACF.replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"1026\"");
        assert!(!parse_app(&partial).unwrap().installed);
        let redist = ACF.replace("100200", "228980");
        assert!(parse_app(&redist).is_none());
        assert!(parse_app("garbage").is_none());
    }

    #[test]
    fn understands_what_people_paste() {
        assert_eq!(parse_app_id("100200").as_deref(), Some("100200"));
        assert_eq!(parse_app_id("  100200 ").as_deref(), Some("100200"));
        assert_eq!(
            parse_app_id("https://store.steampowered.com/app/100200/Example_Game/").as_deref(),
            Some("100200")
        );
        assert_eq!(parse_app_id("https://store.steampowered.com/app/2371090").as_deref(), Some("2371090"));
        assert_eq!(parse_app_id("example"), None);
        assert_eq!(parse_app_id(""), None);
    }

    /// Runs the generated script with a stand-in for SteamCMD and checks the game ends up
    /// where Steam expects it.
    #[test]
    fn install_script_files_the_game_into_the_steam_library() {
        let _guard = crate::util::ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(format!("m2wgame-steam-{}", crate::util::new_id()));
        std::env::set_var("M2WGAME_HOME", &tmp);
        let bottle = Bottle::new("T");
        let steamapps = steamapps_dir(&bottle);
        std::fs::create_dir_all(&steamapps).unwrap();

        // stand-in steamcmd: creates files + manifest in the force_install_dir (argument after the flag)
        let fake = tmp.join("fake-steamcmd.sh");
        std::fs::write(
            &fake,
            "#!/bin/bash\nwhile [ $# -gt 0 ]; do if [ \"$1\" = \"+force_install_dir\" ]; then D=\"$2\"; fi; shift; done\nmkdir -p \"$D/steamapps\" \"$D/Game_Data\"\necho data > \"$D/Game_Data/a.bin\"\nprintf '\"AppState\"\\n{\\n\\t\"appid\"\\t\\t\"4242\"\\n\\t\"name\"\\t\\t\"Fake Game\"\\n\\t\"StateFlags\"\\t\\t\"4\"\\n\\t\"installdir\"\\t\\t\"Fake Game Dir\"\\n}\\n' > \"$D/steamapps/appmanifest_4242.acf\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let script = install_script(&bottle, "4242", &fake);
        assert!(script.contains("+app_update \"$APPID\" validate"));
        let path = tmp.join("run.sh");
        std::fs::write(&path, script).unwrap();
        let out = Command::new("/bin/bash")
            .arg(&path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                // answers: account name, then Enter for the final pause
                c.stdin.take().unwrap().write_all(b"someone\n\n")?;
                c.wait()
            })
            .unwrap();
        assert!(out.success());

        assert!(steamapps.join("common/Fake Game Dir/Game_Data/a.bin").is_file());
        assert!(!steamapps.join("common/Fake Game Dir/steamapps").exists());
        let apps = installed_games(&bottle);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Fake Game");
        assert!(apps[0].installed);

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn spots_a_launch_that_waits_for_a_hidden_dialog() {
        let log = "[2026-11-19 15:55:41] GameAction [AppID 2228280, ActionID 1] : LaunchApp changed task to SynchronizingCloud with \"\"\n[2026-11-19 15:55:47] GameAction [AppID 2228280, ActionID 1] : LaunchApp waiting for user response to SynchronizingCloud \"syncfailed\"\n[2026-11-19 15:56:00] unrelated line\n";
        let (line, why) = blocked_reason(log, "2228280").unwrap();
        assert!(line.contains("syncfailed"));
        assert!(why.contains("đám mây") && why.contains("Chơi lại"));
        // another game, or a launch that moved on, is not blocked
        assert!(blocked_reason(log, "100200").is_none());
        let moved = format!("{log}[2026-11-19 15:56:05] GameAction [AppID 2228280, ActionID 1] : LaunchApp changed task to Completed with \"\"\n");
        assert!(blocked_reason(&moved, "2228280").is_none());
        let eula = log.replace("SynchronizingCloud \"syncfailed\"", "ShowEula \"\"");
        assert!(blocked_reason(&eula, "2228280").unwrap().1.contains("điều khoản"));
    }

    #[test]
    fn assist_args_swap_the_quiet_flags_for_a_port() {
        let a = assist_args("-no-cef-sandbox -silent -nofriendsui -nochatui -applaunch 5", 40123);
        assert_eq!(a, "-no-cef-sandbox -applaunch 5 -cef-enable-debugging -devtools-port 40123");
        // an old port in the arguments is replaced, not duplicated
        let b = assist_args("-cef-enable-debugging -devtools-port 8080 -applaunch 5", 40124);
        assert_eq!(b, "-applaunch 5 -cef-enable-debugging -devtools-port 40124");
    }

    fn fake_pe(path: &Path, machine: u16, pad: usize) {
        let mut b = vec![0u8; 0x80];
        b[0] = b'M';
        b[1] = b'Z';
        b[0x3c] = 0x80;
        b.extend_from_slice(b"PE\0\0");
        b.extend_from_slice(&machine.to_le_bytes());
        b.extend(std::iter::repeat(0u8).take(pad));
        std::fs::write(path, b).unwrap();
    }

    #[test]
    fn tells_32_bit_games_from_64_bit_ones() {
        let tmp = std::env::temp_dir().join(format!("m2wgame-bits-{}", crate::util::new_id()));
        let g32 = tmp.join("g32");
        let g64 = tmp.join("g64");
        let unity32 = tmp.join("u32");
        let none = tmp.join("none");
        for d in [&g32, &g64, &unity32, &none] {
            std::fs::create_dir_all(d).unwrap();
        }
        fake_pe(&g32.join("Game.exe"), 0x014c, 500);
        fake_pe(&g32.join("unins000.exe"), 0x8664, 9000); // bigger, but an uninstaller: ignored
        fake_pe(&g64.join("Game.exe"), 0x8664, 500);
        // Unity: the crash handler's flavour decides, even if the main exe cannot be read
        std::fs::write(unity32.join("Game.exe"), b"not a pe").unwrap();
        std::fs::write(unity32.join("UnityCrashHandler32.exe"), b"x").unwrap();
        std::fs::write(none.join("Game.exe"), b"not a pe").unwrap();

        assert_eq!(game_bits(&g32), Some(32));
        assert_eq!(game_bits(&g64), Some(64));
        assert_eq!(game_bits(&unity32), Some(32));
        assert_eq!(game_bits(&none), None);
        assert_eq!(game_bits(&tmp.join("missing")), None);
        assert_eq!(pe_machine(&g32.join("Game.exe")), Some(0x014c));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn uninstall_removes_only_that_game() {
        let _guard = crate::util::ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(format!("m2wgame-un-{}", crate::util::new_id()));
        std::env::set_var("M2WGAME_HOME", &tmp);
        let bottle = Bottle::new("T");
        let apps = steamapps_dir(&bottle);
        let acf = |id: &str, dir: &str| {
            format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"name\"\t\t\"G{id}\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"{dir}\"\n\t\"SizeOnDisk\"\t\t\"1234\"\n}}\n")
        };
        for (id, dir) in [("1", "One"), ("2", "Two")] {
            std::fs::create_dir_all(apps.join("common").join(dir).join("data")).unwrap();
            std::fs::write(apps.join("common").join(dir).join("data/f.bin"), b"x").unwrap();
            std::fs::write(apps.join(format!("appmanifest_{id}.acf")), acf(id, dir)).unwrap();
            std::fs::create_dir_all(apps.join("shadercache").join(id)).unwrap();
        }
        // a file outside the library that a bad manifest might point at
        std::fs::write(tmp.join("precious.txt"), b"keep").unwrap();

        let app = read_app(&bottle, "1").unwrap();
        assert_eq!(app.size, 1234);
        assert_eq!(uninstall(&bottle, &app).unwrap(), 1234);
        assert!(!apps.join("common/One").exists());
        assert!(!apps.join("appmanifest_1.acf").exists());
        assert!(!apps.join("shadercache/1").exists());
        // the other game is untouched
        assert!(apps.join("common/Two/data/f.bin").is_file());
        assert!(apps.join("appmanifest_2.acf").is_file());
        assert!(read_app(&bottle, "1").is_none());

        // hostile or broken manifests never reach outside the library
        for bad in ["..", ".", "", "../../..", "a/b"] {
            let mut evil = read_app(&bottle, "2").unwrap();
            evil.installdir = bad.to_string();
            assert!(uninstall(&bottle, &evil).is_err(), "accepted {bad:?}");
        }
        assert!(tmp.join("precious.txt").is_file());
        assert!(apps.join("common/Two").is_dir());
        assert!(read_app(&bottle, "../x").is_none());

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn blocked_launch_and_32_bit_texts_speak_english_when_asked() {
        let log = "[t] GameAction [AppID 7, ActionID 1] : LaunchApp waiting for user response to SynchronizingCloud \"syncfailed\"\n";
        let why = crate::i18n::with_lang(crate::i18n::Lang::En, || blocked_reason(log, "7").unwrap().1);
        assert!(why.contains("cloud sync failed"), "{why}");
        let warn = crate::i18n::with_lang(crate::i18n::Lang::En, bits32_warning);
        assert!(warn.starts_with("32-bit game"), "{warn}");
    }

    #[test]
    fn rejects_bad_ids_before_writing_anything() {
        let b = Bottle::new("T");
        assert!(write_install_script(&b, "12; rm -rf /").is_err());
        assert!(write_install_script(&b, "").is_err());
    }
}
