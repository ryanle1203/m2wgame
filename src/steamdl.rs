//! Download a Steam game with SteamCMD, in the background, with progress and the questions
//! SteamCMD asks (password, Steam Guard code) passed on to the window.
//!
//! SteamCMD only behaves when it talks to a terminal, so it runs inside `script` (a pseudo
//! terminal). The password goes straight into its stdin, is never stored or logged, and any
//! echo of it is removed from what is shown.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::steam;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Starting,
    Login,
    NeedPassword,
    NeedCode,
    WaitingMobile,
    Downloading,
    Filing,
    Done,
    Failed,
}

impl Phase {
    pub fn finished(self) -> bool {
        matches!(self, Phase::Done | Phase::Failed)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Job {
    pub appid: String,
    pub bottle_id: String,
    pub name: Option<String>,
    pub phase: Phase,
    pub percent: f32,
    pub message: String,
    /// Last lines SteamCMD printed (secrets removed).
    pub tail: Vec<String>,
    #[serde(skip)]
    installed_ok: bool,
}

impl Job {
    pub fn new(appid: &str, bottle_id: &str, name: Option<String>) -> Self {
        Job {
            appid: appid.into(),
            bottle_id: bottle_id.into(),
            name,
            phase: Phase::Starting,
            percent: 0.0,
            message: t!("Đang khởi động SteamCMD…").into(),
            tail: Vec::new(),
            installed_ok: false,
        }
    }

    fn fail(&mut self, why: impl AsRef<str>) {
        if self.phase != Phase::Failed {
            self.phase = Phase::Failed;
            self.message = why.as_ref().into();
        }
    }

    fn set(&mut self, phase: Phase, message: impl AsRef<str>) {
        if !self.phase.finished() {
            self.phase = phase;
            self.message = message.as_ref().into();
        }
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\u{8}' && c != '\u{4}' {
            out.push(c);
        }
    }
    out
}

/// React to one finished line of SteamCMD output.
pub fn interpret_line(line: &str, job: &mut Job) {
    let l = line.trim();
    if l.is_empty() {
        return;
    }
    let low = l.to_lowercase();

    if low.contains("invalid password") {
        job.fail(t!("Sai mật khẩu hoặc tên tài khoản."));
    } else if low.contains("rate limit exceeded") {
        job.fail(t!("Steam tạm chặn vì đăng nhập quá nhiều lần. Đợi ít nhất 30 phút rồi thử lại."));
    } else if low.contains("two-factor code mismatch") || low.contains("invalid login auth code") {
        job.fail(t!("Mã Steam Guard sai hoặc đã hết hạn. Hãy thử lại."));
    } else if low.contains("account logon denied") {
        job.fail(t!("Steam từ chối đăng nhập. Hãy thử lại và duyệt yêu cầu trên điện thoại."));
    } else if low.contains("no subscription") {
        job.fail(t!("Tài khoản này chưa sở hữu game này, hoặc game không có bản Windows để tải."));
    } else if low.contains("failed to install app") || low.starts_with("error!") {
        job.fail(&t!("SteamCMD báo lỗi: {}", l));
    } else if low.contains("failed (") && low.contains("login") {
        job.fail(&t!("Đăng nhập thất bại: {}", l));
    } else if low.contains("logging in user") {
        job.set(Phase::Login, t!("Đang đăng nhập Steam…"));
    } else if low.contains("logged in ok") || low.contains("waiting for client config...ok") {
        job.set(Phase::Downloading, t!("Đã đăng nhập, bắt đầu tải…"));
    } else if low.contains("update state") {
        if let Some(p) = parse_progress(&low) {
            job.percent = p;
            job.phase = if job.phase.finished() { job.phase } else { Phase::Downloading };
            job.message = if low.contains("validating") {
                t!("Đang kiểm tra file… {}%", format!("{:.0}", p))
            } else {
                t!("Đang tải… {}%", format!("{:.1}", p))
            };
        }
    } else if low.contains("success!") && low.contains("fully installed") {
        job.installed_ok = true;
        job.percent = 100.0;
    }
}

/// Look at text SteamCMD printed without a line ending: it is a question waiting for an answer.
pub fn interpret_prompt(text: &str, job: &mut Job) {
    let low = text.to_lowercase();
    if low.contains("password:") {
        job.set(Phase::NeedPassword, t!("Steam cần mật khẩu của tài khoản."));
    } else if low.contains("steam guard code") || low.contains("two-factor code") {
        job.set(
            Phase::NeedCode,
            t!("Nhập mã Steam Guard, hoặc duyệt đăng nhập trên điện thoại."),
        );
    } else if low.contains("confirm the login in the steam mobile app") || low.contains("confirm this sign-in") {
        job.set(Phase::WaitingMobile, t!("Mở ứng dụng Steam trên điện thoại và duyệt đăng nhập (chọn \"Steam Client\")."));
    }
}

/// "Update state (0x61) downloading, progress: 45.67 (123 / 270)" -> 45.67
fn parse_progress(low: &str) -> Option<f32> {
    let rest = low.split("progress:").nth(1)?;
    let num: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    num.parse().ok()
}

fn redact(line: &str, secrets: &[String]) -> String {
    let mut out = line.to_string();
    for s in secrets {
        if !s.is_empty() {
            out = out.replace(s.as_str(), "••••");
        }
    }
    out
}

pub struct Download {
    pub job: Arc<Mutex<Job>>,
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    secrets: Arc<Mutex<Vec<String>>>,
    pid: u32,
}

impl Download {
    /// Start SteamCMD for `appid`. `tmp` is the download folder, `steamapps` the library to file it into.
    pub fn start(
        steamcmd: &Path,
        account: &str,
        job: Job,
        tmp: PathBuf,
        steamapps: PathBuf,
    ) -> Result<Download, String> {
        use std::os::unix::process::CommandExt;
        std::fs::create_dir_all(&tmp).map_err(|e| t!("Không tạo được thư mục tải: {}", e))?;
        let appid = job.appid.clone();
        let mut child = Command::new("/usr/bin/script")
            .args(["-q", "/dev/null"])
            .arg(steamcmd)
            .args(["+@sSteamCmdForcePlatformType", "windows", "+force_install_dir"])
            .arg(&tmp)
            .args(["+login", account, "+app_update", &appid, "validate", "+quit"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            // Own process group so Cancel can end SteamCMD together with `script`.
            .process_group(0)
            .spawn()
            .map_err(|e| t!("Không chạy được SteamCMD: {}", e))?;
        let pid = child.id();
        let mut out = child.stdout.take().ok_or(t!("Không đọc được đầu ra của SteamCMD"))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take()));
        let job = Arc::new(Mutex::new(job));
        let secrets = Arc::new(Mutex::new(Vec::<String>::new()));

        let job_t = job.clone();
        let secrets_t = secrets.clone();
        std::thread::spawn(move || {
            let mut pending: Vec<u8> = Vec::new();
            let mut buf = [0u8; 4096];
            let mut last_prompt = String::new();
            loop {
                let n = match out.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                pending.extend_from_slice(&buf[..n]);
                // complete lines
                while let Some(pos) = pending.iter().position(|b| *b == b'\n' || *b == b'\r') {
                    let raw: Vec<u8> = pending.drain(..=pos).collect();
                    let line = strip_ansi(&String::from_utf8_lossy(&raw[..raw.len() - 1]));
                    let sec = secrets_t.lock().map(|s| s.clone()).unwrap_or_default();
                    let line = redact(&line, &sec);
                    if line.trim().is_empty() {
                        continue;
                    }
                    let mut j = job_t.lock().unwrap();
                    interpret_line(&line, &mut j);
                    j.tail.push(line.trim().to_string());
                    let len = j.tail.len();
                    if len > 12 {
                        j.tail.drain(..len - 12);
                    }
                    last_prompt.clear();
                }
                // an unfinished line is a question
                if !pending.is_empty() {
                    let text = strip_ansi(&String::from_utf8_lossy(&pending));
                    if text != last_prompt {
                        interpret_prompt(&text, &mut job_t.lock().unwrap());
                        last_prompt = text;
                    }
                }
            }
            let status = child.wait();
            let installed_ok = job_t.lock().unwrap().installed_ok;
            let phase = job_t.lock().unwrap().phase;
            if phase.finished() {
                return;
            }
            if installed_ok {
                job_t.lock().unwrap().set(Phase::Filing, t!("Đang xếp game vào thư viện Steam…"));
                match steam::file_into_library(&tmp, &steamapps, &appid) {
                    Ok((name, dir)) => {
                        let mut j = job_t.lock().unwrap();
                        j.name = Some(name.clone());
                        j.phase = Phase::Done;
                        j.message = t!("Đã tải xong {}. Bấm Thêm để đưa vào thư viện.", name);
                        if steam::game_bits(&dir) == Some(32) {
                            j.message.push_str(&t!(" Lưu ý: {}.", steam::bits32_warning()));
                        }
                    }
                    Err(e) => job_t.lock().unwrap().fail(&e),
                }
            } else {
                let code = status.ok().and_then(|s| s.code());
                job_t.lock().unwrap().fail(&t!("SteamCMD dừng trước khi tải xong{}. Có thể đăng nhập lại và tải tiếp: file đã tải được giữ lại.", code.map(|c| t!(" (mã {})", c)).unwrap_or_default()));
            }
        });

        Ok(Download { job, stdin, secrets, pid })
    }

    /// Answer SteamCMD's current question (password or Steam Guard code). Never logged.
    pub fn send_input(&self, text: &str) -> Result<(), String> {
        if let Ok(mut s) = self.secrets.lock() {
            if text.len() >= 3 {
                s.push(text.to_string());
            }
        }
        let mut guard = self.stdin.lock().map_err(|_| t!("lỗi nội bộ").to_string())?;
        let stdin = guard.as_mut().ok_or(t!("SteamCMD đã đóng, không nhận thêm được."))?;
        stdin
            .write_all(format!("{text}\n").as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(|e| t!("Không gửi được cho SteamCMD: {}", e))?;
        self.job.lock().unwrap().set(Phase::Login, t!("Đang đăng nhập Steam…"));
        Ok(())
    }

    pub fn cancel(&self) {
        let _ = Command::new("/bin/kill")
            .args(["-TERM", "--", &format!("-{}", self.pid)])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        self.job.lock().unwrap().fail(t!("Đã hủy."));
    }

    pub fn snapshot(&self) -> Job {
        self.job.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> Job {
        Job::new("1", "b", None)
    }

    #[test]
    fn reads_progress_and_success() {
        let mut j = job();
        interpret_line("Logging in user 'bob' to Steam Public...", &mut j);
        assert_eq!(j.phase, Phase::Login);
        interpret_line(" Update state (0x61) downloading, progress: 45.67 (123 / 270)", &mut j);
        assert_eq!(j.phase, Phase::Downloading);
        assert!((j.percent - 45.67).abs() < 0.01);
        interpret_line("Success! App '1' fully installed.", &mut j);
        assert!(j.installed_ok);
        assert_eq!(j.percent, 100.0);
    }

    #[test]
    fn questions_and_failures_are_recognised() {
        let mut j = job();
        interpret_prompt("password: ", &mut j);
        assert_eq!(j.phase, Phase::NeedPassword);
        interpret_prompt("Two-factor code:", &mut j);
        assert_eq!(j.phase, Phase::NeedCode);
        interpret_prompt("Please confirm the login in the Steam Mobile app on your phone.", &mut j);
        assert_eq!(j.phase, Phase::WaitingMobile);
        interpret_line("FAILED (Invalid Password)", &mut j);
        assert_eq!(j.phase, Phase::Failed);
        // a failure is final
        interpret_line("Logging in user 'bob' to Steam Public...", &mut j);
        assert_eq!(j.phase, Phase::Failed);

        let mut j = job();
        interpret_line("ERROR! Failed to install app '1' (No subscription)", &mut j);
        assert!(j.message.contains("chưa sở hữu"));
        let mut j = job();
        interpret_line("FAILED login with result code Rate Limit Exceeded", &mut j);
        assert!(j.message.contains("30 phút"));
    }

    #[test]
    fn secrets_are_hidden_and_terminal_noise_is_removed() {
        assert_eq!(redact("echo hunter2 done", &["hunter2".into()]), "echo •••• done");
        assert_eq!(strip_ansi("\u{1b}[1mHello\u{1b}[0m"), "Hello");
    }

    /// Full run against a stand-in for SteamCMD that asks the same questions.
    #[test]
    fn full_download_with_password_and_code() {
        let tmp = std::env::temp_dir().join(format!("m2wgame-dl-{}", crate::util::new_id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let steamapps = tmp.join("lib/steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();
        let fake = tmp.join("fake-steamcmd.sh");
        std::fs::write(
            &fake,
            r#"#!/bin/bash
while [ $# -gt 0 ]; do case "$1" in +force_install_dir) D="$2";; +login) U="$2";; esac; shift; done
echo "Redirecting stderr to something"
echo "Logging in user '$U' to Steam Public..."
printf "password: "
read -r -s PW; echo
if [ "$PW" != "pw12345" ]; then echo "FAILED (Invalid Password)"; exit 5; fi
printf "Two-factor code: "
read -r CODE
if [ "$CODE" != "AB12C" ]; then echo "FAILED (Two-factor code mismatch)"; exit 5; fi
echo "Waiting for client config...OK"
echo " Update state (0x61) downloading, progress: 50.00 (50 / 100)"
sleep 0.2
echo " Update state (0x61) downloading, progress: 100.00 (100 / 100)"
mkdir -p "$D/steamapps" "$D/Data"
echo bin > "$D/Data/a.bin"
printf '"AppState"\n{\n\t"appid"\t\t"4242"\n\t"name"\t\t"Fake Game"\n\t"StateFlags"\t\t"4"\n\t"installdir"\t\t"Fake Dir"\n}\n' > "$D/steamapps/appmanifest_4242.acf"
echo "Success! App '4242' fully installed."
"#,
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let dl = Download::start(
            &fake,
            "bob",
            Job::new("4242", "b", None),
            tmp.join("dl"),
            steamapps.clone(),
        )
        .unwrap();

        let wait_for = |want: Phase| {
            for _ in 0..200 {
                if dl.snapshot().phase == want {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            panic!("never reached {:?}; at {:?}: {:?}", want, dl.snapshot().phase, dl.snapshot().tail);
        };
        wait_for(Phase::NeedPassword);
        dl.send_input("pw12345").unwrap();
        wait_for(Phase::NeedCode);
        dl.send_input("AB12C").unwrap();
        wait_for(Phase::Done);

        let j = dl.snapshot();
        assert_eq!(j.name.as_deref(), Some("Fake Game"));
        assert_eq!(j.percent, 100.0);
        assert!(steamapps.join("common/Fake Dir/Data/a.bin").is_file());
        assert!(steamapps.join("appmanifest_4242.acf").is_file());
        // the password never shows up in what the window can display
        assert!(!j.tail.iter().any(|l| l.contains("pw12345") || l.contains("AB12C")));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn wrong_password_fails_cleanly_and_cancel_stops_it() {
        let tmp = std::env::temp_dir().join(format!("m2wgame-dl-{}", crate::util::new_id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let fake = tmp.join("fake.sh");
        std::fs::write(
            &fake,
            "#!/bin/bash\nprintf 'password: '\nread -r -s PW; echo\necho 'FAILED (Invalid Password)'\nexit 5\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dl = Download::start(&fake, "bob", Job::new("1", "b", None), tmp.join("dl"), tmp.join("steamapps")).unwrap();
        for _ in 0..100 {
            if dl.snapshot().phase == Phase::NeedPassword {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        dl.send_input("nope").unwrap();
        for _ in 0..100 {
            if dl.snapshot().phase == Phase::Failed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let j = dl.snapshot();
        assert_eq!(j.phase, Phase::Failed);
        assert!(j.message.contains("Sai mật khẩu"));

        // cancel on a program that waits forever
        let slow = tmp.join("slow.sh");
        std::fs::write(&slow, "#!/bin/bash\nsleep 30\n").unwrap();
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dl = Download::start(&slow, "bob", Job::new("1", "b", None), tmp.join("dl2"), tmp.join("steamapps")).unwrap();
        let started = std::time::Instant::now();
        dl.cancel();
        assert_eq!(dl.snapshot().phase, Phase::Failed);
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(started.elapsed().as_secs() < 5);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
