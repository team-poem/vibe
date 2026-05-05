# PoC: clap-detector

V.I.B.E의 두 번째 PoC. 오프라인 wav 파일을 입력으로 받아 단발 박수를 구분해 내는
룰 기반 감지기를 만든다. 이중 박수 패턴 매칭은 다음 PoC(`poc/double-clap`).

## 알고리즘 (4단계)

1. **에너지 게이트** — 적응형 노이즈 플로어(EMA, alpha=0.05) 대비 +32 dB 이상 솟은
   프레임만 박수 후보로 마킹.
2. **광대역 체크** — 후보 프레임에 Hann window + FFT(512) → spectral flatness 계산.
   0.20 미만은 거름 (말소리·음악·특정 톤).
3. **지속 시간 게이트** — 60 ms 안에 RMS가 20 dB 이상 떨어져야 박수로 인정.
   끌리는 소음(음악 비트, 문 닫힘)을 거름.
4. **불응기** — 한 박수 인정 후 120 ms 동안 추가 검출 차단. 반향·이중 검출 방지.

## 인터페이스 계약 (다음 PoC `poc/double-clap` 으로 이어짐)

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

- `samples` 는 mono f32 PCM (audio-capture 콜백 출력과 동일 형식).
- 함수는 순수: 동일 입력에 동일 출력. 내부 상태는 호출당 새로 만들어 폐기.

## 실행

```bash
cargo run --release -- samples/<file>.wav
```

`samples/` 의 wav 들은 audio-capture 로 직접 녹음한 파일. 출력은 박수 이벤트 표 +
요약 통계(검출 수, 분당 비율).

## 측정 결과

MacBook Air 내장 마이크로 직접 녹음한 4개 시나리오 + 1개 혼합 wav 검증.

| 샘플 | 길이 | 검출 | 평가 |
|---|---|---|---|
| `claps_solo.wav` (강+약 박수) | 24.5 s | 8 | 강한 박수 8회 모두 검출. peak -4 ~ -18 dBFS. 매우 약한 박수 1회(-29 dBFS)는 임계값 아래로 빠짐 — 의도된 동작 (PoC 단계에서 약한 박수는 우선순위 낮음) |
| `typing.wav` | 39.6 s | 0 | 위양성 0건 ✓. 스페이스바·엔터 강타 포함 |
| `voice.wav` | 29.1 s | 0 | 위양성 0건 ✓. 평소 톤 + 웃음·기침 + "아!" 큰소리 포함 |
| `silence.wav` | 23.2 s | 0 | 위양성 0건 ✓ |
| `test.wav` (혼합) | 21.2 s | 2 | 박수 2회 정확히 검출. 타이핑·말소리 구간은 거름 |

튜닝 후 위양성 0건. recall은 강한 박수 기준 100%, 약한 박수 포함 시 8/9.

## 튜닝한 임계값 (`DetectorConfig::default()`)

| 파라미터 | 값 | 의미 |
|---|---|---|
| `frame_ms` | 10.0 | 프레임 길이 |
| `fft_size` | 512 | FFT 점수 |
| `floor_alpha` | 0.05 | EMA 노이즈 플로어 갱신 속도 |
| `onset_threshold_db` | 32.0 | 박수 후보 임계값 (플로어 대비) |
| `flatness_threshold` | 0.20 | 광대역 통과선 |
| `decay_window_ms` | 60.0 | 감쇠 확인 창 |
| `decay_drop_db` | 20.0 | 감쇠 통과선 |
| `refractory_ms` | 120.0 | 불응기 |

## 테스트

```bash
cargo test            # 단위 9 + 회귀 1
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

- 단위 테스트(`#[cfg(test)] mod tests`): RMS·flatness·EMA·detector 음성 케이스.
- 회귀 테스트(`tests/regression.rs`): `tests/data/claps_short.wav` (claps_solo 첫 6초)
  로드 후 1~3건 검출 확인 + 각 이벤트가 박수 신호 특성 만족하는지 검증.
- `tests/data/*.wav` 는 `.gitignore` 예외로 커밋됨.

## 알려진 한계

- 파라미터는 MacBook Air 내장 마이크 + 조용한 실내 환경 기준 튜닝. 환경 바뀌면
  재튜닝 필요. 적응형 노이즈 플로어로 어느 정도 자동 조정되지만 임계값 자체는 고정.
- 실시간 통합은 본 프로젝트 단계. 인터페이스 시그니처(`fn detect`)가 audio-capture
  콜백 시그니처와 일치하므로 그대로 붙임.
- 매우 약한 박수는 의도적으로 거름. 사용자가 일부러 약하게 친 박수도 트리거하려면
  `onset_threshold_db` 를 30 으로 낮추는 옵션을 고려.
- audio-capture 로 녹음한 wav 가 Ctrl+C 종료 시 헤더 사이즈가 0으로 남는 이슈 발견.
  `python3` 로 헤더 사이즈만 패치해서 진행. audio-capture 본체에서 SIGINT 처리해
  wav writer 를 finalize 하는 후속 작업 필요.
