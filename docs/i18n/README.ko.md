# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [日本語](README.ja.md) · **한국어** · [Português (BR)](README.pt-BR.md) · [Français](README.fr.md) · [Deutsch](README.de.md)

M2W = Mac to Windows(게임). **Apple 실리콘 Mac에서 Windows 게임을 실행**하도록 돕는 앱입니다. Wine "보틀"(독립된 Windows 환경), Wine 엔진, Windows용 Steam, 게임 라이브러리를 관리하고, 방대한 로그 대신 문제를 쉬운 말로 설명해 줍니다.

> **상태: 초기 알파.** 어떤 게임은 되고 어떤 게임은 안 됩니다. 시간을 쓰기 전에 아래 "제한 사항"을 먼저 읽어 주세요. 앱 화면은 현재 **베트남어만** 지원하며 번역을 환영합니다. 영어 README가 기준이며 번역본은 뒤처질 수 있습니다.

M2WGame은 Apple, Valve, CodeWeavers, Wine 프로젝트와 **관련이 없습니다**. 게임, DRM, 라이선스 확인을 포함하거나 내려받거나 우회하지 않습니다. 게임은 정당하게 구매한 것이어야 합니다.

## 기능

- **게임 라이브러리**: 실행 / 중지, 게임별 설정, 로그 보기, 문제 원인을 쉬운 말로 알려 주는 힌트.
- **환경과 엔진**: 환경 만들기, 환경마다 Wine 엔진 선택, Windows용 Steam 원클릭 설치.
- **Steam 창 없이 Steam 사용**(macOS의 Wine에서는 Steam 창이 검게 보입니다): Steam 라이브러리에 이미 있는 게임을 한 번에 추가, 앱 안에서 SteamCMD로 새 게임 다운로드(비밀번호와 Steam Guard 코드는 SteamCMD에 바로 전달되며 저장되지 않음), Steam이 숨긴 대화 상자(EULA, 클라우드 동기화 등)를 앱 안에 보여 주어 직접 응답, 32비트 게임 경고, 디스크 공간 확보를 위한 게임 삭제.
- **시스템 및 네트워크 점검**: Rosetta, macOS 버전, RAM, 디스크 여유 공간, Steam 서버 접속 가능 여부.

## 요구 사항

Apple 실리콘 Mac, macOS 14 이상, Rosetta 2, Xcode 명령줄 도구, [Rust](https://rustup.rs). Wine 엔진은 직접 준비해야 합니다(포함되어 있지 않음). 영어 README의 "Get an engine"을 참고하세요.

## 빌드

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## 제한 사항

- **32비트 Windows 게임은 대개 실행되지 않습니다**(시작하자마자 종료). M2WGame이 경고해 줍니다.
- macOS의 Wine에서는 Steam 화면을 쓸 수 없어 위에서 설명한 방식으로 우회합니다.
- DirectX 12 게임과 안티치트 게임은 대부분 범위 밖입니다.
- macOS 27과 M1 Pro에서만 테스트했습니다. 다른 Mac의 보고를 환영합니다.

## 기여 및 라이선스

[CONTRIBUTING.md](../../CONTRIBUTING.md)(비밀번호와 DRM에 관한 필수 규칙 포함)와 [SECURITY.md](../../SECURITY.md)를 읽어 주세요. 라이선스: [MIT](../../LICENSE).
