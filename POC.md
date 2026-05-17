# PoC: double-clap

V.I.B.E의 세 번째 PoC. `poc/clap-detector` 가 뽑아낸 `ClapEvent` 시퀀스를 입력으로
받아 **이중 박수 패턴**만 추려 `TriggerEvent` 로 내보내는 순수 매처를 만든다.
오디오 신호 처리는 하지 않는다 — `ClapEvent` 가 신호 처리 결과를 압축한
인터페이스라는 가정에 기댄다.

## 알고리즘 (4단계 게이트)

1. **이웃 페어링** — 시간순으로 정렬된 `ClapEvent` 들 중 인접한 두 박수를 후보 쌍으로
   본다. 트리거가 인정된 쌍은 두 박수 모두 소비하고 다음 박수부터 새 쌍을 만든다.
   기각된 쌍은 두 번째 박수가 그 다음 박수와 다시 짝지을 수 있게 한 칸만 전진.
2. **간격 게이트** — `min_interval_ms ≤ Δt ≤ max_interval_ms` (기본 150~600 ms).
   사람의 자연스러운 이중 박수 리듬 범위. 짧으면 반향, 길면 무관한 두 박수.
3. **피크 유사도** — `|peak_db_a - peak_db_b| ≤ max_peak_db_diff` (기본 12 dB).
   "박수-기침-박수" 같이 가운데 박수가 끼어든 경우, 음량 차이로 거름.
4. **광대역 유사도** — `|flatness_a - flatness_b| ≤ max_flatness_diff` (기본 0.25).
   두 박수의 스펙트럴 특성이 비슷해야 같은 사람이 같은 손으로 친 박수로 인정.

통과한 쌍은 `confidence` 점수와 함께 `TriggerEvent` 로 나간다.

## 인터페이스 계약 (다음 PoC `poc/action-runner` 로 이어짐)

```rust
pub struct TriggerEvent {
    pub first_at_ms: u64,
    pub second_at_ms: u64,
    pub interval_ms: u64,
    pub confidence: f32,
}

pub fn match_pattern(events: &[ClapEvent], config: &MatcherConfig) -> Vec<TriggerEvent>
pub fn analyze(events: &[ClapEvent], config: &MatcherConfig) -> Vec<PairOutcome>
```

- `events` 는 시간순 정렬 가정. 정렬 책임은 호출자 (clap-detector 가 이미 시간순).
- 함수는 순수: 동일 입력에 동일 출력. 내부 상태 없음.
- `analyze` 는 기각 사유까지 노출 — PoC 디버깅과 튜닝 용도.
- `match_pattern` 은 트리거만 추출. 실시간 통합 시 사용할 API.

`MatcherConfig::default()` 는 첫 튜닝 값. 본 통합 시 사용자 환경에서 재측정 가능.

## 실행

```bash
# 빌트인 데모 시나리오 5개
cargo run --release

# JSON 파일 입력 (ClapEvent 배열)
cargo run --release -- path/to/events.json
```

JSON 포맷은 `ClapEvent` 의 serde 직렬화 그대로:

```json
[
  {"timestamp_ms": 100, "peak_db": -10.0, "above_floor_db": 50.0, "flatness": 0.35, "confidence": 0.85},
  {"timestamp_ms": 400, "peak_db": -11.0, "above_floor_db": 49.0, "flatness": 0.36, "confidence": 0.82}
]
```

## 데모 시나리오 결과

| 시나리오 | 입력 | 결과 |
|---|---|---|
| 유사 박수 300 ms 간격 | (−10 dB, 0.35) · (−11 dB, 0.36) | TRIGGER conf=0.84 ✓ |
| 간격 120 ms (너무 짧음) | (−10, 0.35) · (−10.5, 0.36) | reject: interval too short ✓ |
| 간격 800 ms (너무 김) | (−10, 0.35) · (−10.5, 0.36) | reject: interval too long ✓ |
| 피크 격차 20 dB | (−8, 0.35) · (−28, 0.36) | reject: peak mismatch ✓ |
| 박수 3개 연속 | 100/400/700 ms | TRIGGER (1,2), 3번 미매칭 ✓ |

## 튜닝한 파라미터 (`MatcherConfig::default()`)

| 파라미터 | 값 | 의미 |
|---|---|---|
| `min_interval_ms` | 150 | 이중 박수 최소 간격. 더 짧으면 반향 |
| `max_interval_ms` | 600 | 이중 박수 최대 간격. 더 길면 별개 박수 |
| `max_peak_db_diff` | 12.0 | 두 박수 음량 차이 허용 한도 |
| `max_flatness_diff` | 0.25 | 두 박수 스펙트럴 광대역 특성 차이 허용 한도 |

값은 일반적인 이중 박수 리듬과 clap-detector 의 측정 결과를 토대로 설정. 실제 사용자
환경에서 위양성/위음성 측정 후 미세 조정.

## 테스트

```bash
cargo test    # 단위 13개
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

단위 테스트 항목:

- 빈 입력 / 단일 박수 → 트리거 없음
- 정상 이중 박수 → 1 트리거 (timestamp, interval, confidence 검증)
- 간격 너무 짧음 / 너무 김 → 기각 사유까지 확인
- 피크 격차 / 광대역 격차 → 기각 사유까지 확인
- 박수 3개 연속 → 첫 쌍만 소비
- 박수 4개 연속 → 트리거 2개
- 기각된 쌍의 두 번째 박수가 다음 박수와 매칭
- 경계값 (interval 최솟값/최댓값, peak diff 한계) 통과

## 비목표 (이번 PoC 안 함)

- 실제 wav 입력. clap-detector 와 합쳐 wav → ClapEvent → TriggerEvent 파이프라인은
  본 통합 시 손으로 연결 (PoC끼리 머지 안 함 정책).
- 실시간 스트리밍 매처. 현재는 슬라이스 전체를 한 번에 받지만, 시그니처를 그대로
  슬라이딩 윈도우 호출로 바꿔도 동작 (각 호출 내부 상태 없음).
- 트리거 후 액션 실행. `poc/action-runner` 책임.
- 두 박수 외 패턴 (트리플 박수, 박수-휘파람 등). 본 제품 스코프 밖.

## 알려진 한계

- `MatcherConfig` 의 임계값은 첫 추정치. 실제 사용자 박수 리듬 데이터로 회귀 테스트
  데이터셋을 만들고 그리드 서치할 여지 있음. 현재는 합성 시나리오만으로 룰 동작 확인.
- "박수-기침-박수" 같은 가짜 패턴은 가운데 소리가 ClapEvent 로 잘못 검출됐을 때만
  매처의 책임이 된다. clap-detector 가 거른다면 매처는 보지도 못함. 현재 PoC 는
  유사도 게이트로 한 번 더 안전망을 둠.
- 박수 3개 연속(예: 박수-박수-박수)을 "잘못 친 이중 박수 두 번"으로 해석하지 않고
  "첫 쌍만 트리거"로 처리. 사용자 의도 추정은 본 통합 시 UX 결정.
