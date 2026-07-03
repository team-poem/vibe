# PoC: window-layout

macOS Accessibility(AXUIElement) API 로 다른 앱의 창을 화면 분할 영역에
배치할 수 있는지 검증하는 PoC. 루틴 실행 시 "앱/URL 을 화면 영역에 스냅"
하는 제품 기능(Rectangle 류 자동 배치)의 선행 리스크 검증.

## 가설

1. Rust 에서 AX API 로 임의 앱 창의 위치/크기를 제어할 수 있다.
2. 앱 실행 → 창 등장 대기 → 배치 흐름의 지연이 수용 가능하다 (수 초 이내).
3. 2/3/4분할 영역 계산 + 다중 앱 일괄 배치가 동작한다.
4. URL 도 새 브라우저 창으로 열어 영역에 배치할 수 있다.

## 사용법

```
cargo run -- trust                  # 권한 확인 (시스템 프롬프트 호출)
cargo run -- list <app>             # 실행 중인 앱의 창 목록
cargo run -- place <app> <region>   # 실행 중인 앱 앞창을 영역으로
cargo run -- launch <app> <region>  # 실행 → 창 대기 → 배치 (단계별 ms 측정)
cargo run -- demo2 <a> <b>          # 좌/우 2분할
cargo run -- demo3 <a> <b> <c>      # 좌/중/우 3분할
cargo run -- demo4 <a> <b> <c> <d>  # 사분면 4분할
```

regions: `full | left | right | left-third | center-third | right-third |
top-left | top-right | bottom-left | bottom-right`

## 측정 결과 (MacBook Air, macOS 15)

| 시나리오 | 결과 |
|---|---|
| 권한 (Cursor 호스트에 부여) | trusted=true, 제어 대상 앱은 권한 불필요 |
| launch Notes → right (cold) | open 68ms / 창 등장 1078ms / 배치 완료 1371ms |
| demo2 Notes+Safari | 442ms / 1611ms (cold Safari 포함), 둘 다 Full |
| demo4 Calc+Notes+Safari+Mail (warm) | 창 등장 ~120-650ms, 배치 자체는 +30ms 내외 |
| warm 앱 배치 전체 | ~150ms |
| Chrome 새 창 + URL → bottom-right | Full 배치, 새 창 정확히 식별됨 (아래 참조) |

## 발견 사항

1. **권한은 호출 프로세스에만 필요.** AX 제어 권한은 창을 옮기는 쪽(개발 중엔
   터미널/IDE, 제품에선 V.I.B.E.app)에만 필요하고 제어 대상 앱은 무관.
   `AXIsProcessTrustedWithOptions(prompt=true)` 가 시스템 다이얼로그 + 설정
   목록 등록까지 해줘서 제품의 Permission Guide UX 는 "다이얼로그 → 토글"
   두 단계면 됨.
2. **고정 크기 창은 리사이즈를 거부한다 (AXError -25200).** Calculator, Mail
   설정창 등. 대응: 위치만 적용하고 `Placement::MovedOnly` 로 보고 (Rectangle
   과 동일한 정책). 에러로 취급하지 않음.
3. **`open --args --new-window` 는 앱이 이미 실행 중이면 무시된다.** 이미 뜬
   Chrome 에 URL 새 창을 만들려면 브라우저 바이너리 직접 호출
   (`.../MacOS/Google Chrome --new-window <url>`) 이 필요. 실행 중인 인스턴스로
   전달돼 진짜 새 창이 열림.
4. **"앞창" 휴리스틱은 레이스가 있다.** AXWindows[0] 을 새 창으로 가정하면
   기존 창을 옮길 수 있음 (실측: 기존 Chrome 창이 옮겨짐). 본 구현에서는
   열기 전 창 id 스냅샷 → 열기 후 diff 로 새 창을 식별해야 함.
5. **창 등장 대기는 폴링으로 충분.** 50ms 간격 폴링으로 cold launch 0.6~1.6s,
   warm 0.1~0.3s. PRD 의 "루틴 1초 내 체감 시작" 과 공존 가능 — 배치는 앱별
   비동기 진행이므로 첫 액션 300ms 목표와 충돌하지 않음.
6. 메뉴바/Dock 영역: `CGDisplayBounds` 는 메뉴바 포함 전체 프레임. AX 가 y 를
   메뉴바 아래로 클램프해줘서 PoC 수준에선 문제없으나, 본 구현에선
   `NSScreen.visibleFrame` 기준이 더 정확.

## 인터페이스 계약 (본 통합으로 이어짐)

```rust
pub enum Region { Full, LeftHalf, RightHalf, LeftThird, CenterThird,
                  RightThird, TopLeft, TopRight, BottomLeft, BottomRight }
impl Region { pub fn frame(self, display: CGRect) -> CGRect }

pub enum Placement { Full, MovedOnly }
pub fn set_window_frame(window: &AxElement, frame: CGRect) -> Result<Placement, AxError>

// 실행 경로: open → pid 대기 → AXWindows 폴링 → set_window_frame
```

제품 통합 시 `Action` 에 `region: Option<Region>` 을 추가하고, 루틴 편집기에
모니터 목업 레이아웃 UI 를 얹는다. URL+영역 액션은 브라우저 바이너리 직접
호출로 새 창을 만든 뒤 창 diff 로 식별해 배치한다.
