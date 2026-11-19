//! Turns "press Play" into a real command: builds it, starts it, logs it, stops it.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::{bottle::Bottle, game::Game, paths, runtime::Runtime, util};

/// Everything needed to start a game. Kept separate from `spawn` so the
/// exact command can be shown to the user and unit-tested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Later entries win over earlier ones.
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

impl LaunchPlan {
    /// Human-readable one-liner (for the "Lệnh sẽ chạy" preview and the log header).
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (k, v) in &self.env {
            if k == "PATH" {
                continue;
            }
            parts.push(format!("{k}={}", quote(v)));
        }
        parts.push(quote(&self.program.to_string_lossy()));
        for a in &self.args {
            parts.push(quote(a));
        }
        parts.join(" ")
    }
}

fn quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-:=,+@".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

fn base_env(runtime: &Runtime, bottle: &Bottle) -> Vec<(String, String)> {
    let old_path = std::env::var("PATH").unwrap_or_default();
    vec![
        ("WINEPREFIX".into(), bottle.path().to_string_lossy().into_owned()),
        (
            "PATH".into(),
            format!("{}:{}", runtime.bin_dir().display(), old_path),
        ),
    ]
}

pub fn build_env(runtime: &Runtime, bottle: &Bottle, game: &Game) -> Vec<(String, String)> {
    let s = &game.settings;
    let mut env = base_env(runtime, bottle);
    env.push((
        "WINEDEBUG".into(),
        if s.debug_log { "err+all,fixme-all" } else { "-all" }.into(),
    ));
    if s.msync {
        env.push(("WINEMSYNC".into(), "1".into()));
    }
    if s.esync {
        env.push(("WINEESYNC".into(), "1".into()));
    }
    if s.metal_hud {
        env.push(("MTL_HUD_ENABLED".into(), "1".into()));
    }
    if s.advertise_avx {
        env.push(("ROSETTA_ADVERTISE_AVX".into(), "1".into()));
    }
    env.extend(util::parse_env_lines(&s.extra_env));
    env
}

pub fn plan(runtime: &Runtime, bottle: &Bottle, game: &Game) -> LaunchPlan {
    let mut args = vec![game.exe.to_string_lossy().into_owned()];
    args.extend(util::split_args(&game.args));
    LaunchPlan {
        program: runtime.wine_bin.clone(),
        args,
        env: build_env(runtime, bottle, game),
        cwd: game
            .exe
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(paths::home),
    }
}

pub fn log_path(label: &str) -> PathBuf {
    paths::logs_dir().join(format!("{}-{}.log", util::slug(label), util::now_secs()))
}

/// Newest log written for this game name, so "Log" still works after M2WGame restarts.
pub fn latest_log(label: &str) -> Option<PathBuf> {
    latest_log_in(&paths::logs_dir(), label)
}

fn latest_log_in(dir: &Path, label: &str) -> Option<PathBuf> {
    let prefix = format!("{}-", util::slug(label));
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Exactly "<slug>-<digits>.log": "steam-1.log" must not match "steam-m-c-nh-2.log".
            let stamp = name.strip_prefix(&prefix)?.strip_suffix(".log")?;
            let secs: u64 = stamp.parse().ok()?;
            Some((secs, entry.path()))
        })
        .max_by_key(|(secs, _)| *secs)
        .map(|(_, path)| path)
}

pub fn spawn(plan: &LaunchPlan, log: &Path) -> std::io::Result<Child> {
    use std::os::unix::process::CommandExt;

    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut file = File::create(log)?;
    writeln!(file, "# {}", plan.describe())?;
    writeln!(file, "# cwd: {}", plan.cwd.display())?;
    let err = file.try_clone()?;

    let mut cmd = Command::new(&plan.program);
    cmd.args(&plan.args)
        .current_dir(&plan.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(err))
        // Own process group so Stop can end the game without touching M2WGame.
        .process_group(0);
    for (k, v) in &plan.env {
        cmd.env(k, v);
    }
    cmd.spawn()
}

fn wineserver_cmd(runtime: &Runtime, bottle: &Bottle, arg: &str) -> Command {
    let mut cmd = Command::new(runtime.wineserver());
    cmd.arg(arg).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in base_env(runtime, bottle) {
        cmd.env(k, v);
    }
    cmd
}

/// Process ids that look like Wine programs (Wine renames them to `C:\\...`) or live in the engine folder.
fn wine_candidates(ps_output: &str, bottle_path: &str, runtimes_dir: &str, me: u32) -> Vec<u32> {
    ps_output
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (pid, command) = line.split_once(char::is_whitespace)?;
            let pid: u32 = pid.parse().ok()?;
            let command = command.trim_start();
            let wine_like = command.starts_with("C:\\")
                || command.contains(runtimes_dir)
                || command.contains(bottle_path);
            (pid != me && wine_like).then_some(pid)
        })
        .collect()
}

/// From `lsof -Fpn` output: the processes that have a file open inside the bottle.
fn holders_of(lsof_output: &str, bottle_path: &str) -> Vec<u32> {
    let mut current = 0u32;
    let mut found = Vec::new();
    for line in lsof_output.lines() {
        if let Some(p) = line.strip_prefix('p') {
            current = p.parse().unwrap_or(0);
        } else if let Some(name) = line.strip_prefix('n') {
            if current != 0 && name.contains(bottle_path) && !found.contains(&current) {
                found.push(current);
            }
        }
    }
    found
}

/// Wine programs of this bottle that are still alive (they survive their server and get adopted by launchd).
fn bottle_processes(bottle: &Bottle) -> Vec<u32> {
    let path = bottle.path().to_string_lossy().into_owned();
    let Ok(ps) = Command::new("/bin/ps").args(["-axo", "pid=,command="]).output() else {
        return Vec::new();
    };
    let runtimes = paths::runtimes_dir().to_string_lossy().into_owned();
    let cands = wine_candidates(&String::from_utf8_lossy(&ps.stdout), &path, &runtimes, std::process::id());
    if cands.is_empty() {
        return Vec::new();
    }
    let list = cands.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let Ok(lsof) = Command::new("/usr/sbin/lsof").args(["-nP", "-Fpn", "-p", &list]).output() else {
        return Vec::new();
    };
    holders_of(&String::from_utf8_lossy(&lsof.stdout), &path)
}

fn signal(sig: &str, pids: &[u32]) {
    if pids.is_empty() {
        return;
    }
    let _ = Command::new("/bin/kill")
        .arg(sig)
        .args(pids.iter().map(u32::to_string))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Close every program running in this bottle. Asks Wine politely first (`-k`); after its server is
/// gone, any helper still alive (Steam's browser processes do this) is ended too.
pub fn kill_bottle(runtime: &Runtime, bottle: &Bottle) {
    let _ = wineserver_cmd(runtime, bottle, "-k").status();
    // `-k0` only tests whether the server is still there.
    for _ in 0..12 {
        let alive = wineserver_cmd(runtime, bottle, "-k0")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !alive {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    for round in 0..5 {
        let left = bottle_processes(bottle);
        if left.is_empty() {
            return;
        }
        signal(if round == 0 { "-TERM" } else { "-KILL" }, &left);
        std::thread::sleep(std::time::Duration::from_millis(700));
    }
}

/// Create the Windows environment on disk. Takes a while the first time.
pub fn init_bottle(runtime: &Runtime, bottle: &Bottle) -> Result<(), String> {
    std::fs::create_dir_all(bottle.path()).map_err(|e| t!("Không tạo được thư mục: {}", e))?;

    let mut cmd = Command::new(&runtime.wine_bin);
    cmd.args(["wineboot", "--init"])
        .stdin(Stdio::null())
        // Skip the pop-ups that offer to download Mono / Gecko.
        .env("WINEDLLOVERRIDES", "mscoree,mshtml=")
        .env("WINEDEBUG", "-all");
    for (k, v) in base_env(runtime, bottle) {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| t!("Không chạy được Wine ({}): {}", runtime.wine_bin.display(), e))?;

    // Wait for Wine's background service to finish writing the registry.
    let mut wait = Command::new(runtime.wineserver());
    wait.arg("-w").stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in base_env(runtime, bottle) {
        wait.env(k, v);
    }
    let _ = wait.status();

    if bottle.is_initialized() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: String = stderr.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        Err(t!("Wine không tạo được môi trường.\n{}", tail))
    }
}

pub const STEAM_INSTALLER_URL: &str =
    "https://cdn.cloudflare.steamstatic.com/client/installer/SteamSetup.exe";

/// Download the official Steam installer for Windows (only when the user asks).
pub fn download_steam_installer() -> Result<PathBuf, String> {
    let dir = paths::downloads_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join("SteamSetup.exe");
    let out = Command::new("/usr/bin/curl")
        .args(["-fL", "--max-time", "600", "-o"])
        .arg(&target)
        .arg(STEAM_INSTALLER_URL)
        .output()
        .map_err(|e| t!("Không chạy được curl: {}", e))?;
    if out.status.success() && target.is_file() {
        Ok(target)
    } else {
        Err(t!("Tải Steam thất bại. Kiểm tra kết nối mạng.\n{}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// Plan for running an arbitrary Windows program (installer, Steam...) in a bottle.
pub fn plan_for_exe(runtime: &Runtime, bottle: &Bottle, exe: &Path, args: &[&str]) -> LaunchPlan {
    let mut env = base_env(runtime, bottle);
    env.push(("WINEDEBUG".into(), "-all".into()));
    env.push(("WINEMSYNC".into(), "1".into()));
    env.push(("ROSETTA_ADVERTISE_AVX".into(), "1".into()));
    let mut a = vec![exe.to_string_lossy().into_owned()];
    a.extend(args.iter().map(|s| s.to_string()));
    LaunchPlan {
        program: runtime.wine_bin.clone(),
        args: a,
        env,
        cwd: exe.parent().map(Path::to_path_buf).unwrap_or_else(paths::home),
    }
}

pub fn open_url(url: &str) {
    let _ = Command::new("/usr/bin/open").arg(url).spawn();
}

pub fn reveal(path: &Path) {
    let _ = Command::new("/usr/bin/open").arg(path).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> (Runtime, Bottle, Game) {
        let rt = Runtime {
            name: "Test".into(),
            wine_bin: PathBuf::from("/Applications/Test.app/Contents/Resources/wine/bin/wine64"),
            d3dmetal: true,
            version: None,
        };
        let bottle = Bottle {
            id: "abc".into(),
            name: "B".into(),
            created: 0,
            runtime: None,
        };
        let game = Game::new(
            "G".into(),
            PathBuf::from("/Games/My Game/game.exe"),
            "abc".into(),
            r#"-windowed -name "A B""#.into(),
        );
        (rt, bottle, game)
    }

    fn get<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
        env.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    #[test]
    fn plan_runs_wine_on_the_exe_from_its_own_folder() {
        let (rt, bottle, game) = fixtures();
        let p = plan(&rt, &bottle, &game);
        assert_eq!(p.program, rt.wine_bin);
        assert_eq!(p.args, vec!["/Games/My Game/game.exe", "-windowed", "-name", "A B"]);
        assert_eq!(p.cwd, PathBuf::from("/Games/My Game"));
    }

    #[test]
    fn default_settings_give_the_expected_environment() {
        let (rt, bottle, game) = fixtures();
        let env = build_env(&rt, &bottle, &game);
        assert_eq!(get(&env, "WINEMSYNC"), Some("1"));
        assert_eq!(get(&env, "ROSETTA_ADVERTISE_AVX"), Some("1"));
        assert_eq!(get(&env, "WINEDEBUG"), Some("-all"));
        assert_eq!(get(&env, "WINEESYNC"), None);
        assert_eq!(get(&env, "MTL_HUD_ENABLED"), None);
        assert!(get(&env, "WINEPREFIX").unwrap().ends_with("/abc"));
        assert!(get(&env, "PATH").unwrap().starts_with("/Applications/Test.app/"));
    }

    #[test]
    fn toggles_and_custom_env_apply_and_override() {
        let (rt, bottle, mut game) = fixtures();
        game.settings.metal_hud = true;
        game.settings.debug_log = true;
        game.settings.extra_env = "WINEDEBUG=+loaddll\nFOO=bar".into();
        let env = build_env(&rt, &bottle, &game);
        assert_eq!(get(&env, "MTL_HUD_ENABLED"), Some("1"));
        assert_eq!(get(&env, "WINEDEBUG"), Some("+loaddll"));
        assert_eq!(get(&env, "FOO"), Some("bar"));
    }

    /// End-to-end check that needs a real engine installed on this Mac.
    /// Run with: `cargo test -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_engine_creates_a_bottle_and_runs_a_windows_program() {
        let engines = crate::runtime::detect_all(&[]);
        let rt = engines.first().expect("no engine installed").clone();
        println!("engine: {} ({})", rt.name, rt.wine_bin.display());

        // Work in a throw-away data folder, after the engine was found.
        let tmp = std::env::temp_dir().join(format!("m2wgame-e2e-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &tmp);

        let bottle = Bottle::new("e2e");
        let started = std::time::Instant::now();
        init_bottle(&rt, &bottle).expect("bottle creation failed");
        println!("bottle created in {:?}", started.elapsed());
        assert!(bottle.is_initialized());

        let cmd_exe = bottle.path().join("drive_c/windows/system32/cmd.exe");
        assert!(cmd_exe.is_file(), "cmd.exe missing in bottle");
        let game = Game::new(
            "e2e".into(),
            cmd_exe,
            bottle.id.clone(),
            "/c echo WINBRIDGE_OK".into(),
        );
        let plan = plan(&rt, &bottle, &game);
        println!("command: {}", plan.describe());
        let log = log_path("e2e");
        let mut child = spawn(&plan, &log).expect("spawn failed");

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let status = loop {
            if let Some(s) = child.try_wait().expect("wait failed") {
                break s;
            }
            assert!(std::time::Instant::now() < deadline, "program did not finish in 120 s");
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        let text = util::tail(&log, 64 * 1024);
        println!("exit: {status:?}\n--- log ---\n{text}");
        kill_bottle(&rt, &bottle);
        std::fs::remove_dir_all(&tmp).ok();
        assert!(status.success(), "program failed");
        assert!(text.contains("WINBRIDGE_OK"), "expected output not in log");
    }

    #[test]
    fn finds_leftover_wine_programs_of_one_bottle() {
        let ps = "  101 /sbin/launchd\n  202 C:\\windows\\system32\\winedevice.exe\n  303 C:\\Program Files (x86)\\Steam\\bin\\cef\\steamwebhelper.exe -lang=en\n  404 /Users/x/Library/Application Support/M2WGame/Runtimes/Wine/wine64 steam.exe\n  505 /usr/bin/python3 tool.py\n  606 C:\\windows\\system32\\services.exe\n";
        assert_eq!(
            wine_candidates(ps, "/B/abc", "/Users/x/Library/Application Support/M2WGame/Runtimes", 606),
            vec![202, 303, 404]
        );

        let lsof = "p202\nn/B/abc/drive_c/windows/x\nn/other\np303\nn/B/other-bottle/file\np404\nn/B/abc/user.reg\nn/B/abc/system.reg\n";
        assert_eq!(holders_of(lsof, "/B/abc"), vec![202, 404]);
        assert!(holders_of("", "/B/abc").is_empty());
    }

    #[test]
    fn finds_the_newest_log_of_exactly_this_game() {
        let dir = std::env::temp_dir().join(format!("m2wgame-logs-{}", util::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["steam-100.log", "steam-250.log", "steam-m-c-nh-999.log", "other-300.log", "steam-x.log"] {
            std::fs::write(dir.join(f), "").unwrap();
        }
        assert_eq!(latest_log_in(&dir, "Steam"), Some(dir.join("steam-250.log")));
        assert_eq!(latest_log_in(&dir, "Steam (Mặc định)"), Some(dir.join("steam-m-c-nh-999.log")));
        assert_eq!(latest_log_in(&dir, "Nothing"), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn description_is_copy_pasteable() {
        let (rt, bottle, game) = fixtures();
        let text = plan(&rt, &bottle, &game).describe();
        assert!(text.contains("'/Games/My Game/game.exe'"));
        assert!(text.contains("'A B'"));
        assert!(!text.contains("PATH="));
    }
}
