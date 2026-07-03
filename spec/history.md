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

## 2026-05-06

### 목적

`poc/clap-detector` 진행. 가설: 적응형 노이즈 플로어 + 온셋 + 스펙트럴 광대역 체크
+ 지속 시간 게이트로 단발 박수를 잡고 타이핑·말소리·정적·음악 비트를 거를 수 있다.

### 결정 사항

- **rust 컨벤션 spec 정리:** `spec/code/rust/` 아래에 idioms / errors / concurrency
  / tooling 4개 문서로 일반 Rust 컨벤션 템플릿 작성. CLAUDE.md 에 라우팅 추가 +
  Spec Reference Disclosure 규칙 추가 (코드 작성 전에 어느 문서의 어느 섹션을
  참고하는지 사용자에게 공시) + Language Convention (코드/식별자/주석/에러는 영어,
  사용자-에이전트 소통만 한국어). AGENTS.md 도 CLAUDE.md 미러로 main 에 커밋.
- **ground truth 단순화:** Audacity 로 정확한 시점 라벨링하는 대신 "박수 횟수" +
  "음성 샘플은 검출 0" 으로 회귀 검증. PoC 단계에 충분.
- **약한 박수 무시:** 본 프로젝트 사용 패턴상 매우 약한 박수까지 잡을 필요가 없다고
  사용자와 합의. `onset_threshold_db = 32` 로 설정해 위양성 0 우선.

### 박수 감지 알고리즘 (4단계)

1. **에너지 게이트** — 적응형 EMA 플로어 대비 +32 dB 이상 솟은 프레임만 후보.
2. **광대역 체크** — Hann window + FFT(512) → spectral flatness ≥ 0.20.
3. **지속 시간 게이트** — 60 ms 안에 RMS 가 20 dB 이상 떨어져야 통과.
4. **불응기 120 ms** — 반향·이중 검출 방지.

### 진행 단계

1. `main` 에서 `poc/clap-detector` 분기.
2. `cargo init` binary crate. 의존성: `hound`, `rustfft`, `anyhow`, `thiserror`.
3. 모듈 구성 (`spec/code/rust/idioms.md`, `spec/code/rust/errors.md` 참고):
   - `src/wav.rs` — `WavError` (thiserror) + `load_mono` (16/24/32-bit PCM, f32 지원).
   - `src/features.rs` — `rms_db`, `FlatnessAnalyzer` (FFT 기반 spectral flatness).
   - `src/floor.rs` — `AdaptiveFloor` (EMA).
   - `src/detector.rs` — `ClapEvent`, `DetectorConfig`, `detect()` 본체.
   - `src/lib.rs` + `src/main.rs` — `tests/` 에서 import 가능하도록 라이브러리 노출.
4. 단위 테스트 (모듈 내 `#[cfg(test)] mod tests`): RMS·flatness·EMA·detector 음성
   케이스. 합성 박수 양성 테스트는 결정적 합성 신호가 실제 박수의 spectral/temporal
   특성을 정확히 흉내내지 못해 위양성 우려 있어 제외. 대신 wav 기반 회귀 테스트로 보강.
5. 회귀 테스트 (`tests/regression.rs`): `tests/data/claps_short.wav` (claps_solo
   첫 6초) 를 로드해 1~3건 검출 + 각 이벤트가 박수 신호 특성 만족하는지 검증.
   `.gitignore` 에 `!tests/data/*.wav` 예외 추가.
6. 4개 wav 시나리오로 임계값 그리드 튜닝 → 위양성 0 + recall 100%(강한 박수) 도달.
7. `cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings`
   통과 (`spec/code/rust/tooling.md` 체크리스트).

### 인터페이스 계약 (다음 PoC `poc/double-clap` 으로 이어짐)

```rust
pub struct ClapEvent {
    pub timestamp_ms: u64,
    pub peak_db: f32,
    pub above_floor_db: f32,
    pub flatness: f32,
    pub confidence: f32,
}

pub fn detect(samples: &[f32], sample_rate: u32) -> Vec<ClapEvent>
```

순수 함수. 동일 입력에 동일 출력. 내부 상태는 호출당 새로 만들어 폐기.

### 측정 결과

| 샘플 | 길이 | 검출 | 평가 |
|---|---|---|---|
| `claps_solo.wav` | 24.5 s | 8 | 강한 박수 모두 검출. 매우 약한 박수 1회(-29 dBFS)는 의도적으로 임계값 아래로 빠짐 |
| `typing.wav` | 39.6 s | 0 | 위양성 0 ✓ |
| `voice.wav` | 29.1 s | 0 | 위양성 0 ✓ |
| `silence.wav` | 23.2 s | 0 | 위양성 0 ✓ |
| `test.wav` (혼합) | 21.2 s | 2 | 실제 박수 2회 정확히 검출 |

튜닝값:
`onset_threshold_db=32`, `flatness_threshold=0.20`, `decay_drop_db=20`,
`decay_window_ms=60`, `refractory_ms=120`, `floor_alpha=0.05`.

### 발견 사항

- **audio-capture wav 헤더 미완료:** audio-capture 가 `std::mem::forget(stream)`
  로 무한 루프를 돌고 Ctrl+C 로 종료되면 `hound::WavWriter` 가 finalize 되지 못해
  RIFF/data chunk 사이즈가 0으로 남음. 결과적으로 hound 가 0 sample 로 읽음.
  단발성으로 Python 한 줄 스크립트(파일 사이즈로 헤더 패치) 로 복구. 후속 작업으로
  audio-capture 에 SIGINT 핸들러 추가해 wav 를 정상 종료시키는 패치 필요.
- **합성 박수 단위 테스트 한계:** 결정적 LCG 기반 합성 박수가 실제 박수의 RMS
  감쇠 프로파일(50ms 안에 60dB drop)을 정확히 흉내내지 못해 detector 가 거부함.
  단위 테스트로는 음성 케이스(silence, low-noise, sustained sine)만 두고 양성
  검증은 wav 기반 회귀 테스트로 분리.
- **tuning trade-off:** `onset_threshold_db = 30` 으로 낮추면 약한 박수까지 잡지만
  타이핑 강타 키 1~2건이 위양성으로 들어옴. PoC 정책상 위양성 0 우선이라 32 채택.

### 다음 단계

- 후속 패치: `poc/audio-capture` 에 SIGINT 핸들러 추가해 wav writer 를 정상 종료.
- `poc/double-clap` 시작: 본 PoC 의 `Vec<ClapEvent>` 를 입력으로 받아 150~600 ms
  간격 + 두 이벤트 유사도(에너지·flatness) 비교로 이중 박수 패턴 추출. 목업
  이벤트 배열로 시작 가능 (clap-detector 결과 안 기다림).

## 2026-05-17

### 목적

`poc/double-clap` 진행. 가설: `poc/clap-detector` 가 뱉는 `ClapEvent` 시퀀스에
대해 간격 게이트(150~600 ms) + 두 박수의 피크·flatness 유사도 비교만으로
"박수-말-박수" 같은 가짜 패턴까지 거른 이중 박수 트리거를 만들 수 있다.

이번 PoC 는 오디오 신호 처리를 다시 하지 않는다. `ClapEvent` 가 신호 처리 결과를
요약한 인터페이스라는 가정에 기댄다 — PoC 끼리 머지 안 함 정책상 clap-detector 의
`ClapEvent` 구조체를 그대로 옮겨 적었다 (의도된 중복).

### 결정 사항

- **인터페이스 계약 보강:** 계획 단계에서는 `match_pattern(...) -> Option<TriggerEvent>`
  였으나, 오프라인 입력이라 한 시퀀스에 트리거가 여러 개 나올 수 있어
  `Vec<TriggerEvent>` 로 확장. 슬라이딩 윈도우로 호출해도 동일하게 동작하도록
  내부 상태는 두지 않음.
- **`analyze` API 추가:** `match_pattern` 외에 기각 사유까지 노출하는 `analyze` 를
  공개. PoC 의 핵심은 "왜 안 됐는지" 가 보여야 튜닝이 되므로 디버깅용으로 분리.
  실시간 통합에서는 `match_pattern` 만 쓰면 됨.
- **회귀 테스트 입력은 합성 `ClapEvent`:** wav 녹음 → clap-detector → 매처 파이프라인
  검증은 본 통합에서 손으로 연결할 일이라 PoC 단계에서는 합성 이벤트 시나리오만으로
  룰 동작을 확인. 13개 단위 테스트(빈/단일/정상/간격 게이트 양 끝/피크 격차/flatness
  격차/3·4 박수 연속/기각 후 재매칭/경계값) 가 알고리즘 모든 분기를 커버.

### 매칭 알고리즘 (4단계)

1. **이웃 페어링** — 시간순 이벤트에서 인접한 두 박수를 후보 쌍으로 본다. 트리거가
   인정된 쌍은 두 박수 모두 소비, 기각된 쌍은 두 번째 박수가 그 다음 박수와
   다시 짝지을 수 있게 한 칸만 전진.
2. **간격 게이트** — `min_interval_ms ≤ Δt ≤ max_interval_ms` (기본 150~600 ms).
3. **피크 유사도** — `|peak_db_a - peak_db_b| ≤ 12 dB`.
4. **광대역 유사도** — `|flatness_a - flatness_b| ≤ 0.25`.

`confidence` 는 두 박수의 confidence 평균 + 피크/flatness/간격이 중심에 얼마나
가까운지로 합성한 점수.

### 진행 단계

1. `main` 에서 `poc/double-clap` 분기.
2. `cargo init` binary crate. 의존성: `anyhow`, `thiserror`, `serde`, `serde_json`
   (JSON 입력 디버깅용). release profile 에 `lto = "thin"`.
3. 모듈 구성 (`spec/code/rust/idioms.md`, `spec/code/rust/errors.md` 참고):
   - `src/event.rs` — `ClapEvent` (clap-detector 와 동일 필드, serde derive 추가).
   - `src/matcher.rs` — `MatcherConfig`, `TriggerEvent`, `PairOutcome`,
     `RejectReason`, `match_pattern`, `analyze`. 매처는 순수 함수.
   - `src/lib.rs` + `src/main.rs` — JSON 파일 인자 또는 빌트인 데모 5개 실행.
4. 단위 테스트 13개를 `matcher` 모듈 내부에 코로케이션. 모든 기각 사유와 경계값,
   3·4 연속 박수 케이스까지 커버.
5. `cargo fmt` + `cargo clippy --all-targets --all-features -- -D warnings` +
   `cargo test` 모두 통과. 데모 출력으로 각 시나리오 결과 눈으로 확인.

### 인터페이스 계약 (다음 PoC `poc/action-runner` 로 이어짐)

```rust
pub struct TriggerEvent {
    pub first_at_ms: u64,
    pub second_at_ms: u64,
    pub interval_ms: u64,
    pub confidence: f32,
}

pub fn match_pattern(events: &[ClapEvent], config: &MatcherConfig) -> Vec<TriggerEvent>
```

순수 함수. 호출자가 시간순 정렬 책임. action-runner 는 `TriggerEvent` 를 받아 실행할
액션을 결정.

### 데모 결과

| 시나리오 | 결과 |
|---|---|
| 유사 박수 300 ms 간격 | TRIGGER conf=0.84 ✓ |
| 간격 120 ms | reject: interval too short ✓ |
| 간격 800 ms | reject: interval too long ✓ |
| 피크 격차 20 dB | reject: peak mismatch ✓ |
| 박수 3개 연속 (100/400/700 ms) | TRIGGER (1,2), 3번 미매칭 ✓ |

### 발견 사항

- **초기 `Option<TriggerEvent>` 시그니처의 한계:** 한 호출에서 트리거가 여러 개
  나올 수 있는데(긴 wav 스트림 처리 시) `Option` 으로 묶으면 호출자가 외부 루프를
  돌아야 한다. `Vec` 으로 받으면 슬라이딩 윈도우로 한 페어씩 잘라 호출해도 길이 0/1
  결과로 동일하게 동작해 본 통합 시점에서 선택 폭이 넓어진다.
- **유사도 게이트의 가짜 패턴 방어 한계:** "박수-기침-박수" 는 clap-detector 가
  기침을 거른다면 매처는 보지도 못한다. 만약 기침이 박수로 잘못 검출됐을 때 유사도
  게이트가 두 번째 안전망 역할을 함. 실제 효과는 통합 단계에서 측정 필요.

### 다음 단계

- `poc/action-runner` 시작: `TriggerEvent` 받아 macOS 액션(앱 실행, URL, osascript,
  shortcuts run) 의 실제 지연 측정. 300 ms 이내 목표.
- 후속 패치 백로그 그대로 유지: audio-capture SIGINT 핸들러.

## 2026-05-17 (PRD 정제)

### 목적

`poc/action-runner` 진행 중 사용자가 PoC 의 액션 종류 4종(앱 실행, URL, osascript,
shortcut) 이 본인이 원하는 제품 형태와 맞는지 의문을 제기. PRD 7.4 의 액션 예시
8종이 모두 진짜 필요한 것은 아니라는 인식이 잡혀, 본격 구현 전에 PRD 자체를 사용자
의도에 맞게 좁히기로 함.

### 사용자 확인된 최종 흐름

1. 최초 설치 시 마이크 권한 + "Mac 시작 시 자동 실행" 토글 한 번만 설정.
2. 그 다음부터는 **Mac 켜기 → 박수 두 번** 두 단계로 활성 루틴 실행.
3. V.I.B.E 자체에 사용자 계정 / 로그인 / 회원가입 없음. 완전 로컬, 외부 통신 X.
4. 액션은 **앱 실행** 과 **URL 열기** 두 종만 사용. 나머지는 차후 확장.
5. 여러 루틴 보유 가능. 한 번에 하나만 "활성 루틴" 으로 박수에 반응. 활성 루틴
   전환은 메뉴바 메뉴에서 수행.

### PRD 수정 사항

- **7.4 액션 및 루틴 시스템:** 액션 예시 8종 → "MVP 2종 (앱 실행, URL 열기)" +
  "확장 후보 6종" 으로 재구성. 루틴 실행은 MVP 에서 순차로 고정 (지연/병렬은 확장).
  활성 루틴 단일 + 메뉴바 전환 정책 명시.
- **10. 보안 및 권한:** "V.I.B.E 자체에는 사용자 계정 / 로그인 / 회원가입이 없다"
  를 첫 항목으로 명시. 모든 사용자 데이터는 로컬 파일에만 저장.
- **11. MVP 범위:** "Shell Script 또는 Shortcuts 실행 액션" 줄 제거. "활성 루틴
  전환 (메뉴바)" 항목 추가. 하단에 "Mac 켜기 → 박수 두 번" 흐름을 우선한다는
  요약 박스 추가.
- **12. 상세 작업 리스트:** Action Runner 를 "앱 실행, URL 열기 (MVP 2종)" 으로
  좁힘. "Active Routine Switcher" 작업 항목 추가.

### PoC 영향

- `poc/action-runner` 의 측정 대상: 4종(osascript/open-app/open-url/shortcut)
  → **2종 (open-app, open-url)** 으로 좁힘.
- osascript 와 shortcut 은 코드 경로(`Action` enum) 와 단위 테스트로 검증된 상태
  유지하되, 실제 측정값 수집 대상에서 제외. 본 통합 단계에서 빠질 수도 있고, 향후
  확장 단계에서 다시 들어올 수도 있음.
- 측정 결과 자체는 5단계에서 이미 수집됨 (open-app dispatch p95=158 ms, open-url
  p95=134 ms — PRD 의 300 ms 목표 절반 수준).

### 결정 사항

- PRD 의 **3. 목표** 와 **6. 사용자 시나리오** 는 손대지 않음. 비전 / 흐름은 그대로.
  좁힌 것은 액션 종류와 MVP 범위뿐.
- 확장 단계 액션(Shell Script, AppleScript, Shortcuts, 음악 제어, 볼륨 등) 은
  PRD 에 "확장 후보" 로 박제. 미래에 들어올 수도 있고 안 들어올 수도 있음.

### 다음 단계

- `poc/action-runner` 의 POC.md 작성 시 MVP 2종에 초점. 5단계에서 측정한 osascript
  결과는 "확장 후보 참고용" 으로 부록 처리.
- `poc/action-runner` 마무리 후 `poc/tauri-shell` 진행. 메뉴바 상주 + Login Items
  자동 실행 + 권한 다이얼로그 검증. 활성 루틴 전환 UI 는 본 통합 단계 책임.

## 2026-05-17 (poc/action-runner)

### 목적

`poc/double-clap` 의 `TriggerEvent` 를 받아 macOS 액션을 실제 subprocess 로 실행
했을 때 PRD 의 "박수 감지 후 첫 액션까지 300 ms" 목표를 깰 가능성이 있는지
확인. 사용자가 단계별로 차근차근 진행하길 원해 PoC 한 덩어리를 8단계로 끊고,
매 단계 시작 전에 "뭘 + 왜" 한 문단으로 공시하는 방식으로 진행.

### 결정 사항

- **타입 설계 (2단계) 에서 spawn vs dispatch 분리:** PRD 는 "300 ms 이내" 만 명시
  하지만 측정 시점이 두 개 — `Command::spawn` 반환 시점(fork 비용) vs subprocess
  종료 시점(LaunchServices 위임 완료). 둘 다 측정해 사용자 체감 지연의 진짜
  주체가 무엇인지 분리.
- **subprocess 동기 모델 유지:** `fn run(&Action) -> Result<ActionResult, RunError>`
  은 호출 스레드에서 `wait()` 까지 블로킹. 본 통합에선 트리거 감지 스레드가 직접
  호출하지 않고 메시지 큐로 액션 워커에 위임 (`spec/code/rust/concurrency.md` 의
  "real-time callbacks only do the minimum" 원칙 그대로).
- **반복 측정 분포 압축:** N=5 측정값을 min/p50/p95/max 로 압축. warmup=1 로 첫
  fork/exec 캐시 cold 비용 흡수. PoC 단계 규모에 충분.
- **PRD 정제 (별도 섹션 참조):** PoC 진행 도중 사용자가 4종 액션 측정의 정당성에
  의문 제기 → PRD 7.4 / 10 / 11 / 12 를 사용자 의도 (앱 + URL 만 사용, 로컬 전용,
  로그인 없음) 에 맞게 좁힘. PoC 의 측정 대상도 MVP 2종에 좁힘.
- **코드는 4종 그대로 유지:** `Action` enum 의 `Osascript` / `Shortcut` 도 단위
  테스트 + parse + Display 까지 모두 구현 상태로 둠. 확장 단계에 다시 켤 때
  코드 작성 비용 0. 측정만 MVP 에 집중.

### 진행 단계 (8개로 쪼갬)

1. `main` 에서 `poc/action-runner` 분기 + cargo init binary crate (anyhow, thiserror).
2. 타입 설계 — `Action` enum 4종, `ActionResult`, `RunError` (Spawn / Wait 분리).
   `program()` / `args()` / `kind_label()` 헬퍼로 subprocess 호출 형태 명세.
3. `fn run` 본체 구현 — `Command::spawn` + `child.wait()` 사이 시각 기록. open-app
   Calculator 로 1회 검증 (spawn 2.37 ms, dispatch 61.78 ms).
4. 3종 액션 (osascript, open-app, open-url) 직렬 실행 — 인터페이스 동일성 확인.
   open-url 호출 시 사용자가 example.com 탭이 실제로 뜬 걸 확인 (의도된 동작).
5. 반복 측정 + 분포 — `Distribution` (min / p50 / p95 / max), `Stats`,
   `measure(action, repetitions, warmup)`. warmup=1, reps=5 로 첫 측정값 수집.
6. CLI 인자 처리 — `Action::parse(kind, param)` 추가, 0 / 2 인자 분기, usage 출력.
7. fmt / clippy / test 통과 확인 (단위 테스트 10개).
8. POC.md + 커밋 + history 기록.

### 측정 결과

| 액션 | spawn p50 / p95 (ms) | dispatch p50 / p95 (ms) | dispatch max (ms) |
|---|---|---|---|
| open-app (Calculator) | 0.45 / 1.33 | 45.97 / 157.56 | 157.56 |
| open-url (example.com) | 1.66 / 3.52 | 99.27 / 133.62 | 133.62 |
| osascript (참고 baseline) | 0.45 / 0.56 | 30.79 / 32.22 | 32.22 |

**PRD 300 ms 목표 검증:** MVP 2종 모두 dispatch p95 < 160 ms — 목표의 절반.
액션 실행부가 PRD 의 반응 속도 목표를 깰 리스크는 거의 없음. 병목은 마이크 → 박수
감지 → 매처 경로 쪽으로 좁혀짐.

### 인터페이스 계약 (다음 PoC `poc/tauri-shell` 로 이어짐)

```rust
pub enum Action {
    OpenApp { name: String },
    OpenUrl { url: String },
    Osascript { script: String },     // 확장 후보, 측정 baseline
    Shortcut { name: String },        // 확장 후보, 미측정
}

pub fn run(action: &Action) -> Result<ActionResult, RunError>
```

`tauri-shell` 은 트레이 / Login Items / 권한 다이얼로그가 책임. `run` 자체는
변경할 일 없고 본 통합 시 호출만 한다.

### 발견 사항

- **dispatch 시간이 spawn 시간보다 30~150 배 큼.** 액션 종류와 무관하게 fork / exec
  자체는 0.5~2 ms 수준. 진짜 비용은 LaunchServices (open-app), 브라우저 IPC
  (open-url), 스크립트 인터프리터 (osascript). 본 통합에서 액션 지연을 줄이려면
  spawn 최적화보다 시스템 컴포넌트 워밍업이 더 효과적.
- **open-app max=157 ms 의 cold variance.** warmup=1 이 다 흡수 못 함. Mac 부팅 후
  첫 박수 트리거는 평소보다 느릴 수 있음. 사용자 체감 차이는 크지 않을 가능성.
- **subprocess stdio inherit.** osascript 의 `return 1` 결과가 부모 stdout 으로
  새어나옴 (`1` 이 측정 표 사이에 끼임). 측정값에는 영향 없음. 본 통합에서
  `.stdout(Stdio::null())` 로 정리.

### 다음 단계

- `poc/tauri-shell` 시작: 마지막 PoC. 메뉴바 상주, Login Items 자동 실행, 마이크
  권한 다이얼로그. Tauri 빈 앱부터 시작해 트레이 아이콘 + 자동 실행까지.
- 본 통합 전 정리할 부수 패치 (그대로):
  - `poc/audio-capture` 의 SIGINT 핸들러 (wav writer finalize).
  - 본 통합 시 액션 실행기에 `.stdout(Stdio::null())` 적용.

## 2026-05-18 (poc/tauri-shell)

### 목적

V.I.B.E 의 마지막 PoC. 사용자가 원하는 "Mac 켜기 → 박수 두 번" 흐름의 선결
조건인 **메뉴바 상주 + Login Items 자동 실행 + 마이크 권한 다이얼로그** 셋을
한 자리에 검증. 이전 PoC 들과 달리 Tauri 2 + React/TS 기반 데스크톱 앱이라
의존성과 빌드 파이프라인이 무거워서 단계를 8개로 잘게 쪼개 진행.

### 결정 사항

- **Tauri 2.x:** 1.x 가 deprecated 되는 시점이라 처음부터 2 로 시작. 트레이 /
  Login Items / 권한 모두 2 의 기본 API + 공식 플러그인으로 커버 가능.
- **패키지 매니저 pnpm 으로 전환:** 1단계 시작 직후 사용자가 pnpm 선호 명시.
  메모리 (`feedback_pnpm_default.md`) 에 기록. 이후 모든 npm 명령 → pnpm.
  esbuild postinstall 차단 이슈는 `pnpm approve-builds --all` 한 번으로 해결,
  결과가 `pnpm-workspace.yaml` 에 영구 저장됨.
- **Dock 아이콘 없이 메뉴바 전용:** `ActivationPolicy::Accessory` + `LSUIElement`
  Info.plist 키 둘 다 적용. PRD 7.2 의 "기본적으로 백그라운드 또는 메뉴바 상태로
  시작" 충족.
- **윈도우는 숨김 시작 + 닫기 무력화:** `tauri.conf.json` 에 `visible: false`,
  `prevent_close` 핸들러로 X 눌러도 hide 만. 메뉴바 트레이가 진짜 라이프사이클
  주체.
- **마이크 권한은 "Test microphone" 메뉴로 명시 호출:** 앱 시작 시 자동 호출하면
  dev 핫리로드마다 다이얼로그가 폭주. 사용자가 메뉴 클릭해야 cpal 이 권한 요청
  하는 방식으로.
- **단위 테스트 0 개:** Tauri shell 은 시스템 통합 위주라 단위 테스트로 검증할
  표면이 거의 없음. CI 통합 시점에 E2E 스모크가 더 의미 있다고 판단.

### 진행 단계 (8 개로 쪼갬)

1. `main` 에서 `poc/tauri-shell` 분기 + `pnpm create tauri-app` 으로 React+TS
   템플릿 스캐폴드 + 저장소 루트로 복사 (`main` 의 README 유지).
2. 메뉴바 트레이 아이콘 + 윈도우 hidden + Accessory 정책 적용. `tray-icon`,
   `image-png` feature 추가.
3. 트레이 메뉴 4 항목 (Show / Detection(check) / 구분선 / Quit) + `CheckMenuItem`
   토글 패턴 + 상태 `Mutex<bool>` 보관.
4. `tauri-plugin-autostart` 추가 → 메뉴에 "Auto-start on login" 체크 항목. 사용자가
   시스템 설정 → 로그인 항목에서 등록 확인.
5. `cpal = "0.15"` + `src-tauri/Info.plist` (NSMicrophoneUsageDescription +
   LSUIElement) → "Test microphone" 메뉴 클릭 시 cpal 이 build_input_stream 호출
   → macOS TCC 다이얼로그 표시 → 사용자가 Allow 클릭 → `MacBook Air 마이크` 잡힘.
6. `AppStatus` enum (Waiting / DetectionPaused / MicPermissionMissing) + 메뉴 맨
   위 비활성 라벨 + Detection 토글에 따라 라벨 자동 갱신.
7. `cargo fmt --check` / `clippy -- -D warnings` / `cargo test` 통과 (`let _ =`
   1 곳 정리). `pnpm tauri build --debug` 로 .app 25 MB + DMG 생성. 생성된 .app
   의 Info.plist 에 우리 키 (`NSMicrophoneUsageDescription`, `LSUIElement`) 박힘
   확인.
8. POC.md + 커밋 + history 기록.

### 검증 결과

| 항목 | 결과 |
|---|---|
| 첫 실행 시 메인 윈도우 숨김 + 메뉴바 V 아이콘 등장 | ✓ |
| Dock 아이콘 안 뜸 (Accessory) | ✓ |
| 트레이 메뉴 8 항목 동작 | ✓ |
| Detection 토글 시 상태 라벨 갱신 | ✓ |
| Auto-start 토글 시 시스템 설정 로그인 항목에 등록 | ✓ |
| Test microphone 클릭 시 macOS TCC 다이얼로그 표시 | ✓ |
| Allow 후 cpal 이 MacBook Air 마이크 잡음 | ✓ |
| Show settings → 윈도우 X 닫기 → 앱 살아있음 | ✓ |
| Quit 시 정상 종료 | ✓ |
| fmt / clippy / test | ✓ |
| `pnpm tauri build --debug` 가 .app + DMG 생성 | ✓ |

### 인터페이스 계약 (본 통합으로 이어짐)

이번 PoC 는 "다른 PoC 들이 통합될 셸". 본 통합 시:

| 통합 대상 | 호출 위치 | 인터페이스 |
|---|---|---|
| `poc/audio-capture` | 별도 스레드, `setup` 내 spawn | `fn on_samples(samples, sample_rate)` |
| `poc/clap-detector` | 위 스레드 콜백 | `fn detect(samples, sample_rate) -> Vec<ClapEvent>` |
| `poc/double-clap` | clap-detector 출력 | `fn match_pattern(events, config) -> Vec<TriggerEvent>` |
| `poc/action-runner` | 액션 워커 스레드 | `fn run(action) -> Result<ActionResult, RunError>` |

마이크 콜백 → 박수 이벤트 → 트리거 → 액션 워커, 각 단계 사이는 채널 (`mpsc`)
로 끊음.

### 발견 사항

- **dev 모드에서도 권한 다이얼로그가 떴다.** Tauri 2 의 build script 가 dev
  binary 에도 일부 Info.plist 를 embed 하는 것으로 추정. 다른 환경 (다른 macOS,
  다른 사용자 권한 상태) 에서는 dev 가 다이얼로그 못 띄울 가능성 있어 본 통합
  단계에서는 .app 으로 재검증 권장.
- **pnpm 11 의 strict postinstall.** esbuild 가 막혀서 `pnpm install` 이 exit 1.
  `pnpm approve-builds --all` 한 번이면 `pnpm-workspace.yaml` 에 영구 저장되고
  이후 클린.
- **Info.plist 머지 자동.** `src-tauri/Info.plist` 를 파일로 두면 Tauri 가
  `tauri.conf.json` 의 자동 생성 Info.plist 와 머지해 최종 .app 의 Info.plist 에
  사용자 키가 들어감. 추가 설정 불필요.
- **Login Items 등록은 서명 없이도 등록 자체는 됨.** 실제 reboot 후 자동 실행
  여부는 unsigned 빌드에서 보장 안 됨. 본 통합 단계에서 서명 .app + /Applications/
  설치로 재검증 필요.

### 다음 단계

- **PoC 5 개 모두 완료.** 본 프로젝트 단계 진입.
- 본 통합 (`main` 의 새 디렉토리 구조에 PoC 코드를 손으로 옮김) 순서 제안:
  1. `poc/tauri-shell` 을 베이스로 본 프로젝트 셸을 시작.
  2. `poc/audio-capture` → `poc/clap-detector` → `poc/double-clap` 을 백엔드
     스레드로 옮김 (mpsc 채널 + tokio 또는 std::thread 결정).
  3. `poc/action-runner` 를 액션 워커 스레드로 옮김.
  4. 프런트엔드 React 페이지에 루틴 편집기 (Routine Editor) + 활성 루틴 전환
     UI 추가.
- 누적된 부수 패치 백로그:
  - `poc/audio-capture` 의 SIGINT 핸들러 (wav writer finalize).
  - 본 통합 시 액션 실행기에 `.stdout(Stdio::null())` 적용.
  - Tauri 셸에 코드 서명 + /Applications/ 설치 후 reboot 자동 실행 재검증.
- 브랜치 전략 전환: PoC 단계는 끝. 이제 `main` → `dev` → `feat/*` 모델로 운영
  (memory `project_branching_strategy.md` 의 본격 단계 정책 적용).

## 2026-07-03 (본 통합 시작: feat/app-shell + feat/audio-engine)

### 목적

PoC 5개 완료 후 본 제품 통합 시작. `main` → `dev` 분기 후 기능 단위 `feat/*`
브랜치로 작업하고 완성되면 `dev` 에 머지하는 모델로 전환. 첫 두 feat 로
(1) tauri-shell PoC 를 제품 토대로 포팅, (2) 가장 큰 미지수였던 **라이브
스트리밍 박수 감지** 를 구현·검증.

### 결정 사항

- **PoC 브랜치는 박제 유지.** 머지하지 않고 인터페이스 계약을 보고 제품 코드를
  손으로 새로 작성. `git checkout poc/<branch> -- <paths>` 로 파일 단위 선별 포팅.
- **`feat/app-shell`:** tauri-shell 의 앱 스캐폴드만 포팅 (문서 제외). 크레이트/
  productName/identifier 를 `vibe` / `V.I.B.E` / `com.vibe.app` 으로 정리.
- **`ClapEvent` 단일 정의로 통합:** PoC 에서 clap-detector 와 double-clap 이
  의도적으로 중복 정의했던 구조체를 `engine/event.rs` 하나로 합침.
- **detector 를 배치 → 스트리밍으로 재작성:** PoC 의 `detect()` 는 wav 한 덩어리
  입력의 순수 함수라 실시간 마이크 콜백(조각 입력)에 그대로 못 씀.
  `StreamingDetector` 가 노이즈 플로어(EMA)·불응기·decay 대기 후보를 호출 사이에
  유지. **조기 확정** 방식: decay 윈도우 끝까지 기다리지 않고 20 dB 하락을
  처음 관측한 프레임에서 즉시 이벤트 확정 → 박수 후 최대 ~60 ms 안에 발화.
- **matcher 는 순수 로직 + 스테이트풀 래퍼:** `evaluate_pair` (순수) 위에
  미소비 박수 1개만 보관하는 `StreamingMatcher`. 기각 페어는 한 칸 전진(배치와
  동일 의미론). PoC 의 디버깅용 `analyze` API 는 제품에서 제외.
- **스레드 구성 (concurrency 스펙 적용):** 오디오 스레드(cpal 스트림 소유,
  콜백은 다운믹스+`try_send` 만) → 감지 스레드(detector+matcher) → 이벤트 워커
  (액션 실행이 감지를 블로킹하지 못하게 분리). 채널은 bounded(64) `sync_channel`,
  cpal `Stream` 이 `!Send` 라 오디오 스레드가 스트림을 소유하고 stop 플래그로 종료.
- **하드코딩 루틴으로 수직 검증:** 루틴 스토어 전에 "박수 2번 → 계산기 실행" 을
  `lib.rs` 에 박아 파이프라인 전체를 실기기로 먼저 검증. `open` 호출에 백로그였던
  `Stdio::null()` 적용.
- **Detection 토글 연결:** 트레이 메뉴의 Detection 체크가 엔진 `AtomicBool` 로
  연결. 일시정지 → 재개 시 `matcher.reset()` 으로 정지 전 박수와 재개 후 박수가
  짝지어지는 것 방지.

### 진행 단계

1. `main` → `dev` 분기 + push. `dev` → `feat/app-shell` 분기.
2. tauri-shell 앱 파일 선별 포팅 + 네이밍 정리 + fmt/clippy 통과 → dev 머지.
3. `dev` → `feat/audio-engine` 분기. `engine/` (event, floor, features, detector,
   matcher) + `audio.rs` + `pipeline.rs` 작성, `lib.rs` 배선.
4. 단위 테스트 23개 + wav 회귀 테스트 3개. 회귀용 `claps_short.wav` 는
   poc/clap-detector 브랜치에서 추출 (`.gitignore` 에 예외 추가).
5. `pnpm tauri dev` 로 라이브 검증: 실제 박수 2번 → `[trigger]` 로그 + 계산기 실행.
6. fmt / clippy `-D warnings` / test 전부 통과 후 커밋.

### 측정 결과

- **라이브 트리거 성공 2회:** interval=320 ms, confidence 0.55 / 0.60. 두 번 모두
  계산기 실행. 위양성 관측 0. 장치는 MacBook Air 마이크 44.1 kHz (PoC 는 48 kHz
  였는데 detector 가 sample rate 기준으로 프레임 크기를 잡아 자동 대응).
- **청크 크기 불변성:** 같은 wav 를 512 / 479 / 4096 샘플 청크로 흘려도 검출
  timestamp 완전 동일 → 스트리밍 재작성이 배치 의미론을 보존함을 회귀로 고정.
- 테스트 26개 통과 (단위 23 + 회귀 3).

### 발견 사항

- **PoC 테스트 헬퍼 `pseudo_noise` 는 사실상 DC 신호.** `(x >> 8) / u32::MAX`
  가 [0, 1/256) 범위라 결과가 -1 근처 상수. PoC 에선 음성 테스트에만 써서 무해
  했지만, 양성 테스트(광대역 버스트)에 쓰면 flatness 게이트에서 기각됨. 양성
  테스트용으로 스테이트풀 LCG 기반 `white_noise` 헬퍼를 따로 만들어 해결.
- 스트리밍 조기 확정의 confidence 는 배치보다 약간 낮게 나올 수 있음 (배치는
  윈도우 내 최소 dB 로 drop 을 계산, 스트리밍은 임계 통과 시점의 drop 사용).
  matcher 의 base 점수에만 쓰여 실사용 영향 없음.

### 인터페이스 계약 (다음 feat 로 이어짐)

```rust
// pipeline.rs — 다음 feat/action-runner 는 이 콜백 안의
// run_hardcoded_routine() 을 액션 실행기로 교체한다.
pub enum EngineEvent {
    Trigger(TriggerEvent),
    CaptureFailed(String),
}
pub fn start(on_event: impl Fn(EngineEvent) + Send + 'static) -> Engine
```

콜백은 이벤트 워커 스레드에서 실행되므로 액션 실행이 블로킹해도 감지에 영향 없음.

### 다음 단계

- `feat/action-runner`: PoC 의 `Action` enum (MVP 2종: OpenApp / OpenUrl) +
  `run()` 포팅, 측정용 `measure`/`Distribution` 은 제외. 하드코딩 루틴 교체.
- `feat/routine-store`: Routine 모델 + JSON 영속화 + Tauri 커맨드.
- 남은 백로그: 코드 서명 + /Applications/ 설치 후 reboot 자동 실행 재검증.

## 2026-07-03 (feat/action-runner)

### 목적

poc/action-runner 의 액션 실행기를 제품으로 포팅하고, audio-engine 이 남긴
하드코딩 계산기 호출(`std::process::Command` 직접 사용)을 정식 `Action` +
`run()` 경로로 교체.

### 결정 사항

- **MVP 2종만 포팅 (OpenApp / OpenUrl).** PRD 확정 범위. PoC 의 Osascript /
  Shortcut 변형은 poc/action-runner 브랜치에 박제돼 있으므로 확장 시 재이식.
- **PoC 의 측정 스캐폴딩 제거:** `measure` / `Distribution` / `Stats` / `parse`
  는 PoC 벤치마크·CLI 용이라 제품에서 제외. `ActionResult` 의 spawn/dispatch
  타이밍은 실행 로그와 Performance Pass 에 쓰이므로 유지.
- **serde derive 추가:** `#[serde(tag = "type", rename_all = "kebab-case")]` 로
  `{"type":"open-app","name":"Cursor"}` 형태 직렬화. 다음 feat(routine-store)
  의 JSON 영속화 대비. 미지원 type 은 역직렬화 시 에러(테스트로 고정).
- **백로그 반영:** 서브프로세스 stdout/stderr 를 `Stdio::null()` 로 차단
  (PoC 에서 발견된 osascript 출력 누출 문제의 근본 대응).
- **루틴 순차 실행 헬퍼:** `run_routine(&[Action])` 이 액션을 순서대로 실행하고
  결과를 로그. MVP 실행 정책(순차) 그대로. 이벤트 워커 스레드에서 돌므로 감지
  경로 블로킹 없음.

### 진행 단계

1. `dev` → `feat/action-runner` 분기.
2. `src-tauri/src/action/mod.rs` (Action enum + serde) + `action/runner.rs`
   (`run`, `RunError`, `ActionResult`) 작성.
3. `lib.rs` 의 `run_hardcoded_routine` → `hardcoded_routine() -> Vec<Action>` +
   `run_routine` 으로 교체.
4. 단위 테스트 5개 (args 매핑 2, serde 라운드트립 2, 미지원 kind 기각 1).
   전체 31개 테스트 / fmt / clippy `-D warnings` 통과.

### 인터페이스 계약 (다음 feat `routine-store` 로 이어짐)

```rust
pub enum Action { OpenApp { name: String }, OpenUrl { url: String } }  // serde 지원
pub fn run(action: &Action) -> Result<ActionResult, RunError>
```

Routine 모델은 `Vec<Action>` 을 담고, 실행은 `run_routine` 패턴을 흡수하면 됨.

### 다음 단계

- `feat/routine-store`: Routine 모델(이름 + `Vec<Action>`) + JSON 로컬 영속화
  (app data dir, 손상 시 기본값 복구) + Tauri 커맨드 노출 + 활성 루틴 개념.
- 그 후 `feat/routine-editor` (React UI) + 메뉴바 활성 루틴 전환.

## 2026-07-03 (feat/routine-store)

### 목적

루틴 데이터 모델과 로컬 영속화를 만들어 audio-engine 이 남긴 하드코딩 루틴을
제거하고, 트리거가 "저장된 활성 루틴" 을 실행하도록 전환. React UI 가 쓸 Tauri
커맨드까지 노출해 다음 feat(routine-editor) 의 백엔드를 완성.

### 결정 사항

- **모델:** `Routine { id, name, actions: Vec<Action> }` +
  `RoutineConfig { active_routine_id: Option<String>, routines }`. 활성 루틴은
  0~1개 (PRD 7.4). JSON 은 camelCase (JS interop).
- **기본 문서에 샘플 루틴 포함:** 첫 실행/복구 시 "Sample — Calculator" 가 활성
  상태로 생성. 사용자가 아무것도 안 만들어도 박수 → 계산기 흐름이 살아 있어
  수직 슬라이스 검증 경로가 유지됨.
- **손상 복구 (PRD 7.5):** 파싱 실패 시 원본을 `routines.json.corrupt` 로 백업
  후 기본값으로 재생성. `LoadReport` (Loaded / CreatedDefault /
  RecoveredFromCorruption) 로 결과를 호출자에 노출 — UI 알림은 프런트 feat 에서.
- **원자적 저장:** `.json.tmp` 에 쓰고 rename. 모든 뮤테이션은 리턴 전에 디스크
  반영 (파일이 항상 source of truth).
- **의도적 스펙 이탈 1건:** concurrency 스펙의 "락 잡고 블로킹 I/O 금지" 를
  `save_locked` 에서 위반. 락 밖에서 저장하면 경쟁 뮤테이션의 스냅샷이 순서
  꼬여 파일에 낡은 상태가 남을 수 있어, 몇 KB 로컬 쓰기는 락 안이 옳다고 판단.
  코드에 주석으로 근거 명시.
- **id 발급:** 빈 id 로 upsert 하면 스토어가 uuid v4 발급 (프런트가 id 생성
  책임을 안 짐). 활성 루틴 삭제 시 active id 는 dangling 대신 None 으로.
- **Tauri 커맨드 4종:** `list_routines` / `save_routine` / `delete_routine` /
  `set_active_routine`. 뮤테이션 커맨드는 갱신된 `RoutineConfig` 전체를 반환해
  UI 동기화를 단순화. 에러는 String 으로 매핑 (UI 표시용).

### 진행 단계

1. `dev` → `feat/routine-store` 분기.
2. `routine/mod.rs` (모델 + default_config + active_routine) +
   `routine/store.rs` (RoutineStore, StoreError, LoadReport) 작성. uuid 의존성 추가.
3. `lib.rs`: setup 에서 app data dir 에 스토어 로드, `Arc<RoutineStore>` 를
   트리거 콜백과 Tauri state 에 공유. `hardcoded_routine()` 제거, 트리거는
   `active_actions()` 조회 (활성 없으면 로그만).
4. 단위 테스트 12개 (모델 3 + 스토어 9: 첫 실행 파일 생성, 리로드, 손상 복구
   백업, id 발급, id 교체, 활성 삭제 시 active 해제, 미존재 id 에러 2, 활성
   전환 반영). 전체 43개 테스트 / fmt / clippy `-D warnings` 통과.
5. 스모크: 앱 실행 → `store ready: CreatedDefault` 로그 +
   `~/Library/Application Support/com.vibe.app/routines.json` 생성, 샘플 루틴
   활성 상태 확인.

### 인터페이스 계약 (다음 feat `routine-editor` 로 이어짐)

```ts
// Tauri invoke 표면 (camelCase JSON)
invoke<RoutineConfig>("list_routines")
invoke<RoutineConfig>("save_routine", { routine })   // routine.id === "" 이면 신규
invoke<RoutineConfig>("delete_routine", { id })
invoke<RoutineConfig>("set_active_routine", { id })  // id: string | null

type RoutineConfig = { activeRoutineId: string | null; routines: Routine[] }
type Routine = { id: string; name: string; actions: Action[] }
type Action = { type: "open-app"; name: string } | { type: "open-url"; url: string }
```

### 다음 단계

- `feat/routine-editor`: React 루틴 편집기 UI (frontend 스펙 준수) + 메뉴바
  활성 루틴 전환(Active Routine Switcher) + 실행 로그 표시.
- 백로그 유지: 코드 서명 + reboot 자동 실행 재검증, Performance Pass.

## 2026-07-03 (feat/routine-editor)

### 목적

사용자가 원하는 앱/URL 을 자유롭게 조합해 루틴을 만드는 React 편집기 UI.
이걸로 MVP 의 핵심 사용자 플로우("루틴 등록 → 활성화 → 박수 두 번")가 하드코딩
없이 완성됨. 템플릿으로 남아 있던 Tauri 기본 화면(greet) 제거.

### 결정 사항

- **구조 (cohesion-coupling 스펙):** `src/domains/routines/` 아래 `types.ts` /
  `api.ts`(invoke 래퍼) / `useRoutines.ts`(문서 상태 훅) / `components/`.
  App.tsx 는 조합만. 추가 의존성 0 (react-query / react-hook-form / zod 안 씀 —
  이 규모 폼에 과함).
- **상태 흐름:** 모든 뮤테이션이 스토어가 반환하는 갱신된 `RoutineConfig` 로
  전체 교체 → UI 가 영속 파일과 어긋날 수 없음. 편집 중 초안은
  `RoutineEditor` 로컬 state, `key={routine.id}` 로 루틴 전환 시 초기화.
  신규 루틴은 빈 id 로 저장 → 스토어가 발급한 id 를 diff 로 찾아 선택 유지.
- **검증 (predictability 스펙):** `ValidationResult` discriminated union.
  이름 비어있음 / 액션 0개 / 값 비어있음 / URL http(s) 아님 기각.
- **디자인 (design-guide):** 다크 인디고 그라디언트 배경 + 앰버 액센트, CSS
  변수 토큰, 로컬 전용 앱이라 원격 폰트 불가 → macOS 내장 Avenir Next
  (Condensed) 스택. 의도된 모션 3개: 활성 dot 펄스, 편집기 fadeSlide 전환,
  버튼/리스트 호버. 카드형 컨테이너는 인터랙션 단위(액션 행)에만.
- **clap 레벨 로그 추가:** 라이브 검증 중 "박수 쳤는데 안 됨" 상황에서 detector
  기각인지 matcher 기각인지 구분이 안 돼 `[clap]` 로그(피크/플로어 대비/
  flatness/confidence)를 pipeline 에 영구 추가. 감도 튜닝 관측성 확보.
- **greet 커맨드 제거:** 프런트가 더 이상 안 쓰는 템플릿 잔재.

### 진행 단계

1. `dev` → `feat/routine-editor` 분기. frontend 스펙 7개 + design-guide 정독.
2. 도메인 레이어 (types / api / useRoutines) → 컴포넌트 (RoutineSidebar,
   RoutineEditor + ActionRow/ActiveToggle) → App 조합 → App.css 토큰 기반 전면 교체.
3. `pnpm build` (tsc) + cargo test 43개 + fmt / clippy 통과.
4. 라이브 검증: 사용자가 편집기에서 "Cursor 실행 + YouTube URL" 루틴을 직접
   만들고 활성화 → 박수 두 번 → Cursor 135 ms / URL 126 ms 에 실행 확인.

### 측정 결과 / 발견 사항

- **트리거 실측:** interval=240 ms, confidence 0.43. open-app(Cursor) 135 ms,
  open-url 126 ms — PRD 300 ms 목표 내.
- **"작동 안 함" 리포트의 실제 원인은 박수 템포.** 사용자의 첫 시도는 두 박수
  간격이 1.47 s 로 max_interval(600 ms) 초과 → 각각 단일 박수로 기각. 코드
  결함 아님. `[clap]` 로그 덕에 즉시 진단됨. 향후 "감도/간격 설정" feat 에서
  사용자가 간격을 조정할 수 있게 하면 완화 가능.
- 루틴 저장/활성 전환/영속화는 첫 시도에 전부 정상 동작 (routines.json 확인).

### 다음 단계

- `feat/menu-switcher`: 메뉴바에서 활성 루틴 전환 (트레이 메뉴 동적 서브메뉴,
  루틴 변경 시 메뉴 재구성).
- `feat/exec-log`: 실행 결과 로그 (PRD 7.6 — 링 버퍼 + UI 표시 + 실패 액션 표시).
- 백로그 유지: 감도/간격 설정 UI, 코드 서명 + reboot 재검증, Performance Pass.

## 2026-07-03 (poc/window-layout)

### 목적

사용자 요청으로 새 기능 방향 확정: 루틴 실행 시 앱/URL 을 여는 데서 끝나지
않고 **화면 분할 영역에 자동 배치** (Rectangle/Spectacle 의 스냅을 루틴에
내장). 편집기에는 모니터 목업에서 2/3/4분할 영역에 액션을 배정하는 UI 가
들어갈 예정. 선행 리스크(Rust 에서 AX API 로 타 앱 창 제어)를 PoC 로 검증.
컨벤션대로 `main` 에서 분기, 머지하지 않음.

### 결정 사항

- **의존성:** `accessibility-sys`(AX FFI) + `core-foundation` +
  `core-graphics`. unsafe 는 `ax.rs` 모듈에만 격리하고 안전 래퍼로 노출.
- **고정 크기 창 관용 정책:** 리사이즈 거부(AXError -25200) 시 위치만 적용하고
  `Placement::MovedOnly` 로 보고. 에러 아님 (Rectangle 과 동일 정책).
- **PoC 브랜치에 .gitignore 필수:** main 엔 .gitignore 가 없어 dev 작업 잔재
  (node_modules 등)가 전부 untracked 로 노출됨. 브랜치 첫 커밋에 포함.

### 검증 결과 (가설 4개 모두 통과)

| 가설 | 결과 |
|---|---|
| Rust 에서 타 앱 창 이동/리사이즈 | ✓ (Notes/Safari Full, Calculator/Mail MovedOnly) |
| 실행→창 대기→배치 지연 | ✓ warm ~150ms, cold 0.6~1.6s (50ms 폴링) |
| 2/3/4분할 일괄 배치 | ✓ demo2/demo4 시각 확인 |
| URL 새 브라우저 창 배치 | ✓ Chrome 바이너리 직접 호출 + bottom-right 배치 |

### 발견 사항 (본 구현에 직결)

- **권한은 호출 프로세스에만.** 제어 대상 앱은 권한 불필요. 제품에선
  V.I.B.E.app 이 `AXIsProcessTrustedWithOptions(prompt)` 호출 → 다이얼로그 →
  설정 목록에 자동 등록 → 사용자는 토글 한 번. Permission Guide UX 확정.
- **`open --args --new-window` 는 실행 중인 앱에 무시됨.** 이미 뜬 Chrome 에
  URL 새 창을 만들려면 브라우저 바이너리 직접 호출 필요. 실측으로 확인.
- **AXWindows[0]=새 창 가정은 레이스.** 기존 창을 옮기는 오동작 실측. 본
  구현은 열기 전 창 스냅샷 → 열기 후 diff 로 새 창 식별해야 함.
- 세부 측정치와 인터페이스 계약은 `poc/window-layout` 브랜치의 POC.md 참조.

### 다음 단계

- PRD 갱신: 7.4 액션에 화면 배치(레이아웃) 확장, 5장 권한에 Accessibility
  실사용 명시, MVP 범위 결정 필요.
- `feat/window-layout`: `Action` 에 `region: Option<Region>` 추가 + AX 배치
  모듈 통합 + 편집기에 모니터 목업 레이아웃 UI.
- 기존 잔여: `feat/menu-switcher`, `feat/exec-log`, 감도 설정, 코드 서명 검증.

## 2026-07-03 (feat/window-layout)

### 목적

PoC 로 검증한 창 배치를 제품에 통합. 루틴의 각 액션에 화면 영역을 지정하면
박수 트리거 시 앱/URL 창이 지정 영역에 자동 스냅되는 기능. PRD 7.4 에 화면
배치 섹션을 먼저 추가하고 구현.

### 결정 사항

- **모델:** `Action` 양 variant 에 `region: Option<Region>` 추가.
  `skip_serializing_if` 로 기존 JSON 과 완전 호환 (region 없는 문서 그대로
  로드됨). `Region` 은 kebab-case serde (`"left-half"` 등) 로 프런트와 공유.
- **2단계 루틴 실행 (`action/execute.rs`):** 1단계에서 모든 액션을 즉시
  실행(빠른 spawn), 2단계에서 창 대기+배치. 느린 창 대기가 뒤 액션의 실행을
  지연시키지 않아 PRD "1초 내 체감 시작" 유지. 단, 영역 지정 URL 은 새
  브라우저 창을 직접 만들어야 하므로(일반 `open` 은 탭으로 흡수) 2단계로 전부
  이연.
- **PoC 발견사항 3개 모두 반영:** (1) 새 창 식별은 열기 전 스냅샷과의 CFEqual
  diff (`same_element`), (2) 실행 중 Chrome 에는 바이너리 직접 호출로 새 창
  생성, (3) 크기 고정 창은 `Placement::MovedOnly` 관용.
- **권한 처리:** 배치 시점에 `AXIsProcessTrusted` (프롬프트 없이) 확인, 미승인
  시 배치만 건너뛰고 열기는 정상 진행 (PRD 정책). 편집기에는 영역이 배정돼
  있고 권한이 없을 때만 힌트 배너 + "Enable…" 버튼 (프롬프트 호출 →
  설정 목록 자동 등록) 노출. `check_accessibility_permission(prompt)` 커맨드.
- **레이아웃 프리셋은 UI 개념:** 저장되는 건 액션별 region 뿐. 편집기의
  2/3/4 split 프리셋은 영역 선택지를 좁히고 모니터 목업 그리드를 결정하며,
  기존 region 에서 역산(derivePreset). 프리셋 전환 시 무효 region 은 해제.
- **모니터 목업:** 디스플레이 설정 느낌의 미니 모니터 (16:10, 베젤+스탠드).
  프리셋 그리드 셀에 배정된 액션 라벨이 실시간 표시, 배정 셀은 액센트 강조.

### 진행 단계

1. PRD 7.4/5/11/12 갱신 커밋 후 `dev` → `feat/window-layout` 분기.
2. `layout/` 모듈: PoC 의 ax.rs 포팅(+`same_element`, 프롬프트 없는 trust
   체크) + Region(serde) + placer (창 대기/diff/배치, Chrome 경로 탐색,
   pgrep -x → 번들 경로 fallback — VS Code 처럼 실행파일명이 다른 앱 대응).
3. `Action` region 확장 + `execute.rs` 2단계 실행기 + lib.rs 배선.
4. 프런트: `layout.ts` (프리셋/라벨/클램프) + 편집기 Layout 섹션 (프리셋
   버튼, MonitorMockup, PlacementPermissionHint) + ActionRow 영역 셀렉트.
5. 테스트 47개 (Region frame/serde 추가) / fmt / clippy / pnpm build 통과.
6. 라이브 검증: 사용자가 2 split 으로 Cursor→왼쪽, YouTube URL→오른쪽 배정
   후 박수 두 번 → 두 창이 좌우 절반에 자동 배치됨을 육안 확인.

### 발견 사항

- **dev 환경 권한 승계:** dev 바이너리는 Cursor(터미널 호스트)의 자식이라
  TCC 가 Cursor 의 Accessibility 승인을 그대로 적용. 서명된 .app 배포 시엔
  V.I.B.E 자체가 승인 대상이 됨 (Permission Guide 흐름 필요).
- **stdout 블록 버퍼링:** 파이프로 리다이렉트된 dev 로그는 println 이 즉시
  flush 되지 않아 프로세스 kill 시 마지막 로그가 유실될 수 있음. 검증은
  육안 확인으로 대체했으나, exec-log feat 에서 로그를 UI 로 올리면 해소됨.

### 다음 단계

- `feat/menu-switcher`: 메뉴바 활성 루틴 전환.
- `feat/exec-log`: 실행 로그 링 버퍼 + UI (PRD 7.6).
- 백로그: 감도/간격 설정, 다중 모니터 지원(현재 메인 디스플레이 고정),
  Chrome 외 브라우저 새 창, 코드 서명 + reboot 검증, Performance Pass.

## 2026-07-03 (feat/menu-switcher)

### 목적

PRD 7.4 의 Active Routine Switcher: 설정 창을 열지 않고 메뉴바에서 활성
루틴을 전환.

### 결정 사항

- **메뉴는 상태에서 전량 재구성.** 기존 셸은 고정 메뉴 항목 핸들을 클로저에
  캡처해 `set_text`/`set_checked` 로 갱신했는데, 루틴 목록이 동적으로 변하면
  캡처된 핸들이 stale 해짐. `build_tray_menu(app)` 가 StatusState / Engine /
  autostart / RoutineStore 스냅샷에서 메뉴 전체를 만들고, 모든 상태 변화가
  `refresh_tray_menu` 로 교체하는 단방향 구조로 리팩터링.
- **루틴 항목:** "Active routine" 라벨 아래 CheckMenuItem (`routine:<id>`).
  클릭 시 활성 전환, 이미 활성인 항목 클릭 시 비활성(None). 루틴 0개면
  비활성 "No routines yet".
- **양방향 동기화:** 루틴 문서가 바뀌는 모든 경로(커맨드 3종 + 트레이 클릭)가
  `notify_routines_changed` 로 수렴 — `routines://changed` 이벤트 emit(웹뷰
  refetch) + 트레이 재구성. useRoutines 훅이 이벤트를 구독해 편집기가 열려
  있어도 트레이 전환이 즉시 반영됨.

### 진행 단계

1. `dev` → `feat/menu-switcher` 분기. lib.rs 메뉴 구성 전면 리팩터링.
2. 커맨드에 `AppHandle` 주입 + notify 배선. useRoutines 에 이벤트 리스너.
3. 테스트 47개 / fmt / clippy / pnpm build 통과. 라이브 확인은 exec-log 와
   묶어서 최종 세션에서 진행.

### 다음 단계

- `feat/exec-log` 진행 후 통합 라이브 검증.