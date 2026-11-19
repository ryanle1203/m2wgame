//! A tiny Chrome DevTools Protocol client (no dependencies) to look at and click inside
//! Steam's own window, which Wine draws black. Only talks to 127.0.0.1.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};

const TIMEOUT: Duration = Duration::from_secs(6);
/// Steam's main window is a 1280x800 page.
pub const PAGE_W: f64 = 1280.0;
pub const PAGE_H: f64 = 800.0;

fn connect(port: u16) -> Result<TcpStream, String> {
    let s = TcpStream::connect_timeout(&format!("127.0.0.1:{port}").parse().unwrap(), TIMEOUT)
        .map_err(|_| t!("Không nối được cổng xem Steam (Steam chưa mở hoặc đã đóng).").to_string())?;
    s.set_read_timeout(Some(TIMEOUT)).ok();
    s.set_write_timeout(Some(TIMEOUT)).ok();
    Ok(s)
}

fn read_until_headers_end(s: &mut TcpStream) -> Result<(String, Vec<u8>), String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        match s.read(&mut byte) {
            Ok(0) | Err(_) => return Err(t!("Steam không trả lời.").into()),
            Ok(_) => buf.push(byte[0]),
        }
        if buf.len() > 16 * 1024 {
            return Err(t!("Phản hồi lạ từ cổng Steam.").into());
        }
    }
    Ok((String::from_utf8_lossy(&buf).into_owned(), Vec::new()))
}

#[derive(Clone, Debug)]
pub struct Page {
    pub title: String,
    pub url: String,
    pub ws: String,
}

/// `GET /json`: the pages Steam exposes.
pub fn pages(port: u16) -> Result<Vec<Page>, String> {
    let mut s = connect(port)?;
    s.write_all(format!("GET /json HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").as_bytes())
        .map_err(|e| e.to_string())?;
    // Steam answers HTTP/1.1 and keeps the connection open: read exactly Content-Length bytes.
    let (head, _) = read_until_headers_end(&mut s)?;
    let len: usize = head
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length").then(|| v.trim().parse().ok())?
        })
        .ok_or(t!("Phản hồi lạ từ cổng Steam."))?;
    let mut body = vec![0u8; len];
    s.read_exact(&mut body).map_err(|_| t!("Steam trả lời thiếu dữ liệu.").to_string())?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| t!("Không đọc được danh sách trang của Steam.").to_string())?;
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .map(|p| Page {
                    title: p["title"].as_str().unwrap_or("").into(),
                    url: p["url"].as_str().unwrap_or("").into(),
                    ws: p["webSocketDebuggerUrl"].as_str().unwrap_or("").into(),
                })
                .collect()
        })
        .unwrap_or_default())
}

/// The big Steam window (not the menus, not the store page inside it).
pub fn main_page(port: u16) -> Result<Page, String> {
    pages(port)?
        .into_iter()
        .find(|p| p.url.contains("createflags=274") && !p.ws.is_empty())
        .ok_or_else(|| t!("Chưa thấy cửa sổ chính của Steam, đợi vài giây rồi thử lại.").to_string())
}

pub struct Ws {
    s: TcpStream,
    next_id: u64,
}

impl Ws {
    pub fn connect(port: u16, ws_url: &str) -> Result<Ws, String> {
        let path = ws_url
            .strip_prefix("ws://")
            .and_then(|r| r.find('/').map(|i| r[i..].to_string()))
            .ok_or(t!("Địa chỉ websocket lạ."))?;
        let mut s = connect(port)?;
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
        let (head, _) = read_until_headers_end(&mut s)?;
        if !head.starts_with("HTTP/1.1 101") {
            return Err(t!("Steam từ chối kết nối xem giao diện.").into());
        }
        Ok(Ws { s, next_id: 1 })
    }

    fn send_text(&mut self, text: &str) -> Result<(), String> {
        let payload = text.as_bytes();
        let mask = [0x37u8, 0xfa, 0x21, 0x3d];
        let mut frame = vec![0x81u8];
        match payload.len() {
            n if n < 126 => frame.push(0x80 | n as u8),
            n if n < 65536 => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.s.write_all(&frame).map_err(|e| t!("Không gửi được cho Steam: {}", e))
    }

    fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, String> {
        let mut out = vec![0u8; n];
        self.s.read_exact(&mut out).map_err(|_| t!("Steam không trả lời kịp.").to_string())?;
        Ok(out)
    }

    /// One complete text message (joins fragments, skips control frames).
    fn read_message(&mut self) -> Result<String, String> {
        let mut message: Vec<u8> = Vec::new();
        loop {
            let h = self.read_exact(2)?;
            let fin = h[0] & 0x80 != 0;
            let opcode = h[0] & 0x0f;
            let masked = h[1] & 0x80 != 0;
            let len = match h[1] & 0x7f {
                126 => {
                    let b = self.read_exact(2)?;
                    u16::from_be_bytes([b[0], b[1]]) as usize
                }
                127 => {
                    let b = self.read_exact(8)?;
                    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as usize
                }
                n => n as usize,
            };
            if len > 64 * 1024 * 1024 {
                return Err(t!("Phản hồi quá lớn từ Steam.").into());
            }
            let mask = if masked { Some(self.read_exact(4)?) } else { None };
            let mut payload = self.read_exact(len)?;
            if let Some(m) = mask {
                for (i, b) in payload.iter_mut().enumerate() {
                    *b ^= m[i % 4];
                }
            }
            match opcode {
                0x0 | 0x1 => {
                    message.extend_from_slice(&payload);
                    if fin {
                        return Ok(String::from_utf8_lossy(&message).into_owned());
                    }
                }
                0x8 => return Err(t!("Steam đã đóng kết nối.").into()),
                _ => {} // ping / pong / binary: ignore
            }
        }
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_text(&json!({"id": id, "method": method, "params": params}).to_string())?;
        for _ in 0..200 {
            let msg = self.read_message()?;
            let v: Value = serde_json::from_str(&msg).unwrap_or(Value::Null);
            if v["id"] == id {
                if let Some(err) = v.get("error") {
                    return Err(t!("Steam báo lỗi: {}", err["message"].as_str().unwrap_or("?")));
                }
                return Ok(v["result"].clone());
            }
        }
        Err(t!("Steam không trả lời yêu cầu.").into())
    }
}

// ---- what the window needs ----

/// A JPEG screenshot of Steam's main window, as base64 text (ready to hand to an image view).
pub fn screenshot(port: u16) -> Result<String, String> {
    let page = main_page(port)?;
    let mut ws = Ws::connect(port, &page.ws)?;
    let r = ws.call("Page.captureScreenshot", json!({"format": "jpeg", "quality": 72}))?;
    r["data"].as_str().map(str::to_string).ok_or_else(|| t!("Steam không trả ảnh.").into())
}

fn mouse(ws: &mut Ws, kind: &str, x: f64, y: f64, extra: Value) -> Result<(), String> {
    let mut p = json!({"type": kind, "x": x, "y": y});
    if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            o.insert(k.clone(), v.clone());
        }
    }
    ws.call("Input.dispatchMouseEvent", p).map(|_| ())
}

/// One left click at page coordinates (0..1280, 0..800).
pub fn click(port: u16, x: f64, y: f64) -> Result<(), String> {
    if !(0.0..=PAGE_W).contains(&x) || !(0.0..=PAGE_H).contains(&y) {
        return Err(t!("Vị trí bấm nằm ngoài cửa sổ Steam.").into());
    }
    let page = main_page(port)?;
    let mut ws = Ws::connect(port, &page.ws)?;
    mouse(&mut ws, "mouseMoved", x, y, json!({}))?;
    mouse(&mut ws, "mousePressed", x, y, json!({"button": "left", "clickCount": 1}))?;
    mouse(&mut ws, "mouseReleased", x, y, json!({"button": "left", "clickCount": 1}))
}

/// Scroll the wheel at a position (positive `dy` = down).
pub fn scroll(port: u16, x: f64, y: f64, dy: f64) -> Result<(), String> {
    let page = main_page(port)?;
    let mut ws = Ws::connect(port, &page.ws)?;
    mouse(&mut ws, "mouseWheel", x.clamp(0.0, PAGE_W), y.clamp(0.0, PAGE_H), json!({"deltaX": 0, "deltaY": dy}))
}

/// Type text into the focused field of the Steam window.
pub fn insert_text(port: u16, text: &str) -> Result<(), String> {
    let page = main_page(port)?;
    let mut ws = Ws::connect(port, &page.ws)?;
    ws.call("Input.insertText", json!({"text": text})).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A stand-in for Steam: serves /json and a websocket that answers captureScreenshot
    /// with a message longer than 64 KB (tests the 8-byte length) and records mouse events.
    fn fake_steam() -> (u16, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_t = seen.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut s) = conn else { continue };
                let seen = seen_t.clone();
                std::thread::spawn(move || {
                    let mut head = Vec::new();
                    let mut b = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") {
                        if s.read(&mut b).unwrap_or(0) == 0 {
                            return;
                        }
                        head.push(b[0]);
                    }
                    let head = String::from_utf8_lossy(&head).into_owned();
                    if head.starts_with("GET /json") {
                        let body = format!(
                            "[{{\"title\":\"Menu\",\"url\":\"about:blank?x\",\"webSocketDebuggerUrl\":\"ws://127.0.0.1:{port}/devtools/page/M\"}},{{\"title\":\"Steam\",\"url\":\"about:blank?createflags=274&minwidth=1\",\"webSocketDebuggerUrl\":\"ws://127.0.0.1:{port}/devtools/page/A\"}}]"
                        );
                        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
                        return;
                    }
                    let _ = s.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n");
                    loop {
                        // read one masked client frame
                        let mut h = [0u8; 2];
                        if s.read_exact(&mut h).is_err() {
                            return;
                        }
                        let mut len = (h[1] & 0x7f) as usize;
                        if len == 126 {
                            let mut e = [0u8; 2];
                            s.read_exact(&mut e).unwrap();
                            len = u16::from_be_bytes(e) as usize;
                        }
                        let mut mask = [0u8; 4];
                        s.read_exact(&mut mask).unwrap();
                        let mut payload = vec![0u8; len];
                        s.read_exact(&mut payload).unwrap();
                        for (i, x) in payload.iter_mut().enumerate() {
                            *x ^= mask[i % 4];
                        }
                        let req: Value = serde_json::from_slice(&payload).unwrap();
                        seen.lock().unwrap().push(req.clone());
                        let result = if req["method"] == "Page.captureScreenshot" {
                            json!({"data": "A".repeat(70_000)})
                        } else {
                            json!({})
                        };
                        let text = json!({"id": req["id"], "result": result}).to_string();
                        // an unrelated event first, then the answer in two fragments
                        let ev = b"{\"method\":\"Page.something\"}";
                        let mut f = vec![0x81, ev.len() as u8];
                        f.extend_from_slice(ev);
                        let _ = s.write_all(&f);
                        let bytes = text.as_bytes();
                        let split = bytes.len() / 2;
                        let frame = |first: bool, fin: bool, chunk: &[u8]| {
                            let mut f = vec![(if fin { 0x80 } else { 0 }) | (if first { 0x1 } else { 0x0 })];
                            f.push(127);
                            f.extend_from_slice(&(chunk.len() as u64).to_be_bytes());
                            f.extend_from_slice(chunk);
                            f
                        };
                        let _ = s.write_all(&frame(true, false, &bytes[..split]));
                        let _ = s.write_all(&frame(false, true, &bytes[split..]));
                    }
                });
            }
        });
        (port, seen)
    }

    #[test]
    fn finds_the_main_window_and_takes_a_big_screenshot() {
        let (port, _) = fake_steam();
        let page = main_page(port).unwrap();
        assert_eq!(page.title, "Steam");
        let shot = screenshot(port).unwrap();
        assert_eq!(shot.len(), 70_000);
    }

    #[test]
    fn click_scroll_and_text_reach_steam_as_the_right_events() {
        let (port, seen) = fake_steam();
        click(port, 745.0, 718.0).unwrap();
        scroll(port, 600.0, 400.0, 300.0).unwrap();
        insert_text(port, "abc").unwrap();
        let log = seen.lock().unwrap().clone();
        let kinds: Vec<String> = log
            .iter()
            .map(|r| format!("{}:{}", r["method"].as_str().unwrap(), r["params"]["type"].as_str().unwrap_or("")))
            .collect();
        assert_eq!(
            kinds,
            vec![
                "Input.dispatchMouseEvent:mouseMoved",
                "Input.dispatchMouseEvent:mousePressed",
                "Input.dispatchMouseEvent:mouseReleased",
                "Input.dispatchMouseEvent:mouseWheel",
                "Input.insertText:"
            ]
        );
        assert_eq!(log[1]["params"]["x"], 745.0);
        assert_eq!(log[1]["params"]["button"], "left");
        assert_eq!(log[3]["params"]["deltaY"], 300.0);
        assert_eq!(log[4]["params"]["text"], "abc");
        // clicks outside the window are refused before anything is sent
        assert!(click(port, 5000.0, 5.0).is_err());
    }

    /// Needs a Steam started with `-devtools-port 8080`: `cargo test -- --ignored real_steam`.
    #[test]
    #[ignore]
    fn real_steam_on_8080() {
        let shot = screenshot(8080).unwrap();
        println!("screenshot base64 length: {}", shot.len());
        assert!(shot.len() > 1000);
    }

    #[test]
    fn a_closed_port_is_a_plain_error() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        assert!(screenshot(port).unwrap_err().contains("cổng xem Steam"));
    }
}
