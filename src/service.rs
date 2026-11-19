//! UI-independent application core.
//!
//! `Core` owns the config, the engines, the running games and the background jobs.
//! Front ends (the SwiftUI app through the C ABI in `ffi.rs`)
//! talk to it with JSON commands (`handle`) and read one JSON snapshot (`state`).
//! Nothing here knows about windows; texts for the user are Vietnamese.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use serde::Serialize;
use serde_json::{json, Value};

use crate::bottle::Bottle;
use crate::config::Config;
use crate::game::Game;
use crate::runtime::{self, Runtime};
use crate::system::{Level, SystemInfo};
use crate::{diagnose, i18n, launcher, net, paths, steam, steamdl, util};

enum Msg {
    BottleReady {
        bottle_id: String,
        result: Result<(), String>,
    },
    SteamInstaller {
        bottle_id: String,
        result: Result<PathBuf, String>,
    },
    /// `wine --version` results: (wine binary, version text).
    Versions(Vec<(PathBuf, Option<String>)>),
    Net(Vec<net::ProbeResult>),
    /// A stop request finished: the game's processes are gone.
    Stopped { id: String, label: String },
    /// A game's files were deleted (name, bytes freed) or that failed.
    Uninstalled { bottle_id: String, appid: String, result: Result<(String, u64), String> },
    /// Result of looking a Steam game up in the store.
    Lookup { appid: String, result: Result<(String, bool), String> },
}

/// Debug ports of Steam sessions opened for the user to answer a hidden dialog: game id -> port.
/// Kept outside `Core` so looking at the window never waits for the core's lock.
pub static ASSIST_PORTS: StdMutex<Option<HashMap<String, u16>>> = StdMutex::new(None);

fn free_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0").ok().and_then(|l| l.local_addr().ok()).map(|a| a.port())
}

/// Commands that only talk to an open Steam port (screenshot, click...). Handled without the core lock.
pub fn handle_steam_ui(req: &Value) -> Option<Value> {
    let cmd = req.get("cmd")?.as_str()?;
    if !cmd.starts_with("steam_ui_") {
        return None;
    }
    let id = arg_str(req, "id").unwrap_or_default();
    let port = ASSIST_PORTS.lock().ok()?.as_ref().and_then(|m| m.get(&id).copied());
    let Some(port) = port else {
        return Some(json!({"ok": false, "error": t!("Cổng xem Steam của game này chưa mở.")}));
    };
    let num = |k: &str| req.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let result: Result<Value, String> = match cmd {
        "steam_ui_shot" => crate::cdp::screenshot(port).map(|d| json!({"ok": true, "image": d})),
        "steam_ui_click" => crate::cdp::click(port, num("x"), num("y")).map(|_| json!({"ok": true})),
        "steam_ui_scroll" => crate::cdp::scroll(port, num("x"), num("y"), num("dy")).map(|_| json!({"ok": true})),
        "steam_ui_text" => crate::cdp::insert_text(port, &arg_str(req, "text").unwrap_or_default()).map(|_| json!({"ok": true})),
        _ => Err(t!("lệnh không hỗ trợ: {}", cmd)),
    };
    Some(result.unwrap_or_else(|e| json!({"ok": false, "error": e})))
}

struct Running {
    /// Set when this session was started with Steam's debug port open (random port).
    assist_port: Option<u16>,
    /// How long Steam's console log was when this game was started (older lines are not ours).
    steam_log_offset: u64,
    child: Child,
    started: Instant,
    label: String,
    bottle_id: String,
}

/// The latest message for the user. `id` grows with every new one so a front end
/// can show each notice once.
#[derive(Clone, Debug, Serialize)]
pub struct Notice {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

#[derive(Clone, Debug, Serialize)]
struct LookupState {
    appid: String,
    busy: bool,
    name: Option<String>,
    windows: bool,
    error: Option<String>,
}

pub struct Core {
    cfg: Config,
    runtimes: Vec<Runtime>,
    sys: SystemInfo,
    running: HashMap<String, Running>,
    /// Games being stopped right now (killing Wine takes a few seconds).
    stopping: HashSet<String>,
    /// Steam games whose launch waits for a hidden dialog: game id -> explanation.
    steam_blocked: HashMap<String, String>,
    /// Games to restart with the debug port as soon as their current run has stopped.
    assist_pending: HashSet<String>,
    /// What Steam last said about a game's launch and when we first saw it.
    steam_watch: HashMap<String, (String, Instant)>,
    last_steam_check: Instant,
    /// Seconds a Steam question may stay unanswered before we warn (a field so tests can shorten it).
    steam_block_after: u64,
    /// Steam games being deleted from disk right now: "bottle/appid".
    uninstalling: HashSet<String>,
    /// 32/64-bit of Steam games already looked up: "bottle/appid" -> bits.
    bits_cache: std::cell::RefCell<HashMap<String, u32>>,
    last_logs: HashMap<String, PathBuf>,
    /// How each game's last run ended (None inside = killed by a signal).
    exits: HashMap<String, Option<i32>>,
    pending_play: HashSet<String>,
    busy_bottles: HashSet<String>,
    busy_steam: HashSet<String>,
    net: Option<Vec<net::ProbeResult>>,
    net_busy: bool,
    lookup: Option<LookupState>,
    download: Option<steamdl::Download>,
    /// Phase we last told the user about, so each question is announced once.
    download_announced: Option<steamdl::Phase>,
    notice: Option<Notice>,
    notice_seq: u64,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    dirty: bool,
}

fn level_str(level: Level) -> &'static str {
    match level {
        Level::Ok => "ok",
        Level::Warn => "warn",
        Level::Bad => "bad",
    }
}

fn net_status_str(status: net::NetStatus) -> &'static str {
    match status {
        net::NetStatus::Ok => "ok",
        net::NetStatus::Blocked => "blocked",
        net::NetStatus::Slow => "slow",
        net::NetStatus::NoDns => "no_dns",
        net::NetStatus::Failed => "failed",
    }
}

fn net_level(status: net::NetStatus) -> Level {
    match status {
        net::NetStatus::Ok => Level::Ok,
        net::NetStatus::Slow => Level::Warn,
        _ => Level::Bad,
    }
}

fn arg_str(req: &Value, key: &str) -> Option<String> {
    req.get(key).and_then(Value::as_str).map(str::to_string)
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    pub fn new() -> Self {
        let _ = paths::ensure_dirs();
        let mut cfg = Config::load();
        cfg.ensure_default_bottle();
        let runtimes = runtime::detect_all(&cfg.custom_runtimes);
        let (tx, rx) = channel();
        let mut core = Core {
            cfg,
            runtimes,
            sys: SystemInfo::detect(),
            running: HashMap::new(),
            stopping: HashSet::new(),
            steam_blocked: HashMap::new(),
            assist_pending: HashSet::new(),
            steam_watch: HashMap::new(),
            last_steam_check: Instant::now(),
            steam_block_after: 45,
            uninstalling: HashSet::new(),
            bits_cache: Default::default(),
            last_logs: HashMap::new(),
            exits: HashMap::new(),
            pending_play: HashSet::new(),
            busy_bottles: HashSet::new(),
            busy_steam: HashSet::new(),
            net: None,
            net_busy: false,
            lookup: None,
            download: None,
            download_announced: None,
            notice: None,
            notice_seq: 0,
            tx,
            rx,
            dirty: true,
        };
        core.fix_active_runtime();
        core.probe_versions();
        // Logs from earlier sessions stay reachable.
        let found: Vec<(String, PathBuf)> = core
            .cfg
            .games
            .iter()
            .filter_map(|g| launcher::latest_log(&g.name).map(|p| (g.id.clone(), p)))
            .collect();
        core.last_logs.extend(found);
        core.persist();
        core
    }

    // ---------- small helpers ----------

    fn active_runtime(&self) -> Option<&Runtime> {
        let path = self.cfg.active_runtime.as_ref()?;
        self.runtimes.iter().find(|r| &r.wine_bin == path)
    }

    /// The engine a bottle really uses: its own choice, or the default one.
    fn runtime_for(&self, bottle: &Bottle) -> Option<&Runtime> {
        bottle
            .runtime
            .as_ref()
            .and_then(|p| self.runtimes.iter().find(|r| &r.wine_bin == p))
            .or_else(|| self.active_runtime())
    }

    fn is_steam_game(&self, id: &str) -> bool {
        self.cfg
            .game(id)
            .and_then(|g| {
                g.exe
                    .file_name()
                    .map(|n| n.to_string_lossy().eq_ignore_ascii_case("steam.exe"))
            })
            .unwrap_or(false)
    }

    /// Plain-language explanation of what the game's last log says.
    fn hints_for(&self, id: &str, failed: bool) -> Vec<diagnose::Hint> {
        let Some(path) = self.last_logs.get(id) else {
            return Vec::new();
        };
        diagnose::diagnose(&util::tail(path, 256 * 1024), failed, self.is_steam_game(id))
    }

    /// Read every engine's version in the background (it takes a moment).
    fn probe_versions(&self) {
        let bins: Vec<PathBuf> = self
            .runtimes
            .iter()
            .filter(|r| r.version.is_none())
            .map(|r| r.wine_bin.clone())
            .collect();
        if bins.is_empty() {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let found: Vec<(PathBuf, Option<String>)> = bins
                .into_iter()
                .map(|b| {
                    let v = runtime::detect_version(&b);
                    (b, v)
                })
                .collect();
            let _ = tx.send(Msg::Versions(found));
        });
    }

    fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.notice_seq += 1;
        self.notice = Some(Notice {
            id: self.notice_seq,
            text: text.into(),
            error,
        });
    }

    fn info(&mut self, text: impl Into<String>) {
        self.notify(text, false);
    }

    fn err(&mut self, text: impl Into<String>) {
        self.notify(text, true);
    }

    /// Keep the chosen engine valid; otherwise pick the best one available.
    fn fix_active_runtime(&mut self) {
        let still_there = self
            .cfg
            .active_runtime
            .as_ref()
            .map(|p| self.runtimes.iter().any(|r| &r.wine_bin == p))
            .unwrap_or(false);
        if !still_there {
            let best = self
                .runtimes
                .iter()
                .find(|r| r.d3dmetal)
                .or_else(|| self.runtimes.first())
                .map(|r| r.wine_bin.clone());
            if best != self.cfg.active_runtime {
                self.cfg.active_runtime = best;
                self.dirty = true;
            }
        }
    }

    fn rescan(&mut self) {
        let known: HashMap<PathBuf, String> = self
            .runtimes
            .iter()
            .filter_map(|r| r.version.clone().map(|v| (r.wine_bin.clone(), v)))
            .collect();
        self.runtimes = runtime::detect_all(&self.cfg.custom_runtimes);
        for r in &mut self.runtimes {
            r.version = known.get(&r.wine_bin).cloned();
        }
        self.sys = SystemInfo::detect();
        self.fix_active_runtime();
        self.probe_versions();
    }

    fn persist(&mut self) {
        if self.dirty {
            self.dirty = false;
            if let Err(e) = self.cfg.save() {
                self.err(t!("Không lưu được cài đặt: {}", e));
            }
        }
    }

    // ---------- background work ----------

    fn start_init(&mut self, bottle_id: &str) {
        let Some(bottle) = self.cfg.bottle(bottle_id).cloned() else {
            return;
        };
        let Some(rt) = self.runtime_for(&bottle).cloned() else {
            self.err(t!("Chưa có động cơ chạy game. Vào mục Động cơ để cài."));
            return;
        };
        if !self.busy_bottles.insert(bottle.id.clone()) {
            return;
        }
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = launcher::init_bottle(&rt, &bottle);
            let _ = tx.send(Msg::BottleReady {
                bottle_id: bottle.id,
                result,
            });
        });
    }

    fn start_steam_download(&mut self, bottle_id: &str) {
        if !self.busy_steam.insert(bottle_id.to_string()) {
            return;
        }
        self.info(t!("Đang tải bộ cài Steam chính thức từ Valve..."));
        let tx = self.tx.clone();
        let id = bottle_id.to_string();
        std::thread::spawn(move || {
            let result = launcher::download_steam_installer();
            let _ = tx.send(Msg::SteamInstaller { bottle_id: id, result });
        });
    }

    fn poll_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::BottleReady { bottle_id, result } => {
                    self.busy_bottles.remove(&bottle_id);
                    let waiting: Vec<String> = self
                        .pending_play
                        .iter()
                        .filter(|gid| {
                            self.cfg
                                .game(gid)
                                .map(|g| g.bottle_id == bottle_id)
                                .unwrap_or(false)
                        })
                        .cloned()
                        .collect();
                    for gid in &waiting {
                        self.pending_play.remove(gid);
                    }
                    match result {
                        Ok(()) => {
                            self.info(t!("Môi trường đã sẵn sàng."));
                            for gid in waiting {
                                self.play(&gid);
                            }
                        }
                        Err(e) => self.err(e),
                    }
                }
                Msg::SteamInstaller { bottle_id, result } => {
                    self.busy_steam.remove(&bottle_id);
                    match result {
                        Ok(path) => self.run_installer(&bottle_id, &path),
                        Err(e) => self.err(e),
                    }
                }
                Msg::Net(results) => {
                    self.net = Some(results);
                    self.net_busy = false;
                }
                Msg::Uninstalled { bottle_id, appid, result } => {
                    self.uninstalling.remove(&format!("{bottle_id}/{appid}"));
                    match result {
                        Ok((name, freed)) => {
                            let marker = format!("-applaunch {appid}");
                            let gone: Vec<String> = self
                                .cfg
                                .games
                                .iter()
                                .filter(|g| g.bottle_id == bottle_id && g.args.contains(&marker))
                                .map(|g| g.id.clone())
                                .collect();
                            self.cfg.games.retain(|g| !gone.contains(&g.id));
                            for id in &gone {
                                self.last_logs.remove(id);
                                self.exits.remove(id);
                            }
                            self.bits_cache.borrow_mut().remove(&format!("{bottle_id}/{appid}"));
                            self.dirty = true;
                            let freed = if freed > 0 { t!(", giải phóng {}", util::human_bytes(freed)) } else { String::new() };
                            self.info(t!("Đã gỡ {} khỏi máy{}.", name, freed));
                        }
                        Err(e) => self.err(e),
                    }
                }
                Msg::Lookup { appid, result } => {
                    // ignore answers for a search the user has since replaced
                    if self.lookup.as_ref().map(|l| l.appid == appid).unwrap_or(false) {
                        self.lookup = Some(match result {
                            Ok((name, windows)) => LookupState { appid, busy: false, name: Some(name), windows, error: None },
                            Err(e) => LookupState { appid, busy: false, name: None, windows: true, error: Some(e) },
                        });
                    }
                }
                Msg::Stopped { id, label } => {
                    self.stopping.remove(&id);
                    self.info(t!("Đã dừng {}.", label));
                    if self.assist_pending.remove(&id) {
                        self.play_with(&id, true);
                    }
                }
                Msg::Versions(found) => {
                    for (bin, version) in found {
                        if let Some(r) = self.runtimes.iter_mut().find(|r| r.wine_bin == bin) {
                            r.version = version.or_else(|| Some(t!("không đọc được").to_string()));
                        }
                    }
                }
            }
        }
    }

    fn run_installer(&mut self, bottle_id: &str, exe: &Path) {
        let Some(bottle) = self.cfg.bottle(bottle_id).cloned() else {
            return;
        };
        let Some(rt) = self.runtime_for(&bottle).cloned() else {
            return;
        };
        let plan = launcher::plan_for_exe(&rt, &bottle, exe, &[]);
        let log = launcher::log_path("steam-setup");
        match launcher::spawn(&plan, &log) {
            Ok(child) => {
                self.running.insert(
                    format!("steam-setup-{bottle_id}"),
                    Running {
                        assist_port: None,
                        steam_log_offset: 0,
                        child,
                        started: Instant::now(),
                        label: t!("Bộ cài Steam").into(),
                        bottle_id: bottle_id.to_string(),
                    },
                );
                self.info(t!("Đã mở bộ cài Steam. Làm theo cửa sổ cài đặt, xong thì quay lại đây bấm Thêm Steam vào thư viện."));
            }
            Err(e) => self.err(t!("Không mở được bộ cài Steam: {}", e)),
        }
    }

    fn poll_running(&mut self) {
        let mut finished: Vec<(String, Option<i32>)> = Vec::new();
        for (id, r) in self.running.iter_mut() {
            if let Ok(Some(status)) = r.child.try_wait() {
                finished.push((id.clone(), status.code()));
            }
        }
        for (id, code) in finished {
            if let Some(r) = self.running.remove(&id) {
                // Steam's updater exits with 42 when it is about to start the new client:
                // that is a restart, not a failure.
                let steam_restart = code == Some(42) && self.is_steam_game(&id);
                let code = if steam_restart { Some(0) } else { code };
                self.exits.insert(id.clone(), code);
                let tip = if code != Some(0) {
                    self.hints_for(&id, true)
                        .into_iter()
                        .next()
                        .map(|h| t!(" Gợi ý: {}", h.text))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                match code {
                    Some(0) if steam_restart => self.info(
                        t!("Steam vừa cập nhật xong và đang tự khởi động lại. Đợi khoảng một phút, cửa sổ Steam sẽ hiện ra. Nếu sau đó không thấy gì, bấm Chơi lần nữa."),
                    ),
                    Some(0) => self.info(t!("{} đã đóng.", r.label)),
                    Some(c) => self.err(t!("{} dừng với mã lỗi {}.{} Bấm Log để xem chi tiết.", r.label, c, tip)),
                    None => self.err(t!("{} bị tắt đột ngột.{} Bấm Log để xem chi tiết.", r.label, tip)),
                }
            }
        }
    }

    /// A Steam game that has not started after Steam asked a question in its hidden window.
    fn check_steam_blocks(&mut self) {
        if self.last_steam_check.elapsed().as_secs() < 3 && self.steam_block_after > 0 {
            return;
        }
        self.last_steam_check = Instant::now();
        let ids: Vec<String> = self.running.keys().cloned().collect();
        for id in ids {
            let Some(game) = self.cfg.game(&id).cloned() else { continue };
            let Some(appid) = game
                .args
                .split("-applaunch ")
                .nth(1)
                .and_then(|r| r.split_whitespace().next())
                .map(str::to_string)
            else {
                continue;
            };
            let (Some(bottle), Some(run)) = (self.cfg.bottle(&game.bottle_id).cloned(), self.running.get(&id)) else {
                continue;
            };
            let offset = run.steam_log_offset;
            let text = std::fs::read(steam::console_log(&bottle))
                .map(|b| {
                    let from = if (b.len() as u64) >= offset { offset as usize } else { 0 };
                    String::from_utf8_lossy(&b[from..]).into_owned()
                })
                .unwrap_or_default();
            match steam::blocked_reason(&text, &appid) {
                Some((line, why)) => {
                    let first_seen = match self.steam_watch.get(&id) {
                        Some((l, t)) if *l == line => *t,
                        _ => {
                            let now = Instant::now();
                            self.steam_watch.insert(id.clone(), (line.clone(), now));
                            now
                        }
                    };
                    // A question that stays unanswered for this long is not going to answer itself.
                    if first_seen.elapsed().as_secs() >= self.steam_block_after && !self.steam_blocked.contains_key(&id) {
                        self.steam_blocked.insert(id.clone(), why.clone());
                        self.err(format!("{}: {why}.", game.name));
                    }
                }
                None => {
                    self.steam_watch.remove(&id);
                    self.steam_blocked.remove(&id);
                }
            }
        }
        let running: Vec<String> = self.running.keys().cloned().collect();
        self.steam_blocked.retain(|id, _| running.contains(id));
        self.steam_watch.retain(|id, _| running.contains(id));
    }

    /// Tell the user when a download needs them or has ended (once per change).
    fn poll_download(&mut self) {
        let Some(job) = self.download.as_ref().map(|d| d.snapshot()) else {
            return;
        };
        if self.download_announced == Some(job.phase) {
            return;
        }
        self.download_announced = Some(job.phase);
        use steamdl::Phase::*;
        match job.phase {
            NeedPassword | NeedCode | WaitingMobile => {
                self.info(t!("Tải game: {} Mở Thêm game > Từ Steam để trả lời.", job.message));
            }
            Done => self.info(job.message.clone()),
            Failed => self.err(t!("Tải game không thành công: {}", job.message)),
            _ => {}
        }
    }

    /// Call regularly: collects finished background work and saves changes.
    pub fn tick(&mut self) {
        if let Ok(mut m) = ASSIST_PORTS.lock() {
            *m = Some(self.running.iter().filter_map(|(id, r)| r.assist_port.map(|p| (id.clone(), p))).collect());
        }
        self.check_steam_blocks();
        self.poll_download();
        self.poll_messages();
        self.poll_running();
        self.persist();
    }

    // ---------- actions ----------

    fn play(&mut self, game_id: &str) {
        self.play_with(game_id, false);
    }

    /// Start a game. With `assist` (Steam games only) Steam keeps its windows hidden but opens a
    /// debugging port on a random free port, so M2WGame can show Steam's dialogs to the user.
    fn play_with(&mut self, game_id: &str, assist: bool) {
        let Some(mut game) = self.cfg.game(game_id).cloned() else {
            return;
        };
        let mut assist_port = None;
        if assist {
            let Some(port) = free_port() else {
                self.err(t!("Không tìm được cổng trống để mở hộp thoại Steam."));
                return;
            };
            game.args = steam::assist_args(&game.args, port);
            assist_port = Some(port);
        }
        let Some(bottle) = self.cfg.bottle(&game.bottle_id).cloned() else {
            self.err(t!("Game này trỏ tới một môi trường đã bị xóa. Mở Cài đặt của game và chọn lại môi trường."));
            return;
        };
        let Some(rt) = self.runtime_for(&bottle).cloned() else {
            self.err(t!("Chưa có động cơ chạy game. Vào mục Động cơ để cài."));
            return;
        };
        if game.exe_missing() {
            self.err(t!("Không tìm thấy file game. Ổ đĩa có thể chưa cắm, hoặc game đã bị di chuyển."));
            return;
        }
        if self.stopping.contains(&game.id) {
            self.info(t!("Game đang được dừng, đợi vài giây rồi bấm Chơi lại."));
            return;
        }
        if self.running.contains_key(&game.id) {
            return;
        }
        if !bottle.is_initialized() {
            self.pending_play.insert(game.id.clone());
            self.info(t!("Lần đầu dùng môi trường này nên cần khởi tạo (vài phút). Game sẽ tự chạy khi xong."));
            self.start_init(&bottle.id);
            return;
        }
        let plan = launcher::plan(&rt, &bottle, &game);
        let log = launcher::log_path(&game.name);
        match launcher::spawn(&plan, &log) {
            Ok(child) => {
                self.exits.remove(&game.id);
                self.last_logs.insert(game.id.clone(), log);
                let steam_log_offset = std::fs::metadata(steam::console_log(&bottle)).map(|m| m.len()).unwrap_or(0);
                self.steam_watch.remove(&game.id);
                self.steam_blocked.remove(&game.id);
                self.running.insert(
                    game.id.clone(),
                    Running {
                        assist_port,
                        steam_log_offset,
                        child,
                        started: Instant::now(),
                        label: game.name.clone(),
                        bottle_id: bottle.id.clone(),
                    },
                );
                self.info(t!("Đang mở {}. Lần đầu có thể mất 1-2 phút.", game.name));
            }
            Err(e) => self.err(t!("Không chạy được game: {}", e)),
        }
    }

    /// Stop a game without freezing the window: the slow part (killing Wine) runs in the background.
    fn stop(&mut self, id: &str) {
        self.pending_play.remove(id);
        let Some(mut r) = self.running.remove(id) else {
            return;
        };
        let target = self.cfg.bottle(&r.bottle_id).cloned().and_then(|b| {
            let rt = self.runtime_for(&b).cloned()?;
            Some((rt, b))
        });
        // Steam must be asked to quit, not killed: after a hard kill it keeps a "game still
        // running" cloud session and waits for an answer in its (invisible) window next time.
        let steam_exe = if self.is_steam_game(id) {
            self.cfg.game(id).map(|g| g.exe.clone())
        } else {
            None
        };
        self.stopping.insert(id.to_string());
        let tx = self.tx.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            if let (Some(exe), Some((rt, bottle))) = (&steam_exe, &target) {
                let plan = launcher::plan_for_exe(rt, bottle, exe, &["-shutdown"]);
                if let Ok(mut asker) = launcher::spawn(&plan, &launcher::log_path("steam-shutdown")) {
                    // Give Steam (and the game inside it) up to 40 s to close and sync saves.
                    for _ in 0..160 {
                        if matches!(r.child.try_wait(), Ok(Some(_))) {
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                    let _ = asker.kill();
                    let _ = asker.wait();
                }
            }
            let _ = r.child.kill();
            if let Some((rt, bottle)) = &target {
                launcher::kill_bottle(rt, bottle);
            }
            let _ = r.child.wait();
            let _ = tx.send(Msg::Stopped { id, label: r.label });
        });
    }

    fn add_steam(&mut self, bottle_id: &str) {
        let Some(bottle) = self.cfg.bottle(bottle_id).cloned() else {
            return;
        };
        let exe = bottle.steam_exe();
        if self.cfg.games.iter().any(|g| g.exe == exe) {
            self.info(t!("Steam đã có trong thư viện."));
            return;
        }
        // -no-cef-sandbox: Steam's built-in browser does not start in Wine's sandbox.
        // -cef-disable-gpu: without it the browser's GPU process crashes on macOS and
        // Steam's window stays hidden (tested on Wine 11.21: window appears with it).
        self.cfg.games.push(Game::new(
            format!("Steam ({})", bottle.name),
            exe,
            bottle.id,
            "-no-cef-sandbox -cef-disable-gpu".into(),
        ));
        self.dirty = true;
        self.info(t!("Đã thêm Steam vào thư viện. Nếu Steam báo lỗi hoặc màn hình đen, mở Cài đặt của nó để thử tùy chọn khác."));
    }

    fn create_bottle(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            self.err(t!("Hãy đặt tên cho môi trường mới."));
            return Err(t!("Hãy đặt tên cho môi trường mới.").into());
        }
        self.cfg.bottles.push(Bottle::new(name));
        self.dirty = true;
        Ok(())
    }

    fn set_bottle_runtime(&mut self, id: &str, rt: Option<PathBuf>) {
        let mut was_ready = false;
        if let Some(b) = self.cfg.bottles.iter_mut().find(|b| b.id == id) {
            was_ready = b.is_initialized();
            b.runtime = rt;
            self.dirty = true;
        }
        if was_ready {
            self.info(t!("Đã đổi động cơ. Môi trường này đã được tạo bằng động cơ khác, nếu game lỗi hãy tạo một môi trường mới cho động cơ này."));
        }
    }

    fn add_runtime(&mut self, path: PathBuf) {
        let ok = matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some("wine64") | Some("wine")
        );
        if !ok {
            self.err(t!("File này không phải wine64 hoặc wine. Thường nằm trong thư mục bin của động cơ."));
        } else if !self.cfg.custom_runtimes.contains(&path) {
            self.cfg.custom_runtimes.push(path.clone());
            self.rescan();
            self.cfg.active_runtime = Some(path);
            self.dirty = true;
            self.info(t!("Đã thêm động cơ."));
        }
    }

    fn start_net_check(&mut self) {
        if self.net_busy {
            return;
        }
        self.net_busy = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let results = net::run_all();
            let _ = tx.send(Msg::Net(results));
        });
    }

    fn remove_game(&mut self, id: &str) {
        self.stop(id);
        self.cfg.games.retain(|g| g.id != id);
        self.last_logs.remove(id);
        self.exits.remove(id);
        self.dirty = true;
        self.info(t!("Đã xóa khỏi thư viện. File game trên máy vẫn còn nguyên."));
    }

    fn remove_bottle(&mut self, id: &str) -> Result<(), String> {
        let active = self.busy_bottles.contains(id)
            || self.busy_steam.contains(id)
            || self.running.values().any(|r| r.bottle_id == id)
            || !self.stopping.is_empty();
        if active {
            let msg = t!("Môi trường này đang được dùng (game đang chạy, đang dừng hoặc đang khởi tạo). Hãy đợi xong hoặc dừng game trước.").to_string();
            self.err(msg.clone());
            return Err(msg);
        }
        let in_use = self.cfg.games.iter().filter(|g| g.bottle_id == id).count();
        if in_use > 0 {
            let msg = t!("Còn {} game đang dùng môi trường này. Hãy chuyển hoặc xóa các game đó trước.", in_use);
            self.err(msg.clone());
            return Err(msg);
        }
        if self.cfg.bottles.len() <= 1 {
            self.err(t!("Cần giữ ít nhất một môi trường."));
            return Err(t!("Cần giữ ít nhất một môi trường.").into());
        }
        let Some(bottle) = self.cfg.bottle(id).cloned() else {
            return Err(t!("Không tìm thấy môi trường.").into());
        };
        let dir = bottle.path();
        // Safety: only ever delete inside M2WGame's own Bottles folder.
        if !dir.starts_with(paths::bottles_dir()) || dir == paths::bottles_dir() {
            let msg = t!("Đường dẫn môi trường không hợp lệ, đã hủy để an toàn.").to_string();
            self.err(msg.clone());
            return Err(msg);
        }
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                let msg = t!("Không xóa được thư mục môi trường: {}", e);
                self.err(msg.clone());
                return Err(msg);
            }
        }
        self.cfg.bottles.retain(|b| b.id != id);
        self.dirty = true;
        self.info(t!("Đã xóa môi trường {}.", bottle.name));
        Ok(())
    }

    /// 32 or 64 for a Steam game installed in `bottle` (cached once known).
    fn bits_of(&self, bottle: &Bottle, appid: &str) -> Option<u32> {
        let key = format!("{}/{appid}", bottle.id);
        if let Some(b) = self.bits_cache.borrow().get(&key) {
            return Some(*b);
        }
        let app = steam::installed_games(bottle).into_iter().find(|a| a.appid == appid && a.installed)?;
        let bits = steam::game_bits(&steam::game_dir(bottle, &app))?;
        self.bits_cache.borrow_mut().insert(key, bits);
        Some(bits)
    }

    fn applaunch_id(args: &str) -> Option<String> {
        args.split("-applaunch ").nth(1).and_then(|r| r.split_whitespace().next()).map(str::to_string)
    }

    // ---------- snapshot ----------

    /// Everything a front end needs to draw the whole window.
    pub fn state(&self) -> Value {
        let games: Vec<Value> = self
            .cfg
            .games
            .iter()
            .map(|g| {
                json!({
                    "id": g.id,
                    "name": g.name,
                    "exe": g.exe,
                    "bottle_id": g.bottle_id,
                    "bottle_name": self.cfg.bottle(&g.bottle_id).map(|b| b.name.clone()),
                    "args": g.args,
                    "settings": g.settings,
                    "exe_missing": g.exe_missing(),
                    "is_steam": self.is_steam_game(&g.id),
                    "running_secs": self.running.get(&g.id).map(|r| r.started.elapsed().as_secs()),
                    "waiting": self.pending_play.contains(&g.id),
                    "stopping": self.stopping.contains(&g.id),
                    "steam_blocked": self.steam_blocked.get(&g.id),
                    "steam_appid": Self::applaunch_id(&g.args),
                    "size_bytes": self.cfg.bottle(&g.bottle_id).and_then(|b| {
                        Self::applaunch_id(&g.args).and_then(|id| steam::read_app(b, &id)).map(|a| a.size)
                    }),
                    "uninstalling": Self::applaunch_id(&g.args)
                        .map(|id| self.uninstalling.contains(&format!("{}/{id}", g.bottle_id)))
                        .unwrap_or(false),
                    "bits": self
                        .cfg
                        .bottle(&g.bottle_id)
                        .and_then(|b| Self::applaunch_id(&g.args).and_then(|id| self.bits_of(b, &id))),
                    "assist": self.running.get(&g.id).map(|r| r.assist_port.is_some()).unwrap_or(false),
                    "has_log": self.last_logs.contains_key(&g.id),
                })
            })
            .collect();

        let bottles: Vec<Value> = self
            .cfg
            .bottles
            .iter()
            .map(|b| {
                let rt = self.runtime_for(b);
                json!({
                    "id": b.id,
                    "name": b.name,
                    "path": b.path(),
                    "initialized": b.is_initialized(),
                    "busy": self.busy_bottles.contains(&b.id),
                    "runtime": b.runtime,
                    "effective_runtime": rt.map(|r| r.name.clone()),
                    "effective_old": rt.map(|r| r.is_old()).unwrap_or(false),
                    "steam_installed": b.steam_exe().is_file(),
                    "steam_in_library": self.cfg.games.iter().any(|g| g.exe == b.steam_exe()),
                    "installing_steam": self.busy_steam.contains(&b.id)
                        || self.running.contains_key(&format!("steam-setup-{}", b.id)),
                    "game_count": self.cfg.games.iter().filter(|g| g.bottle_id == b.id).count(),
                })
            })
            .collect();

        let runtimes: Vec<Value> = self
            .runtimes
            .iter()
            .map(|r| {
                json!({
                    "path": r.wine_bin,
                    "name": r.name,
                    "d3dmetal": r.d3dmetal,
                    "version": r.version,
                    "old": r.is_old(),
                    "active": self.cfg.active_runtime.as_ref() == Some(&r.wine_bin),
                })
            })
            .collect();

        let has_rt = self.active_runtime().is_some();
        let checks: Vec<Value> = self
            .sys
            .checks(has_rt)
            .iter()
            .map(|c| json!({"label": c.label, "detail": c.detail, "level": level_str(c.level)}))
            .collect();

        let net_json = json!({
            "busy": self.net_busy,
            "results": self.net.as_ref().map(|rs| {
                rs.iter()
                    .map(|r| {
                        json!({
                            "id": r.id,
                            "label": r.label,
                            "why": r.why,
                            "status": net_status_str(r.status),
                            "level": level_str(net_level(r.status)),
                            "detail": r.detail,
                        })
                    })
                    .collect::<Vec<_>>()
            }),
            "advice": self.net.as_ref().map(|rs| {
                net::advice(rs)
                    .into_iter()
                    .map(|(l, t)| json!({"level": level_str(l), "text": t}))
                    .collect::<Vec<_>>()
            }),
        });

        let steam_games: Vec<Value> = self
            .cfg
            .bottles
            .iter()
            .filter(|b| b.steam_exe().is_file())
            .flat_map(|b| {
                steam::installed_games(b).into_iter().map(move |a| (b, a))
            })
            .map(|(b, a)| {
                let marker = format!("-applaunch {}", a.appid);
                json!({
                    "bottle_id": b.id,
                    "bottle_name": b.name,
                    "appid": a.appid,
                    "name": a.name,
                    "installed": a.installed,
                    "size_bytes": a.size,
                    "uninstalling": self.uninstalling.contains(&format!("{}/{}", b.id, a.appid)),
                    "bits": if a.installed { self.bits_of(b, &a.appid) } else { None },
                    "in_library": self.cfg.games.iter().any(|g| g.bottle_id == b.id && g.args.contains(&marker)),
                })
            })
            .collect();

        json!({
            "ok": true,
            "language": i18n::code(),
            "games": games,
            "steam_games": steam_games,
            "lookup": self.lookup,
            "download": self.download.as_ref().map(|d| d.snapshot()),
            "steam_account": self.cfg.steam_account,
            "steamcmd_ready": steam::steamcmd_path().is_file(),
            "bottles": bottles,
            "runtimes": runtimes,
            "default_runtime": self.cfg.active_runtime,
            "system": {
                "ready": self.sys.ready(has_rt),
                "checks": checks,
            },
            "net": net_json,
            "running_count": self.running.len(),
            "notice": self.notice,
            "home": paths::home(),
            "logs_dir": paths::logs_dir(),
        })
    }

    // ---------- commands ----------

    /// Run one JSON command and return a JSON answer: `{"ok":true,...}` or `{"ok":false,"error":"..."}`.
    pub fn handle(&mut self, req: &Value) -> Value {
        self.tick();
        let cmd = req.get("cmd").and_then(Value::as_str).unwrap_or("");
        let result = self.dispatch(cmd, req);
        self.tick();
        match result {
            Ok(Value::Null) => json!({"ok": true}),
            Ok(v) => v,
            Err(e) => json!({"ok": false, "error": e}),
        }
    }

    fn dispatch(&mut self, cmd: &str, req: &Value) -> Result<Value, String> {
        let id = || arg_str(req, "id").ok_or_else(|| t!("thiếu id").to_string());
        match cmd {
            "state" => return Ok(self.state()),
            "play" => self.play(&id()?),
            "stop" => self.stop(&id()?),
            "add_game" => {
                let name = arg_str(req, "name").unwrap_or_default();
                let exe = arg_str(req, "exe").unwrap_or_default();
                let bottle_id = arg_str(req, "bottle_id").unwrap_or_default();
                if name.trim().is_empty() || exe.is_empty() || self.cfg.bottle(&bottle_id).is_none() {
                    return Err(t!("Cần có tên, file .exe và môi trường hợp lệ.").into());
                }
                self.cfg.games.push(Game::new(
                    name.trim().to_string(),
                    PathBuf::from(exe),
                    bottle_id,
                    arg_str(req, "args").unwrap_or_default().trim().to_string(),
                ));
                self.dirty = true;
                self.info(t!("Đã thêm game. Bấm Chơi để chạy."));
            }
            "update_game" => {
                let game: Game = serde_json::from_value(req.get("game").cloned().unwrap_or(Value::Null))
                    .map_err(|e| t!("game không hợp lệ: {}", e))?;
                let Some(slot) = self.cfg.games.iter_mut().find(|g| g.id == game.id) else {
                    return Err(t!("Không tìm thấy game.").into());
                };
                let changed = *slot != game;
                *slot = game;
                self.dirty = true;
                let id = slot.id.clone();
                if changed && (self.running.contains_key(&id) || self.stopping.contains(&id)) {
                    self.info(t!("Đã lưu. Game đang chạy nên thay đổi chỉ có hiệu lực ở lần chạy sau: bấm Dừng rồi Chơi lại."));
                }
            }
            "remove_game" => self.remove_game(&id()?),
            "create_bottle" => self.create_bottle(&arg_str(req, "name").unwrap_or_default())?,
            "remove_bottle" => self.remove_bottle(&id()?)?,
            "rename_bottle" => {
                let name = arg_str(req, "name").unwrap_or_default();
                if name.trim().is_empty() {
                    return Err(t!("Tên không được để trống.").into());
                }
                let id = id()?;
                if let Some(b) = self.cfg.bottles.iter_mut().find(|b| b.id == id) {
                    b.name = name.trim().to_string();
                    self.dirty = true;
                }
            }
            "init_bottle" => self.start_init(&id()?),
            "set_bottle_runtime" => {
                let rt = req
                    .get("runtime")
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                self.set_bottle_runtime(&id()?, rt);
            }
            "install_steam" => {
                let bottle_id = arg_str(req, "bottle_id").ok_or(t!("thiếu bottle_id"))?;
                self.start_steam_download(&bottle_id);
            }
            "add_steam" => {
                let bottle_id = arg_str(req, "bottle_id").ok_or(t!("thiếu bottle_id"))?;
                self.add_steam(&bottle_id);
            }
            "use_runtime" => {
                let path = arg_str(req, "path").ok_or(t!("thiếu path"))?;
                self.cfg.active_runtime = Some(PathBuf::from(path));
                self.dirty = true;
            }
            "add_runtime" => {
                let path = arg_str(req, "path").ok_or(t!("thiếu path"))?;
                self.add_runtime(PathBuf::from(path));
            }
            "rescan" => {
                self.rescan();
                let n = self.runtimes.len();
                self.info(t!("Đã quét lại. Tìm thấy {} động cơ.", n));
            }
            "check_net" => self.start_net_check(),
            "steam_lookup" => {
                let input = arg_str(req, "input").unwrap_or_default();
                let Some(appid) = steam::parse_app_id(&input) else {
                    return Err(t!("Hãy dán đường dẫn trang game trên cửa hàng Steam (có chữ /app/ và một dãy số) hoặc chỉ mã số của game.").into());
                };
                self.lookup = Some(LookupState { appid: appid.clone(), busy: true, name: None, windows: true, error: None });
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result = steam::lookup(&appid);
                    let _ = tx.send(Msg::Lookup { appid, result });
                });
            }
            "steam_lookup_clear" => self.lookup = None,
            "set_language" => {
                // The window picks the language (its own setting, or the system's); every text the
                // core produces from now on uses it.
                let code = arg_str(req, "lang").unwrap_or_default();
                i18n::set_lang(&code);
                return Ok(json!({"ok": true, "language": i18n::code()}));
            }
            "steam_uninstall" => {
                let bottle_id = arg_str(req, "bottle_id").ok_or(t!("thiếu bottle_id"))?;
                let appid = arg_str(req, "appid").ok_or(t!("thiếu appid"))?;
                let bottle = self.cfg.bottle(&bottle_id).cloned().ok_or(t!("Không tìm thấy môi trường."))?;
                let app = steam::read_app(&bottle, &appid).ok_or(t!("Steam trong môi trường này không có game đó."))?;
                // Steam and the game keep files open while they run: wait until everything of this bottle has stopped.
                let busy = self.running.values().any(|r| r.bottle_id == bottle_id)
                    || !self.stopping.is_empty()
                    || self.busy_steam.contains(&bottle_id)
                    || self.download.as_ref().map(|d| {
                        let j = d.snapshot();
                        !j.phase.finished() && j.bottle_id == bottle_id
                    }).unwrap_or(false);
                if busy {
                    return Err(t!("Steam hoặc một game của môi trường này đang chạy. Bấm Dừng rồi gỡ game.").into());
                }
                let key = format!("{bottle_id}/{appid}");
                if !self.uninstalling.insert(key) {
                    return Ok(json!({"ok": true}));
                }
                self.info(t!("Đang gỡ {}…", app.name));
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result = steam::uninstall(&bottle, &app).map(|freed| (app.name.clone(), freed));
                    let _ = tx.send(Msg::Uninstalled { bottle_id, appid, result });
                });
            }
            "steam_assist_start" => {
                let id = id()?;
                if !self.is_steam_game(&id) {
                    return Err(t!("Chỉ game chạy qua Steam mới có hộp thoại Steam.").into());
                }
                if self.running.get(&id).map(|r| r.assist_port.is_some()).unwrap_or(false) {
                    return Ok(json!({"ok": true}));
                }
                self.info(t!("Đang mở lại Steam để hiện hộp thoại cho bạn (khoảng 1 phút). Cổng xem chỉ mở trong lần chạy này."));
                if self.running.contains_key(&id) {
                    self.assist_pending.insert(id.clone());
                    self.stop(&id);
                } else if self.stopping.contains(&id) {
                    self.assist_pending.insert(id);
                } else {
                    self.play_with(&id, true);
                }
            }
            "steam_assist_relaunch" => {
                // After a dialog was answered Steam may not carry on by itself: ask it again to start the game.
                let id = id()?;
                let game = self.cfg.game(&id).cloned().ok_or(t!("Không tìm thấy game."))?;
                let appid = game
                    .args
                    .split("-applaunch ")
                    .nth(1)
                    .and_then(|r| r.split_whitespace().next())
                    .map(str::to_string)
                    .ok_or(t!("Game này không phải game Steam có mã."))?;
                let bottle = self.cfg.bottle(&game.bottle_id).cloned().ok_or(t!("Không tìm thấy môi trường."))?;
                let rt = self.runtime_for(&bottle).cloned().ok_or(t!("Chưa có động cơ."))?;
                let plan = launcher::plan_for_exe(&rt, &bottle, &bottle.steam_exe(), &["-applaunch", &appid]);
                let mut child = launcher::spawn(&plan, &launcher::log_path("steam-relaunch")).map_err(|e| e.to_string())?;
                std::thread::spawn(move || {
                    for _ in 0..120 {
                        if matches!(child.try_wait(), Ok(Some(_))) {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                });
                self.steam_watch.remove(&id);
                self.steam_blocked.remove(&id);
                self.info(t!("Đã nhắc Steam mở lại game."));
            }
            "steam_dl_start" => {
                if self.download.as_ref().map(|d| !d.snapshot().phase.finished()).unwrap_or(false) {
                    return Err(t!("Đang có một game khác đang tải. Đợi xong hoặc hủy nó trước.").into());
                }
                let bottle_id = arg_str(req, "bottle_id").ok_or(t!("thiếu bottle_id"))?;
                let appid = arg_str(req, "appid").ok_or(t!("thiếu appid"))?;
                let account = arg_str(req, "account").unwrap_or_default().trim().to_string();
                if account.is_empty() || account.contains(char::is_whitespace) {
                    return Err(t!("Hãy nhập tên tài khoản Steam (tên đăng nhập, không có dấu cách).").into());
                }
                if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) {
                    return Err(t!("Mã game không hợp lệ.").into());
                }
                let bottle = self.cfg.bottle(&bottle_id).cloned().ok_or(t!("Không tìm thấy môi trường."))?;
                if !steam::steamcmd_path().is_file() {
                    return Err(t!("Chưa có SteamCMD. Cần file: {}", steam::steamcmd_path().display()));
                }
                let steamapps = steam::steamapps_dir(&bottle);
                if !steamapps.is_dir() {
                    return Err(t!("Môi trường này chưa cài Steam. Vào mục Môi trường để cài Steam trước.").into());
                }
                if steam::installed_games(&bottle).iter().any(|a| a.appid == appid) {
                    return Err(t!("Game này đã có trong Steam của môi trường này.").into());
                }
                let name = self.lookup.as_ref().filter(|l| l.appid == appid).and_then(|l| l.name.clone());
                let tmp = paths::downloads_dir().join(format!("steamcmd-{appid}"));
                let dl = steamdl::Download::start(
                    &steam::steamcmd_path(),
                    &account,
                    steamdl::Job::new(&appid, &bottle_id, name),
                    tmp,
                    steamapps,
                )?;
                self.download = Some(dl);
                self.download_announced = None;
                if self.cfg.steam_account != account {
                    self.cfg.steam_account = account;
                    self.dirty = true;
                }
            }
            "steam_dl_input" => {
                let text = arg_str(req, "text").unwrap_or_default();
                if text.is_empty() {
                    return Err(t!("Chưa nhập gì.").into());
                }
                let dl = self.download.as_ref().ok_or(t!("Không có game nào đang tải."))?;
                dl.send_input(&text)?;
                self.download_announced = None;
            }
            "steam_dl_cancel" => {
                if let Some(dl) = &self.download {
                    dl.cancel();
                }
            }
            "steam_dl_clear" => {
                if self.download.as_ref().map(|d| d.snapshot().phase.finished()).unwrap_or(true) {
                    self.download = None;
                    self.download_announced = None;
                }
            }
            "add_steam_game" => {
                let bottle_id = arg_str(req, "bottle_id").ok_or(t!("thiếu bottle_id"))?;
                let appid = arg_str(req, "appid").ok_or(t!("thiếu appid"))?;
                let bottle = self.cfg.bottle(&bottle_id).cloned().ok_or(t!("Không tìm thấy môi trường."))?;
                let app = steam::installed_games(&bottle)
                    .into_iter()
                    .find(|a| a.appid == appid)
                    .ok_or(t!("Steam trong môi trường này chưa có game đó."))?;
                if !app.installed {
                    return Err(t!("Game này chưa tải xong trong Steam. Đợi tải xong rồi thêm.").into());
                }
                let marker = format!("-applaunch {appid}");
                if self.cfg.games.iter().any(|g| g.bottle_id == bottle_id && g.args.contains(&marker)) {
                    return Err(t!("Game này đã có trong thư viện.").into());
                }
                self.cfg.games.push(Game::new(
                    app.name.clone(),
                    bottle.steam_exe(),
                    bottle.id.clone(),
                    format!("{} {marker}", steam::STEAM_QUIET_ARGS),
                ));
                self.dirty = true;
                if self.bits_of(&bottle, &appid) == Some(32) {
                    self.err(t!("Đã thêm {}. Lưu ý: {}.", app.name, steam::bits32_warning()));
                } else {
                    self.info(t!("Đã thêm {}. Bấm Chơi: M2WGame sẽ mở Steam (chạy ngầm, mất ~40 giây) rồi mở game.", app.name));
                }
            }
            "steam_install" => {
                let bottle_id = arg_str(req, "bottle_id").ok_or(t!("thiếu bottle_id"))?;
                let appid = arg_str(req, "appid").ok_or(t!("thiếu appid"))?;
                let bottle = self.cfg.bottle(&bottle_id).cloned().ok_or(t!("Không tìm thấy môi trường."))?;
                let script = steam::write_install_script(&bottle, &appid)?;
                let opened = std::process::Command::new("/usr/bin/open")
                    .args(["-a", "Terminal"])
                    .arg(&script)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !opened {
                    return Err(t!("Không mở được Terminal. Bạn có thể tự chạy file: {}", script.display()));
                }
                self.info(t!("Đã mở Terminal. Nhập tài khoản và mật khẩu Steam ở đó (M2WGame không thấy mật khẩu). Tải xong, quay lại bấm Quét lại."));
            }
            "dismiss_notice" => self.notice = None,
            "log" => {
                let id = id()?;
                let Some(path) = self.last_logs.get(&id).cloned() else {
                    return Err(t!("Game này chưa được chạy lần nào nên chưa có log.").into());
                };
                let failed = matches!(self.exits.get(&id), Some(code) if *code != Some(0));
                let hints: Vec<Value> = self
                    .hints_for(&id, failed)
                    .into_iter()
                    .map(|h| json!({"level": level_str(h.level), "text": h.text}))
                    .collect();
                return Ok(json!({
                    "ok": true,
                    "text": util::tail(&path, 24 * 1024),
                    "path": path,
                    "hints": hints,
                }));
            }
            "preview" => {
                let id = id()?;
                let game = self.cfg.game(&id).cloned().ok_or(t!("Không tìm thấy game."))?;
                let bottle = self.cfg.bottle(&game.bottle_id).cloned();
                let text = match bottle.as_ref().and_then(|b| self.runtime_for(b)) {
                    Some(rt) => launcher::plan(rt, bottle.as_ref().unwrap(), &game).describe(),
                    None => t!("(cần có động cơ và môi trường hợp lệ)").to_string(),
                };
                return Ok(json!({"ok": true, "command": text}));
            }
            other => return Err(t!("lệnh không hỗ trợ: {}", other)),
        }
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::util::ENV_LOCK;

    fn bottle_ids(state: &Value) -> Vec<String> {
        state["bottles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["id"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn library_and_bottle_flow_through_json_commands() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);

        let mut core = Core::new();
        let st = core.handle(&json!({"cmd": "state"}));
        assert_eq!(st["ok"], true);
        let default_bottle = bottle_ids(&st)[0].clone();
        assert!(st["games"].as_array().unwrap().is_empty());
        assert_eq!(st["bottles"][0]["name"], "Mặc định");

        // add a game (the .exe does not exist: it is listed but flagged)
        let r = core.handle(&json!({
            "cmd": "add_game", "name": "  Test ", "exe": "/nonexistent/test.exe",
            "bottle_id": default_bottle, "args": "-windowed"
        }));
        assert_eq!(r["ok"], true);
        let st = core.handle(&json!({"cmd": "state"}));
        let g = st["games"][0].clone();
        assert_eq!(g["name"], "Test");
        assert_eq!(g["exe_missing"], true);
        assert_eq!(g["settings"]["msync"], true);
        assert_eq!(st["notice"]["error"], false);

        // invalid input is refused, nothing is added
        let r = core.handle(&json!({"cmd": "add_game", "name": "", "exe": "x", "bottle_id": default_bottle}));
        assert_eq!(r["ok"], false);
        let r = core.handle(&json!({"cmd": "add_game", "name": "A", "exe": "/x.exe", "bottle_id": "nope"}));
        assert_eq!(r["ok"], false);

        // edit settings through update_game
        let mut edited = g.clone();
        edited["settings"]["metal_hud"] = json!(true);
        edited["name"] = json!("Renamed");
        let r = core.handle(&json!({"cmd": "update_game", "game": {
            "id": edited["id"], "name": edited["name"], "exe": edited["exe"],
            "bottle_id": edited["bottle_id"], "args": edited["args"], "settings": edited["settings"]
        }}));
        assert_eq!(r["ok"], true);
        let st = core.handle(&json!({"cmd": "state"}));
        assert_eq!(st["games"][0]["name"], "Renamed");
        assert_eq!(st["games"][0]["settings"]["metal_hud"], true);

        // playing a missing file only produces an error notice
        let gid = g["id"].as_str().unwrap().to_string();
        core.handle(&json!({"cmd": "play", "id": gid}));
        let st = core.handle(&json!({"cmd": "state"}));
        assert_eq!(st["notice"]["error"], true);
        assert!(st["games"][0]["running_secs"].is_null());

        // the config file was written and can be read back
        let saved = Config::load();
        assert_eq!(saved.games.len(), 1);
        assert_eq!(saved.games[0].name, "Renamed");

        // bottles: cannot remove the one in use or the last one
        let r = core.handle(&json!({"cmd": "remove_bottle", "id": default_bottle}));
        assert_eq!(r["ok"], false);
        let r = core.handle(&json!({"cmd": "create_bottle", "name": "Second"}));
        assert_eq!(r["ok"], true);
        let st = core.handle(&json!({"cmd": "state"}));
        let ids = bottle_ids(&st);
        assert_eq!(ids.len(), 2);
        let second = ids.iter().find(|i| **i != default_bottle).unwrap().clone();
        let r = core.handle(&json!({"cmd": "rename_bottle", "id": second, "name": "Dành cho Steam"}));
        assert_eq!(r["ok"], true);
        let r = core.handle(&json!({"cmd": "remove_bottle", "id": second}));
        assert_eq!(r["ok"], true);
        assert_eq!(bottle_ids(&core.handle(&json!({"cmd": "state"}))).len(), 1);
        let r = core.handle(&json!({"cmd": "remove_bottle", "id": default_bottle}));
        assert_eq!(r["ok"], false);

        // removing a game does not touch files and clears it from the list
        let r = core.handle(&json!({"cmd": "remove_game", "id": gid}));
        assert_eq!(r["ok"], true);
        assert!(core.handle(&json!({"cmd": "state"}))["games"].as_array().unwrap().is_empty());

        // unknown commands and missing arguments are errors, not crashes
        assert_eq!(core.handle(&json!({"cmd": "nope"}))["ok"], false);
        assert_eq!(core.handle(&json!({"cmd": "play"}))["ok"], false);
        assert_eq!(core.handle(&Value::Null)["ok"], false);

        // the notice can be dismissed
        core.handle(&json!({"cmd": "dismiss_notice"}));
        assert!(core.handle(&json!({"cmd": "state"}))["notice"].is_null());

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn stop_runs_in_the_background_and_blocks_risky_actions_meanwhile() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);
        let mut core = Core::new();
        let bottle = core.cfg.bottles[0].id.clone();
        core.handle(&json!({"cmd": "add_game", "name": "Sleeper", "exe": "/x.exe", "bottle_id": bottle}));
        let gid = core.cfg.games[0].id.clone();

        // pretend the game is running: a harmless long-lived process
        let child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        core.running.insert(
            gid.clone(),
            Running { assist_port: None, steam_log_offset: 0, child, started: Instant::now(), label: "Sleeper".into(), bottle_id: bottle.clone() },
        );
        let st = core.handle(&json!({"cmd": "state"}));
        assert!(st["games"][0]["running_secs"].is_u64());

        // a running game protects its environment from deletion
        let r = core.handle(&json!({"cmd": "remove_bottle", "id": bottle}));
        assert_eq!(r["ok"], false);

        // saving changed settings of a running game tells the user they apply next time
        let mut game = serde_json::to_value(&core.cfg.games[0]).unwrap();
        game["settings"]["metal_hud"] = json!(true);
        core.handle(&json!({"cmd": "update_game", "game": game}));
        let st = core.handle(&json!({"cmd": "state"}));
        assert!(st["notice"]["text"].as_str().unwrap().contains("lần chạy sau"));

        // stop: the call returns at once, the process dies, the flag clears
        core.handle(&json!({"cmd": "stop", "id": gid}));
        let mut cleared = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let st = core.handle(&json!({"cmd": "state"}));
            if st["games"][0]["stopping"] == false {
                assert!(st["games"][0]["running_secs"].is_null());
                cleared = true;
                break;
            }
        }
        assert!(cleared, "stop never finished");
        assert!(core.running.is_empty());

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn games_already_in_steam_can_be_added_in_one_step() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);
        let mut core = Core::new();
        let bottle = core.cfg.bottles[0].clone();

        // no Steam in the bottle yet: nothing listed, install refused
        assert!(core.handle(&json!({"cmd": "state"}))["steam_games"].as_array().unwrap().is_empty());
        let r = core.handle(&json!({"cmd": "steam_install", "bottle_id": bottle.id, "appid": "4242"}));
        assert_eq!(r["ok"], false);

        // fake Steam with one finished and one half-downloaded game
        let exe = bottle.steam_exe();
        let apps = exe.parent().unwrap().join("steamapps");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        let acf = |id: &str, name: &str, flags: &str| {
            format!("\"AppState\"\n{{\n\t\"appid\"\t\t\"{id}\"\n\t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\t\"installdir\"\t\t\"{name}\"\n}}\n")
        };
        std::fs::write(apps.join("appmanifest_111.acf"), acf("111", "Alpha", "4")).unwrap();
        std::fs::write(apps.join("appmanifest_222.acf"), acf("222", "Beta", "1026")).unwrap();
        // Alpha is a 32-bit game: a tiny fake PE file in its folder
        let alpha = apps.join("common").join("Alpha");
        std::fs::create_dir_all(&alpha).unwrap();
        let mut pe = vec![0u8; 0x80];
        pe[0] = b'M';
        pe[1] = b'Z';
        pe[0x3c] = 0x80;
        pe.extend_from_slice(b"PE\0\0\x4c\x01");
        std::fs::write(alpha.join("Alpha.exe"), pe).unwrap();

        let st = core.handle(&json!({"cmd": "state"}));
        let listed = st["steam_games"].as_array().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0]["name"], "Alpha");
        assert_eq!(listed[0]["in_library"], false);
        assert_eq!(listed[0]["bits"], 32);
        assert!(listed[1]["bits"].is_null());

        // unfinished game refused, finished game added once
        let r = core.handle(&json!({"cmd": "add_steam_game", "bottle_id": bottle.id, "appid": "222"}));
        assert_eq!(r["ok"], false);
        let r = core.handle(&json!({"cmd": "add_steam_game", "bottle_id": bottle.id, "appid": "111"}));
        assert_eq!(r["ok"], true);
        let r = core.handle(&json!({"cmd": "add_steam_game", "bottle_id": bottle.id, "appid": "111"}));
        assert_eq!(r["ok"], false);
        let st = core.handle(&json!({"cmd": "state"}));
        assert_eq!(st["games"][0]["name"], "Alpha");
        assert_eq!(st["games"][0]["is_steam"], true);
        assert!(st["games"][0]["args"].as_str().unwrap().ends_with("-applaunch 111"));
        assert_eq!(st["games"][0]["bits"], 32);
        assert!(st["notice"]["text"].as_str().unwrap().contains("32-bit"));
        assert_eq!(st["steam_games"][0]["in_library"], true);

        // a made-up id is refused without any network access
        let r = core.handle(&json!({"cmd": "steam_lookup", "input": "not a link"}));
        assert_eq!(r["ok"], false);

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn download_from_the_window_asks_questions_and_ends_in_the_library() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);
        let mut core = Core::new();
        let bottle = core.cfg.bottles[0].clone();

        // bottle with Steam, and a stand-in SteamCMD where the app expects the real one
        let exe = bottle.steam_exe();
        std::fs::create_dir_all(exe.parent().unwrap().join("steamapps")).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        let cmd = steam::steamcmd_path();
        std::fs::create_dir_all(cmd.parent().unwrap()).unwrap();
        std::fs::write(
            &cmd,
            "#!/bin/bash\nwhile [ $# -gt 0 ]; do case \"$1\" in +force_install_dir) D=\"$2\";; esac; shift; done\nprintf 'password: '\nread -r -s PW; echo\n[ \"$PW\" = ok123 ] || { echo 'FAILED (Invalid Password)'; exit 5; }\necho ' Update state (0x61) downloading, progress: 100.00 (1 / 1)'\nmkdir -p \"$D/steamapps\" \"$D/Data\"\necho x > \"$D/Data/f\"\nprintf '\"AppState\"\\n{\\n\\t\"appid\"\\t\\t\"777\"\\n\\t\"name\"\\t\\t\"Stand In\"\\n\\t\"StateFlags\"\\t\\t\"4\"\\n\\t\"installdir\"\\t\\t\"Stand In\"\\n}\\n' > \"$D/steamapps/appmanifest_777.acf\"\necho \"Success! App '777' fully installed.\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&cmd, std::fs::Permissions::from_mode(0o755)).unwrap();

        // bad input is refused up front
        let r = core.handle(&json!({"cmd": "steam_dl_start", "bottle_id": bottle.id, "appid": "777", "account": "two words"}));
        assert_eq!(r["ok"], false);

        let r = core.handle(&json!({"cmd": "steam_dl_start", "bottle_id": bottle.id, "appid": "777", "account": "bob"}));
        assert_eq!(r["ok"], true);
        // only one download at a time
        let r = core.handle(&json!({"cmd": "steam_dl_start", "bottle_id": bottle.id, "appid": "888", "account": "bob"}));
        assert_eq!(r["ok"], false);

        let wait = |core: &mut Core, phase: &str| {
            for _ in 0..200 {
                let st = core.handle(&json!({"cmd": "state"}));
                if st["download"]["phase"] == phase {
                    return st;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            panic!("never reached {phase}");
        };
        let st = wait(&mut core, "need_password");
        assert_eq!(st["steam_account"], "bob");
        assert!(st["notice"]["text"].as_str().unwrap().contains("Mở Thêm game"));
        core.handle(&json!({"cmd": "steam_dl_input", "text": "ok123"}));
        let st = wait(&mut core, "done");
        assert_eq!(st["download"]["name"], "Stand In");
        assert!(!st.to_string().contains("ok123"));

        // the finished game is listed and can be added; the finished job can be cleared
        let st = core.handle(&json!({"cmd": "state"}));
        assert_eq!(st["steam_games"][0]["name"], "Stand In");
        let r = core.handle(&json!({"cmd": "add_steam_game", "bottle_id": bottle.id, "appid": "777"}));
        assert_eq!(r["ok"], true);
        core.handle(&json!({"cmd": "steam_dl_clear"}));
        assert!(core.handle(&json!({"cmd": "state"}))["download"].is_null());
        // the account name (not the password) was remembered on disk
        assert_eq!(Config::load().steam_account, "bob");

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn warns_when_steam_waits_for_a_hidden_dialog() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);
        let mut core = Core::new();
        core.steam_block_after = 0; // warn at once in this test
        let bottle = core.cfg.bottles[0].clone();
        let exe = bottle.steam_exe();
        let logs = exe.parent().unwrap().join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        let log = steam::console_log(&bottle);
        // an old session's question must not count against the new launch
        std::fs::write(&log, "[old] GameAction [AppID 555, ActionID 1] : LaunchApp waiting for user response to SynchronizingCloud \"syncfailed\"\n").unwrap();
        core.handle(&json!({"cmd": "add_game", "name": "G", "exe": exe.to_string_lossy(), "bottle_id": bottle.id, "args": "-quiet -applaunch 555"}));
        let gid = core.cfg.games[0].id.clone();
        let offset = std::fs::metadata(&log).unwrap().len();
        let child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        core.running.insert(gid.clone(), Running { assist_port: None, steam_log_offset: offset, child, started: Instant::now(), label: "G".into(), bottle_id: bottle.id.clone() });

        let st = core.handle(&json!({"cmd": "state"}));
        assert!(st["games"][0]["steam_blocked"].is_null());

        // the new launch gets stuck
        let mut text = std::fs::read_to_string(&log).unwrap();
        text.push_str("[new] GameAction [AppID 555, ActionID 2] : LaunchApp waiting for user response to SynchronizingCloud \"syncfailed\"\n");
        std::fs::write(&log, text.clone()).unwrap();
        let st = core.handle(&json!({"cmd": "state"}));
        assert!(st["games"][0]["steam_blocked"].as_str().unwrap().contains("đám mây"));
        assert_eq!(st["notice"]["error"], true);

        // once the launch moves on the warning goes away
        text.push_str("[new2] GameAction [AppID 555, ActionID 2] : LaunchApp changed task to Completed with \"\"\n");
        std::fs::write(&log, text).unwrap();
        let st = core.handle(&json!({"cmd": "state"}));
        assert!(st["games"][0]["steam_blocked"].is_null());

        core.handle(&json!({"cmd": "stop", "id": gid}));
        std::env::remove_var("M2WGAME_HOME");
        std::thread::sleep(std::time::Duration::from_millis(300));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn steam_window_commands_need_an_open_port_and_skip_other_commands() {
        assert!(handle_steam_ui(&json!({"cmd": "play", "id": "x"})).is_none());
        let r = handle_steam_ui(&json!({"cmd": "steam_ui_shot", "id": "no-such-game"})).unwrap();
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("chưa mở"));
    }

    #[test]
    fn uninstalling_a_game_frees_its_files_and_removes_it_from_the_library() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);
        let mut core = Core::new();
        let bottle = core.cfg.bottles[0].clone();
        let exe = bottle.steam_exe();
        let apps = exe.parent().unwrap().join("steamapps");
        std::fs::create_dir_all(apps.join("common/Big Game")).unwrap();
        std::fs::write(&exe, b"x").unwrap();
        std::fs::write(apps.join("common/Big Game/data.bin"), b"123456").unwrap();
        std::fs::write(
            apps.join("appmanifest_900.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"900\"\n\t\"name\"\t\t\"Big Game\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"installdir\"\t\t\"Big Game\"\n\t\"SizeOnDisk\"\t\t\"5952396374\"\n}\n",
        )
        .unwrap();
        core.handle(&json!({"cmd": "add_steam_game", "bottle_id": bottle.id, "appid": "900"}));
        let st = core.handle(&json!({"cmd": "state"}));
        assert_eq!(st["games"][0]["steam_appid"], "900");
        assert_eq!(st["games"][0]["size_bytes"], 5952396374u64);

        // refused while something of this bottle runs
        let gid = core.cfg.games[0].id.clone();
        let child = std::process::Command::new("sleep").arg("30").spawn().unwrap();
        core.running.insert(gid.clone(), Running { assist_port: None, steam_log_offset: 0, child, started: Instant::now(), label: "G".into(), bottle_id: bottle.id.clone() });
        let r = core.handle(&json!({"cmd": "steam_uninstall", "bottle_id": bottle.id, "appid": "900"}));
        assert_eq!(r["ok"], false);
        assert!(apps.join("common/Big Game/data.bin").is_file());
        core.running.get_mut(&gid).unwrap().child.kill().ok();
        core.running.remove(&gid);

        // unknown game refused
        let r = core.handle(&json!({"cmd": "steam_uninstall", "bottle_id": bottle.id, "appid": "12345"}));
        assert_eq!(r["ok"], false);

        // now it goes
        let r = core.handle(&json!({"cmd": "steam_uninstall", "bottle_id": bottle.id, "appid": "900"}));
        assert_eq!(r["ok"], true);
        let mut done = false;
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let st = core.handle(&json!({"cmd": "state"}));
            if st["games"].as_array().unwrap().is_empty() {
                assert!(st["notice"]["text"].as_str().unwrap().contains("5,95 GB"));
                done = true;
                break;
            }
        }
        assert!(done, "game never left the library");
        assert!(!apps.join("common/Big Game").exists());
        assert!(!apps.join("appmanifest_900.acf").exists());
        assert!(Config::load().games.is_empty());
        assert!(core.handle(&json!({"cmd": "state"}))["steam_games"].as_array().unwrap().is_empty());

        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn notices_get_increasing_ids() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let home = std::env::temp_dir().join(format!("m2wgame-core-{}", util::new_id()));
        std::env::set_var("M2WGAME_HOME", &home);
        let mut core = Core::new();
        core.info("a");
        let first = core.notice.as_ref().unwrap().id;
        core.err("b");
        let second = core.notice.as_ref().unwrap().id;
        assert!(second > first);
        std::env::remove_var("M2WGAME_HOME");
        std::fs::remove_dir_all(&home).ok();
    }
}
