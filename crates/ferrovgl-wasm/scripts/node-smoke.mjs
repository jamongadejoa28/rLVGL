// Headless smoke test of the wasm-pack output in Node (no browser, no npm
// packages). A minimal fake DOM provides exactly the classes and members the
// generated bindings touch, so the real module is instantiated and driven:
// it renders the gallery through putImageData, runs the rAF loop and reacts
// to synthetic pointer and keyboard events.
//
//   wasm-pack build crates/ferrovgl-wasm --target web --release --out-dir www/pkg
//   node crates/ferrovgl-wasm/scripts/node-smoke.mjs [path/to/pkg]

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const pkg = path.resolve(process.argv[2] ?? path.join(here, "../www/pkg"));

// ---- fake DOM ------------------------------------------------------------
// Listeners keep their options: like the DOM's "inner invoke", a passive
// listener's preventDefault() does nothing.
class FakeEventTarget {
  constructor() { this.listeners = new Map(); }
  addEventListener(type, cb, opts) {
    if (!this.listeners.has(type)) this.listeners.set(type, new Map());
    this.listeners.get(type).set(cb, opts);
  }
  removeEventListener(type, cb) { this.listeners.get(type)?.delete(cb); }
  passive(type) { return [...(this.listeners.get(type)?.values() ?? [])].map((o) => o?.passive === true); }
  dispatch(type, event) {
    for (const [cb, opts] of this.listeners.get(type) ?? []) {
      event.inPassiveListener = opts?.passive === true;
      cb(event);
      event.inPassiveListener = false;
    }
  }
}
class Event {
  constructor(props) { Object.assign(this, props); this.defaultPrevented = false; }
  preventDefault() { if (!this.inPassiveListener) this.defaultPrevented = true; }
}
class MouseEvent extends Event {}
class PointerEvent extends MouseEvent {}
class WheelEvent extends MouseEvent {}
class KeyboardEvent extends Event {}
class ImageData {
  constructor(data, width, height) {
    assert.equal(data.length, width * height * 4);
    this.data = new Uint8ClampedArray(data);
    this.width = width;
    this.height = height;
  }
}
class CanvasRenderingContext2D {
  constructor(canvas) { this.canvas = canvas; this.puts = 0; }
  putImageData(img, x, y) {
    const { width: cw } = this.canvas;
    for (let row = 0; row < img.height; row++) {
      const src = img.data.subarray(row * img.width * 4, (row + 1) * img.width * 4);
      this.canvas.pixels.set(src, ((y + row) * cw + x) * 4);
    }
    this.puts++;
  }
  clearRect() { this.canvas.pixels.fill(0); }
}
class HTMLElement extends FakeEventTarget {}
class HTMLCanvasElement extends HTMLElement {
  constructor() {
    super();
    this.width = 300;
    this.height = 150;
    this.style = { props: {}, setProperty(k, v) { this.props[k] = v; } };
    this.ctx = new CanvasRenderingContext2D(this);
  }
  set width(v) { this._w = v; this.pixels = new Uint8ClampedArray(v * (this._h ?? 0) * 4); }
  get width() { return this._w; }
  set height(v) { this._h = v; this.pixels = new Uint8ClampedArray((this._w ?? 0) * v * 4); }
  get height() { return this._h; }
  getContext(kind) { return kind === "2d" ? this.ctx : null; }
  // Shown at 2x CSS size at (100, 50): exercises the coordinate scaling.
  getBoundingClientRect() {
    return { left: 100, top: 50, width: this.width * 2, height: this.height * 2 };
  }
  focus() { this.focused = true; }
  setPointerCapture() {}
  pixel(x, y) { const o = (y * this.width + x) * 4; return [...this.pixels.subarray(o, o + 4)]; }
}
class Window {
  static [Symbol.hasInstance](v) { return v === globalThis; }
}
const canvas = new HTMLCanvasElement();
const windowTarget = new FakeEventTarget();
const frameQueue = [];
Object.assign(globalThis, {
  Window, HTMLElement, HTMLCanvasElement, CanvasRenderingContext2D, ImageData,
  PointerEvent, WheelEvent, KeyboardEvent,
  devicePixelRatio: 2,
  document: { getElementById: (id) => (id === "ferrovgl" ? canvas : null) },
  requestAnimationFrame: (cb) => { frameQueue.push(cb); return frameQueue.length; },
  cancelAnimationFrame: () => {},
  addEventListener: windowTarget.addEventListener.bind(windowTarget),
  removeEventListener: windowTarget.removeEventListener.bind(windowTarget),
});

// ---- run -----------------------------------------------------------------
const mod = await import(pathToFileURL(path.join(pkg, "ferrovgl_wasm.js")).href);
const bytes = await readFile(path.join(pkg, "ferrovgl_wasm_bg.wasm"));
mod.initSync({ module: bytes });
console.log(`instantiated ${bytes.length} byte module`);

assert.equal(mod.scene_names()[0], "gallery");
console.log(`scenes: ${mod.scene_names().join(", ")} (${mod.example_count()} examples)`);
assert.throws(() => new mod.WebDisplay("missing", 10, 10), /no element/);
assert.throws(() => new mod.WebDisplay("ferrovgl", 10, 10, "i1"), /unknown color format/);

const display = new mod.WebDisplay("ferrovgl", 800, 480, "xrgb8888", "partial", 130);
assert.equal(canvas.width, 800);
assert.equal(canvas.height, 480);
// pixel_scale round(2) at dpr 2: CSS size equals the display size.
assert.equal(canvas.style.props.width, "800px");
display.set_pixel_scale(1);
assert.equal(canvas.style.props.width, "400px");

display.show("gallery");
for (let t = 0; t <= 400; t += 40) assert.equal(display.step(t), true);
const firstPuts = canvas.ctx.puts;
assert.ok(firstPuts >= 10, `partial mode puts ${firstPuts} areas`);
assert.ok(display.flushed_areas() >= 10);
// The default theme's screen background is light grey, everything opaque.
assert.deepEqual(canvas.pixel(2, 2).slice(3), [255]);
assert.ok(canvas.pixel(2, 2)[0] > 200);
// The blue "Click me" button of the second card.
const [r, g, b] = canvas.pixel(214, 62);
assert.ok(b > 200 && r < 100, `button pixel ${[r, g, b]}`);

// Real loop: start() installs listeners and requests frames.
display.start();
assert.ok(display.is_running());
let now = 1000;
function frames(n) {
  for (let i = 0; i < n; i++) {
    const cb = frameQueue.shift();
    assert.ok(cb, "a frame was requested");
    now += 16.7;
    cb(now);
  }
}
frames(5);
// Every listener that calls preventDefault() must be non-passive.
for (const type of ["pointerdown", "keydown", "wheel", "contextmenu"]) {
  assert.deepEqual(canvas.passive(type), [false], `${type} listener is not passive`);
}

// The "Clicked 0x" label of the button card (the spinner animates elsewhere).
const statusRegion = () => {
  const rows = [];
  for (let y = 88; y < 106; y++) rows.push(...canvas.pixels.subarray((y * 800 + 195) * 4, (y * 800 + 285) * 4));
  return rows;
};
const statusBefore = statusRegion();
// Click the button at display (240, 62) = CSS (100 + 480, 50 + 124).
const ptr = (props) => new PointerEvent({ clientX: 580, clientY: 174, isPrimary: true, pointerId: 1, ...props });
const down = ptr({ button: 0, buttons: 1 });
canvas.dispatch("pointerdown", down);
assert.ok(down.defaultPrevented && canvas.focused);
frames(3);
canvas.dispatch("pointerup", ptr({ button: 0, buttons: 0 }));
frames(6);
const statusClicked = statusRegion();
assert.notDeepEqual(statusClicked, statusBefore, "the click changed the status label");

// Chorded buttons: a press or release that is not the first or last one
// arrives as `pointermove`. Releasing left while middle is held still
// releases (and clicks) the pointer.
canvas.dispatch("pointerdown", ptr({ button: 0, buttons: 1 }));
frames(2);
canvas.dispatch("pointermove", ptr({ button: 1, buttons: 5 }));
frames(2);
canvas.dispatch("pointermove", ptr({ button: 0, buttons: 4 }));
frames(6);
assert.notDeepEqual(statusRegion(), statusClicked, "the chorded left release clicked");
canvas.dispatch("pointerup", ptr({ button: 1, buttons: 0 }));
frames(6);

// Keyboard: Tab is handled (default prevented) and redraws the focus.
const tab = new KeyboardEvent({ key: "Tab", code: "Tab", shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, isComposing: false });
canvas.dispatch("keydown", tab);
assert.ok(tab.defaultPrevented);
const f1 = new KeyboardEvent({ key: "F1", code: "F1", shiftKey: false, ctrlKey: false, altKey: false, metaKey: false, isComposing: false });
canvas.dispatch("keydown", f1);
assert.ok(!f1.defaultPrevented);
const wheel = new WheelEvent({ deltaY: 120, deltaMode: 0 });
canvas.dispatch("wheel", wheel);
assert.ok(wheel.defaultPrevented);
frames(4);
assert.equal(display.last_error(), undefined);

display.stop();
assert.ok(!display.is_running());
assert.equal(canvas.listeners.get("pointerdown").size, 0);
display.free();
console.log(`ok: ${canvas.ctx.puts} putImageData calls`);
