//! The wasm-bindgen API: a `<canvas>` [`Surface`] using
//! `ImageData`/`putImageData`, DOM event listeners feeding
//! [`InputEvent`]s, and a `requestAnimationFrame` loop.
//!
//! Everything here only forwards to the host-tested modules
//! ([`crate::backend`], [`crate::convert`], [`crate::geometry`],
//! [`crate::keys`], [`crate::clock`]). The functions compile on every
//! target, but calling them outside a browser (wasm32 with a DOM) fails.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use ferrovgl_core::Point;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{
    AddEventListenerOptions, CanvasRenderingContext2d, Event, EventTarget, HtmlCanvasElement, ImageData,
    KeyboardEvent, PointerEvent, WheelEvent,
};

use crate::backend::{Backend, BackendConfig, Surface};
use crate::convert::{parse_color_format, parse_render_mode};
use crate::gallery;
use crate::geometry::{self, CssRect};
use crate::input::InputEvent;
use crate::keys::{self, Modifiers};

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            value
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

/// Draws converted areas into a 2D canvas context.
struct CanvasSurface(CanvasRenderingContext2d);

impl Surface for CanvasSurface {
    fn put_rgba(&mut self, x: i32, y: i32, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
        let image = ImageData::new_with_u8_clamped_array_and_sh(Clamped(rgba), w, h).map_err(js_error)?;
        self.0
            .put_image_data(&image, f64::from(x), f64::from(y))
            .map_err(js_error)
    }
}

struct State {
    backend: Option<Backend>,
    config: BackendConfig,
    scene: String,
    error: Option<String>,
}

type Queue = Rc<RefCell<VecDeque<InputEvent>>>;
type FrameCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

struct Listener {
    target: EventTarget,
    kind: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

/// A ferrovgl display rendered into a `<canvas>` element.
///
/// ```js
/// import init, { WebDisplay } from "./pkg/ferrovgl_wasm.js";
/// await init();
/// const display = new WebDisplay("ferrovgl", 800, 480, "xrgb8888", "partial", 130);
/// display.show("gallery");
/// display.start();
/// ```
#[wasm_bindgen]
pub struct WebDisplay {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    state: Rc<RefCell<State>>,
    queue: Queue,
    pixel_scale: Rc<Cell<f64>>,
    running: Rc<Cell<bool>>,
    frame_id: Rc<Cell<Option<i32>>>,
    frame_cb: FrameCallback,
    listeners: Vec<Listener>,
}

#[wasm_bindgen]
impl WebDisplay {
    /// Binds to the canvas with id `canvas_id` and creates a `width` x
    /// `height` display.
    ///
    /// `color_format` is `"xrgb8888"` (default), `"argb8888"`, `"rgb888"` or
    /// `"rgb565"`; `render_mode` is `"partial"` (default), `"direct"` or
    /// `"full"`; `dpi` sets `lv_display_set_dpi()` (default `LV_DPI_DEF`).
    /// The canvas backing store is resized to the display and its CSS size
    /// follows `devicePixelRatio` (see [`Self::set_pixel_scale`]).
    #[wasm_bindgen(constructor)]
    pub fn new(
        canvas_id: &str,
        width: u32,
        height: u32,
        color_format: Option<String>,
        render_mode: Option<String>,
        dpi: Option<i32>,
    ) -> Result<WebDisplay, JsError> {
        let mut config = BackendConfig {
            width,
            height,
            dpi,
            ..BackendConfig::default()
        };
        if let Some(name) = color_format {
            config.color_format = parse_color_format(&name)
                .ok_or_else(|| JsError::new(&format!("unknown color format {name:?}")))?;
        }
        if let Some(name) = render_mode {
            config.render_mode = parse_render_mode(&name)
                .ok_or_else(|| JsError::new(&format!("unknown render mode {name:?}")))?;
        }
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| JsError::new("no document: WebDisplay needs a browser page"))?;
        let canvas: HtmlCanvasElement = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsError::new(&format!("no element with id {canvas_id:?}")))?
            .dyn_into()
            .map_err(|_| JsError::new(&format!("#{canvas_id} is not a <canvas>")))?;
        let context: CanvasRenderingContext2d = canvas
            .get_context("2d")
            .map_err(|e| JsError::new(&js_error(e)))?
            .ok_or_else(|| JsError::new("the canvas has no 2d context"))?
            .dyn_into()
            .map_err(|_| JsError::new("unexpected 2d context type"))?;
        canvas.set_width(width);
        canvas.set_height(height);
        canvas.set_tab_index(0);
        let style = canvas.style();
        for (name, value) in [
            ("image-rendering", "pixelated"),
            ("touch-action", "none"),
            ("outline", "none"),
        ] {
            style
                .set_property(name, value)
                .map_err(|e| JsError::new(&js_error(e)))?;
        }
        let backend = Backend::new(config.clone(), Box::new(CanvasSurface(context.clone())))
            .map_err(|e| JsError::new(&e))?;
        let display = WebDisplay {
            canvas,
            context,
            state: Rc::new(RefCell::new(State {
                backend: Some(backend),
                config,
                scene: String::new(),
                error: None,
            })),
            queue: Rc::default(),
            pixel_scale: Rc::new(Cell::new(0.0)),
            running: Rc::new(Cell::new(false)),
            frame_id: Rc::new(Cell::new(None)),
            frame_cb: Rc::default(),
            listeners: Vec::new(),
        };
        apply_css_size(&display.canvas, width, height, 0.0);
        Ok(display)
    }

    /// Display width in pixels.
    pub fn width(&self) -> u32 {
        self.state.borrow().config.width
    }

    /// Display height in pixels.
    pub fn height(&self) -> u32 {
        self.state.borrow().config.height
    }

    /// Device pixels per display pixel; 0 (the default) uses
    /// `round(devicePixelRatio)`. The CSS size becomes
    /// `width * scale / devicePixelRatio`.
    pub fn set_pixel_scale(&self, scale: f64) {
        self.pixel_scale.set(scale);
        let (w, h) = (self.width(), self.height());
        apply_css_size(&self.canvas, w, h, scale);
    }

    /// `lv_display_set_dpi()` for the current and later scenes.
    pub fn set_dpi(&self, dpi: i32) {
        let mut state = self.state.borrow_mut();
        state.config.dpi = Some(dpi);
        if let Some(backend) = state.backend.as_mut() {
            backend.set_dpi(dpi);
        }
    }

    /// Replaces the UI with a fresh context showing `scene`: `"gallery"` or
    /// the name of a ported example (`ferrovgl_examples::find`).
    pub fn show(&self, scene: &str) -> Result<(), JsError> {
        let run =
            gallery::find_scene(scene).ok_or_else(|| JsError::new(&format!("unknown scene {scene:?}")))?;
        let mut state = self.state.borrow_mut();
        if let Some(mut old) = state.backend.take() {
            old.ui.deinit();
        }
        self.queue.borrow_mut().clear();
        self.context.clear_rect(
            0.0,
            0.0,
            f64::from(state.config.width),
            f64::from(state.config.height),
        );
        let mut backend = Backend::new(
            state.config.clone(),
            Box::new(CanvasSurface(self.context.clone())),
        )
        .map_err(|e| JsError::new(&e))?;
        run(&mut backend.ui);
        state.backend = Some(backend);
        state.scene = scene.to_owned();
        state.error = None;
        Ok(())
    }

    /// The scene last passed to [`Self::show`].
    pub fn scene(&self) -> String {
        self.state.borrow().scene.clone()
    }

    /// Runs one frame at `now_ms` without `requestAnimationFrame` (for
    /// tests or custom loops). Returns `false` once the display is gone.
    pub fn step(&self, now_ms: f64) -> Result<bool, JsError> {
        step(&self.state, &self.queue, now_ms).map_err(|e| JsError::new(&e))
    }

    /// Number of areas flushed to the canvas so far in this scene.
    pub fn flushed_areas(&self) -> f64 {
        self.state
            .borrow()
            .backend
            .as_ref()
            .map_or(0.0, |b| b.flushed_areas() as f64)
    }

    /// The error that stopped the loop, if any.
    pub fn last_error(&self) -> Option<String> {
        self.state.borrow().error.clone()
    }

    /// Whether the animation-frame loop is running.
    pub fn is_running(&self) -> bool {
        self.running.get()
    }

    /// Installs the input listeners and starts the `requestAnimationFrame`
    /// loop (`lv_tick_inc()` from the frame timestamps, then
    /// `lv_timer_handler()`).
    pub fn start(&mut self) -> Result<(), JsError> {
        if self.running.get() {
            return Ok(());
        }
        if self.listeners.is_empty() {
            self.install_listeners().map_err(|e| JsError::new(&js_error(e)))?;
        }
        self.running.set(true);
        let state = self.state.clone();
        let queue = self.queue.clone();
        let running = self.running.clone();
        let frame_id = self.frame_id.clone();
        let holder = self.frame_cb.clone();
        *self.frame_cb.borrow_mut() = Some(Closure::new(move |now: f64| {
            frame_id.set(None);
            if !running.get() {
                return;
            }
            match step(&state, &queue, now) {
                Ok(true) => {}
                Ok(false) => running.set(false),
                Err(e) => {
                    web_sys::console::error_1(&JsValue::from_str(&format!("ferrovgl: {e}")));
                    state.borrow_mut().error = Some(e);
                    running.set(false);
                }
            }
            if running.get() {
                frame_id.set(request_frame(&holder));
            }
        }));
        self.frame_id.set(request_frame(&self.frame_cb));
        if self.frame_id.get().is_none() {
            self.running.set(false);
            return Err(JsError::new("requestAnimationFrame is not available"));
        }
        Ok(())
    }

    /// Stops the loop and removes the listeners.
    pub fn stop(&mut self) {
        self.running.set(false);
        if let (Some(id), Some(window)) = (self.frame_id.take(), web_sys::window()) {
            let _ = window.cancel_animation_frame(id);
        }
        for l in self.listeners.drain(..) {
            let _ = l
                .target
                .remove_event_listener_with_callback(l.kind, l.callback.as_ref().unchecked_ref());
        }
    }

    fn install_listeners(&mut self) -> Result<(), JsValue> {
        let canvas: EventTarget = self.canvas.clone().into();
        let (w, h) = (self.width() as i32, self.height() as i32);

        let point = {
            let canvas = self.canvas.clone();
            move |e: &PointerEvent| {
                let r = canvas.get_bounding_client_rect();
                let rect = CssRect {
                    left: r.left(),
                    top: r.top(),
                    width: r.width(),
                    height: r.height(),
                };
                let (x, y) =
                    geometry::client_to_display(f64::from(e.client_x()), f64::from(e.client_y()), rect, w, h);
                Point::new(x, y)
            }
        };

        let q = self.queue.clone();
        let focus_canvas = self.canvas.clone();
        let p = point.clone();
        self.listen(&canvas, "pointerdown", false, move |e| {
            let Some(e) = e.dyn_ref::<PointerEvent>() else {
                return;
            };
            if !e.is_primary() {
                return;
            }
            match e.button() {
                0 => {
                    let _ = focus_canvas.set_pointer_capture(e.pointer_id());
                    let _ = focus_canvas.focus();
                    q.borrow_mut().push_back(InputEvent::PointerDown(p(e)));
                }
                1 => q.borrow_mut().push_back(InputEvent::EncoderButton(true)),
                _ => return,
            }
            e.prevent_default();
        })?;

        let q = self.queue.clone();
        let p = point;
        self.listen(&canvas, "pointermove", false, move |e| {
            if let Some(e) = e.dyn_ref::<PointerEvent>()
                && e.is_primary()
            {
                q.borrow_mut().push_back(InputEvent::PointerMove(p(e)));
            }
        })?;

        let q = self.queue.clone();
        self.listen(&canvas, "pointerup", false, move |e| {
            let Some(e) = e.dyn_ref::<PointerEvent>() else {
                return;
            };
            if !e.is_primary() {
                return;
            }
            match e.button() {
                0 => q.borrow_mut().push_back(InputEvent::PointerUp),
                1 => q.borrow_mut().push_back(InputEvent::EncoderButton(false)),
                _ => {}
            }
        })?;

        for kind in ["pointerleave", "pointercancel"] {
            let q = self.queue.clone();
            self.listen(&canvas, kind, false, move |e| {
                if e.dyn_ref::<PointerEvent>().is_none_or(|e| e.is_primary()) {
                    q.borrow_mut().push_back(InputEvent::PointerLeave);
                }
            })?;
        }

        let q = self.queue.clone();
        self.listen(&canvas, "keydown", false, move |e| {
            let Some(e) = e.dyn_ref::<KeyboardEvent>() else {
                return;
            };
            if e.is_composing() {
                return;
            }
            let mods = Modifiers {
                shift: e.shift_key(),
                ctrl: e.ctrl_key(),
                alt: e.alt_key(),
                meta: e.meta_key(),
            };
            let mapping = keys::map_key(&e.key(), &e.code(), mods);
            if mapping.is_handled() {
                q.borrow_mut().extend(mapping.keys().map(InputEvent::Key));
                e.prevent_default();
            }
        })?;

        let q = self.queue.clone();
        self.listen(&canvas, "wheel", true, move |e| {
            let Some(e) = e.dyn_ref::<WheelEvent>() else {
                return;
            };
            q.borrow_mut()
                .push_back(InputEvent::Wheel(geometry::wheel_steps(e.delta_y())));
            e.prevent_default();
        })?;

        let q = self.queue.clone();
        self.listen(&canvas, "blur", false, move |_| {
            q.borrow_mut().push_back(InputEvent::Blur);
        })?;

        self.listen(&canvas, "contextmenu", true, move |e| e.prevent_default())?;

        // devicePixelRatio changes (browser zoom, moving to another screen)
        // fire `resize`; keep whole device pixels per display pixel.
        if let Some(window) = web_sys::window() {
            let target: EventTarget = window.into();
            let canvas = self.canvas.clone();
            let scale = self.pixel_scale.clone();
            let (uw, uh) = (w as u32, h as u32);
            self.listen(&target, "resize", false, move |_| {
                apply_css_size(&canvas, uw, uh, scale.get());
            })?;
        }
        Ok(())
    }

    fn listen(
        &mut self,
        target: &EventTarget,
        kind: &'static str,
        cancelable: bool,
        f: impl FnMut(Event) + 'static,
    ) -> Result<(), JsValue> {
        let callback = Closure::<dyn FnMut(Event)>::new(f);
        let options = AddEventListenerOptions::new();
        // `wheel` and `contextmenu` call preventDefault(), which a passive
        // listener may not do.
        options.set_passive(!cancelable);
        target.add_event_listener_with_callback_and_add_event_listener_options(
            kind,
            callback.as_ref().unchecked_ref(),
            &options,
        )?;
        self.listeners.push(Listener {
            target: target.clone(),
            kind,
            callback,
        });
        Ok(())
    }
}

impl Drop for WebDisplay {
    fn drop(&mut self) {
        self.stop();
        // Break the closure's self-reference.
        self.frame_cb.borrow_mut().take();
        if let Some(mut backend) = self.state.borrow_mut().backend.take() {
            backend.ui.deinit();
        }
    }
}

fn request_frame(cb: &FrameCallback) -> Option<i32> {
    let window = web_sys::window()?;
    let cb = cb.borrow();
    window
        .request_animation_frame(cb.as_ref()?.as_ref().unchecked_ref())
        .ok()
}

fn step(state: &Rc<RefCell<State>>, queue: &Queue, now_ms: f64) -> Result<bool, String> {
    let events: Vec<InputEvent> = queue.borrow_mut().drain(..).collect();
    let mut state = state.borrow_mut();
    match state.backend.as_mut() {
        Some(backend) => backend.frame(now_ms, events),
        None => Ok(false),
    }
}

fn apply_css_size(canvas: &HtmlCanvasElement, width: u32, height: u32, pixel_scale: f64) {
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
    let style = canvas.style();
    let _ = style.set_property(
        "width",
        &format!("{}px", geometry::css_length(width, pixel_scale, dpr)),
    );
    let _ = style.set_property(
        "height",
        &format!("{}px", geometry::css_length(height, pixel_scale, dpr)),
    );
}

/// Names of every scene [`WebDisplay::show`] accepts: `"gallery"` first,
/// then each ported `lv_example_*` in upstream order.
#[wasm_bindgen]
pub fn scene_names() -> js_sys::Array {
    gallery::scene_names()
        .into_iter()
        .map(JsValue::from_str)
        .collect()
}

/// Number of ported `lv_example_*` functions (`ferrovgl_examples::EXAMPLES`).
#[wasm_bindgen]
pub fn example_count() -> u32 {
    ferrovgl_examples::EXAMPLES.len() as u32
}

/// Convenience entry for the demo page: an 800x480 display on
/// `canvas_id` showing `scene` (the gallery when omitted), already running.
#[wasm_bindgen]
pub fn start_demo(canvas_id: &str, scene: Option<String>) -> Result<WebDisplay, JsError> {
    let mut display = WebDisplay::new(canvas_id, 800, 480, None, None, None)?;
    display.show(scene.as_deref().unwrap_or(gallery::GALLERY))?;
    display.start()?;
    Ok(display)
}
