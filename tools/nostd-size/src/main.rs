//! no_std size probe: the smallest complete ferrovgl application.
//!
//! Builds for `thumbv7em-none-eabihf` and `thumbv6m-none-eabi` only. The
//! reset handler creates a [`Ui`], a 320x240 RGB565 display with a
//! 1/10-screen partial buffer, a label and a button (with a label child),
//! renders one frame and then runs the timer loop forever. Everything
//! reachable from [`Reset`] is what an application pays for: the linker
//! script (`link.x`) keeps only the vector table and lets `--gc-sections`
//! drop the rest. `cargo xtask size-report` measures the result.
//!
//! The global allocator is a bump allocator over a static pool that can
//! roll back / grow in place its most recent block (enough for the
//! `Vec` growth patterns of the library). The probe is never executed on
//! hardware by CI; it is only linked and measured.

#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks, missing_docs)]

extern crate alloc;

use alloc::boxed::Box;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::panic::PanicInfo;
use core::ptr;

use ferrovgl_core::display::FlushResult;
use ferrovgl_core::draw::draw_buf::BufStorage;
use ferrovgl_core::{Align, Area, ColorFormat, DisplayDriver, DisplayId, FontRef, RenderMode, Ui, UiConfig};
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
    /// Start of `.bss` (from `link.x`).
    static mut __sbss: u32;
    /// End of `.bss` (from `link.x`).
    static mut __ebss: u32;
    /// Start of `.data` in RAM (from `link.x`).
    static mut __sdata: u32;
    /// End of `.data` in RAM (from `link.x`).
    static mut __edata: u32;
    /// Load address of `.data` in flash (from `link.x`).
    static __sidata: u32;
}

/// Reset handler: initialise `.bss` / `.data` and run the application.
///
/// # Safety
///
/// Only the hardware may call this, once, out of reset.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Reset() -> ! {
    // SAFETY: the linker script defines these symbols as word-aligned
    // bounds of `.bss` and `.data`. Nothing has touched static memory yet
    // (this runs first out of reset, single-threaded), so writing the
    // regions through raw pointers cannot alias a live reference.
    unsafe {
        let mut dst = &raw mut __sbss;
        let end = &raw mut __ebss;
        while dst < end {
            ptr::write_volatile(dst, 0);
            dst = dst.add(1);
        }
        let mut dst = &raw mut __sdata;
        let end = &raw mut __edata;
        let mut src = &raw const __sidata;
        while dst < end {
            ptr::write_volatile(dst, ptr::read(src));
            dst = dst.add(1);
            src = src.add(1);
        }
    }
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

/// A static, 8-byte aligned byte pool.
#[repr(C, align(8))]
struct Pool<const N: usize>(UnsafeCell<[u8; N]>);

/// Bump allocator over [`Pool`]; `dealloc` / `realloc` of the most recent
/// block shrink / grow it in place, anything else is never reused.
struct BumpAlloc {
    pool: Pool<HEAP_BYTES>,
    /// Offset of the first free byte in `pool`.
    next: UnsafeCell<usize>,
}

// SAFETY: the probe is single-threaded and no interrupt handler allocates,
// so `next` and the pool are never accessed concurrently.
unsafe impl Sync for BumpAlloc {}

impl BumpAlloc {
    /// Start of the pool.
    fn base(&self) -> *mut u8 {
        self.pool.0.get().cast::<u8>()
    }
}

// SAFETY: blocks are disjoint, in bounds of the pool and aligned as
// requested; a block is only resized in place when it is the last one, so
// no live block is ever handed out twice.
unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: single-threaded (see the `Sync` impl), so this is the only
        // access to `next` for the duration of the call.
        let next = unsafe { &mut *self.next.get() };
        let base = self.base();
        let Some(start) = (base.addr() + *next)
            .checked_next_multiple_of(layout.align())
            .map(|a| a - base.addr())
        else {
            return ptr::null_mut();
        };
        let Some(end) = start.checked_add(layout.size()) else {
            return ptr::null_mut();
        };
        if end > HEAP_BYTES {
            return ptr::null_mut();
        }
        *next = end;
        base.wrapping_add(start)
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        // SAFETY: as in `alloc`.
        let next = unsafe { &mut *self.next.get() };
        let start = block.addr() - self.base().addr();
        if start + layout.size() == *next {
            *next = start;
        }
    }

    unsafe fn realloc(&self, block: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let start = block.addr() - self.base().addr();
        {
            // SAFETY: as in `alloc`; the borrow ends before `alloc` below.
            let next = unsafe { &mut *self.next.get() };
            if start + layout.size() == *next {
                if let Some(end) = start.checked_add(new_size)
                    && end <= HEAP_BYTES
                {
                    *next = end;
                    return block;
                }
                return ptr::null_mut();
            }
        }
        // SAFETY: the caller guarantees `layout.align()` is valid and
        // `new_size` does not overflow when rounded up to it.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: `new_layout` has the non-zero size of the caller's request.
        let new_block = unsafe { self.alloc(new_layout) };
        if !new_block.is_null() {
            // SAFETY: both blocks are valid for the copied length and
            // disjoint (the new block lies after every existing one).
            unsafe { ptr::copy_nonoverlapping(block, new_block, layout.size().min(new_size)) };
        }
        new_block
    }
}

#[global_allocator]
static HEAP: BumpAlloc = BumpAlloc {
    pool: Pool(UnsafeCell::new([0; HEAP_BYTES])),
    next: UnsafeCell::new(0),
};

/*********************
 * Display
 *********************/

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
