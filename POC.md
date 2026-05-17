# PoC: action-runner

V.I.B.E의 네 번째 PoC. `poc/double-clap` 이 발화한 `TriggerEvent` 를 받아 macOS
액션을 실제로 실행하고, 각 액션의 spawn / dispatch 지연을 측정해 PRD 의 "박수
감지 후 첫 액션까지 300 ms" 목표를 검증한다.

PRD 가 좁혀진 상태(MVP 액션 = 앱 실행 + URL 열기 2종)에서 시작하지만, 코드 자체는
4종(`OpenApp`, `OpenUrl`, `Osascript`, `Shortcut`) 전부 유지한다. 확장 단계 액션을
나중에 다시 켤 때 코드 변경 없이 재가동 가능.

## 알고리즘

각 액션은 단일 subprocess 호출로 매핑된다. `Command::spawn` 호출 직전 시각을
저장하고:

1. **spawn 시각** — `Child` 가 반환된 시점. fork/exec 비용만 포함.
2. **dispatch 시각** — subprocess 가 종료(`wait()`) 된 시점. macOS 가 LaunchServices
   같은 시스템 컴포넌트로 의도를 전달 완료한 시점.

`spawn_ms` 는 거의 일정한 fork 비용. `dispatch_ms` 가 사용자가 체감하는 "액션이
시작됐다" 지점에 더 가깝다.

| Action 종류 | program | args |
|---|---|---|
| `OpenApp` | `open` | `-a <name>` |
| `OpenUrl` | `open` | `<url>` |
| `Osascript` | `osascript` | `-e <script>` |
| `Shortcut` | `shortcuts` | `run <name>` |

반복 측정은 `warmup` 회 버린 뒤 `repetitions` 회 측정값을 모아 min / p50 / p95 / max
분포로 압축. cold/warm 차이가 dispatch 시간에서 표면화돼 첫 회 비용은 warmup 에서
흡수.

## 인터페이스 계약 (다음 PoC `poc/tauri-shell` 로 이어짐)

```rust
pub enum Action {
    OpenApp { name: String },
    OpenUrl { url: String },
    Osascript { script: String },
    Shortcut { name: String },
}

pub struct ActionResult {
    pub action: Action,
    pub spawn_ms: f64,
    pub dispatch_ms: f64,
    pub exit_status: ExitStatus,
}

pub fn run(action: &Action) -> Result<ActionResult, RunError>
pub fn measure(action: &Action, repetitions: usize, warmup: usize) -> Result<Stats, RunError>
```

- `run` 은 1회 실행 + 측정. 본 통합 시 루틴 실행기가 액션마다 호출.
- `measure` 는 N회 반복 + 분포 계산. PoC 와 미래 진단 도구용.
- 두 함수 모두 동기. 액션 실행이 spawn 이후 wait 까지 백그라운드 스레드에서
  진행되도록 본 통합 시점에 감싸면 됨 (`spec/code/rust/concurrency.md` 의 "real-time
  callbacks only do the minimum" 원칙 — 실제 통합에선 트리거 감지 스레드가 직접
  `run` 을 부르지 않고 메시지로 액션 큐에 넘김).

## 실행

```bash
# 빌트인 suite — 3종 액션 반복 측정 (warmup=1, reps=5)
cargo run --release

# 단일 액션 측정
cargo run --release -- open-app Calculator
cargo run --release -- open-url https://example.com
cargo run --release -- osascript "return 1"
cargo run --release -- shortcut test-shortcut
```

## 측정 결과 (MacBook Air M1, macOS, warmup=1, repetitions=5)

| 액션 | spawn p50 / p95 (ms) | dispatch p50 / p95 (ms) | dispatch max (ms) | 성공 |
|---|---|---|---|---|
| **open-app** (Calculator) | 0.45 / 1.33 | 45.97 / 157.56 | 157.56 | 5/5 |
| **open-url** (example.com) | 1.66 / 3.52 | 99.27 / 133.62 | 133.62 | 5/5 |
| osascript (`return 1`) — 참고 baseline | 0.45 / 0.56 | 30.79 / 32.22 | 32.22 | 5/5 |

**PRD 300 ms 목표 검증 결과:**

- MVP 2종 (open-app, open-url) 모두 p95 dispatch < 160 ms — **목표의 절반 수준**.
- 최대값(open-app max=158 ms) 도 300 ms 의 절반.
- spawn 만 보면 모든 액션에서 ≤ 4 ms — 사용자 체감 지연의 주체는 spawn 이 아니라
  macOS 시스템(LaunchServices, 브라우저 IPC) 의 dispatch 비용.
- 결론: **본 통합에서 액션 실행부가 PRD 의 300 ms 목표를 깰 리스크는 거의 없다.**
  병목은 마이크 → 박수 감지 → 매처 경로에 있을 가능성이 더 큼.

## 알려진 한계

- **샘플 크기 작음.** N=5 라 외곽 분위수(p95, max) 추정 정밀도 낮음. 본 통합 단계에서
  N=20~30 으로 다시 측정하면 분포 모양이 더 또렷이 보임.
- **cold start 변동성.** open-app max=157 ms 는 warmup 1회가 다 흡수하지 못한 cold
  variance. Mac 부팅 직후 첫 박수 트리거에서 한 번 더 cold 비용이 들 수 있음.
  실제 사용 시 부팅 후 첫 액션은 조금 느려도 사용자가 인지하기 어려운 영역.
- **stdout 노출.** osascript 가 `return 1` 결과를 stdout 에 찍어 표가 더러워짐.
  subprocess stdio 가 부모로 inherit 됨. PoC 측정값에는 영향 없음. 본 통합 시
  `.stdout(Stdio::null())` 로 정리.
- **MVP 범위 밖 액션:** Shortcuts (`shortcut`), AppleScript (`osascript`) 는 코드
  경로 + 단위 테스트로 검증된 상태로 남겨둠. 실측은 안 함. 확장 단계에서 다시
  필요해지면 동일 인터페이스로 그대로 활용 가능.

## 비목표 (이번 PoC 안 함)

- 실제 트리거 감지와 연결한 end-to-end 측정. `TriggerEvent → Routine → run()` 의
  통합은 본 프로젝트 단계 책임.
- 루틴(여러 액션의 묶음) 실행. 본 PoC 는 단일 액션의 spawn 비용을 측정. 루틴
  실행은 `for action in routine.actions { run(action) }` 형태로 본 통합에서 조립.
- 액션 실패 시 사용자 피드백. `RunError` 와 `exit_status` 를 그대로 노출만 함.
  UI 표시 정책은 본 통합 책임.
- 액션별 지연 / 병렬 실행 옵션. PRD 7.4 가 MVP 에서 순차 실행으로 좁아짐.

## 테스트

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

단위 테스트 10개:

- `action`: 4종 variant 각각의 program / args 매핑 + `Action::parse` 양/음 케이스
- `runner`: `distribution` 헬퍼의 빈 입력 / 단일값 / 정렬 입력 / 무작위 입력
