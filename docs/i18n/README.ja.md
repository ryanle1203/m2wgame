# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · **日本語** · [한국어](README.ko.md) · [Português (BR)](README.pt-BR.md) · [Français](README.fr.md) · [Deutsch](README.de.md)

M2W = Mac to Windows（ゲーム）。**Apple シリコンの Mac で Windows ゲームを動かす**ためのアプリです。Wine の「ボトル」（独立した Windows 環境）、Wine エンジン、Windows 版 Steam、ゲームライブラリを管理し、大量のログを読ませる代わりに、問題を分かりやすい言葉で説明します。

> **状態：初期アルファ版。** 動くゲームもあれば動かないゲームもあります。時間をかける前に下の「制限事項」をお読みください。アプリの画面は現在**ベトナム語のみ**です。翻訳を歓迎します。英語版 README が基準で、翻訳は遅れることがあります。

M2WGame は Apple、Valve、CodeWeavers、Wine プロジェクトと**一切関係ありません**。ゲーム、DRM、ライセンス確認を含めたり、ダウンロードしたり、回避したりすることはありません。ゲームは正規に購入している必要があります。

## できること

- **ゲームライブラリ**：起動 / 停止、ゲームごとの設定、ログの表示、原因を分かりやすく説明するヒント。
- **環境とエンジン**：環境の作成、環境ごとの Wine エンジン選択、Windows 版 Steam のワンクリックインストール。
- **Steam のウィンドウなしで Steam を使う**（macOS の Wine では Steam のウィンドウが真っ黒になります）：Steam ライブラリにあるゲームをワンクリックで追加、アプリ内で SteamCMD を使って新しいゲームをダウンロード（パスワードと Steam Guard コードは SteamCMD に直接渡され、保存されません）、Steam が隠しているダイアログ（EULA、クラウド同期など）をアプリ内に表示してご自身で回答、32 ビットゲームの警告、ディスク容量を空けるためのアンインストール。
- **システムとネットワークの確認**：Rosetta、macOS のバージョン、メモリ、空き容量、Steam サーバーへの接続性。

## 必要なもの

Apple シリコンの Mac、macOS 14 以降、Rosetta 2、Xcode コマンドラインツール、[Rust](https://rustup.rs)。Wine エンジンはご自身で用意してください（同梱していません）。英語版 README の “Get an engine” を参照してください。

## ビルド

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## 制限事項

- **32 ビットの Windows ゲームはたいてい動きません**（起動直後にクラッシュ）。M2WGame が警告します。
- macOS の Wine では Steam の画面が使えません。上記の方法で回避しています。
- DirectX 12 やアンチチートを使うゲームはほとんど対象外です。
- macOS 27 と M1 Pro でのみ動作確認しています。他の Mac での報告を歓迎します。

## 貢献とライセンス

[CONTRIBUTING.md](../../CONTRIBUTING.md)（パスワードと DRM に関する厳格なルールを含みます）と [SECURITY.md](../../SECURITY.md) をお読みください。ライセンス：[MIT](../../LICENSE)。
