# Third-party notices

## Rust crates linked into the app

| Crate | Licence |
|---|---|
| serde, serde_derive | MIT OR Apache-2.0 |
| serde_json | MIT OR Apache-2.0 |
| (their transitive dependencies: proc-macro2, quote, syn, itoa, ryu, memchr, ...) | MIT OR Apache-2.0 (see `Cargo.lock`; run `cargo license` for the full list) |

## Used at run time but NOT bundled or redistributed

M2WGame drives these programs that the user installs separately. Their licences apply to them,
not to M2WGame:

| Program | Licence / terms | Notes |
|---|---|---|
| Wine / Wine Staging | LGPL-2.1+ | Builds by third parties (for example Gcenx) |
| Apple Game Porting Toolkit / D3DMetal | Apple's own licence | **Must not be redistributed.** Users get it from Apple or a build they trust |
| DXVK / DXVK-macOS | zlib | Optional, copied by the user into a bottle |
| MoltenVK | Apache-2.0 | Part of DXVK-macOS packages |
| Steam client (Windows) | Valve Steam Subscriber Agreement | Downloaded from Valve by the user's request |
| SteamCMD | Valve SteamCMD terms | Installed by the user |
| Games | Their publishers' licences | M2WGame ships no game content |

## Trademarks

Apple, macOS, Rosetta, Metal and Game Porting Toolkit are trademarks of Apple Inc. Steam and
SteamCMD are trademarks of Valve Corporation. Wine is a trademark of its respective owners. This
project is not endorsed by any of them.
