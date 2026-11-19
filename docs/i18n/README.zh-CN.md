# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · **简体中文** · [Español](README.es.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · [Português (BR)](README.pt-BR.md) · [Français](README.fr.md) · [Deutsch](README.de.md)

M2W = Mac 到 Windows（游戏）。让你在 **Apple 芯片 Mac 上运行 Windows 游戏** 的应用：管理 Wine“容器”（独立的 Windows 环境）、Wine 引擎、Windows 版 Steam 和你的游戏库，并用通俗的话解释问题，而不是丢给你一堆日志。

> **状态：早期 alpha 版。** 部分游戏可以运行，部分不行。请先阅读下面的“限制”。应用界面目前**只有越南语**，欢迎提供翻译。英文 README 为准，译文可能略有滞后。

M2WGame **与** Apple、Valve、CodeWeavers 或 Wine 项目**没有任何关联**。它不包含、不下载、也不绕过任何游戏、DRM 或授权检查：你需要拥有合法的游戏。

## 功能

- **游戏库**：开始 / 停止、每个游戏单独设置、查看日志，并给出通俗易懂的故障提示。
- **环境与引擎**：创建环境、为每个环境选择 Wine 引擎、一键安装 Windows 版 Steam。
- **无需 Steam 窗口使用 Steam**（Steam 窗口在 macOS 的 Wine 下是黑的）：一键添加 Steam 库中已有的游戏；在应用内用 SteamCMD 下载新游戏（密码和 Steam 令牌码直接交给 SteamCMD，绝不保存）；在应用内显示 Steam 隐藏的对话框（EULA、云同步等），由你自己回答；提醒 32 位游戏；卸载游戏以释放磁盘空间。
- **系统与网络检查**：Rosetta、macOS 版本、内存、磁盘空间，以及能否连接 Steam 服务器。

## 要求

Apple 芯片 Mac、macOS 14 或更新、Rosetta 2、Xcode 命令行工具、[Rust](https://rustup.rs)。需要自备 Wine 引擎（本项目不附带），详见英文 README 的 “Get an engine”。

## 构建

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## 限制

- **32 位 Windows 游戏通常无法运行**（启动即崩溃），M2WGame 会给出警告。
- Steam 界面在 macOS 的 Wine 下无法使用，项目采用上述方式绕开。
- DirectX 12 和带反作弊的游戏基本无法运行。
- 仅在 macOS 27、M1 Pro 上测试过，欢迎其他机型的反馈。

## 参与贡献与许可

请阅读 [CONTRIBUTING.md](../../CONTRIBUTING.md)（包含关于密码与 DRM 的硬性规则）和 [SECURITY.md](../../SECURITY.md)。许可证：[MIT](../../LICENSE)。
