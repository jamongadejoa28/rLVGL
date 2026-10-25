//! no_std size probe: the smallest complete ferrovgl application.
//!
//! Builds for `thumbv7em-none-eabihf` and `thumbv6m-none-eabi` only. The
//! application ([`app`], entered from [`Reset`]) creates a [`Ui`], a 320x240 RGB565 display with a
//! 1/10-screen partial buffer, a label and a button (with a label child),
//! renders one frame and then runs the timer loop forever. Everything
//! reachable from [`Reset`] is what an application pays for: the linker
//! script (`link.x`) keeps only the vector table and lets `--gc-sections`
//! drop the rest. `cargo xtask size-report` measures the result.
//!
//! The global allocator ([`FreeListAlloc`], in the library part of this
//! package so it can be tested on the host) is a first-fit free list over
//! a static pool: it reuses and coalesces freed blocks, so the timer loop
//! runs on a fixed heap like upstream's TLSF pool. `.bss` / `.data` are
//! initialised in assembly before any Rust code runs. The probe is never
//! executed on hardware by CI; it is only linked and measured.

#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks, missing_docs)]

extern crate alloc;

use alloc::boxed::Box;
use core::arch::global_asm;
use core::cell::UnsafeCell;
use core::panic::PanicInfo;

use ferrovgl_core::display::FlushResult;
use ferrovgl_core::draw::draw_buf::BufStorage;
use ferrovgl_core::{Align, Area, ColorFormat, DisplayDriver, DisplayId, FontRef, RenderMode, Ui, UiConfig};
use ferrovgl_nostd_size::heap::FreeListAlloc;
use ferrovgl_widgets::{Button, Label};

/// Display width in pixels.
const HOR_RES: i32 = 320;
/// Display height in pixels.
const VER_RES: i32 = 240;
/// Lines of the partial draw buffer (1/10 of the screen, as upstream suggests).
const BUF_LINES: usize = 24;
/// Bytes of the partial draw buffer (RGB565).
const BUF_BYTES: usize = HOR_RES as usize * BUF_LINES * 2;
/// Bytes of the heap pool behind the global allocator.
const HEAP_BYTES: usize = 64 * 1024;

/*********************
 * Startup
 *********************/

/// The reset vector, placed right after the initial stack pointer by `link.x`.
#[unsafe(link_section = ".vector_table.reset_vector")]
#[unsafe(no_mangle)]
pub static RESET_VECTOR: unsafe extern "C" fn() -> ! = Reset;

unsafe extern "C" {
    /// Reset handler (assembly, below): zero `.bss`, copy `.data`, enable
    /// the FPU on hard-float targets and call [`probe_main`].
    fn Reset() -> !;
}

/// Emits `Reset`. Static memory is initialised in assembly, as
/// cortex-m-rt >= 0.7.3 does: walking the linker-defined bounds with Rust
/// pointers derived from one-word extern statics would step outside their
/// provenance, which is undefined behaviour. `$pre` are extra instructions
/// run first (the FPU enable).
macro_rules! reset_handler {
    ($($pre:literal,)*) => {
        global_asm!(
            ".section .text.Reset, \"ax\", %progbits",
            ".global Reset",
            ".type Reset, %function",
            ".thumb_func",
            "Reset:",
            $($pre,)*
            // Zero `.bss` (word-aligned bounds from `link.x`).
            "ldr r0, =__sbss",
            "ldr r1, =__ebss",
            "movs r2, #0",
            "0:",
            "cmp r1, r0",
            "beq 1f",
            "stm r0!, {{r2}}",
            "b 0b",
            "1:",
            // Copy `.data` from its load address in flash.
            "ldr r0, =__sdata",
            "ldr r1, =__edata",
            "ldr r2, =__sidata",
            "2:",
            "cmp r1, r0",
            "beq 3f",
            "ldm r2!, {{r3}}",
            "stm r0!, {{r3}}",
            "b 2b",
            "3:",
            "bl {main}",
            "udf #0",
            ".ltorg",
            ".size Reset, . - Reset",
            main = sym probe_main,
        );
    };
}

// Hard-float targets: grant full access to CP10/CP11 (CPACR) before any
// floating-point instruction can run.
#[cfg(target_abi = "eabihf")]
reset_handler!(
    "ldr r0, =0xE000ED88",
    "ldr r1, [r0]",
    "ldr r2, =0x00F00000",
    "orrs r1, r2",
    "str r1, [r0]",
    "dsb",
    "isb",
);
#[cfg(not(target_abi = "eabihf"))]
reset_handler!();

/// Rust entry, called by `Reset` once static memory is initialised.
extern "C" fn probe_main() -> ! {
    app()
}

/// Minimal panic handler: park the core.
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/*********************
 * Allocator
 *********************/

/// The heap (see [`FreeListAlloc`]).
#[global_allocator]
// SAFETY: the probe is single-threaded and installs no interrupt handler,
// so the allocator is only ever used from one execution context.
static HEAP: FreeListAlloc<HEAP_BYTES> = unsafe { FreeListAlloc::new() };

/*********************
 * Display
 *********************/

/// A static, 8-byte aligned byte pool.
#[repr(C, align(8))]
struct Pool<const N: usize>(UnsafeCell<[u8; N]>);

/// The partial draw buffer (`static` like an application's frame buffer).
static DISP_BUF: Pool<BUF_BYTES> = Pool(UnsafeCell::new([0; BUF_BYTES]));

// SAFETY: see `disp_buf`: the pool is only reached through the single
// `&'static mut` taken at startup.
unsafe impl<const N: usize> Sync for Pool<N> {}

/// The unique mutable view of [`DISP_BUF`].
fn disp_buf() -> &'static mut [u8] {
    // SAFETY: called exactly once (from `app`), so this is the only
    // reference ever created to the buffer; it lives for the whole program.
    unsafe { &mut *DISP_BUF.0.get() }
}

/// Stand-in for an LCD driver: a real one would start a DMA transfer.
struct Lcd;

impl DisplayDriver for Lcd {
    fn flush(&mut self, _disp: DisplayId, area: &Area, px_map: &mut [u8]) -> FlushResult {
        // Keep the rendered pixels observable so nothing is optimised away.
        core::hint::black_box((area, px_map));
        FlushResult::Done
    }
}

/*********************
 * Application
 *********************/

/// The application: one screen with a label and a button, rendered once,
/// then the usual `lv_timer_handler` loop.
fn app() -> ! {
    let mut ui = Ui::new(UiConfig {
        font_default: Some(FontRef::Static(&ferrovgl_fonts::MONTSERRAT_14)),
        ..UiConfig::default()
    });
    ferrovgl_draw_sw::init(&mut ui);

    let disp = ui.display_create(HOR_RES, VER_RES);
    ui.display_set_color_format(disp, ColorFormat::Rgb565);
    ui.display_set_buffers(
        disp,
        BufStorage::StaticMut(disp_buf()),
        None,
        BUF_BYTES as u32,
        RenderMode::Partial,
    );
    ui.display_set_driver(disp, Some(Box::new(Lcd)));

    let screen = ui.display_get_screen_active(Some(disp));
    let title = Label::create(&mut ui, screen);
    title.set_text(&mut ui, "ferrovgl size probe");
    ui.obj_align(title.obj(), Align::TopMid, 0, 10);

    let button = Button::create(&mut ui, screen);
    ui.obj_center(button.obj());
    let caption = Label::create(&mut ui, button.obj());
    caption.set_text(&mut ui, "Button");
    ui.obj_center(caption.obj());

    ui.refr_now(Some(disp));

    loop {
        ui.tick_inc(5);
        core::hint::black_box(ui.timer_handler());
    }
}
