# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Português (BR)](README.pt-BR.md) · [Français](README.fr.md) · **Deutsch**

M2W = Mac to Windows (Spiele). Eine App, mit der du **Windows-Spiele auf Apple-Silicon-Macs ausführst**: Sie verwaltet Wine-„Bottles“ (getrennte Windows-Umgebungen), Wine-Engines, Steam für Windows und deine Spielebibliothek und erklärt Probleme in einfacher Sprache, statt dich mit einer Wand aus Logs allein zu lassen.

> **Status: frühe Alpha.** Manche Spiele laufen, andere nicht. Lies die „Einschränkungen“, bevor du Zeit in ein Spiel steckst. Die Oberfläche der App ist derzeit **nur auf Vietnamesisch**; Übersetzungen sind willkommen. Die englische README ist maßgeblich, Übersetzungen können hinterherhinken.

> **Umfang:** M2WGame konzentriert sich auf ein Problem: **bei Steam gekaufte Windows-Spiele** auszuführen. Andere Shops und Launcher (Epic, GOG, EA, Ubisoft, Battle.net, itch.io …) werden noch nicht unterstützt. Einzelne `.exe`-Dateien lassen sich von Hand hinzufügen, dieser Weg ist aber ungetestet.

M2WGame ist **nicht verbunden** mit Apple, Valve, CodeWeavers oder dem Wine-Projekt. Es enthält, lädt oder umgeht keine Spiele, kein DRM und keine Lizenzprüfung: Du musst deine Spiele besitzen.

## Funktionen

- **Spielebibliothek**: Starten / Beenden, Einstellungen pro Spiel, Logs und Hinweise in einfacher Sprache, was schiefgelaufen ist.
- **Umgebungen und Engines**: Umgebungen anlegen, pro Umgebung die Wine-Engine wählen, Steam für Windows mit einem Klick installieren.
- **Steam ohne Steam-Fenster** (es bleibt unter Wine auf macOS schwarz): Spiele aus der Steam-Bibliothek mit einem Klick hinzufügen; neue Spiele in der App mit SteamCMD herunterladen (Passwort und Steam-Guard-Code gehen direkt an SteamCMD und werden nie gespeichert); versteckte Steam-Dialoge (EULA, Cloud-Sync) in der App anzeigen, damit du sie selbst beantwortest; Warnung bei 32-Bit-Spielen; Spiele deinstallieren, um Speicherplatz freizugeben.
- **System- und Netzwerkprüfung**: Rosetta, macOS-Version, RAM, freier Speicher und ob die Steam-Server erreichbar sind.

## Voraussetzungen

Apple-Silicon-Mac, macOS 14 oder neuer, Rosetta 2, Xcode Command Line Tools und [Rust](https://rustup.rs). Eine Wine-Engine musst du selbst mitbringen (nicht enthalten); siehe „Get an engine“ in der englischen README.

## Download

Lade die `.dmg` unter [Releases](https://github.com/ryanle1203/m2wgame/releases) herunter und ziehe sie in „Programme“. Die App ist nicht von Apple notarisiert: Beim ersten Start Rechtsklick > Öffnen (Details im Abschnitt „Install“ der englischen README).

## Bauen

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## Einschränkungen

- **32-Bit-Windows-Spiele laufen meist nicht** (Absturz beim Start); M2WGame warnt davor.
- Die Steam-Oberfläche ist unter Wine auf macOS nicht nutzbar; das Projekt umgeht das wie oben beschrieben.
- Spiele mit DirectX 12 oder Anti-Cheat sind größtenteils außer Reichweite.
- Nur auf macOS 27 mit einem M1 Pro getestet; Berichte von anderen Macs sind sehr willkommen.

## Mitwirken und Lizenz

Bitte lies [CONTRIBUTING.md](../../CONTRIBUTING.md) (mit festen Regeln zu Passwörtern und DRM) und [SECURITY.md](../../SECURITY.md). Lizenz: [MIT](../../LICENSE).
