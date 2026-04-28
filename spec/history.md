# [HISTORY] V.I.B.E

> Volume-Initiated Background Establisher

## 2026-04-26

- 프론트엔드 코딩 가이드를 `spec/code/frontend/` 아래의 주제별 문서로 분리하고,
  에이전트가 코드 작성 전에 관련 스펙을 읽도록 `CLAUDE.md`의 문서 라우팅 규칙을
  정리함.

## 2026-04-28

### 목적

PRD 검토 후 개발 착수. 가장 큰 리스크인 "내장 MacBook 마이크로 박수가 안정적으로
잡히는가"를 가장 먼저 검증하기로 함. 본 구현 전 PoC 단계를 거쳐 리스크부터 정리한다.

### 결정 사항

- **데스크톱 스택:** Tauri (Rust) + React/TS. 백그라운드 저부하와 300ms 반응 속도
  요구 때문에 Electron 대신 Tauri 채택. 오디오 캡처도 Rust(`cpal`)로 직접 처리해
  트리거 → 액션 경로를 짧게 유지.
- **PoC 5개로 분할, main에서 각각 분기, 서로 머지하지 않음.**
  - `poc/audio-capture` (오늘 진행)
  - `poc/clap-detector` (wav 파일 입력 기반)
  - `poc/double-clap` (목업 이벤트로 시작)
  - `poc/action-runner` (실행 지연 측정)
  - `poc/tauri-shell` (트레이 + 자동 실행)
- 각 PoC는 자급자족 구조. 통합은 본 프로젝트에서 인터페이스 계약대로 손으로 옮김.

### 박수 감지 설계 방향 (다음 PoC용 메모)

내장 마이크 환경(타이핑·팬소음·반향)을 기준으로 룰 기반 감지기를 우선 적용.

- 적응형 노이즈 플로어 (EMA 기반 RMS 추정 → 임계값을 배경 대비 비율로 잡음).
- 온셋 검출 (직전 프레임 대비 +6~10dB 급상승).
- 스펙트럴 플럭스 / 광대역 체크 (FFT, 박수=광대역 / 말소리=저주파 / 키보드=고주파).
- 지속 시간 게이트 (피크가 30~80ms 내 80% 감쇠).
- 불응기 50~80ms (반향·이중 검출 방지).
- 이중 박수: 150~600ms 간격 + 두 피크의 에너지·스펙트럼 유사도 비교로 가짜 패턴 거름.

### 진행 단계 (`poc/audio-capture`)

1. `main` 에서 `poc/audio-capture` 브랜치 분기.
2. macOS에 Rust 설치(`rustup` 공식 스크립트, `stable` 툴체인).
3. 저장소 루트에 `cargo init` 으로 binary crate 생성. 의존성: `cpal`, `hound`,
   `anyhow`. 릴리즈 프로파일에 `lto = "thin"` 적용.
4. 코드 구성:
   - `src/capture.rs` — 기본 입력 장치 열기, f32/i16/u16 sample format 처리,
     mono 다운믹스, 콜백 통계(콜백 횟수, 누적 샘플, 최대 콜백 간격), wav 덤프.
   - `src/rms.rs` — 10ms 프레임 RMS, 200ms 윈도우 피크 dBFS + 콘솔 막대 그래프.
   - `src/main.rs` — 콜백 안에서 `RmsMeter` 호출, 5초마다 통계 출력.
5. `.gitignore` 에 `/target`, `*.wav` 추가.
6. `cargo build` 로 컴파일 검증 (경고 0개).

### 인터페이스 계약 (다음 PoC로 이어짐)

```rust
fn on_samples(samples: &[f32], sample_rate: u32)
```

- `samples` 는 mono로 다운믹스된 f32 PCM (-1.0 ~ 1.0).
- `poc/clap-detector` 의 감지기는 이 시그니처에 그대로 붙는다.

### 측정 결과

MacBook Air 내장 마이크에서 약 5분간 캡처하며 박수·타이핑·정적을 섞어서 테스트.

- **장치:** MacBook Air 마이크 / 48,000 Hz / 1ch / F32
- **콜백 max_gap:** 11.25 ~ 11.55 ms (목표 ≤ 20 ms 만족)
- **캡처 안정성:** 5분간 드롭 없이 연속 콜백 (`callbacks 27,676`, `samples 14,170,112`)
- **노이즈 플로어:** EMA로 약 -65 dBFS 근처 수렴 (조용한 실내 기준)
- **박수 피크:** -1.8 / -3.8 dBFS (플로어 대비 +63~65 dB)
- **타이핑 피크:** -52 ~ -57 dBFS (플로어 대비 +15~20 dB)
- **wav 재생 검증:** `samples/test.wav` 로 저장, `afplay` 재생 시 박수·타이핑·말소리 모두 또렷하게 들림
- **박수 vs 타이핑:** 진폭 차이 약 50 dB. 다음 PoC의 룰 기반 감지기로 충분히 분리 가능.

### 발견 사항

- IDE 통합 터미널에서 `\r\x1b[2K` 단일 라인 갱신이 일관되지 않음. 200ms마다 RMS를
  찍으면 스크롤이 폭주하고 `pe peak` 같은 잔재가 보임. 해결: **이벤트 기반 출력으로
  전환** — 노이즈 플로어보다 +8 dB 이상 솟을 때만 `[event]` 라인 출력, 10초 동안
  이벤트 없으면 `[idle]` 라인 한 번. 부수 효과로 적응형 노이즈 플로어(EMA)
  추정도 같이 들어가서 다음 PoC가 그대로 가져다 쓸 수 있는 상태가 됐다.
- 처음에는 `samples/` 디렉토리가 없으면 wav 생성이 실패해 `mkdir -p samples` 필요.
  인자 처리 단계에서 디렉토리 자동 생성으로 보강할지는 본 프로젝트 통합 시 결정.

### 다음 단계

- 박수/타이핑/기침/음악/말소리/문 닫힘 각각 따로 짧게 녹음해 회귀 테스트 셋 확보
  (선택. 통합 wav `samples/test.wav` 만으로도 다음 PoC 시작 가능).
- `poc/clap-detector` 브랜치 시작: 같은 콜백 시그니처(`fn on_samples(samples: &[f32], sample_rate: u32)`)
  를 입력으로 받아 적응형 노이즈 플로어 + 온셋 + 스펙트럴 플럭스 + 지속 시간 게이트로
  박수 1회를 분리. wav 파일 입력으로 회귀 테스트.