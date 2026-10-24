# LVGL v9.6.0 → 순수 Rust 재구현(ferrovgl) 및 오픈소스 배포 계획

## Context

- 목표: LVGL(C, MIT) 최신 릴리스 **v9.6.0**(2026-09-16, v9 시리즈 마지막 안정화 릴리스)의 아키텍처와 동작(obj 트리, 스타일, 소프트웨어 렌더러, flex/grid, 위젯)을 **C 코드를 링크하지 않고 순수 Rust로 재구현**하고 crates.io/GitHub에 오픈소스로 배포한다.
- 디렉토리명 `lvgl-cv-rust`의 cv = **convert**. 작업 디렉토리는 현재 비어 있는 greenfield.
- 사용자 결정 사항:
  - 구현 방식: 순수 Rust 재구현 (바인딩 아님, unsafe 최소화)
  - API 스타일: Rust 관용적 API + LVGL 개념 1:1 매핑 (`lv_obj_set_width(o,100)` → `o.set_width(100)`, 타입명은 Label/Slider/Style/Flex 등 LVGL 문서 그대로)
  - v0.1 타깃: 데스크톱 SDL2 시뮬레이터 우선(wasm32 보조), 코어 크레이트는 처음부터 `#![no_std]` + `alloc`. 임베디드 HAL 백엔드는 v0.2
  - 이름: LVGL 상표 혼동을 피하는 새 이름, 라이선스 **MIT OR Apache-2.0**
- 환경: WSL2, rustc/cargo 1.99.1 (stable), 타깃 `x86_64-unknown-linux-gnu`, `wasm32-unknown-unknown` 설치됨; `sdl2-config`, cmake, gcc, clang, `trunk`, `wasm-pack` 사용 가능.

## 1. 조사 결과 요약 (LVGL v9.6.0)

### 소스 구조 (`src/`)
| 디렉토리 | 내용 |
|---|---|
| core | lv_obj, lv_obj_class(클래스 상속), lv_obj_pos, lv_obj_scroll, lv_obj_style(+cache), lv_obj_tree, lv_obj_event, lv_obj_draw, lv_obj_property, lv_group, lv_observer, lv_refr |
| display / indev | lv_display(draw buffer, flush, partial/direct/full), lv_indev(pointer/keypad/encoder/button), gesture, indev_scroll, gridnav |
| draw | lv_draw(task/layer/unit), draw_buf, rect/label/image/line/arc/triangle/mask/blur/vector/3d; 유닛: **sw**, vg_lite, nema_gfx, opengles, nanovg, sdl, dma2d, pxp, ppa, dave2d, eve, sifli |
| draw/sw | fill, border, box_shadow, grad, img, letter, line, mask, mask_rect, transform, triangle, arc, blur, vector, `blend/`(포맷별), arm2d |
| misc | anim, anim_timeline, area, array, async, bidi, circle_buf, color, color_op, event, grad, iter, ll, lru, math, matrix, palette, rb, style, style_gen, text, timer, tree, utils, cache/ |
| font | lv_font, fmt_txt, binfont_loader, tiny_ttf, freetype, imgfont, font_manager; 내장 montserrat 8~48(+14_aligned, 28_compressed), unscii 8/16, dejavu persian/hebrew, source_han_sans CJK |
| layouts / themes | flex, grid / default, simple, mono |
| widgets (42) | arc, arclabel, bar, button, buttonmatrix, calendar, canvas, chart, checkbox, dropdown, image, imagebutton, keyboard, label, led, line, msgbox, roller, scale, slider, span, spinbox, spinner, switch, table, tabview, textarea, tileview, animimage, qrcode, barcode, gif, lottie, 3dtexture, ime, ffmpeg, gstreamer, property; **deprecated(v10 제거)**: list, menu, win, file_explorer |
| 기타 | stdlib, osal, tick, fs, image(디코더), libs, others(snapshot, sysmon, monkey, fragment, translation, test), logging, debugging |

### v9.6.0 핵심 변화
- sw 렌더러 재작성(픽셀→워드 단위, 평균 20~25% 빨라짐), 공개 API `include/lvgl/` 분리, `LV_CHECK_ARG` 인자 검사, `LV_COLOR_FORMAT_DEFAULT`(기본 RGB565)가 `LV_COLOR_DEPTH` 대체, 플래그별 전용 API, obj state API, Kconfig 단일 소스, list/menu/win 폐기.
- 테스트: `tests/ref_imgs/*.png` 85장 + `draw/`, `libs/`, `widgets/` 하위; 800x480, 32bit(XRGB8888), DPI 160, montserrat 14, `LV_USE_FLOAT=y` 구성으로 스크린샷 비교(`tests/main.py`).
- 기본 lv_conf: `LV_MEM_SIZE` 64KB, 리프레시 33ms, DPI 130, montserrat_14, flex/grid on.

### Rust 선행 프로젝트와 차별점
| 프로젝트 | 성격 | 상태 |
|---|---|---|
| `lvgl` (공식 lv_binding_rust) | bindgen 바인딩 | LVGL v8, 2023 이후 미유지 |
| `oxivgl` | C v9.5 위 safe 바인딩 | 활발, nightly 필요 |
| `rlvgl` (SoftOboros) | 순수 Rust 재구현 | 0.2.x, 50 stars, embedded-graphics 기반, 특정 버전/픽셀 호환 주장 없음 |

**차별점**: v9.6.0 **알고리즘을 1:1 포팅**(스타일 조회, flex/grid, sw 렌더러의 정수 연산까지)하여 **업스트림 ref_imgs와 바이트 단위 픽셀 패리티**를 헤드라인 지표로 삼음. stable Rust, no_std 코어, 안전한 공개 API, unsafe는 2개 모듈로 한정.

### 이름 / 라이선스
- crates.io(2026-10-24): `lvgl`, `rlvgl`, `oxivgl`, `lvr` 사용 중. `ferrovgl`, `rustvgl`, `lvglrs`, `lvgl-rs`, `rvgl` 비어 있음(단 `rvgl`은 Re-Volt 게임 포트 "RVGL"과 충돌).
- **결정: `ferrovgl`** (crates.io, GitHub 모두 충돌 없음, LVGL 접두어 미사용). 하위 크레이트는 `ferrovgl-core` 등. M0에서 접두어는 한 번의 치환으로 변경 가능.
- 라이선스 MIT OR Apache-2.0. `NOTICE`에 "LVGL(Copyright (c) 2021 LVGL Kft, MIT)에서 파생된 재구현" 명시, 포팅한 파일 헤더에 업스트림 경로 기재. `TRADEMARK.md` + README 배너: "LVGL LLC와 무관한 독립 프로젝트. LVGL은 LVGL LLC의 상표." Montserrat 폰트는 `OFL.txt` 동봉.

## 2. 설계 원칙
1. **동작이 아니라 알고리즘을 포팅한다.** `lv_sqrt`, `lv_trigo_sin`, `lv_bezier3`, 마스크/그림자/그라디언트 연산, 텍스트 줄바꿈, 리프레시 순서를 반올림까지 1:1로 옮긴다. 픽셀 패리티의 전제.
2. **Rust 소유권만 재설계한다.** 데이터 모델·이름·열거값(`StyleProp` 번호, `EventCode`, `ColorFormat` 값, `LV_PART_*`, `LV_STATE_*`)은 C와 동일하게 유지해 LVGL 문서가 그대로 통하게 한다.
3. **unsafe는 두 모듈로 한정**: `draw_buf::alloc`(정렬 버퍼), `draw_sw::blend::words`(`&[u8]` ↔ `&[u16]/[u32]` 뷰). 둘 다 Miri 검증. 나머지는 `#![deny(unsafe_code)]`.
4. `Ui`는 단일 스레드 컨텍스트(`!Send`). LVGL과 동일하며 Cortex-M0(CAS 없음)도 지원 가능.

## 3. 워크스페이스 구조

```
ferrovgl/                         (Cargo workspace, resolver = "3")
  crates/
    ferrovgl-core/      no_std+alloc  misc, core(obj/class/pos/scroll/style/tree/event/refr), display, indev, group,
                                     observer, timer, anim, draw 모델(buf/layer/task/DrawUnit trait), layouts(flex/grid),
                                     font trait + fmt_txt 리더 + binfont, text/bidi, image dsc + .bin 디코더, snapshot
    ferrovgl-draw-sw/   no_std+alloc  소프트웨어 DrawUnit, src/draw/sw 미러
    ferrovgl-fonts/     no_std        Montserrat 8~48 등 내장 폰트 블롭(.lvfnt), 선택 feature `ttf`
    ferrovgl-widgets/   no_std+alloc  위젯 + 테마(default/simple/mono) + lv_example 포팅
    ferrovgl-sdl/       std           SDL2 디스플레이(RGB565/XRGB8888) + 마우스/키보드/휠 indev
    ferrovgl-wasm/      std           wasm-bindgen Canvas2D 디스플레이 + 포인터/키보드
    ferrovgl-embedded/  no_std        (v0.2) embedded-graphics DrawTarget flush, embedded-hal 입력
    ferrovgl-test/      std           헤드리스 800x480 하네스, 스크린샷 비교, 합성 indev
    ferrovgl/           umbrella      re-export + feature passthrough, `ferrovgl::prelude::*`
  xtask/                `cargo xtask fonts | refimgs sync | nostd-check | parity-report | gen-style`
  tools/font-conv/      업스트림 lv_font_*.c → .lvfnt 변환 라이브러리
  tests/upstream/       (git-ignored) lvgl v9.6.0 sparse checkout: tests/ref_imgs, tests/src/test_cases, src/font/*.c
  docs/                 mdBook: "LVGL → ferrovgl 매핑", 포팅 노트, 패리티 현황표
```
의존 그래프(비순환): core ← draw-sw, fonts, widgets ← sdl/wasm/embedded ← umbrella.
no_std 크레이트 허용 외부 의존: `bitflags`, `libm`(feature), `heapless`(optional). `hashbrown`/`slotmap`/`smallvec` 불사용(아레나 ~200줄 직접 작성).

### Feature flag (lv_conf.h 매핑)
| lv_conf.h | feature | 기본 | 크레이트 |
|---|---|---|---|
| `LV_USE_FLOAT` | `float` | on (ref_imgs 구성) | core |
| `LV_USE_LOG` | `log` | off | core |
| `LV_USE_ASSERT_*`, `LV_CHECK_ARG_*` | `assert-style`, `assert-obj`, `check-arg-panic` | debug on | core |
| `LV_USE_OBJ_ID/NAME` | `obj-id`, `obj-name` | off | core |
| `LV_USE_FLEX/GRID/OBSERVER/SNAPSHOT/BIDI/GRIDNAV/GESTURE_RECOGNITION` | 동일명 kebab | flex/grid/observer/snapshot on | core |
| `LV_DRAW_SW_COMPLEX`, `LV_USE_DRAW_SW_COMPLEX_GRADIENTS` | `complex`, `complex-gradients` | on | draw-sw |
| `LV_DRAW_SW_SUPPORT_<CF>` | `cf-rgb565`, `cf-rgb888`, `cf-xrgb8888`, `cf-argb8888`, `cf-l8`, `cf-al88`, `cf-a8`, `cf-i1` | all on | draw-sw |
| `LV_FONT_MONTSERRAT_N`, `LV_FONT_DEFAULT` | `montserrat-N`, `default-montserrat-N` | 14 | fonts |
| `LV_USE_TINY_TTF` | `ttf` | off | fonts |
| `LV_USE_<WIDGET>`, `LV_USE_THEME_*` | 위젯명, `theme-default/simple/mono` | v0.1 위젯 on | widgets |
| `LV_USE_OS` | `std` | off | core |
수치 옵션(`LV_DEF_REFR_PERIOD` 33, `LV_DPI_DEF` 130, indev 400/100ms, `LV_INV_BUF_SIZE` 32, `LV_DRAW_LAYER_SIMPLE_BUF_SIZE` 24KiB, `LV_GRADIENT_MAX_STOPS` 2 등)은 `UiConfig` 런타임 필드. `LV_COLOR_DEPTH`는 없음: `Color`는 24bit, 디스플레이 포맷은 런타임 값(v9와 동일).

## 4. 핵심 설계

### 4.1 객체 트리 소유권: 아레나 + 세대 핸들 + `&mut Ui` 컨텍스트
`Rc<RefCell>` 대신 선택. 이유: 이벤트 콜백 안에서 송신 객체를 변경/삭제하는 LVGL의 일반적 패턴이 `RefCell`에서는 이중 차용 패닉이 되고, 부모 소유 + 나머지는 약참조라는 LVGL 의미론은 아레나와 정확히 일치.
```rust
pub struct Key { index: u32, gen: NonZeroU32 }           // Option<Key> 8바이트
pub struct Obj(ObjId);  pub struct Label(Obj);            // Copy 핸들, 위젯 핸들은 newtype
pub(crate) struct ObjData { class: &'static ObjClass, parent: Option<ObjId>, children: Vec<ObjId>, coords: Area,
    flags: ObjFlags, state: State, styles: Vec<ObjStyle>, style_cache: [u32;2], bits: ObjBits,
    spec: Option<Box<SpecAttr>>, widget: Box<dyn WidgetState>, user_data: Option<Box<dyn Any>> }
pub struct Ui { objs: Arena<ObjData>, displays, indevs, groups, timers, anims, styles: Arena<Style>,
    event_cbs: Arena<EventCb>, layouts: Vec<LayoutCb>, draw: DrawCtx, tick, config: UiConfig,
    async_queue: Vec<AsyncJob>, app: Option<Box<dyn Any>> }
```
공개 API 두 층(구현은 한 번): 뷰 `ObjMut<'a> { ui: &mut Ui, id }` / `LabelMut` (Deref→ObjMut, 체이닝) + 핸들의 포워딩 메서드(매크로 생성).
```rust
let lbl = Label::create(ui, screen);                 // lv_label_create(screen)
ui.label(lbl).set_text("Hello").set_width(100);      // 체이닝
lbl.set_text(ui, "Hello");                           // lv_label_set_text(lbl, "Hello")와 1:1 grep 가능
```
- `ObjClass { base, name, new_state: fn()->Box<dyn WidgetState>, constructor, destructor, event_cb, width_def, height_def, editable, group_def }` — C의 `lv_obj_class_t`와 동일. 생성자는 base→derived, 소멸자는 derived→base.
- 위젯 상태 상속은 C 구조체 임베딩과 동일하게 `SliderState { bar: BarState, .. }` + `WidgetState::query_mut(TypeId)`(COM식 QueryInterface)로 다운캐스트. 사용자 정의 위젯도 같은 방식.
- **이벤트**: 콜백 `Rc<dyn Fn(&mut Ui, &mut Event)>`는 트리 바깥 아레나에 저장. 디스패치 시 id 목록을 스냅샷하고 `Rc`를 복제해 꺼낸 뒤 호출하므로 재진입, 디스패치 중 콜백 추가/삭제, **콜백 안 `delete()`** 모두 C와 동일하게 동작(매 콜백 후 `is_valid` 확인). 순서: preprocess 사용자 cb → 클래스 event_cb 체인 → 사용자 cb → bubble/trickle.
- `Event { code, target, current_target, param: EventParam<'p>(typed), stop_processing, stop_bubbling, prev }`. `EventParam`은 `void*` 대신 `Point/Key/Layer(LayerId)/DrawTask/HitTest(&mut)/CoverCheck(&mut)/SelfSize/...` 열거형.
- 앱 상태 변경은 `ui.set_app(MyApp)` + 콜백 안 `ui.app_mut::<MyApp>()`. `add_event_cb_mut(FnMut)`는 `RefCell` 래핑 후 재진입 시 스킵(문서화된 유일한 의미 차이).
- `delete()`: `EVENT_DELETE` → 자식 깊은 순 삭제 → 소멸자 → 부모/그룹/indev/anim/timer 참조 제거 → 슬롯 해제. `delete_async`는 `timer_handler()` 진입 시 처리.

### 4.2 스타일
- `StyleProp` `#[repr(u8)]`, 번호/플래그(`INHERITABLE|EXT_DRAW_UPDATE|LAYOUT_UPDATE|PARENT_LAYOUT_UPDATE|LAYER_UPDATE|TRANSFORM`)는 `include/lvgl/core/lv_style.h` 그대로(WIDTH=1 … 176, NUM_BUILT_IN=178, ANY=0xFF; 그룹 = prop>>3 → `has_group: u32`).
- `StyleValue` enum: `Num(i32)`(PCT/SIZE_CONTENT 인코딩 유지), `Color`, `Font(FontRef)`, `Image(ImageSrc)`, `Transition`, `Tracks`(grid dsc: `&'static [i32]`|`Rc<[i32]>`), `ColorFilter`, `Anim`. `Style { entries: Vec<StyleEntry>, has_group }`, `ConstStyle { &'static [StyleEntry] }`(`LV_STYLE_CONST_INIT`).
- `Selector(u32)` = state(low16) | part(`LV_PART_*` 값). `State` bitflags u16 = `LV_STATE_*`.
- 객체별 `ObjStyle { src: Local(Box)|Trans(Box)|Shared(StyleId)|Const(&'static), selector }`. 공유 스타일은 `Ui` 아레나 소유(`StyleId` = `lv_style_t*`), 변경 후 `report_style_change`.
- 조회는 `lv_obj_style.c: get_prop_core` 1:1(순서, 상태 가중치, 동률 첫 승), 상속 → 부모 `Part::Main`, 기본값 `match`. 2워드 존재 캐시 재계산도 동일.
- 타입별 접근자 178개는 `xtask gen-style`이 `style_props.toml`에서 `style_gen.rs`로 생성(enum, 플래그 테이블, Style/ObjMut setter·getter).
- 트랜지션: `lv_obj_style_state_compare` 포팅, 변경 prop마다 `Trans` 로컬 엔트리 + `AnimExec::StyleTrans` 애니메이션. `skip_trans`, `rendered` 비트 유지.
- 테마: `Theme { apply: fn(&mut Ui,&Theme,Obj), parent, fonts, colors, flags }`, 디스플레이에 등록. `theme_default`는 `lv_theme_default.c`의 정적 스타일 세트를 디스플레이당 1회 생성.

### 4.3 드로우 파이프라인
- 타입: `Color{b,g,r}`, `Color32`, `Color16(u16)`, `Opa=u8`, `ColorFormat` `#[repr(u8)]`(값 동일), `ImageHeader`, `DrawBuf { header, data: Owned(Box<[u64]>)|StaticMut|Static|Shared(Rc<[u8]>) }`. 공개 모델은 런타임 포맷(임의 포맷 이미지를 임의 레이어에 그려야 하므로).
- `DrawTask { kind, area, real_area, clip_area, target_layer: LayerId, opa, state, draw_unit, preference_score, dsc: DrawDsc }`, `DrawDsc` = `Fill|Border|BoxShadow|Label|Image|Layer|Line|Arc|Triangle|MaskRect|MaskBitmap|Blur`(Box). `LabelDsc.text`는 `Rc<str>`(프레임당 복사 없음).
- `Layer { draw_buf, color_format, buf_area, phy_clip_area, clip_area, opa, recolor, tasks: VecDeque<DrawTask>, parent, all_tasks_added, partial_y_offset }`; `DrawCtx`가 `layers: Arena<Layer>`, `units: Vec<Box<dyn DrawUnit>>`, circle/shadow 캐시 소유.
- `trait DrawUnit { evaluate(&mut DrawTask)->Evaluation; dispatch(&mut DrawCtx, LayerId)->Dispatch; wait_for_finish; delete }` — `lv_draw_dispatch*` 프로토콜 유지(현재 동기, `std` feature에서 스레드/GPU 유닛 확장).
- 객체 드로잉은 이벤트 기반 그대로: `refr_obj`가 `DRAW_MAIN_BEGIN/MAIN/MAIN_END/POST_*`를 `EventParam::Layer`로 전달, `init_draw_rect_dsc` 등 `lv_obj_draw.c` 포팅, `ui.draw_rect(layer, dsc, &area)`가 task 추가 + `DRAW_TASK_ADDED`.
- **sw 렌더러(`ferrovgl-draw-sw`)** 파일 구성은 `src/draw/sw` 미러: fill, border, box_shadow(+shadow cache, 정수 블러 비트 정확), grad(linear; `complex-gradients`로 radial/conical), img, letter(A1/2/4/8 마스크 블렌드, 밑줄/취소선/외곽선), line, mask(line/angle/radius/fade/map + circle cache), mask_rect, transform, triangle, arc, blur, `blend/`.
- 블렌딩: task당 1회 `(dest_cf, src_cf)` match → `fill::<D: DestFormat>()`, `image::<D, S: SrcFormat>()` 모노모피제이션. 내부 루프는 C 루프 직역(`LV_OPA_MIX2`, `lv_color_mix` 8bit 연산 동일) → 16/32bit 출력 바이트 일치. `cf-*` feature로 match arm 제거(MCU 바이너리 크기). `blend/words.rs`가 유일한 unsafe(워드 단위 채우기, v9.6 동작). `blend/simd/`는 이후 `core::arch` 커널.
- 디스플레이: `enum RenderMode { Partial, Direct, Full }`, `trait DisplayDriver { flush(FlushRequest)->Done|Pending; flush_wait; on_resolution_change }`, `ui.display_create(w,h)`, `DisplayMut::set_buffers/set_color_format/set_driver/set_dpi/set_rotation/set_theme`.
- 리프레시: `lv_refr.c` 함수 단위 포팅(`refr_join_areas → refr_invalid_areas → refr_area(Partial 버퍼 높이 청킹) → refr_area_part → refr_get_top_obj → refr_obj_and_children → refr_obj(opa/transform 서브 레이어, simple layer 청킹) → lv_draw_dispatch → wait → flush`), Direct 모드 `sync_areas`, `LV_INV_BUF_SIZE` 슬롯 무효화.

### 4.4 폰트
- `trait Font { line_height, base_line, subpx, underline_*, kerning, glyph_dsc(&mut GlyphDsc, letter, next)->bool, glyph_bitmap(&GlyphDsc, scratch)->Option<&DrawBuf>, release_glyph, fallback }`, `FontRef = Static(&'static dyn Font) | Shared(Rc<dyn Font>)`.
- **내장 폰트**: `xtask fonts`가 v9.6.0 태그의 `lv_font_montserrat_N.c`(sha256 고정)를 파싱해 `.lvfnt`(LE 테이블: glyph_dsc 8B/glyph, cmaps, kern pair/class, bitmap, bpp/format/line_height/base_line…)로 변환, 체크인(`--check`로 CI 드리프트 검출, `data/SOURCE.md`에 출처/lv_font_conv 플래그). `FmtTxtFont<'a>`는 `include_bytes!` 위 zero-copy 리더(transmute 없음). `lv_font_fmt_txt.c` 알고리즘(cmap 이진 탐색, kern, **RLE+prefilter 압축 해제**, A1~A8) 1:1 포팅. feature `montserrat-N`마다 `pub static MONTSERRAT_N`(RAM 0).
- `binfont` feature: 같은 리더를 `Vec<u8>` 백킹으로 재사용(lv_font_conv `--format bin`).
- 글리프 캐시: `misc/cache`에 ~150줄 LRU 직접 구현(`(font ptr, letter, size)` 키), 압축/TTF 글리프에만 사용(C와 동일).
- `ttf` feature: `ttf-parser` + `ab_glyph_rasterizer`(둘 다 no_std) → A8. 안티앨리어싱이 stb_truetype과 달라 패리티 보장 범위 밖임을 명시. FreeType/imgfont 범위 밖.
- `misc/text.rs`(`lv_text.c`: get_size, next_line, next_word, break chars, `LV_TXT_LINE_BREAK_LONG_LEN`), `misc/bidi.rs`(feature).

### 4.5 입력 / 그룹 / 타이머 / 애니메이션 / tick
- tick: `ui.tick_inc(ms)`, `tick_get`, `tick_elaps`, `set_tick_source(fn()->u32)`.
- 타이머: `ui.timer_create(period, Fn(&mut Ui, TimerId)) -> TimerId`, `TimerMut`(set_period/repeat_count/pause/resume/ready/reset/delete), `ui.timer_handler() -> ms_until_next`.
- 애니메이션: `Anim { var: AnimTarget(Obj|None|Tag), exec: AnimExec(ObjX|ObjY|ObjWidth|…|StyleProp|StyleTrans|BarValue|…|Custom{tag,f}), path: AnimPath(Linear|EaseIn|…|Custom), start/end, duration, delay, playback_*, repeat_*, early_apply, start/completed/deleted_cb }`. `AnimExec`가 `PartialEq`라 `lv_anim_delete(var, exec_cb)` 대응 가능. `lv_anim.c` + `anim_timeline` 포팅, bezier3 고정소수점 동일.
- indev: `IndevType {Pointer, Keypad, Encoder, Button}`, `trait IndevDriver { read(&mut IndevData) }`, `ui.indev_create(ty, Box<dyn IndevDriver>)`, `IndevMut`(set_group/cursor/button_points/display/mode(Timer|Event)/long_press_time…), `ui.indev_read(i)`. `lv_indev.c`/`indev_scroll.c`/`indev_gesture.c` 상태기계 1:1(press/release/long press 400/100ms, scroll throw/elastic/snap/chain, gesture limit 50/velocity 3, 모든 `LV_EVENT_*` 코드). 백엔드는 코어 제공 `IndevQueue` 링버퍼에 OS 이벤트를 넣음.
- 그룹: `GroupId` 아레나, `focus_next/prev/obj`, `send_data(key)`, `set_default`, editing/wrap/refocus_policy/frozen; `gridnav` feature.
- 통합 루프(ferrovgl-sdl `SdlApp::run`이 래핑):
```rust
let mut ui = Ui::new(UiConfig::default());
let disp = ferrovgl_sdl::create_display(&mut ui, 800, 480, ColorFormat::Xrgb8888)?;
build_ui(&mut ui);
loop { sdl.pump_events(&mut ui); ui.tick_inc(elapsed); let next = ui.timer_handler(); sdl.sleep(next.min(5)); }
```

### 4.6 레이아웃
taffy 불사용(CSS와 다름). `lv_flex.c`/`lv_grid.c`를 `core/layouts/{flex,grid}.rs`로 직역하고, 의존하는 `lv_obj_pos.c`(`layout_update_core`, `calc_content_width/height`, `refr_size`(SIZE_CONTENT/PCT/min/max/w_layout), `refr_pos`(translate/align/base_dir), `align_to`) 포팅. 좌표 인코딩 `i32` 비트 트릭 유지(`COORD_TYPE_SHIFT=29`, `pct()`, `SIZE_CONTENT`, `grid_fr()`, `GRID_CONTENT`, `GRID_TEMPLATE_LAST`). 레이아웃 레지스트리 `LAYOUT_FLEX=1`, `LAYOUT_GRID=2`, 사용자 등록 가능. grid dsc는 `TracksRef`로 수명 문제 제거. 수용 테스트: 업스트림 `test_flex.c`/`test_grid.c` + `flex_*.png`/`grid_*.png`.

### 4.7 위젯 단계
- **v0.1 (13개)**: label(long mode 전부, 선택, recolor), button, image(.bin/C array, 회전/줌/pivot, tiled, recolor), bar(symmetrical/range, anim), slider(Bar 상속), switch, checkbox, arc, line, led, spinner(arc 상속), buttonmatrix, textarea(커서/비밀번호/one-line/accepted chars/max length). 테마 default+simple, snapshot.
- **v0.2**: dropdown, roller, table, tabview, tileview, msgbox, keyboard, chart, scale, spinbox, calendar, canvas, span, imagebutton, animimage, arclabel, theme mono, observer 바인딩, gridnav, PNG/QOI 디코더(`ferrovgl-image`), `ferrovgl-embedded`.
- **v0.3**: qrcode, barcode, ime(pinyin), translation, sysmon, monkey, property API, GIF(feature).
- **제외**: list, menu, win, file_explorer(폐기), lottie/ffmpeg/gstreamer/3dtexture, fragment.
- 위젯 디렉토리는 업스트림 미러(`widgets/slider/{mod.rs,draw.rs}`): `static SLIDER_CLASS`, `SliderState`, `Slider` 핸들, `SliderMut` 뷰, `tests/parity_slider.rs`(업스트림 `test_slider.c` 시나리오 재현).

## 5. 테스트 전략
1. **단위 테스트**: 아레나(세대 wrap 포함), Area/Color 연산, `lv_sqrt/trigo_sin/bezier3/atan2`를 업스트림에서 1회 덤프한 `tests/fixtures/math.json`과 대조, 줄바꿈, 스타일 우선순위/상속/기본값, flex/grid 기하(수치), 이벤트 순서, delete-during-event, 재진입.
2. **픽셀 패리티(헤드라인 지표)**: `ferrovgl-test` 하네스가 업스트림 `lv_test_display_create`/`lv_test_init`를 미러(800x480, XRGB8888, DPI 160, montserrat 14, float on, light default theme, 합성 pointer/keypad/encoder, `mouse_click_at`, `key_hit`, `encoder_turn`, `wait(ms)`). `xtask refimgs sync`가 v9.6.0을 sparse checkout(커밋 해시 `xtask/pins.toml` 고정). 포팅한 테스트가 `screenshot_compare("widgets/label_1.png")` 호출 → `png` dev-dep으로 디코드 후 RGB 정확 비교, 실패 시 `target/parity/<name>_{actual,expected,diff}.png`. `tests/parity_allowlist.toml`에 알려진 차이와 이유. `xtask parity-report`가 matched/total 출력, README에 퍼센트 표시. **M0 과제**: `src/others/test/lv_test_display.c`, `lv_test_screenshot_compare.c`를 확인해 렌더 모드/버퍼/비교 허용오차를 `docs/parity.md`에 기록.
3. **스냅샷 테스트**(Rust 고유 시나리오, `tests/snapshots/`, `UPDATE_SNAPSHOTS=1`).
4. **퍼징**(`cargo-fuzz`, 주간 cron): `style_ops`, `text_break`, `binfont_loader`, `image_bin_decoder`, `indev_sequence`, `tree_mutation`(부모↔자식 일관성, dangling id 없음).
5. **Miri**: core/draw-sw의 아레나, `draw_buf::alloc`, `blend::words`, 블렌드 커널.
6. **no_std 빌드 체크**: `cargo xtask nostd-check` = `thumbv7em-none-eabihf`, `thumbv6m-none-eabi` 최소 feature 빌드 + `examples/nostd-size` `.text` 크기 측정(5% 초과 증가 시 CI 실패, `size-ok` 라벨로 해제). wasm32도 빌드.
7. **벤치마크**: `criterion`으로 블렌드 커널 + `lv_demo_benchmark`류 장면, CI 아티팩트(게이트 아님).

## 6. CI / 릴리스 / 거버넌스
- `ci.yml`: fmt; clippy(`-D warnings`, all-features 및 no-default-features); test(ubuntu/macos/windows, stable); msrv(stable−2, 연 2회 갱신); `cargo hack --each-feature`(core/draw-sw/widgets); nostd; wasm; docs(`--cfg docsrs`); `cargo-deny`(허용: MIT/Apache-2.0/BSD/Zlib/Unicode-3.0/OFL-1.1); `cargo-semver-checks`(0.1.0 이후); `cargo-llvm-cov`→Codecov; parity(diff 아티팩트 + 요약).
- `nightly.yml`(cron): Miri, fuzz 타깃당 10분, `cargo update` dry-run.
- `release.yml`: **release-plz**(conventional commits → 크레이트별 버전, git-cliff CHANGELOG, 릴리스 PR, 머지 시 의존 순서 publish). SDL은 `sdl2` crate `bundled,static-link`; 테스트는 헤드리스.
- 리포 파일: `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `TRADEMARK.md`, `crates/ferrovgl-fonts/OFL.txt`, `CONTRIBUTING.md`(DCO, conventional commits, "렌더러 코드는 개선하지 말고 포팅" 규칙, 위젯 PR에 패리티 테스트 필수), `CODE_OF_CONDUCT.md`, `SECURITY.md`, `deny.toml`, `rust-toolchain.toml`(stable), `.cargo/config.toml`(xtask alias), `[workspace.lints] unsafe_code="deny"`, `missing_docs="warn"`, `[package.metadata.docs.rs] all-features=true`.

## 7. 마일스톤 (1인 개발 기준 추정, 2026-10-24 시작)

| 마일스톤 | 기간 | 산출물 / 종료 기준 |
|---|---|---|
| **M0 스캐폴딩** | ~2026-11-07 | 전 크레이트 스텁 컴파일, CI 전 job 녹색(no_std, deny 포함); `misc/{arena,area,color,math}` 포팅 + 업스트림 덤프 대조 테스트; `xtask fonts`로 `montserrat_14.lvfnt` + `FmtTxtFont` 리더(글리프 비트맵 바이트 일치); `xtask refimgs sync`; NOTICE/TRADEMARK/라이선스; mdBook 골격; 업스트림 테스트 하네스 파라미터 확인 → `docs/parity.md` |
| **M1 코어 + sw 렌더러 첫 픽셀** | ~2026-12-12 | `Ui`/`ObjData`/클래스/트리/좌표(레이아웃 제외)/무효화/`lv_refr` 루프/`DrawBuf`/`Layer`/`DrawTask`/`SwDrawUnit`; fill(radius, linear grad), border, letter, draw_label, `blend/to_{xrgb8888,rgb565}`; SDL 창에 둥근 사각형 + Montserrat 14 라벨; 테스트 하네스; **업스트림 ref 이미지 5장 이상 바이트 일치** |
| **M2 스타일·이벤트·입력·레이아웃·애니** | ~2027-01-23 | 스타일 전체(캐시/상속/트랜지션/생성 접근자), 이벤트 완성, 타이머, anim+timeline, indev 4종 + scroll/gesture, 그룹, flex+grid, theme default, box_shadow/img(+transform)/line/arc/triangle/mask_rect, 나머지 `blend/to_*`, snapshot; 패리티: `flex_*`, `grid_*`, `style_*`, `scroll_*`; fuzz 3종 가동 |
| **M3 위젯 + wasm** | ~2027-03-12 | v0.1 위젯 13개 + 포팅 테스트 + 패리티, theme simple, `ferrovgl-wasm` 데모, `lv_example_*` ~20개 포팅, "LVGL 함수 → ferrovgl 메서드" 매핑 문서 자동 생성, Miri 녹색, thumbv7em 크기 리포트 |
| **M4 0.1.0 릴리스** | ~2027-04-09 | API 네이밍 감사(`#[must_use]`, `#[non_exhaustive]`), semver 베이스라인, 범위 내 ref 이미지 패리티 ≥90%(나머지 allowlist 사유), CHANGELOG, release-plz로 crates.io publish, 패리티 표 포함 공개 글 |

## 8. 리스크
1. 정수 연산 차이로 패리티 드리프트 → math/mask/shadow/grad 직역, 중간 테이블 1회 덤프, M1부터 패리티 테스트.
2. 업스트림 하네스 세부(렌더 모드/허용오차) 미확인 → M0 sparse checkout으로 확인(설계는 영향 없음).
3. `&mut Ui` 전달 방식에 대한 사용성 반발 → 타입 핸들 + 체이닝 뷰 + `app_mut::<T>()` + 예제 다수, 0.1 전 피드백 수집.
4. `Fn` 전용 콜백 → 문서화된 차이, `add_event_cb_mut` 탈출구.
5. 바이너리 크기/컴파일 시간(`StyleValue`, 포맷별 모노모피제이션) → `cf-*` feature, M1부터 크기 CI 게이트, `Box<dyn WidgetState>` 오버헤드 측정.
6. 폰트 라이선스/크기 → OFL 동봉, feature 게이트(Montserrat 48 단독 ~100KB).
7. ref 이미지가 `LV_USE_FLOAT=y`, DPI 160 전제 → 하네스 고정, `float` 기본 on.
8. Windows CI의 SDL2 → `bundled`, 테스트 헤드리스.
9. 범위 확장 → 마일스톤별 위젯 목록 동결, 폐기 위젯 거부.
10. v10 분기 → 본 프로젝트는 v9.6.0(마지막 v9) 추적, 폐기 항목 미포팅으로 v10 전환 용이.

## 9. 검증 방법 (구현 단계에서 end-to-end)
- 매 커밋: `cargo fmt --check && cargo clippy --all-targets --all-features -D warnings && cargo test --workspace`.
- no_std: `cargo xtask nostd-check` (thumbv7em/thumbv6m, 타깃 미설치 시 `rustup target add`).
- 시뮬레이터: `cargo run -p ferrovgl-sdl --example hello`로 창 표시 확인(WSL2 WSLg), `cargo run -p ferrovgl-wasm` + `trunk serve`로 브라우저 확인.
- 패리티: `cargo xtask refimgs sync && cargo test -p ferrovgl-test` → `cargo xtask parity-report`로 matched/total 확인, 실패 시 `target/parity/*_diff.png` 검토.
- 릴리스 전: `cargo deny check`, `cargo semver-checks`, `cargo publish --dry-run` 의존 순서(core → draw-sw → fonts → widgets → sdl/wasm → umbrella).

## 10. 다음 단계 (실행 시 M0부터)
1. `git init`, 워크스페이스/크레이트 스텁, 라이선스·NOTICE·TRADEMARK, CI 워크플로.
2. `xtask refimgs sync` 구현 → 업스트림 v9.6.0 sparse checkout → `lv_test_display.c`/`lv_test_screenshot_compare.c`/`lv_style.h`/`lv_font_montserrat_14.c` 확인.
3. `misc` 포팅(arena, area, color, math) + `tools/font-conv` + `FmtTxtFont`.
4. M1 진입.
