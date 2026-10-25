// Loader for the ferrovgl browser demo. Build the module first:
//   wasm-pack build crates/ferrovgl-wasm --target web --release --out-dir www/pkg
// URL parameters: scene, w, h, cf (xrgb8888|argb8888|rgb888|rgb565),
// mode (partial|direct|full), dpi, scale (device pixels per display pixel).
import init, { WebDisplay, scene_names } from "./pkg/ferrovgl_wasm.js";

const params = new URLSearchParams(location.search);
const status = document.getElementById("status");
const picker = document.getElementById("scene");

function num(name) {
  const v = params.get(name);
  return v === null ? undefined : Number(v);
}

function fail(err) {
  status.textContent = String(err);
  status.className = "error";
}

try {
  await init();
  const display = new WebDisplay(
    "ferrovgl",
    num("w") ?? 800,
    num("h") ?? 480,
    params.get("cf") ?? undefined,
    params.get("mode") ?? undefined,
    num("dpi"),
  );
  if (num("scale") !== undefined) display.set_pixel_scale(num("scale"));

  const names = scene_names();
  const initial = params.get("scene") ?? "gallery";
  // The picker only appears once ported lv_example_* functions exist.
  if (names.length > 1) {
    for (const name of names) picker.add(new Option(name, name, false, name === initial));
    picker.hidden = false;
    picker.addEventListener("change", () => {
      try {
        display.show(picker.value);
        params.set("scene", picker.value);
        history.replaceState(null, "", `?${params}`);
        document.getElementById("ferrovgl").focus();
      } catch (err) {
        fail(err);
      }
    });
  }

  display.show(initial);
  display.start();
  status.textContent = `${display.width()}x${display.height()}, scene "${display.scene()}"`;
  setInterval(() => {
    const err = display.last_error();
    if (err) fail(err);
  }, 1000);
  globalThis.ferrovgl = display; // handy for the devtools console
} catch (err) {
  fail(err);
  console.error(err);
}
