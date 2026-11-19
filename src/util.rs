//! Small helpers with no dependencies.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Unique-enough id (nanoseconds since epoch in hex + a counter).
pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}{:02x}", nanos, COUNTER.fetch_add(1, Ordering::Relaxed) % 256)
}

/// File-name-safe version of a title.
pub fn slug(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    let mut out = String::new();
    let mut last_dash = false;
    for c in trimmed.chars() {
        if c == '-' {
            if !last_dash {
                out.push(c);
            }
            last_dash = true;
        } else {
            out.push(c);
            last_dash = false;
        }
    }
    if out.is_empty() {
        "game".to_string()
    } else {
        out
    }
}

/// Split a command line the way a shell would for the simple case:
/// whitespace separates words, single or double quotes group words.
pub fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has_token = false;
    for c in input.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                has_token = true;
            }
            None if c.is_whitespace() => {
                if has_token || !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            None => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token || !cur.is_empty() {
        args.push(cur);
    }
    args
}

/// `KEY=VALUE` per line. Blank lines and lines starting with `#` are ignored.
pub fn parse_env_lines(input: &str) -> Vec<(String, String)> {
    input
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            let k = k.trim();
            if k.is_empty() || k.contains(char::is_whitespace) {
                return None;
            }
            Some((k.to_string(), v.trim().to_string()))
        })
        .collect()
}

/// Last `max_bytes` of a text file (for the log viewer).
pub fn tail(path: &Path, max_bytes: u64) -> String {
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max_bytes);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        // Drop the (probably cut) first line.
        text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or(text)
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_plain_and_quoted_args() {
        assert_eq!(split_args("-a -b"), vec!["-a", "-b"]);
        assert_eq!(
            split_args(r#"-windowed "C:\My Games\x" -k='a b'"#),
            vec!["-windowed", r"C:\My Games\x", "-k=a b"]
        );
        assert!(split_args("   ").is_empty());
        assert_eq!(split_args(r#""" x"#), vec!["", "x"]);
    }

    #[test]
    fn parses_env_lines() {
        let parsed = parse_env_lines("A=1\n# comment\n\n B = two words \nbad line\n=x\n");
        assert_eq!(
            parsed,
            vec![
                ("A".to_string(), "1".to_string()),
                ("B".to_string(), "two words".to_string())
            ]
        );
    }

    #[test]
    fn slugs_are_file_safe() {
        assert_eq!(slug("Elden Ring: Nightreign!"), "elden-ring-nightreign");
        assert_eq!(slug("???"), "game");
    }

    #[test]
    fn sizes_are_readable() {
        assert_eq!(human_bytes(5_952_396_374), "5,95 GB");
        assert_eq!(human_bytes(1_500_000), "1,5 MB");
        assert_eq!(human_bytes(10), "10 B");
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(new_id(), new_id());
    }
}

/// 5952396374 -> "5,95 GB". Decimal units like Finder, with a Vietnamese decimal comma.
pub fn human_bytes(b: u64) -> String {
    let (value, unit, decimals) = match b {
        b if b >= 1_000_000_000 => (b as f64 / 1e9, "GB", 2),
        b if b >= 1_000_000 => (b as f64 / 1e6, "MB", 1),
        b if b >= 1_000 => (b as f64 / 1e3, "KB", 0),
        b => return format!("{b} B"),
    };
    format!("{value:.decimals$} {unit}").replace('.', ",")
}

/// Tests that change M2WGAME_HOME must not run at the same time.
#[cfg(test)]
pub static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
