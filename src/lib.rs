//! M2WGame core: everything except the windows.
//!
//! Detect Wine engines, manage "bottles" (Wine prefixes), launch and stop games, check the
//! Mac and the network, explain logs. The front end is the SwiftUI app in `swift/`, which talks
//! to this library through the C ABI in `ffi`.

pub mod bottle;
pub mod cdp;
pub mod config;
pub mod diagnose;
pub mod ffi;
pub mod game;
pub mod launcher;
pub mod net;
pub mod paths;
pub mod runtime;
pub mod service;
pub mod steam;
pub mod steamdl;
pub mod system;
pub mod util;
