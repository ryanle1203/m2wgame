# Contributing to M2WGame

Thanks for helping! Bug reports, compatibility notes, translations, docs and code are all welcome.

## Scope

M2WGame is **Steam-first**: its job is to run Windows games bought on Steam. Support for other
stores (Epic, GOG, EA, Ubisoft, Battle.net, itch.io...) is not planned for now. If you want to
work on one, please open an issue to discuss it first, so the design stays small and the
project's rules below (no DRM circumvention, no stored secrets) keep holding.

## Ground rules (please read)

These are non-negotiable because they protect users:

1. **Never store, log or print a password, Steam Guard code or token.** Secrets go straight to the
   program that needs them (for example SteamCMD's stdin) and are redacted from anything shown.
2. **No DRM circumvention.** No cracked or emulated Steam APIs, no `steam_appid.txt` tricks to
   dodge a licence check, no bypassing Steam Guard or rate limits, no shipping game files.
3. **The user decides about anything legal or destructive.** Accepting an EULA, deleting game
   files, opening a debugging port: always an explicit action by the user, with a clear
   confirmation text. Deleting must be limited to M2WGame's own folders and covered by tests.
4. **Do not bundle or redistribute** Apple's D3DMetal / Game Porting Toolkit, Steam, SteamCMD or
   any game. Link to where users can get them.
5. **Plain language.** User-facing text should make sense to someone who has never heard of Wine.
   Technical details belong in an "Advanced" section.

## Development setup

```bash
xcode-select --install            # Swift compiler
curl https://sh.rustup.rs -sSf | sh
cargo test --lib                  # the core's tests (fast, no UI)
./scripts/build-swift.sh          # builds the .app (Rust static library + SwiftUI)
```

If your checkout lives in a folder that syncs to the cloud (iCloud, Dropbox), keep build output
out of it: create `.cargo/config.toml` with `[build] target-dir = "target.nosync"` (the folder name
`*.nosync` is ignored by iCloud; `build-swift.sh` expects `target.nosync`).

## Taking screenshots

Run the app against throwaway data so no personal games or paths show up:
`open -n --env M2WGAME_HOME=/some/demo/folder --env M2WGAME_START_SECTION=engines build/M2WGame.app`.
`M2WGAME_START_SECTION` is `library | environments | engines | system`; `M2WGAME_OPEN` is
`add_game | settings | check_net`.

## Architecture in one minute

- `src/` is the Rust core. Everything is driven by JSON commands (`Core::handle`) and one JSON
  snapshot (`Core::state`). Nothing in it knows about windows.
- `swift/Sources/` is the SwiftUI app. It never touches files or processes itself: it sends
  commands through `wb_request` (see `src/ffi.rs`, `swift/Sources/bridge.h`) and draws the snapshot.
- To add a feature: add a command in `service.rs` (`dispatch`), put logic in a module with unit
  tests, expose data in `state()`, then draw it in SwiftUI. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Tests

- Every new piece of core logic needs tests. Tests that change `M2WGAME_HOME` must hold
  `util::ENV_LOCK`. Use a temporary folder, never the real `~/Library/Application Support/M2WGame`.
- Real-world tests that need an engine or the network are `#[ignore]`d:
  `cargo test --lib -- --ignored --nocapture`.
- For UI changes please describe (or attach a screenshot of) what you checked by hand.

## Making a release

Releases are built by GitHub Actions (`.github/workflows/release.yml`), not on a maintainer's Mac:

1. Bump `version` in `Cargo.toml` (and run `cargo build` so `Cargo.lock` follows), commit, push.
2. Tag and push the tag: `git tag v0.1.1 && git push origin v0.1.1`.
3. The workflow checks that the tag matches `Cargo.toml`, builds `M2WGame-<version>-arm64.dmg` with
   `scripts/make-dmg.sh`, and publishes a GitHub Release with the `.dmg`, its `.sha256` and notes
   from `.github/release-notes.md` (versions `0.x` are marked as pre-releases).

To only test the build, run the workflow by hand (Actions tab, Release, *Run workflow*): it uploads
the disk image as a workflow artifact and publishes nothing. You can also run
`./scripts/make-dmg.sh` locally (output in `dist/`). Never commit the `.dmg` to git.
The app is ad-hoc signed only (not notarized).

## Pull requests

- Keep them small and focused; explain *why*.
- `cargo test --lib` must pass; `./scripts/build-swift.sh` must build.
- Do not commit personal data (account names, Steam IDs, paths under `/Users/<you>`).

## Translations

The English `README.md` is the reference. Translated READMEs live in `docs/i18n/README.<code>.md`
(currently vi, zh-CN, es, ja, ko, pt-BR, fr, de). To add or fix one, copy an existing file, keep the
same structure, and add your language to the switcher line at the top of every README. The app's
own interface speaks Vietnamese (the language of the code) and English. To add another language:

- **Core texts** (Rust): Vietnamese is the source, written as `t!("…")`. The English table is
  `src/i18n_en.rs`; a test fails if a text has no English entry. For a new language, add a table
  and a branch in `src/i18n.rs`.
- **Window texts** (SwiftUI): copy `swift/Resources/en.lproj/Localizable.strings` to
  `<code>.lproj`, translate the right-hand side, and add the language to `LanguageChoice` in
  `swift/Sources/Language.swift`. `python3 scripts/check-swift-strings.py` lists texts that have
  no entry.

## Reporting compatibility

Open an issue with: Mac model and macOS version, engine used (name and version), game and Steam
app id, whether it is 32-bit or 64-bit, and the game's log (the Log button in the app).
