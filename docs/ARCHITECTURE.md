# Architecture

```
SwiftUI app (swift/Sources)          Rust core (src/)
  Views  ──────────────►  Store  ─►  wb_request(json) ─►  Core::handle(cmd) ─►  modules
  (draw the snapshot)    (polls)  ◄─  json answer      ◄─  Core::state()
```

- **One door:** `src/ffi.rs` exposes `wb_request(const char*) -> char*` and `wb_free`. Requests and
  answers are JSON. A global `Core` sits behind a mutex. Commands that only talk to Steam's
  debugging port (`steam_ui_*`) are handled before the lock so a slow Steam never freezes the app.
- **State:** `Core::state()` returns everything the window needs in one JSON object (games,
  bottles, engines, system checks, network results, running state, download job, notices).
  The Swift `Store` polls it every 1-1.5 s.
- **Background work** (starting a bottle, downloads, stopping, uninstalling, network checks)
  runs on threads and reports back through a channel drained by `Core::tick()`.

## Modules (`src/`)

| Module | Role |
|---|---|
| `paths`, `util` | Data folders (override with `M2WGAME_HOME` in tests), argument/env parsing, helpers |
| `system` | Mac readiness checks (chip, macOS, RAM, Rosetta, disk) |
| `runtime` | Finds Wine engines, reads versions, flags old ones |
| `bottle`, `game`, `config` | Data model and the JSON config file |
| `launcher` | Builds launch plans (env + command), spawns, stops (`wineserver -k`, sweeps leftovers) |
| `net` | Probes Steam/GitHub servers and explains the result |
| `diagnose` | Turns log text into plain-language hints |
| `steam` | Steam library helpers: appmanifests, 32/64-bit detection, uninstall, blocked-launch detection |
| `steamdl` | SteamCMD downloads in a pseudo-terminal with progress and prompts |
| `cdp` | Dependency-free Chrome DevTools Protocol client for Steam's hidden window |
| `service` | `Core`: ties everything together, JSON commands and snapshot |
| `ffi` | The C interface |

## JSON commands (`Core::handle`)

`state`, `play`, `stop`, `add_game`, `update_game`, `remove_game`, `create_bottle`,
`rename_bottle`, `remove_bottle`, `init_bottle`, `set_bottle_runtime`, `use_runtime`,
`add_runtime`, `rescan`, `install_steam`, `add_steam`, `check_net`, `log`, `preview`,
`dismiss_notice`, and the Steam family: `steam_lookup`, `steam_lookup_clear`,
`add_steam_game`, `steam_install`, `steam_dl_start`, `steam_dl_input`, `steam_dl_cancel`,
`steam_dl_clear`, `steam_uninstall`, `steam_assist_start`, `steam_assist_relaunch`,
`steam_ui_shot`, `steam_ui_click`, `steam_ui_scroll`, `steam_ui_text`.

Every answer is `{"ok": true, ...}` or `{"ok": false, "error": "..."}`.

## Data on disk

`~/Library/Application Support/M2WGame/` (pre-release builds were named WinBridge; if only a `WinBridge` folder exists it keeps being used, so nothing needs moving): `config.json`, `Bottles/<id>/` (Wine prefixes),
`Runtimes/` (engines), `SteamCMD/`, `Downloads/`; logs in `~/Library/Logs/M2WGame/`.

## Design decisions worth knowing

- Steam's window is black under Wine on macOS, so Steam runs with `-silent` and its dialogs are
  shown through its debugging port only on request (random port, one run).
- Stopping a Steam game asks Steam to quit first (`steam.exe -shutdown`) so saves sync; killing
  it makes the next launch stall on a hidden "pending cloud session" prompt.
- Deleting is confined to a bottle's own `steamapps/common/<installdir>`, with path checks and tests.
