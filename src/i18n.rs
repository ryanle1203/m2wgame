//! Language of the texts the core produces for the user (notices, errors, hints, checks).
//!
//! Vietnamese is the source language: every user-facing text in the code is a Vietnamese
//! template written as `t!("... {} ...", arg)`. When English is selected the template is looked
//! up in `i18n_en::EN` (the key is the Vietnamese template) and filled in the same way.
//! A test makes sure no template in the code is missing from the table.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Vi,
    En,
}

static LANG: AtomicU8 = AtomicU8::new(0);

thread_local! {
    /// Lets one thread (a test) use another language without touching the others.
    static OVERRIDE: std::cell::Cell<Option<Lang>> = const { std::cell::Cell::new(None) };
}

/// Run `f` with `lang` on this thread only.
pub fn with_lang<T>(lang: Lang, f: impl FnOnce() -> T) -> T {
    let before = OVERRIDE.with(|o| o.replace(Some(lang)));
    let out = f();
    OVERRIDE.with(|o| o.set(before));
    out
}

pub fn lang() -> Lang {
    if let Some(l) = OVERRIDE.with(|o| o.get()) {
        return l;
    }
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::En
    } else {
        Lang::Vi
    }
}

/// `"en"` (or `"en-US"`...) means English; anything else is Vietnamese, the source language.
pub fn parse_lang(code: &str) -> Lang {
    if code.trim().to_lowercase().starts_with("en") {
        Lang::En
    } else {
        Lang::Vi
    }
}

pub fn set_lang(code: &str) {
    LANG.store(if parse_lang(code) == Lang::En { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn code() -> &'static str {
    match lang() {
        Lang::En => "en",
        Lang::Vi => "vi",
    }
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| crate::i18n_en::EN.iter().copied().collect())
}

/// The template in the current language (Vietnamese when it is not translated).
pub fn tr(vi: &str) -> String {
    match lang() {
        Lang::Vi => vi.to_string(),
        Lang::En => table().get(vi).copied().unwrap_or(vi).to_string(),
    }
}

/// Fill `{}` (in order) and `{0}`, `{1}`... (by position) with `args`.
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0usize;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        let mut name = String::new();
        let mut closed = false;
        for n in chars.by_ref() {
            if n == '}' {
                closed = true;
                break;
            }
            name.push(n);
        }
        if !closed {
            out.push('{');
            out.push_str(&name);
            break;
        }
        let idx = if name.is_empty() {
            let i = next;
            next += 1;
            Some(i)
        } else {
            name.parse::<usize>().ok()
        };
        match idx.and_then(|i| args.get(i)) {
            Some(a) => out.push_str(a),
            None => {
                out.push('{');
                out.push_str(&name);
                out.push('}');
            }
        }
    }
    out
}

pub fn tr_fmt(vi: &str, args: &[String]) -> String {
    fill(&tr(vi), args)
}

/// `t!("Đã thêm {}", name)` -> the text in the current language.
#[macro_export]
macro_rules! t {
    ($lit:literal) => {
        $crate::i18n::tr($lit)
    };
    ($lit:literal, $($arg:expr),+ $(,)?) => {
        $crate::i18n::tr_fmt($lit, &[$(($arg).to_string()),+])
    };
}

/// All `t!("...")` templates in a source file (the text between the quotes, unescaped).
#[cfg(test)]
pub(crate) fn templates_in(src: &str) -> Vec<String> {
    let src = src.split("#[cfg(test)]").next().unwrap_or(src);
    let b: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < b.len() {
        let is_t = b[i] == 't' && b[i + 1] == '!' && b[i + 2] == '(' && (i == 0 || !(b[i - 1].is_alphanumeric() || b[i - 1] == '_'));
        if is_t {
            let mut j = i + 3;
            while j < b.len() && b[j].is_whitespace() {
                j += 1;
            }
            if j < b.len() && b[j] == '"' {
                j += 1;
                let mut s = String::new();
                while j < b.len() && b[j] != '"' {
                    if b[j] == '\\' && j + 1 < b.len() {
                        j += 1;
                        s.push(match b[j] {
                            'n' => '\n',
                            other => other,
                        });
                    } else {
                        s.push(b[j]);
                    }
                    j += 1;
                }
                out.push(s);
                i = j;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCES: [&str; 11] = [
        include_str!("service.rs"),
        include_str!("steam.rs"),
        include_str!("steamdl.rs"),
        include_str!("net.rs"),
        include_str!("system.rs"),
        include_str!("diagnose.rs"),
        include_str!("cdp.rs"),
        include_str!("launcher.rs"),
        include_str!("runtime.rs"),
        include_str!("config.rs"),
        include_str!("ffi.rs"),
    ];

    fn placeholders(s: &str) -> usize {
        s.matches("{}").count() + (0..9).filter(|n| s.contains(&format!("{{{n}}}"))).count()
    }

    #[test]
    fn every_template_in_the_code_has_an_english_text_and_vice_versa() {
        let mut used: Vec<String> = SOURCES.iter().flat_map(|s| templates_in(s)).collect();
        // texts that live in tables instead of t! calls
        for p in crate::net::PROBES.iter() {
            used.push(p.label.to_string());
            used.push(p.why.to_string());
        }
        used.retain(|t| t.chars().any(|c| !c.is_ascii()) || t == "Chip Apple Silicon");
        let table: std::collections::HashSet<&str> = crate::i18n_en::EN.iter().map(|(k, _)| *k).collect();
        let missing: Vec<&String> = used.iter().filter(|t| !table.contains(t.as_str())).collect();
        assert!(missing.is_empty(), "no English text for: {missing:#?}");
        let stale: Vec<&&str> = table.iter().filter(|k| !used.iter().any(|u| u == **k)).collect();
        assert!(stale.is_empty(), "English texts whose Vietnamese source is gone: {stale:#?}");
    }

    #[test]
    fn english_texts_keep_the_same_placeholders() {
        for (vi, en) in crate::i18n_en::EN {
            assert_eq!(placeholders(vi), placeholders(en), "placeholder count differs:\n  {vi}\n  {en}");
            assert!(!en.is_empty());
        }
    }

    #[test]
    fn language_codes_are_understood() {
        assert_eq!(parse_lang("en"), Lang::En);
        assert_eq!(parse_lang(" EN-us "), Lang::En);
        assert_eq!(parse_lang("vi"), Lang::Vi);
        assert_eq!(parse_lang(""), Lang::Vi);
        assert_eq!(parse_lang("fr"), Lang::Vi);
    }

    #[test]
    fn language_can_be_forced_for_one_thread() {
        let vi = tr("Đã hủy.");
        let en = with_lang(Lang::En, || tr("Đã hủy."));
        assert_eq!(vi, "Đã hủy.");
        assert_eq!(en, "Cancelled.");
        assert_eq!(with_lang(Lang::En, || t!("Đã xóa môi trường {}.", "Demo")), "Deleted environment Demo.");
        // back to normal afterwards
        assert_eq!(t!("Đã xóa môi trường {}.", "Demo"), "Đã xóa môi trường Demo.");
    }

    #[test]
    fn fills_placeholders_in_order_and_by_index() {
        let a = vec!["A".to_string(), "B".to_string()];
        assert_eq!(fill("{} then {}", &a), "A then B");
        assert_eq!(fill("{1} before {0}", &a), "B before A");
        assert_eq!(fill("no args", &a), "no args");
        assert_eq!(fill("missing {} {}", &a[..1]), "missing A {}");
    }
}
