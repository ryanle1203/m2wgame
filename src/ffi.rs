//! C ABI for front ends written in other languages (the SwiftUI app).
//!
//! One entry point: send a JSON command, get a JSON answer. Both strings are UTF-8,
//! NUL-terminated. The answer must be released with `wb_free`.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

use serde_json::Value;

use crate::service::Core;

static CORE: Mutex<Option<Core>> = Mutex::new(None);

fn run(request: &str) -> String {
    let req: Value = serde_json::from_str(request).unwrap_or(Value::Null);
    if !req.is_object() {
        // Refuse before touching the core (and the user's data on disk).
        return r#"{"ok":false,"error":"yêu cầu không hợp lệ"}"#.to_string();
    }
    // Looking at / clicking in Steam's window talks to Steam only; it must not wait for the core.
    if let Some(answer) = crate::service::handle_steam_ui(&req) {
        return answer.to_string();
    }
    // A panic in one command must not poison the core for every later call.
    let mut guard = CORE.lock().unwrap_or_else(|p| p.into_inner());
    let core = guard.get_or_insert_with(Core::new);
    core.handle(&req).to_string()
}

/// Send one JSON command. Returns a newly allocated JSON string, or NULL if it cannot be built.
///
/// # Safety
/// `request` must be NULL or a valid NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn wb_request(request: *const c_char) -> *mut c_char {
    let text = if request.is_null() {
        String::new()
    } else {
        CStr::from_ptr(request).to_string_lossy().into_owned()
    };
    let answer = std::panic::catch_unwind(|| run(&text))
        .unwrap_or_else(|_| r#"{"ok":false,"error":"lỗi nội bộ"}"#.to_string());
    CString::new(answer).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

/// Release a string returned by `wb_request`.
///
/// # Safety
/// `ptr` must come from `wb_request` and not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn wb_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_input_gives_a_json_error_not_a_crash() {
        let ptr = unsafe { wb_request(std::ptr::null()) };
        assert!(!ptr.is_null());
        let text = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string();
        unsafe { wb_free(ptr) };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["ok"], false);
    }
}
