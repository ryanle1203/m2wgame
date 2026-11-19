**M2WGame @VERSION@**

A friendly Mac app that helps you run **Windows games you bought on Steam** on Apple Silicon Macs, through Wine. Steam-only for now: other stores and launchers are not supported yet.

### Install
Download `M2WGame-@VERSION@-arm64.dmg` below, open it and drag **M2WGame** to **Applications**.

The app is **not notarized by Apple**, so on first launch macOS may block it. Right-click the app, choose **Open**, then **Open** again (or *System Settings → Privacy & Security → Open Anyway*, or `xattr -dr com.apple.quarantine /Applications/M2WGame.app`).

You also need a Wine engine; M2WGame does not include one. See [Get an engine](https://github.com/ryanle1203/m2wgame#get-an-engine).

### Known limitations
- Early software, tested on few Macs
- 32-bit Windows games usually crash at start-up on free Wine builds on Apple Silicon
- The app's interface is in English and Vietnamese (docs are in 9 languages)
- DirectX 12 and anti-cheat games are mostly out of reach

### Checksum (SHA-256)
```
@SHA256@  M2WGame-@VERSION@-arm64.dmg
```

*This disk image was built automatically by GitHub Actions from the tagged commit.*
