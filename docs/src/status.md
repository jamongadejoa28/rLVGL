# 포팅 현황과 통합 재개 기록

2026-10-25 재개 시점의 기준 커밋은 `e8cc7b3`이다. 이 문서는 계획상의
기능 목록과 실제 코드의 구현 상태를 구별한다. 기준 커밋은 M1-B 병합 후
컴파일되지 않았지만, 아래 재개 작업으로 **M1 첫 픽셀 종료 조건을
충족했다.** 과거 감사 수치와 최신 구현/검증 상태를 구분해서 읽어야 한다.

## 9차 재개: 하네스 정합, core 보완, 브라우저 백엔드, 예제 132개, 매핑·Miri·크기

공유 설정 커밋(`6dd3f7b`: `ferrovgl-examples` 크레이트, web-sys/js-sys,
xtask 명령 자리, `Cargo.lock` 1회 갱신) 뒤 6개 트랙을 worktree에서 병렬로
진행하고 트랙마다 다른 에이전트가 리뷰·수정했다. 사용량 한도로 끊긴 4개
작업은 기존 worktree에서 이어서 마무리했다.

- 하네스: `lv_test_init`/`lv_test_wait`/`lv_test_fast_forward`/indev
  헬퍼/`lv_test_display_create`/`lv_test_screenshot_compare`를 함수 단위로
  맞췄다. 장치는 생성 시 바로 만들고, `wait`는 1ms씩 진행 후 pointer·encoder·
  keypad를 읽고, `move_to_obj`는 레이아웃을 갱신하지 않는다. 스크린샷은
  upstream이 지원하는 모든 포맷을 변환한다. 남은 차이(테마 기본 적용, fs,
  gesture 장치, 테스트 간 공유 상태)는 크레이트 문서에 적었다.
- core: `LV_USE_OBJ_ID`(자동 할당, find/stringify), 객체 ext_data,
  delete cb 해제 핸들, `lv_label_bind_text`, `lv_arc_bind_value`, widgets
  `float` feature. 건너뛰었던 observer 테스트를 포팅했다.
- 브라우저: Canvas2D 백엔드(부분 flush, DPR 좌표, 포인터·키·휠 입력,
  rAF 루프)와 위젯 갤러리 + 예제 선택 데모. wasm-pack release 1,208,760 B
  (complex gradients와 예제 포함). node 스모크 통과.
- 예제: `lv_example_*` **132개**(get_started, styles 23, anim, event,
  flex/grid/split 전부, scroll 전부, 위젯 13종). 스모크·상호작용·스냅샷 해시
  테스트와 `cargo run -p ferrovgl-sdl --example lv_examples -- <name>` 실행기.
  chart/keyboard/table/anim_timeline/대형 asset이 필요한 예제는 미포팅.
- 매핑: `cargo xtask gen-mapping [--check]`가 `docs/src/mapping.md`를 생성한다.
  upstream 공개 함수·별칭 2899개 중 1519개 매핑(52.4%), 711 제외, 662 미포팅.
- Miri: unsafe 두 모듈과 arena, core/draw-sw lib 테스트에서 UB 없음.
- 크기: `tools/nostd-size`(label+button, gc-sections)와
  `cargo xtask size-report`(기준 대비 .text 5% 게이트).
- 후속 처리: SDL/wasm 데모가 패리티 설정대로 complex gradients를 켜도록
  수정, CI에 wasm 데모·크기·매핑 검사·예제 실행·nightly Miri 확장 추가,
  parity CI를 `--all-features`로 수정, 하네스 README 갱신.

### 9차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **1467 통과, 0 실패, 83 ignored** |
| `ferrovgl-test --include-ignored` | **403 통과**; 새 실행에서 참조 PNG **140/140** byte-exact |
| `ferrovgl-examples --no-default-features` | 156 통과 |
| SDL 예제 실행기(dummy, 3프레임) | 132/132 |
| size-report | thumbv7em .text 202,328 B, thumbv6m 207,432 B; 기준 5% 이내 |
| clippy, nostd-check, fonts/mapping `--check`, wasm clippy, rustdoc | 통과 |

다음 범위: M4(0.1.0) 준비 — API 이름 감사(`#[must_use]`, `#[non_exhaustive]`),
semver 기준선, CHANGELOG, `cargo publish --dry-run` 순서, 원격 CI 실제 실행.

## 8차 재개: Arc·Spinner·Line·LED·Buttonmatrix·Textarea, observer, BiDi

foundation 커밋(`00493f5`)이 위젯 6종·observer·bidi/text_ap 스켈레톤과
theme apply 분기, feature 연결을 먼저 깔고, 8개 트랙을 worktree에서 병렬로
포팅한 뒤 트랙마다 다른 에이전트가 C 원본과 교차 리뷰했다.

- `lv_arc.c`, `lv_spinner.c`(arc 상속), `lv_line.c`, `lv_led.c`,
  `lv_buttonmatrix.c`, `lv_textarea.c` 전 함수와 upstream 테스트를 포팅했다.
- `lv_observer.c`(subject/observer, obj flag/state 바인딩, bar/slider
  `bind_value`)를 포팅했다. label/arc/roller/dropdown/scale 바인딩과
  `test_bindings.c`는 해당 위젯 또는 `lv_label_bind_text`가 없어 미포팅이다.
- `lv_bidi.c`, `lv_text_ap.c`와 `dejavu_16_persian_hebrew` 폰트를 추가해
  `checkbox_rtl_1.png`, `label_rtl_dot_long_mode.png`가 일치한다.
- `arc_2.png`는 upstream 테스트가 `LV_BIN_DECODER_RAM_LOAD`(parity 설정에서
  꺼짐) 안에 있어 비교 대상이 아니다. `test_led_properties`도
  `LV_USE_OBJ_PROPERTY`가 꺼져 upstream에서 빈 테스트다.
- 후속 처리: font-conv 목록에 dejavu 추가(`xtask fonts --check` 대상),
  `nostd-check` feature 목록에 새 위젯·observer·bidi 반영,
  checkbox RTL 주석 정리, rustdoc private link 수정.

### 8차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **1246 통과, 0 실패, 83 ignored** |
| `ferrovgl-test --include-ignored` | **392 통과**; `target/parity` 삭제 후 새 실행에서 고유 참조 PNG **140/140** byte-exact |
| clippy all-targets/all-features, core/draw-sw/fonts/widgets no-default-features | 경고 0 |
| nostd-check(새 feature 포함), fonts --check, wasm32, rustdoc `-D warnings` | 통과 |

v0.1 위젯 13종이 모두 구현됐다. 남은 후속 과제:
- `TestDisplay::wait`가 upstream `lv_test_wait`의 마지막 indev read를 하지
  않고, `move_to_obj`가 layout을 갱신하는 차이(textarea 에이전트 보고).
- widgets 크레이트의 `float` feature 전달, `lv_label_bind_text`, 객체 id.
- worktree에는 git-ignored `tests/upstream`이 없어 에이전트가 심볼릭 링크로
  우회했다(커밋되지 않음).

## 7차 재개: Bar·Slider·Switch·Checkbox와 M1-A 독립 감사

기준 커밋 `8fb6442`(6차까지의 미커밋 작업을 검증 후 커밋)에서 출발했다.
foundation 에이전트가 default/simple theme의 스타일 전체(`my_theme_styles_t`
66개)를 이름 있는 필드로 옮기고 네 위젯의 클래스·API 스켈레톤과 theme apply
분기를 먼저 커밋했다(`4911e93`). 이후 위젯 4종과 M1-A 감사 13개 그룹을
worktree에서 병렬로 진행하고, 위젯마다 다른 에이전트가 C 원본과 교차 리뷰했다.

- `lv_bar.c`, `lv_slider.c`(bar 상태 임베드), `lv_switch.c`, `lv_checkbox.c`
  전 함수와 `test_bar.c`/`test_slider.c`/`test_switch.c`/`test_checkbox.c`를
  포팅했다. 새 참조 이미지 **14장** byte-exact: bar 9, slider 3, switch 1,
  checkbox 1. `bar_2.png`는 upstream이 화면을 공유해 이전 테스트의 flex/pad
  설정을 물려받는 점까지 재현해야 일치했다.
- `checkbox_rtl_1.png`는 BiDi 알고리즘, Arabic/Persian 처리,
  `dejavu_16_persian_hebrew` 폰트가 없어 미포팅이다(기하 테스트만 대체).
  observer binding(`lv_bar_bind_value`, `lv_slider_bind_value`)도 core
  observer가 없어 미포팅이다.
- M1-A 감사: blend 4그룹은 발견 0건. style·text·timer/tick·grad·mask·draw
  core·anim/palette·rect/label/mask·theme 9그룹에서 찾은 차이를 반영했다.
  예: fade mask의 `LV_OPA_MIX2` u8 절단, anim 속도 clamp 순서
  (`MAX(min, MIN(v, max))`), bg image 소스 판별, theme 전역 공유 의미,
  timer/anim ext_data. 기존 참조 82장은 모두 그대로 일치한다.
- `ferrovgl-test`가 새 위젯 feature를 켜도록 `Cargo.toml`을 보강했다.

### 7차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **1015 통과, 0 실패, 60 ignored** |
| `ferrovgl-test --include-ignored` | **173 통과**; 고유 참조 PNG 82 + 14 = **96장** RGB byte-exact |
| clippy all-targets/all-features, core/draw-sw/widgets no-default-features | 경고 0 |
| nostd-check, wasm32 widgets check, rustdoc | 통과 |

다음 범위: arc, line, led, spinner, buttonmatrix, textarea 위젯과 테마,
BiDi/Arabic·추가 폰트, observer, 객체 id.

## 6차 재개: Button·Image·테마 확장과 event/draw buffer 감사

Button/테마, Image, M1-A event/draw buffer 원본 감사를 세 에이전트에
분담했다. root가 feature·umbrella·SDL 데모를 연결하고 Button/테마 C
구현을 대조했으며, 구현 담당자와 다른 에이전트가 event/draw buffer와
Image를 교차 리뷰했다. 기존 미커밋 변경과 worktree/upstream을 보존했다.

- 실제 `BUTTON_CLASS`와 `Button` 핸들을 구현했다. content 크기,
  default focus group, scrollable=false/scroll_on_focus=true는 C와 같다.
  입력·checkable 상태·draw는 base object를 상속한다.
- default theme에 버튼의 primary/secondary 색상, pressed recolor/grow,
  keyboard focus outline, disabled와 80ms transition/70ms release delay를
  연결했다. 그림자 PNG 비교에서 발견한 `LV_OPA_50`은 정확한 C 상수
  127로 맞췄다. simple theme는 screen/base/scrollbar/button 범위의
  고정 픽셀 스타일과 display별 소유·재사용·복사·해제를 구현했다.
  Label/Image에 별도 테마 스타일이 없는 것도 원본과 같다.
  교차 리뷰에서 STYLE_CHANGED가 객체를 삭제한 뒤 다음 style 추가로
  진행하던 문제를 찾아 style 경계마다 유효성을 확인하고, public
  theme_apply를 통한 default/simple 삭제 회귀 2개를 추가했다.
- 실제 `IMAGE_CLASS`, `Image`, `ImageAlign`을 구현했다. 메모리 이미지와
  font symbol, 회전·축별 scale·percentage pivot·offset·9가지 정렬,
  stretch/contain/downscale/cover/tile, cover/hit/self-size/ext-draw와
  callback 삭제를 다룬다. 교차 리뷰에서 rotation/scale setter는 refresh
  callback 이전 pivot을 유지하고 pivot setter만 다시 읽는 C의 차이를
  찾아 반영했다. 실제 INVALIDATE_AREA event 영역을 검사하는 회귀로
  세 경로를 구분한다. `Rc<DrawBuf>`가 widget과 pending draw task의
  픽셀 수명을 유지한다. core decoder metadata 함수를 공개해 재사용한다.
  production filesystem/PNG decoder·registry, observer/property reflection은
  미구현이다. bitmap mask는 descriptor까지 전달하지만 direct SW image
  경로는 pinned C처럼 적용하지 않는다(layer 합성 경로에서 적용).
- 이전 snapshot PNG 4장을 실제 Image 위젯으로 재표시하도록 전환했다.
  새 image PNG 21장은 upstream 로고 asset을 테스트에서만 ARGB로 decode한
  뒤 실제 widget API로 렌더링한다. C 배열과 PNG asset의 바이트 동일성과
  SHA-256을 확인했고 참조 PNG 픽셀은 입력으로 사용하지 않았다.
  Button의 press-lock/scroll PNG 2장과 min-size PNG 3장도 추가했다.
- `lv_event.c` 전함수와 public/private header를 대조해 event external
  data 설정·콜백 전달·삭제 훅을 `Rc<dyn Any>`로 구현했다. 삭제 훅은 C의
  mark/deferred cleanup 순서를 지키되 실행 중 event는 안전하게 소유권을
  유지한다. dispatch 중 추가한 callback이 중첩 콜백에서 객체/display/
  indev와 함께 삭제될 때 초기 snapshot 밖의 descriptor가 남던 누수를
  고쳤다. 신규 event 회귀 4개를 포함해 25개가 통과했다.
- `lv_draw_buf.c` 전함수·헤더·generic premultiply 분기를 대조했다.
  `DrawBuf::init`의 실제 저장소 길이 검증을 보강하고, static이 아닌
  저장소에서 빈 이미지처럼 보이던 `to_image`를 `Option<ImageDsc>`로
  바꿨다. heap/shared/mutable buffer는 `ImageSrc::DrawBuf(Rc<_>)`를 쓴다.
  I8 palette 256개 검증에서 u8 변환 때문에 0개만 확인하던 테스트를
  고치고 indexed palette·stride·reshape·소유권 회귀를 추가했다(총 18개).
  custom allocator/cache/copy/clear/stride handler와 HW accelerator,
  raw-pointer/deprecated aliases, big-endian/YUV ABI는 미구현이며
  OOM/임의 악성 header에 대한 전체 증명은 이번 감사 범위가 아니다.
- SDL 데모의 base-object 타일을 실제 Button/Label/Image로 교체했다.
  여섯 버튼이 ARGB 화살표 버퍼 하나를 공유하고 활성화마다 45도 회전한다.
  기본 테마의 pressed/focus 상태를 사용하며 `activated - total`은 ASCII
  hyphen을 유지한다. 첫 버튼 활성화 후 실제 framebuffer를 PNG로 내보내
  레이아웃·회전·상태 문구를 육안 확인했다. 임시 캡처 소스/컴파일 산출물은
  정리하고 `target/interactive-demo.png`만 보존했다.

새 고유 참조는 **26장**(Button 5 + Image 21)이며 총 **82장**이다.
기존 label frame 0와 button scroll 초기 장면 재비교를 포함해 총 비교
호출은 **84회**다. 참조 이미지와 pinned upstream 소스는 수정하지 않았다.
전체 LVGL 위젯이나 M2 완료를 의미하지 않는다. 다음 범위에는 나머지
M1-A 독립 감사, bar/slider/switch 등 위젯과 테마, 객체 id/ext_data,
RTL/Arabic·고급 gesture·browser backend가 남아 있다.

### 6차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **900 통과, 0 실패, 53 ignored**; 이전 대비 일반 회귀 30개 추가 |
| `ferrovgl-test --include-ignored` | **108 통과**; upstream 함수 52개, 고유 PNG **82장 RGB byte-exact** |
| workspace all-targets/all-features clippy | 경고 0 |
| core/draw-sw/fonts/widgets no-default-features all-targets clippy | 경고 0; 기존 SW test helper의 feature cfg 누락도 수정 |
| Image 단독 / Button 없는 두 theme strict clippy | 경고 0; Image는 Label feature 없이 동작 |
| thumbv7em / thumbv6m release | core/draw-sw/fonts/widgets 8개 빌드 통과; 마지막 Image 수정 후 widgets 두 타깃 재확인 |
| wasm32 | 기존 backend 빌드 및 Label/Button/Image/default/simple widget feature check 통과 |
| fmt / whitespace / workspace rustdoc | 통과; rustdoc `-D warnings` |
| SDL interactive | dummy 3회 loop 통과; 별도 실제 activation framebuffer 육안 확인 |

ignored 53개는 upstream 함수 52개와 기존 문서 예제 1개다. Image 단독
회귀는 12개, theme 단위 16개, Button class 단위 1개다. reference 결과
82개 모두 matched이며 comparison tolerance는 계속 0이다. 퍼징 캠페인은
이번에 다시 실행하지 않았고 5차 결과를 새 실행으로 계산하지 않았다.
원격 CI 실행이나 사람의 마우스/키보드 조작을 확인했다는 뜻은 아니다.

기본 `debug=0`, `incremental=true`와 기존 증분 캐시를 유지했다. 최종
`target`은 **4,456,220 KiB(약 4.25 GiB)**이며, 그중 `target/fuzz`는
676,900 KiB(약 0.65 GiB)로 이전과 같다. 새 widget/feature/target 조합과
테스트 캐시만큼 총량은 늘었다. 검증용 임시 캡처 빌드와 raw framebuffer
약 9.2 MiB(logical bytes)만 정리했다. 변경 없는 SDL 재빌드는 wall time
**0.060/0.053초**였다. cold build나 수정 후 재컴파일 시간의 개선을
의미하지 않는다. 모든 로컬 개발·빌드·테스트 명령은 `uv run --locked`를
통했고 native Cargo lock도 유지했다. commit/push는 하지 않았다.

## 5차 재개: snapshot·스타일/스크롤 검증·퍼징 3종

snapshot 구현, 스타일/기본 테마 원본 대조, 스크롤 원본 대조를 세 에이전트에
분담했다. root가 퍼징과 예제·공유 feature 설정을 통합하고, 다른 담당자가
C 원본과 결과를 교차 리뷰했다. 현재 작업 트리에 반영했으며 commit/push는
하지 않았다. 이전 변경과 `.claude` worktree/upstream을 보존했다.

- `snapshot` feature에 `Ui::snapshot_create_draw_buf`,
  `snapshot_reshape_draw_buf`, `snapshot_take_to_draw_buf`, `snapshot_take`를
  구현했다. 레이아웃 갱신·ext draw 영역·cover 최적화·자식/형제/부모 post
  draw 순서, 재사용 버퍼, 중첩 호출, 기존 pending queue와 refresh display
  복원을 포함한다. 작업 수가 변하지 않는 dispatch가 513회 이어지면 C처럼
  실패 처리하고 하위 레이어를 회수한다.
- 스냅샷 회귀 13개는 실제 픽셀과 버퍼 주소·layer lifetime을 검사한다.
  객체/display 삭제와 `Ui::deinit` 콜백에서도 버퍼를 회수한다. 리뷰에서
  callback이 만든 같은 display의 독립 root까지 삭제하던 문제를 찾아
  snapshot 소유 계보만 정리하도록 수정했다. 독립 root의 pending child
  합성 작업 보존 → 이후 dispatch → 픽셀 → cleanup도 회귀로 남겼다.
  CHILD_CREATED는 버퍼 설치 후 한 번만 보내도록 C 순서를 맞췄다.
- C header의 계약대로 요청한 색상 포맷으로 실제 reshape한다. C 구현은
  이전 format을 유지해 요청 format과 stride가 달라질 수 있어 그대로
  복제하지 않았다. 크기/stride/용량 오버플로와 읽기 전용 버퍼는 거부한다.
  deprecated raw pointer/free는 `DrawBuf::init`과 Drop으로 대체한다.
  format enum 수락과 backend 렌더 지원을 구별하며 packed ARGB 일부는
  아직 SW 지원이 없다. 콜백이 `Ui::draw` 자체를 교체하거나 snapshot
  layer를 직접 삭제하는 경우 caller의 원래 버퍼 보존은 계약 밖이다.
- 스타일 통합 10개(ignored 1개 포함)는 margin PNG 2장, state refresh,
  transform mutation 6경로, transition step/kill 무효화, shared font
  상속/cache 갱신, theme 제거 시 local style 보존을 검증한다.
  default theme에는 light/dark × DPI 3개 × 해상도 경계 6개와 selector/
  scrollbar transition 순서 테스트 2개를 추가했다. 기존 구현이 C 동작을
  충족해 이 범위 production 코드는 수정할 필요가 없었다.
- 스크롤은 horizontal/vertical scrollbar PNG 2장과 783ms 간격으로
  text를 바꾸는 Scroll/ScrollCircular 라벨 20프레임을 재현했다.
  rotated child drag 2개와 animation callback 삭제 회귀도 추가했다.
  GET_SELF_SIZE 또는 SCROLL/SCROLL_END에서 객체가 삭제된 뒤 다음
  getter/다른 축/ancestor를 접근하던 경로를 수정하고 단위 회귀 4개를
  추가했다. 살아 있는 객체의 C 계산·이벤트 순서는 유지한다.
- `fuzz/`에 독립 Cargo.lock을 갖는 `style_ops`, `text_break`,
  `tree_mutation`을 추가했다. cargo-fuzz 0.13.2와 libfuzzer-sys 0.4.13,
  nightly 1.101.0(2026-10-24 commit)을 사용해 ASan·coverage instrumentation·
  debug assertions·overflow checks로 실행했다. 입력/객체 수를 제한하고,
  style 독립 모델·UTF-8 진행/경계·트리 양방향 일관성·삭제를 검증한다.
  `tools/fuzz.py`는 locked fetch 후 offline으로 실행하고 lock 변경을
  검사한다. cargo-fuzz에 --locked 전달 기능이 없는 점을 보완한다.
  주간/수동 CI에는 타깃당 600초와 corpus/crash artifact 보존을 설정했다.
  원격 CI가 실제 실행됐다는 뜻은 아니다.

새 스냅샷 테스트의 all-features 성공만으로는 충분하지 않았다. 별도
`ferrovgl-test` 실행에서 회전 장면 2장이 1단계 색상 차이를 보였다.
하네스의 `cf-argb8888-premultiplied` 누락이 원인이었다. pinned C Kconfig의
기본값 y와 transform 경로를 확인해 feature를 켰고, renderer/PNG를
보정하지 않고 단독 실행도 일치시켰다. Montserrat 28도 해당 장면에만
명시적으로 사용한다.

참조 이미지는 **29 → 56장**이다. 스크롤 22프레임 중 `label_scroll_0`는
기존 이미지와 겹치므로 새 고유 이미지는 스크롤/라벨 21 + 스타일 2 +
snapshot 4 = 27장이다. 전체 실행은 비교 57회, 고유 reference 56장이다.
`test_style.c` 자체에는 PNG assertion이 없어 새 스타일 PNG는
`test_margin_align.c`에서 가져왔다. snapshot PNG는 실제 API와 Label/base
object를 사용하되, 미포팅 image 위젯의 재표시 부분은 동일 버퍼를 image
primitive로 그린다. 전체 위젯 테마나 image 위젯 구현으로 계산하지 않는다.

### 5차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **870 통과, 0 실패, 30 ignored**; 이전 대비 일반 회귀 31개 추가 |
| `ferrovgl-test --include-ignored` | **71 통과**; upstream 함수 29개, 고유 PNG **56장 RGB byte-exact** |
| workspace all-targets/all-features 및 no-default-features clippy | 경고 0 |
| core no-default-features + snapshot clippy | 경고 0; float 없이 snapshot 빌드 확인 |
| fmt / fuzz package fmt / whitespace | 통과 |
| thumbv7em / thumbv6m release | core(snapshot 포함)/draw-sw/fonts/widgets, 8개 빌드 통과 |
| wasm32 / workspace rustdoc | 통과; rustdoc `-D warnings` |
| SDL interactive / object_snapshot 예제 | dummy 3회 loop / 390×170 RGBA PNG 생성 통과 |
| fuzz style_ops | 60초 제한, 실제 61초, **88,039회** 실패 없음 |
| fuzz text_break | 60초 제한, 실제 61초, **105,639회** 실패 없음 |
| fuzz tree_mutation | 60초 제한, 실제 61초, **31,249회** 실패 없음 |

ignored는 upstream 함수 29개와 기존 문서 예제 1개다. 참조 이미지는 그대로
두었고 최종 결과 56개 모두 matched다. 퍼징 합계 224,927회는 제한된
캠페인 결과이며 exhaustive coverage나 C 픽셀 패리티를 증명하지 않는다.
설치/실행/재현 안내는 [fuzz README](../../fuzz/README.md)에 있다.

`uv run --locked cargo run --locked -p ferrovgl-test --example object_snapshot`
으로 `target/object-snapshot.png`를 생성할 수 있다. 객체·자식 라벨·투명
그림자 여백을 포함하며 display refresh 전에 offscreen으로 렌더링한다.

기본 빌드의 debug=0/incremental=true를 유지했다. 최종 target은
3,978,976 KiB(약 **3.79 GiB**)이며 그 안의 ASan 전용 `target/fuzz`가
676,900 KiB(약 0.65 GiB)다. 새 feature 조합과 테스트 산출물만큼 캐시는
늘었다. 일반/퍼징 증분 캐시는 보존하고 1회 도구 설치용 `target/tool-build`
122.6 MiB만 정리했다. nightly compiler는 rustup 경로에 별도 설치된다.
변경 없는 SDL 재빌드는 wall time 0.049/0.055초(Cargo 내부 0.01초)였다.
이는 cold build나 코드 수정 후 재컴파일 성능 비교가 아니다.

M2 전체 완료로 표시하지 않는다. 미완료 M1-A 독립 fidelity 감사, 더 넓은
style/input/scroll 참조 검증, 나머지 위젯과 그 default/simple theme,
객체 id/ext_data, RTL/Arabic, 고급 gesture와 browser backend가 남아 있다.

## 4차 재개: 데모 글리프 수정과 M2 선·원호·삼각형·블러

인터랙티브 데모의 `activated`와 `total` 사이에 쓰인 문자는 em dash
`—`(U+2014)였다. 번들 Montserrat 14 subset에 이 글리프가 없어 네모로
표시됐다. 상태 문자열을 `Tile N activated - total M`으로 고쳤다.
폰트 데이터를 임의로 바꾸지 않았고 창 제목도 ASCII 하이픈으로 통일했다.

세 에이전트가 line, arc, blur를 맡고 root가 triangle, 공유 그림자 연결,
통합 검증을 담당했다. line/arc 담당자는 서로 C 원본을 검토했고 blur
담당자가 triangle과 그림자 완료 경로를 독립 검토했다. 기존 작업 트리의
변경을 보존했으며 `.claude` worktree와 upstream checkout은 정리하지 않았다.

- core line/arc/triangle의 descriptor, task 생성, getter, bounds/polyline
  API와 Ui dispatch 경로를 구현했다. SW 선의 수평/수직/기울기, 점선,
  둥근 끝, 원호의 wrap/full-ring/round-cap/image, 삼각형 line mask와
  horizontal/vertical gradient를 C 계산·정수 반올림 순서에 맞췄다.
- SW blur의 네 방향 IIR, quality별 skip, radius/corner, 바이트 포맷별
  처리를 구현했다. 1/3바이트 포맷의 horizontal 끝점 처리와 ARGB의
  alpha 보존/skip-row 복제 차이 등 원본의 특이점도 유지한다.
  packed sub-byte 포맷은 거부하고, 작은 버퍼의 skip 복제는 실제 경계를
  넘지 않게 제한한다. 정상 입력은 원본과 일치하며 이 제한은 안전성 적응이다.
- 그림자 생성·완료를 TaskSink의 Ui 경로로 연결했다. 기존 ctx-only
  완료가 blur/layer task를 draw unit에 평가시키지 못하던 문제를 고쳤다.
  자식 레이어의 CHILD_CREATED 알림도 전달한다. A8 silhouette → blur →
  parent composite, 중첩 ARGB layer, 작업 큐 완료와 레이어 회수를 검증했다.
  SDL과 headless 하네스에 그림자용 A8 feature를 켰다.
- 삼각형의 단일 gradient stop은 C처럼 상수색을 지원한다. clipping이
  mask를 Changed에서 FullCover로 바꾸면 부분 opacity의 반올림 결과가
  1만큼 달라지는 C 동작도 별도 회귀로 남겼다. 기준 이미지를 맞추려고
  렌더링 결과나 참조 픽셀을 보정하지 않았다.
- draw-sw의 오래된 crate-wide `unused_variables` 억제를 제거하고 실제
  미사용 좌표와 feature-off 분기를 정리했다. 최소 feature 빌드도 경고 0이다.

실제 참조 이미지가 **16 → 29장**으로 늘었다. 새 13장은 line 2, arc 4,
blur 4, triangle 1, drop-shadow/nested layer 2다. 새 primitive 장면은
upstream C의 draw 명령과 geometry를 callback/offscreen buffer로 재현한다.
**line/arc/canvas 위젯 API를 구현했다는 뜻은 아니다.** Arc 이미지 fixture는
pinned C asset의 원본 40,000바이트이며 source/hash를 테스트에 기록하고
다른 에이전트가 별도 추출해 확인했다. Blur에는 고정 C 실행 결과 25개
조합의 byte hash도 검증 자료로 추가했다.

### 4차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **839 통과, 0 실패, 22 ignored** |
| `ferrovgl-test --include-ignored` | **38 통과**, upstream 함수 21개에서 PNG **29장 RGB byte-exact** |
| 새 단위/통합 회귀 | line 10, arc 9, blur 3, triangle 6, shadow lifecycle/dispatch 2: 총 30개 |
| all-targets/all-features 및 no-default-features clippy | 모두 경고 0 |
| fmt / whitespace | 통과 |
| thumbv7em / thumbv6m release | core/draw-sw/fonts/widgets, 8개 빌드 통과 |
| wasm32-unknown-unknown | 빌드 통과 |
| workspace rustdoc | `RUSTDOCFLAGS=-D warnings`, 통과 |
| SDL interactive | dummy driver에서 `--frames 3` 정상 종료 |

ignored 22개는 upstream 함수 21개와 기존 문서 예제 1개다. upstream 함수는
최종 통합 코드에서 별도 실행했고 저장 결과 29개 모두 matched다.
SDL 검증은 자동화된 dummy 실행이며 사람이 창을 조작한 검증은 아니다.
실행 `todo!()`는 **18 → 0개**가 되었지만, 비어 있는 backend/위젯과
부분 구현은 이 수치에 드러나지 않으므로 M2나 전체 포팅 완료를 뜻하지 않는다.

빌드 설정은 dev/test `debug=0`, `incremental=true`를 유지했다.
이번 단계에는 캐시 삭제를 하지 않았다. 최종 `du -sk target`은
2,675,896 KiB(약 **2.55 GiB**)다. 3차 종료의 2.06 GiB에서 새 코드·추가
feature 조합과 검증 산출물만큼 증가했으며 최초 약 4.0 GiB보다는 작다.
변경 없는 SDL 예제 재빌드는 wall time 0.051/0.049초(Cargo 내부 0.01초)였다.
이는 cold build나 수정 후 재컴파일 벤치마크가 아니다.

다음 범위는 나머지 theme/snapshot, style/scroll 이미지 패리티 확대,
fuzz 3종과 미완료 M1-A fidelity 감사다. 객체 id/ext_data, 나머지 위젯,
RTL/Arabic 텍스트, 고급 gesture와 browser backend도 남아 있다.
현재 변경은 commit/push하지 않았다.

## 3차 재개: M2 입력·그룹·flex/grid와 빌드 용량 축소

이번 단계는 M2의 입력/레이아웃 기반을 구현한다. M2 전체 종료를 뜻하지
않는다. 입력·flex·grid를 세 에이전트가 나누고 root가 group, SDL 연결,
합성 입력 하네스, 빌드 설정을 담당했다. 서로 다른 담당자가 C 원본과
이벤트/계산 순서를 검토한 뒤 발견 사항을 수정했다.

- `indev`의 남은 미구현 API 60개를 채웠다. pointer/keypad/encoder/button,
  long press/repeat/click streak, directional gesture, hit testing, cursor,
  reset/wait-release, 이벤트, 타이머, scroll chain/elastic/momentum/snap을
  연결했다. `IndevDriver::read_with_ui`의 기본 구현은 기존 `read`로 위임하며
  UI 접근이 필요한 드라이버도 콜백 중 삭제·교체·해제를 안전하게 처리한다.
- group API 26개를 구현했다. 생성 순서, 기본 Prev refocus, wrap/freeze,
  hidden/disabled 순회, 편집 모드, key 전달, membership 정리, 이벤트 중
  객체/그룹 삭제를 포함한다. 직접 focus와 next/prev의 서로 다른 callback
  순서, DEFOCUSED 중 현재 focus 관측을 C와 맞췄다.
- flex는 row/column/wrap/reverse/RTL, alignment, grow/min/max 재분배,
  content size, padding/margin, floating/hidden 제외를 포팅했다.
  grid는 fixed/CONTENT/FR, span, alignment/stretch, RTL, subgrid를 구현했다.
  grid의 별도 minimum callback은 C에 없어 일반 child bounds로 위임한다.
- SDL에 pointer/keypad/encoder를 설치하고 마우스·control key·UTF-8 text·휠·
  middle-button을 연결했다. `set_group`으로 focus navigation을 설정한다.
  새 `interactive` 예제는 flex/grid에 배치한 base object 6개를 클릭하거나
  Tab/Shift+Tab/휠/Enter로 조작한다. button 위젯 구현으로 계산하지 않는다.
- `TestDisplay::create_inputs`, `TestInputs`, `SyntheticInput`을 추가했다.
  upstream의 50ms 간격 click/key/encoder helpers와 실제 input timer를 쓴다.
  press-state 픽셀 변경, 클릭 이벤트 순서, focus 이동, 단발 encoder 소비,
  button callback 중 삭제, pointer drag scroll을 통합 검증한다.
- 실제 참조 이미지는 **16장 일치**: label 5 + flex 9 + grid 2.
  원본 `tests/main.py`는 `depth_32.defconfig` 뒤에 `run_tests.defconfig`를
  적용한다. 후자의 reference DPI는 **130**이다. 이전 하네스 160을 공통
  설정에서 바로잡아 grid scrollbar의 245픽셀 차이를 해결했고 label도
  재검증했다. 과거 M1의 DPI160 기록은 이 감사로 정정한다.

독립 리뷰로 group의 defocus 관측 시점, layout size/event callback 중 삭제,
입력 read callback에서 명시적으로 해제한 driver가 복원되는 문제를 수정했다.
원본 release throw transform의 비균일 배율 계산에는 갱신된 scale_x로
scale_y를 구하는 특이점이 있어, drag transform과 구별해 C 동작을 보존한다.
GET_SELF_SIZE를 발생시키는 scroll getter들의 호출 순서도 유지한다.

안전성 적응: 재귀 same-device read/delete를 막고 callback 후 세대 핸들을
재검증한다. 잘못된 button index는 무시하고, 수렴하지 않는 throw prediction은
0을 반환하며, snap child가 사라졌을 때 sentinel 거리만큼 스크롤하지 않는다.
SDL Shift+Tab과 focus-loss release는 명시적인 host 편의 기능이다.
멀티터치 인식(`gesture-recognition` feature), SDL touch/IME composition,
입력 EXT_DATA API와 resize는 아직 구현하지 않았다.

실행 `todo!()`는 **114 → 18개**로 줄었지만 완성률이 아니다. 남은 함수는
core line/arc/triangle와 SW line/arc/triangle/blur에 있다. M2의 다음 범위는
이 렌더 프리미티브, 나머지 theme/snapshot, style/scroll 참조 확대,
fuzz 3종과 미완료 fidelity 감사다. 객체 id/ext_data, 브라우저 backend,
RTL 텍스트와 나머지 위젯도 후속 목록에 남아 있다.

### 빌드 설정

workspace `Cargo.toml`에서 dev/test/release의 debug 정보를 끄고,
dev/test의 incremental compilation은 유지했다. 기본 병렬 codegen, debug
assertion/overflow check와 외부 dependency의 기존 opt-level=2는 보존한다.
`--profile dev-debug`는 필요한 경우에만 full debug 정보를 별도 디렉터리에
만든다. 기본 빌드는 소스/변수 디버그 정보를 포함하지 않는다.

이전 설정의 debug 및 cross-target 산출물만 한 번 정리했고, PNG·parity 기록·
문서와 `.claude` worktree/upstream은 보존했다. 이후 빌드는 같은 target에
캐시를 누적한다. 일상적인 cargo clean이나 incremental 비활성화는 쓰지 않는다.
최종 `du -sk target`은 2,159,728 KiB(약 **2.06 GiB**, 표시 반올림 2.1G)다.
시작 시점 약 4.0 GiB에서 대략 절반으로 줄었다. 새 입력/레이아웃 코드와
전체 테스트·no_std·wasm·문서 산출물을 포함하고 host incremental 약 1.6 GiB를
유지한 결과다. SDL interactive 예제의 변경 없는 Cargo 재빌드 wall time은
두 번 모두 **0.050초**(Cargo 내부 표시는 0.01초)였다. 이 측정은 cold build나
코드 편집 후 재컴파일 속도 비교가 아니다. `target/build-footprint.txt`에
명령과 수치를 저장했다.

### 3차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **809 통과, 0 실패, 10 ignored** |
| `ferrovgl-test --include-ignored` | 25개 테스트 통과, 그중 upstream 함수 9개가 PNG **16장 RGB byte-exact** 비교 |
| group / input / flex / grid 단위 테스트 | 각각 10 / 21 / 18 / 11개, workspace 총계 포함 |
| 합성 입력 / SDL | 4개 / 3개 통과; 실제 timer·객체·framebuffer 및 dummy SDL event queue |
| all-targets/all-features 및 no-default-features clippy | 모두 경고 0 |
| fmt / whitespace | 통과 |
| thumbv7em / thumbv6m release | core/draw-sw/fonts/widgets 통과 |
| wasm32-unknown-unknown | 빌드 통과, browser backend 구현과 별개 |
| workspace rustdoc | `RUSTDOCFLAGS=-D warnings`, 통과 |

일반 실행의 ignored는 upstream test 함수 9개와 기존 문서 예제 1개다.
참조 테스트는 같은 최종 소스에서 별도로 실행했고 `.result` 16개 모두
matched, mismatch/error 0이다. 참조 파일을 바꾸지 않았다.

`interactive --frames 3`은 SDL dummy driver에서 event/render loop 3회,
presentation 1회로 정상 종료했고 `first-frame.png`를 DPI130으로 다시 생성했다.
SDL TextInput의 UTF-8 매핑은 safe sdl2 `push_event`의 해당 타입 직렬화 제한으로
adapter를 직접 호출해 검증하며, 나머지 mouse/control-key/wheel은 SDL event
queue에 주입한다. GUI 창을 사람이 조작했다는 의미는 아니다.
기존 변경을 보존했고 이번 작업도 commit/push하지 않았다.

## 2차 재개: label·기본 테마·SDL·5개 이미지 패리티

2026-10-25, label 구현·SDL 구현·독립 fidelity 리뷰를 분담하고 root가
기본 테마와 실제 upstream 시나리오를 통합했다.

- `ferrovgl-widgets::Label`은 실제 클래스/상태/event로 구현했다.
  Wrap/Dots/Scroll/ScrollCircular/Clip, recolor, selection, max-lines,
  owned/static/formatted text, UTF-8 위치 조회·삽입·삭제를 지원한다.
  크기 캐시와 UPDATE_LAYOUT_COMPLETED 이후 텍스트 갱신 순서를 C와 맞췄다.
- `theme::default`는 M1의 screen/card/scrollbar/scrolled 스타일을
  구현했다. light/dark 색상, DPI·해상도별 크기와 스타일 재초기화,
  scrollbar transition을 포함한다. 일반 label에는 card 스타일을 붙이지
  않고 screen의 text/font를 상속한다. 다른 위젯의 테마는 후속 범위다.
- 테마 state는 `Theme::extension`으로 사용자 데이터와 분리했다.
  PLAN에 따라 display당 스타일을 소유하며, `theme_copy`로 생긴 복사본을
  원본 default theme로 잘못 찾지 않도록 owner를 구분한다. `Ui::deinit`
  또는 명시적인 `theme::default::deinit`이 자원을 정리한다. 후자는
  live display/child theme/copy 참조가 있으면 해제를 거부한다.
- SDL은 단일 XRGB8888 direct 버퍼, software presentation, monotonic
  tick/timer/refresh, quit/close/expose를 구현했다. 실제 RGB readback 및
  타이머 중 display 삭제·deinit·invalid buffer 변경을 dummy SDL에서
  검증했다. widget 입력·창 크기 변경은 아직 지원하지 않는다.
- `ferrovgl-test/tests/parity_label.rs`에 원본 `test_label.c`의 다섯
  시나리오를 옮겼다. reference를 생성·수정하지 않고 실제 Rust 객체와
  renderer의 RGB를 비교해 **5/5 일치**했다. 목록과 재현 명령은
  [parity.md](parity.md)에 있다.
- `first_frame` PNG 예제도 수동 DRAW_MAIN 글자 그리기를 실제 Label로
  바꿨다. SDL `hello`는 같은 M1 기능을 실제 창에 표시한다.

독립 리뷰에서 Dots 텍스트를 cut한 뒤 UTF-8가 깨져 panic하는 사례를
재현했다. C처럼 같은 버퍼에서 NUL까지 옮기고 숨겨진 suffix를 유지해
수정했다. Dots 상태의 insertion은 C 자체가 잘못된 UTF-8를 만들 수 있어
원본 텍스트를 먼저 복원하고 삽입하는 Rust 적응을 적용한다. 이 차이는
문자열 유효성을 보존하기 위한 것이며 API/크레이트 문서에 명시했다.

한계: BiDi paragraph/RTL mapping, Arabic shaping, observer/translation은
미지원이다. 다섯 장면은 LTR 정적 label 검증이며 animated scrolling/RTL
이미지 패리티까지 증명하지 않는다. 기본 테마는 M1 부분 구현이고
SDL 입력·브라우저 backend는 미완료다. 실행 `todo!()`는 114개로 그대로이며,
새 label/theme/SDL은 기존 빈 crate에 추가됐기 때문에 그 수치에 반영되지 않는다.

다음 M2 작업은 group/input과 합성 입력 하네스·SDL 입력 연결, flex/grid,
남은 line/arc/triangle/blur와 관련 이미지 패리티다. 객체 id/ext_data,
M1-A 전체 독립 fidelity 감사도 후속 목록에 유지한다.

### 2차 검증 결과

| 검사 | 결과 |
|---|---|
| workspace all-features 테스트 | **746 통과, 0 실패**; upstream 5개와 기존 문서 예제 1개는 기본 실행에서 ignored |
| upstream `parity_label --include-ignored` | **5 통과, RGB byte-exact** |
| widgets 단위 테스트 | 14 통과, workspace 총계에 포함 |
| SDL dummy 테스트 | 2 통과, RGB readback·expose·quit·타이머 중 삭제/deinit/invalid stride 포함 |
| all-targets/all-features 및 no-default-features clippy | 경고 0 |
| fmt 및 whitespace 검사 | 통과 |
| thumbv7em / thumbv6m release | core/draw-sw/fonts/widgets 통과; widgets의 label와 theme-default 포함 |
| wasm32-unknown-unknown | 빌드 통과; browser backend 구현은 후속 범위 |
| rustdoc, `RUSTDOCFLAGS=-D warnings` | workspace/all-features 통과 |

SDL 실행은 `SDL_VIDEODRIVER=dummy`의 실제 SDL software renderer에서
검증했으며, 사람이 GUI 창을 조작하는 수동 검증까지 수행했다는 뜻은 아니다.
`hello --frames 3`은 3회 event/render loop와 정적 장면 1회 presentation으로
정상 종료했다. `target/first-frame.png`도 새 Label 기반 예제로 다시 생성했다.
Cargo.lock에는 SDL 예제용 기존 workspace font 의존성만 추가했고 외부
crate 버전은 바꾸지 않았다. 현재 작업 트리에 반영했으며 commit/push하지 않았다.

## 1차 재개: M1-B 통합 복구와 헤드리스 하네스

구조 감사, core 통합, SW 렌더러 검토, 하네스 구현을 분담하고 다른
담당자가 수정 결과를 C 원본과 교차 검토했다. 기존 worktree를 재사용하거나
병합하지 않고 현재 main의 작업 트리에서 파일 담당 범위를 나눠 수정했다.

- 중복 `Ui` 헬퍼를 단일 정의로 통합했다. style의 part 인자처럼 호출
  의미가 다른 곳은 인자를 명시했고, blur invalidation은 `obj_pos`의
  구현에 위임했다. 중복 deinit 스텁과 DisplayData 테스트 초기화 누락도
  해결했다.
- `obj_delete_async`와 취소를 현재 `Ui`의 타이머 기반 async API에
  연결했다. 타이머 테스트는 `Ui::new`가 생성하는 paused animation
  timer를 목록에서 명시적으로 검증한다.
- refresh의 하위 레이어 합성 3곳을 `Ui::draw_layer`로 연결해 draw unit
  평가가 실행되게 했다. 사용자 display sync는 `DisplayDriver::has_sync`
  계약으로 활성화했다. sync 영역 clip/offset/wait와 작업 평가를 검증했다.
- 테마 공통 API 14개와 layout registry 3개, `indev_get_next`, blur
  descriptor/task API를 포팅했다. 테마는 부모·클래스 상속 순서를
  보존한다. `theme_copy`의 사용자 데이터 공유는 `Rc<RefCell<Box<dyn Any>>>`
  로 표현하며 getter는 안전한 borrow guard를 반환한다. layout callback은
  등록된 `user_data: u32`도 전달받는다.
- SW renderer가 프레임 종료 시 circle mask cache를 정리하도록 연결했다.
  자식 ARGB 레이어를 부모 XRGB 레이어에 합성하는 실제 픽셀과 메모리
  회수 회귀 테스트를 추가했다.
- `ferrovgl-test`에 실제 Ui/display/refresh 기반 헤드리스 하네스와 PNG
  정규화·RGB 정확 비교·진단 산출물을 구현했다. 사각형/폰트 그리기,
  재렌더링·삭제·정리 및 비교 실패 처리를 테스트한다. default theme와
  label widget은 이 하네스에 아직 포함되지 않는다.
- `parity-report`는 중첩 경로의 `.result`를 읽어 matched/mismatch/error를
  구분하며, 실행 기록이 없으면 명시적으로 0건·미검증으로 표시한다.
- 프로젝트 로컬 uv 설정을 추가했다. Cargo.lock은 그대로 보존했고,
  xtask의 내부 cargo build에도 `--locked`를 적용했다.

실행 `todo!()`는 **135개에서 114개**로 줄었다. 이 수치는 작업량이나
전체 완성률을 나타내지 않는다. SDL/wasm backend, label과 default theme,
입력·그룹·flex/grid의 본체, SW line/arc/triangle/blur는 여전히 미완료다.
객체의 별도 LVGL id/ext_data 및 M1-A 전체 독립 fidelity 감사도 남아 있다.
이번 독립 리뷰는 수정 범위와 렌더 연결을 대상으로 했으며 전체 코드의
1:1 동일성을 보증하지 않는다.

업스트림 참조 이미지 검증은 **0건**이다. 헤드리스 RGB 회귀 테스트 성공과
M1의 참조 이미지 5장 일치 조건을 구분해야 한다. 다음 구현 단위는
default theme의 M1 부분과 label widget, 이후 SDL 예제와 실제
`test_label.c` 참조 이미지 대조다.

### 검증 결과

2026-10-25, 현재 작업 트리에서 아래 검사를 실행했다. 모든 개발 명령은
`uv run --locked`를 통했고, Cargo 의존성도 `--locked`로 고정했다.

| 검사 | 결과 |
|---|---|
| `cargo fmt --all --check` | 통과 |
| workspace/all-targets/all-features clippy, `-D warnings` | 통과, 경고 0 |
| core/draw-sw/fonts/widgets no-default-features clippy | 통과, 경고 0 |
| `cargo test --locked --workspace --all-features` | **730 통과, 0 실패**, 기존 매크로 문서 예제 1개 ignored |
| core 단위 테스트 | 474 통과 (위 workspace 총계에 포함) |
| SW renderer 단위 테스트 | 235 통과 (위 총계에 포함) |
| headless/PNG 비교 통합 테스트 | 12 통과 (위 총계에 포함) |
| `cargo xtask nostd-check` | thumbv7em 및 thumbv6m의 core/draw-sw/fonts/widgets release 빌드 통과 |
| wasm32-unknown-unknown `ferrovgl-wasm` 빌드 | 통과; backend 구현 여부와 별개 |
| workspace/all-features rustdoc, `RUSTDOCFLAGS=-D warnings` | 통과; 기존 잘못된 문서 링크도 수정 |
| `cargo xtask parity-report` | **0 matched / 0 run, 미검증**을 정상 표시 |

`uv run --locked cargo run --locked -p ferrovgl-test --example first_frame`
으로 `target/first-frame.png`를 생성하고 시각 검토했다. 800×480 화면의
객체 배경·둥근 모서리·테두리·Montserrat 14 텍스트를 확인했다. 텍스트는
`DRAW_MAIN`에서 draw descriptor로 그리므로 label 위젯 구현의 증거는 아니다.

이 표는 로컬에서 실행한 검사만 포함한다. 다른 OS의 CI, MSRV, 개별
feature 전체 조합, cargo-deny, Miri/fuzz 실행까지 완료했다는 뜻은 아니다.
변경사항은 현재 작업 트리에 있으며 새 커밋이나 원격 push는 하지 않았다.

## 진행 이력

| 커밋 | 작업 | 확인 가능한 범위 |
|---|---|---|
| `e4ebd1e` | M0: workspace, 기본 misc 포팅, 폰트 변환 파이프라인 | 초기 골격과 테스트, 24개 `.lvfnt` 자산 |
| `620992d` | M1 공용 타입 골격 | LVGL 헤더에 대응하는 객체·디스플레이·드로우·입력 등의 타입/API |
| `37a0a1b` | M1-A 첫 통합 | style, text, timer/tick, gradient, mask, draw-core 6개 그룹. 커밋 메시지에 240개 테스트 및 게이트 통과 기록 |
| `86c5e8f` | M1-A 재실행 8개 브랜치 병합 | event, draw-buf, blend 4개 그룹, anim/palette, draw rect/label/mask. 이전 작업 보고에 419개 테스트 및 게이트 통과 기록 |
| `e8cc7b3` | M1-B 14개 브랜치 병합 | 객체 6개 그룹, refresh/display/init, SW draw unit/fill-border-letter/shadow-mask/image/transform |

M1-A 첫 통합은 여러 포팅 결과를 한 커밋으로 가져왔다. 이후 두 통합은
각각 실제 8개와 14개 브랜치를 부모로 갖는 merge commit이다. M1-B에는
12개 그룹의 `fix(...): apply fidelity review` 커밋이 존재하며, init과
transform은 포팅 커밋만 존재한다. 이것은 수정 이력의 증거이며,
독립 검토가 모든 행과 실행 경로의 동일성을 입증했다는 뜻은 아니다.
별도의 영구 검토 보고서는 저장소에서 확인되지 않았다.

재개 시점에는 ferrovgl 저장소에 등록된 M1-B worktree 14개가
`.claude/worktrees/`에 남아 있었다. 같은 디렉토리에는 이전 실행의
추가 디렉토리 3개도 있다. 업스트림 LVGL 저장소에는 잘못 생성됐던
worktree 10개가 별도로 등록돼 있다. 따라서 명령 실행 시 반드시
ferrovgl 루트와 upstream 루트를 구별해야 한다. 이 기록을 위해 기존
worktree나 브랜치를 삭제하지 않았다.

## 기준 커밋의 구조

Cargo workspace는 라이브러리 9개와 개발 도구 2개로 구성된다.

| 구성 | 현재 구현 |
|---|---|
| `ferrovgl-core` | `no_std + alloc`. Ui, 세대 핸들 arena, 객체 클래스/트리/좌표/스크롤/스타일/이벤트, refresh/display, timer/animation/text/font, draw model 포팅. 입력·그룹·테마·레이아웃 등은 일부 골격 |
| `ferrovgl-draw-sw` | `no_std + alloc`. SW 작업 평가/디스패치, fill/border/letter/label/shadow/mask/gradient/image/layer/transform, 포맷별 blend 구현. line/arc/triangle/blur는 골격 |
| `ferrovgl-fonts` | 내장 `.lvfnt` 24개, feature별 정적 폰트 제공. 일반/압축 Montserrat 등 데이터 테스트 |
| `ferrovgl-widgets` | 빈 `no_std` 라이브러리. Cargo feature 이름은 선언됐지만 label 등 위젯과 default/simple theme 구현은 없음 |
| `ferrovgl-test` | 해상도·DPI·upstream 경로 상수만 있음. display 생성/스크린샷 비교/입력 합성 하네스 없음 |
| `ferrovgl-sdl` | 창 제목 상수만 있음. SDL backend나 예제 없음 |
| `ferrovgl-wasm` | canvas id 상수만 있음. 브라우저 backend 없음 |
| `ferrovgl-embedded` | 빈 `no_std` 라이브러리. 계획상 v0.2 범위 |
| `ferrovgl` | feature별 crate 재노출과 기본 도형/색상 prelude |
| `tools/font-conv` | 업스트림 C 폰트 배열 파싱과 `.lvfnt` 인코딩 |
| `xtask` | `refimgs sync`, `fonts [--check]`, `nostd-check`, 초기 `parity-report` |

소유권의 중심은 `Ui`다. 객체·디스플레이·입력 장치·그룹·스타일·테마는
arena에 저장되고 `ObjId`, `DisplayId` 등의 세대 핸들로 참조된다.
별도의 순서 벡터를 유지해 arena 슬롯 재사용이 upstream linked-list
순서를 바꾸지 않도록 했다. 위젯 상태는 `WidgetState`와 타입 질의로
클래스 상속을 표현한다. 이벤트 콜백은 `Rc`와 이벤트 프레임 스택을
통해 콜백 중 삭제와 재진입을 지원한다.

렌더링 경로는 객체 스타일/이벤트 → `Ui::draw_*` → `DrawTask`/`Layer`
→ 등록된 `DrawUnit` → SW rasterizer/blend → display flush다.
`DrawCtx`와 draw unit 목록은 Rust의 가변 차용을 분리하기 위해 `Ui`의
서로 다른 필드다. 일부 `DrawCtx::draw_*` API는 unit 없이 작업만 만드는
경로이므로, 실제 UI에서 전체 평가·디스패치를 검증할 때는 `Ui` 경로를
사용해야 한다.

README와 PLAN의 `ui.obj(obj).set_width(...)` 체이닝 API는 목표 형태다.
현재 `Ui::obj`는 `ObjData`를 빌려주는 API이고, 실제 변경은
`ui.obj_set_width(obj, ...)` 같은 메서드 중심이다. 문서의 API 예시를
이미 구현된 공개 인터페이스로 간주하면 안 된다.

## 업스트림과 패리티 기준

로컬 원본은 `tests/upstream/lvgl/`에 있고 HEAD는 고정된
`80ca777e37a2b176770726a02e07a6fb79ef0b39` (`v9.6.0`)이다.
`xtask/src/main.rs`가 URL, 태그, 전체 commit hash, sparse 경로를 관리한다.
원본 `src`, `include`, 테스트 시나리오·설정·참조 이미지가 이미 내려받아져
있으며 PNG 참조 파일은 하위 디렉토리를 포함해 **976개**다.

정확 비교의 기준은 [parity.md](parity.md)에 기록돼 있다: 800×480,
XRGB8888, direct mode, DPI 160, Montserrat 14, RGB 각 채널 허용 오차 0,
alpha 비교 제외. 목표는 PNG 압축 파일 자체의 바이트 동일성이 아니라
디코딩한 픽셀의 RGB 동일성이다.

현재 `parity-report`는 `target/parity`의 최상위 `_diff.png` 파일을 셀 뿐,
실행한 시나리오 수나 성공 수를 기록하지 않는다. 따라서
“No parity failures recorded”는 **0개 실패 파일**이라는 뜻이며,
이미지 패리티를 통과했다는 증거가 아니다. 현재 하네스가 없어 M1의
“업스트림 이미지 5개 이상 일치” 조건은 미검증이다.

## 미구현 수치와 통합 위험

기준 커밋의 Rust 소스에는 `todo!()` 문자열이 147번 나타난다.
그중 주석을 제외한 실행 가능한 `todo!()` 문장은 **135개**다.

| 영역 | 실행 `todo!()` |
|---|---:|
| 입력 장치 `indev` | 61 |
| 그룹 `group` | 26 |
| 테마 공통 API `theme` | 14 |
| layout registry / flex / grid | 3 / 5 / 5 |
| core draw line / arc / triangle / blur | 4 / 4 / 3 / 3 |
| SW line / arc / triangle / blur | 4 / 1 / 1 / 1 |
| 합계 | 135 |

이 수치는 구현량 백분율이 아니다. 빈 widgets/backend crate에는
`todo!()`조차 없고, 일부 기능 제한은 정상 반환이나 no-op으로 구현돼
있다. 소스의 `#[test]` 선언은 716개지만 이것도 feature 조합에 따른
실제 실행·성공 개수와 다르다.

재개 시점에 파악한 주요 위험은 다음과 같다.

1. `impl Ui` 안의 private helper 이름도 파일별 namespace로 분리되지
   않는다. M1-B가 중복 `style_num`, `style_color`, `child_at`,
   `invalidate_area_core`, `obj_style_deinit`, `blur_expand_walk_cb`를
   합치지 않고 병합해 컴파일 오류를 냈다. 공용 기능인지 다른 의미의
   helper인지 확인한 뒤 통합하거나 이름을 구별해야 한다.
2. `display_create`가 기본 screen을 만들면서 `obj_class_init_obj`를
   거치고, 이 함수는 아직 `todo!()`인 `theme_apply`를 호출한다.
   기본 display 하나 생성하는 경로부터 실행 검증이 필요하다.
3. content size와 layout 갱신 경로는 아직 `todo!()`인
   `layout_get_min_size`/`layout_apply`에 도달한다. no-layout 객체도
   정상적으로 지나갈 공통 registry 동작이 필요하다.
4. 여러 단위 테스트는 `raw_obj`나 직접 arena 삽입으로 미구현 서비스를
   우회한다. 단위 테스트 증가만으로 생성→갱신→렌더링→삭제 경로가
   동작한다고 판단할 수 없다.
5. `bidi` feature의 문단 재배열/기본 방향 탐지는 아직 identity/LTR
   대체 경로다. 입력, observer, snapshot 등 feature 선언 역시 실제
   포팅 완료 여부와 구별해야 한다.
6. 이미지 decoder registry와 파일 시스템은 없다. 메모리 이미지와
   레이어를 지원하지만 파일 경로, symbol/custom decoder 경로는
   지원하지 않는다. matrix transform은 해당 upstream 설정의 비활성
   분기를 따르고 있으며 일반 matrix 구현은 아직 없다.

## 공용 타입 변경 요청의 실제 상태

| 항목 | 기준 커밋에서 확인한 상태 |
|---|---|
| `Ui::new`에서 animation 초기화 | 반영됨: `anim_core_init()` 호출 |
| display vsync 콜백의 동일성 비교 | 반영됨: `Rc` pointer identity와 user-data identity 사용 |
| `DisplayData::ext_data` | 타입·기본 생성자·setter·해제 경로는 반영됨. refresh 테스트의 직접 초기화 literal 한 곳이 누락되어 통합 오류 |
| `ObjData::ext_data` | 미반영. `obj_class` 생성/소멸자의 관련 주석에 누락 명시 |
| 객체의 LVGL `id` | 미반영. 세대 핸들 `ObjId`와 별개이며 tree dump는 `obj0` fallback 사용 |
| 공통 `Ui::draw_blur` | 미반영. `obj.rs`의 private helper에 전체 task finalization이 있으나 `draw/blur.rs`는 골격 |
| display sync callback 존재 여부 | 미반영. `refr.rs::disp_has_sync_cb`가 항상 false라 사용자 sync 분기가 실행되지 않음 |
| blur invalidation 소유권 | `obj_pos`와 `refr` 양쪽 구현/관련 helper가 겹침. 원본 소유 파일과 위임 경로 정리 필요 |

## 최초 감사에서 제안했던 작업 순서

아래 1~5번의 M1 범위는 두 차례 재개에서 완료했다. 현재 후속 범위는
6번이며, 최신 결과는 문서 상단의 2차 검증을 기준으로 한다.

1. M1-B 통합 오류를 해결하고 fmt, clippy(all/no-default), workspace
   tests, Cortex-M 두 타깃, wasm 검사를 실제로 다시 실행한다.
2. 테마 공통 API와 no-layout 공통 경로를 포팅하고, 공개 API만으로
   display 생성→객체 생성/스타일→첫 프레임→삭제까지 통합 테스트한다.
   실패를 숨기기 위한 catch/panic 우회는 완료로 간주하지 않는다.
3. 헤드리스 `ferrovgl-test` 하네스를 구현한다. 실제·기대·diff PNG와
   실행/성공/실패 수를 남기며, 참조가 없거나 테스트가 실행되지 않은
   상태를 성공으로 보고하지 않는다.
4. M1에 필요한 default theme 부분과 label widget을 upstream과 대조해
   구현한다. renderer 단위 호출과 정상 객체 경로를 모두 검증한다.
5. SDL display backend와 둥근 사각형+Montserrat 14 예제를 연결하고,
   upstream 시나리오 5개 이상에서 RGB byte parity를 달성한다.
6. 남은 입력/그룹/flex/grid/도형/blur와 M1-A 독립 fidelity review를
   별도 범위로 진행한다. 단일 파일 테스트와 병합 후 전체 게이트를
   구분하고, 공용 타입 변경은 한 담당자가 통합한다.

개발 명령은 루트의 uv 환경에서 `uv run --locked cargo ...`로 실행한다.
Rust 의존성은 Cargo.lock도 함께 유지한다. 과거 커밋의 테스트 성공
기록은 현재 통합 상태의 성공으로 재사용하지 않는다.
