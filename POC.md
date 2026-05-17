# PoC: tauri-shell

V.I.B.E의 다섯 번째이자 마지막 PoC. **메뉴바 상주 + 부팅 시 자동 실행 + 마이크
권한 다이얼로그** 세 가지가 사용자가 원하는 "Mac 켜기 → 박수 두 번" 흐름의
선결 조건이라, 이 셋을 한 자리에 검증하는 게 목표.

이전 PoC 들 (audio-capture / clap-detector / double-clap / action-runner) 와 달리
이번 것은 Tauri 2 + React/TS 기반 데스크톱 앱이라 의존성과 빌드 파이프라인이
훨씬 무거움. 그래서 단계도 8 개로 잘게 쪼개 진행했음.

## 알고리즘 / 구조

Tauri 2 의 `App::setup` 안에서 다음을 등록:

1. `set_activation_policy(ActivationPolicy::Accessory)` — Dock 아이콘 없이 메뉴바
   상주 (macOS LSUIElement 와 동등한 효과).
2. `TrayIconBuilder` 로 메뉴바 아이콘 + 메뉴 8개 항목 등록. 메뉴 최상단에 비활성
   `Status: ...` 라벨, 아래에 Show / Detection / Auto-start / Test microphone /
   구분선 / Quit.
3. `tauri-plugin-autostart` 로 macOS Login Items 등록 (LaunchAgent 방식). 토글
   클릭 시 `enable()` / `disable()` 호출.
4. 메인 윈도우 `visible: false` 로 첫 실행 시 안 뜸. 사용자가 "Show settings"
   클릭하면 `window.show()`. X 눌러 닫으면 `prevent_close()` 후 `hide()` 처리해
   앱 종료 안 됨.
5. `cpal::default_input_device` → `build_input_stream` 으로 마이크 권한 요청.
   처음 호출 시 macOS TCC 가 권한 다이얼로그 표시. NSMicrophoneUsageDescription
   은 `src-tauri/Info.plist` 에 박아두고 Tauri 가 빌드 시 .app 의 Info.plist 로
   머지.

## 인터페이스 계약 (본 통합으로 이어짐)

이번 PoC 는 "통합 받을 인터페이스" 가 아니라 "다른 PoC 들이 통합될 셸" 의 역할.
본 통합 시 다음 패턴으로 다른 PoC 코드를 손으로 옮김:

| 통합 대상 | 호출 위치 | 인터페이스 |
|---|---|---|
| `poc/audio-capture` | 별도 스레드, `setup` 내에서 spawn | `fn on_samples(samples: &[f32], sample_rate: u32)` |
| `poc/clap-detector` | 위 스레드 내 콜백 | `fn detect(samples, sample_rate) -> Vec<ClapEvent>` |
| `poc/double-clap` | clap-detector 출력에 직접 | `fn match_pattern(events, config) -> Vec<TriggerEvent>` |
| `poc/action-runner` | 액션 워커 스레드 | `fn run(action) -> Result<ActionResult, RunError>` |

트리거 감지 → 액션 실행 경로는 `setup` 안에서 채널(`std::sync::mpsc` 또는
`tokio::sync::mpsc`) 로 분리. 마이크 콜백 → 박수 이벤트 → 트리거 → 액션 워커,
각 단계 사이는 채널이 끊어줌. `spec/code/rust/concurrency.md` 의 "real-time
callbacks only do the minimum" 원칙 그대로.

## 실행

개발 모드 (핫리로드):

```bash
pnpm tauri dev
```

프로덕션 .app 번들 + DMG 생성:

```bash
pnpm tauri build --debug   # debug profile + 번들링
pnpm tauri build           # release profile (서명/공증 필요할 수 있음)
```

산출물 경로 (debug):

- `src-tauri/target/debug/bundle/macos/vibe-poc-tauri-shell.app`
- `src-tauri/target/debug/bundle/dmg/vibe-poc-tauri-shell_0.1.0_aarch64.dmg`

## 검증 결과

| 항목 | 결과 |
|---|---|
| 첫 실행 시 메인 윈도우 숨김 + 메뉴바 V 아이콘 등장 | ✓ |
| macOS Dock 아이콘 안 뜸 (Accessory 정책) | ✓ |
| 트레이 메뉴 8 항목 모두 클릭/토글 동작 | ✓ |
| Detection 토글 시 상태 라벨 `Waiting` ↔ `Detection paused` 갱신 | ✓ |
| Auto-start 토글 시 macOS 시스템 설정 → 로그인 항목에 V.I.B.E 등록 | ✓ (사용자 확인) |
| Test microphone 클릭 시 macOS TCC 권한 다이얼로그 표시 | ✓ (사용자가 Allow 클릭) |
| 권한 허용 후 cpal 이 기본 입력 장치 (MacBook Air 마이크) 잡음 | ✓ |
| Show settings 클릭 → 윈도우 등장 → X 닫으면 hide (앱 살아있음) | ✓ |
| Quit 클릭 시 앱 정상 종료 + 트레이 아이콘 사라짐 | ✓ |
| `cargo fmt --check` / `clippy -- -D warnings` / `cargo test` | ✓ |
| `pnpm tauri build --debug` 가 .app + DMG 생성 | ✓ (.app 25 MB) |
| 생성된 .app 의 `Info.plist` 가 NSMicrophoneUsageDescription + LSUIElement 포함 | ✓ |

## 알려진 한계

- **코드 서명 없음.** dev 빌드는 unsigned. Login Items 자동 실행이 실제 reboot 후
  동작하는지는 서명된 .app 을 `/Applications/` 에 설치한 뒤 재검증 필요. PoC 는
  Login Items API 호출이 성공하고 OS 가 등록 항목으로 인지하는 시점까지만 검증.
- **권한 다이얼로그가 dev 모드에서도 뜬 것은 우연성 있음.** Tauri 2 build script
  가 dev 빌드에도 기본 Info.plist 일부를 binary 에 embed 하는 것으로 추정. 다른
  환경 (다른 macOS 버전, 다른 사용자 권한 상태) 에서는 dev 빌드가 다이얼로그를
  못 띄울 수 있음. 본 통합에서는 .app 으로 검증 권장.
- **상태 표시는 현재 두 가지만 (`Waiting`, `Detection paused`, 거부 시 `Mic
  permission missing`).** PRD 7.6 의 "트리거 감지됨 / 루틴 실행 중 / 루틴 실행
  완료 / 루틴 실행 실패" 는 본 통합에서 트리거 감지/루틴 실행이 붙은 뒤 같은
  패턴으로 확장.
- **단위 테스트 0개.** Tauri shell 은 시스템 통합 위주 (트레이 / Login Items /
  TCC 다이얼로그) 라 단위 테스트로 검증할 표면이 거의 없음. CI 통합 시점에
  E2E 스모크 (예: 빌드된 .app 이 메뉴바에 뜨는지) 가 더 의미 있음.
- **활성 루틴 전환 UI 없음.** PRD 11 의 "활성 루틴 전환 (메뉴바)" 는 본 통합
  단계 책임 (`Routine Editor` 작업과 묶임). 현재 PoC 는 트레이 메뉴 패턴 검증만.

## 비목표 (이번 PoC 안 함)

- 실제 박수 감지와 트리거 → 액션 파이프라인 연결. 이전 PoC 들과의 통합은 본
  프로젝트 단계 책임 (PoC 끼리 머지 안 함 정책).
- 설정 UI (루틴 편집기, 감도 슬라이더 등). React 프런트엔드 페이지는 Tauri
  스캐폴드 기본 그대로. UI 구체화는 본 통합.
- 코드 서명, 공증, App Store 배포 준비. 사용자가 직접 .app 을 실행하는 PoC 수준.
- 다른 트리거 (휘파람, 단축키 등) 확장. PRD 2 에 미래 확장으로만 명시.

## 의존성 메모

이번 PoC 부터 패키지 매니저는 **pnpm** 사용 (이전 PoC 들은 npm 이 기본이었으나
사용자 선호로 pnpm 으로 전환). `pnpm approve-builds --all` 한 번 실행해
esbuild postinstall 승인 → `pnpm-workspace.yaml` 에 영구 저장.

주요 Rust 의존성:

- `tauri = "2"` (features: `tray-icon`, `image-png`)
- `tauri-plugin-opener = "2"` (스캐폴드 기본)
- `tauri-plugin-autostart = "2"` (Login Items)
- `cpal = "0.15"` (마이크 권한 요청)
- `serde`, `serde_json` (Tauri 명령 직렬화)

프런트:

- `react`, `react-dom` (19.x)
- `@tauri-apps/api`, `@tauri-apps/plugin-opener`
- `vite`, `typescript`
