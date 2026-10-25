//! First-fit free-list allocator over a static pool.
//!
//! Upstream LVGL's builtin heap (`src/stdlib/builtin/lv_tlsf.c`) reuses
//! freed blocks, so an application can run its timer loop forever on a
//! fixed pool. The library frees in arbitrary order (not LIFO) on every
//! display refresh, so a bump allocator would leak a little per refresh
//! period; this allocator keeps an address-ordered free list instead,
//! coalesces neighbouring free blocks and gives a freed block that touches
//! the untouched end of the pool back to it.
//!
//! Layout: every block offset and size is a multiple of [`UNIT`]. A free
//! block stores `[len, next]` (two `usize`) in its first granule; allocated
//! blocks have no header because `dealloc` receives the block's `Layout`.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

/// Allocation granule: room for a free block's `[len, next]` header.
pub const UNIT: usize = 2 * size_of::<usize>();
/// `next` value that ends the free list.
const NIL: usize = usize::MAX;

/// The byte pool, aligned to a multiple of [`UNIT`] on every target.
#[repr(C, align(16))]
struct Arena<const N: usize>(UnsafeCell<[u8; N]>);

/// Allocator bookkeeping. All-zero when empty, so the allocator static
/// (pool included) lands in `.bss` instead of taking its size in flash.
struct State {
    /// Bitwise NOT of the offset of the lowest free block (`!NIL == 0`
    /// for an empty list); read and written through [`State::head`] and
    /// [`State::set_head`].
    head_not: usize,
    /// Offset of the first byte never handed out (or handed back); the
    /// free list only holds blocks below it.
    top: usize,
}

impl State {
    /// Offset of the lowest free block, or [`NIL`].
    fn head(&self) -> usize {
        !self.head_not
    }

    /// Set the lowest free block (or [`NIL`]).
    fn set_head(&mut self, at: usize) {
        self.head_not = !at;
    }
}

/// First-fit free-list allocator over an `N`-byte pool (see the module
/// documentation).
pub struct FreeListAlloc<const N: usize> {
    /// The pool blocks are carved from.
    pool: Arena<N>,
    /// Free list head and pool top.
    state: UnsafeCell<State>,
}

// SAFETY: `FreeListAlloc::new` is `unsafe` and requires that the allocator
// is never used from two contexts at once (single thread, no allocating
// interrupt handler), so `state` and the pool are never accessed
// concurrently even though the value is shared.
unsafe impl<const N: usize> Sync for FreeListAlloc<N> {}

impl<const N: usize> FreeListAlloc<N> {
    /// An allocator with an empty, unused pool.
    ///
    /// # Safety
    ///
    /// The allocator must only ever be used from one execution context at
    /// a time: one thread and no interrupt handler that allocates.
    pub const unsafe fn new() -> Self {
        assert!(
            N.is_multiple_of(UNIT) && N < NIL,
            "pool size must be a multiple of UNIT"
        );
        Self {
            pool: Arena(UnsafeCell::new([0; N])),
            state: UnsafeCell::new(State {
                head_not: !NIL,
                top: 0,
            }),
        }
    }

    /// Bytes from the start of the pool up to the first never used byte
    /// (the high-water mark minus what was handed back to the end).
    pub fn top(&self) -> usize {
        // SAFETY: single execution context (see `new`); no `&mut State`
        // is live outside the `GlobalAlloc` methods.
        unsafe { (*self.state.get()).top }
    }

    /// Bytes currently on the free list.
    pub fn free_listed(&self) -> usize {
        // SAFETY: as in `top`.
        let mut cur = unsafe { (*self.state.get()).head() };
        let mut sum = 0;
        while cur != NIL {
            // SAFETY: `cur` is on the free list.
            let [len, next] = unsafe { self.read(cur) };
            sum += len;
            cur = next;
        }
        sum
    }

    /// Start of the pool.
    fn base(&self) -> *mut u8 {
        self.pool.0.get().cast::<u8>()
    }

    /// Read the header of the free block at offset `at`.
    ///
    /// # Safety
    ///
    /// `at` is a free block offset: a multiple of [`UNIT`] with
    /// `at + UNIT <= N`, not part of any allocated block.
    unsafe fn read(&self, at: usize) -> [usize; 2] {
        // SAFETY: in bounds and aligned (the pool is 16-byte aligned and
        // `at` a multiple of `UNIT`); free memory is only accessed here.
        unsafe { self.base().add(at).cast::<[usize; 2]>().read() }
    }

    /// Write the header of the free block at offset `at`.
    ///
    /// # Safety
    ///
    /// As for [`Self::read`].
    unsafe fn write(&self, at: usize, header: [usize; 2]) {
        // SAFETY: as in `read`.
        unsafe { self.base().add(at).cast::<[usize; 2]>().write(header) }
    }

    /// Make the free block `pred` (or the list head if `pred` is [`NIL`])
    /// point to `to`.
    ///
    /// # Safety
    ///
    /// `pred` is [`NIL`] or a free block offset.
    unsafe fn link(&self, st: &mut State, pred: usize, to: usize) {
        if pred == NIL {
            st.set_head(to);
        } else {
            // SAFETY: `pred` is a free block (caller).
            let [len, _] = unsafe { self.read(pred) };
            // SAFETY: as above.
            unsafe { self.write(pred, [len, to]) };
        }
    }

    /// Offset of the first `align`-aligned address at or after offset `at`.
    fn align_up(&self, at: usize, align: usize) -> Option<usize> {
        let base = self.base().addr();
        (base + at).checked_next_multiple_of(align).map(|a| a - base)
    }

    /// Put `[at, at + len)` on the free list, merging it with adjacent
    /// free blocks, or lower `top` if it ends there.
    ///
    /// # Safety
    ///
    /// The range is a multiple of [`UNIT`] in offset and length, lies below
    /// `top`, and is neither allocated nor already free.
    unsafe fn free_range(&self, st: &mut State, at: usize, len: usize) {
        let (mut pprev, mut prev, mut cur) = (NIL, NIL, st.head());
        while cur != NIL && cur < at {
            pprev = prev;
            prev = cur;
            // SAFETY: `cur` is on the free list.
            cur = unsafe { self.read(cur) }[1];
        }
        let (mut start, mut len, mut next) = (at, len, cur);
        if cur != NIL && at + len == cur {
            // SAFETY: `cur` is on the free list.
            let [clen, cnext] = unsafe { self.read(cur) };
            len += clen;
            next = cnext;
        }
        if prev != NIL {
            // SAFETY: `prev` is on the free list.
            let [plen, _] = unsafe { self.read(prev) };
            if prev + plen == at {
                start = prev;
                len += plen;
            }
        }
        if start + len == st.top {
            // Every free block lies below `top`, so nothing follows this one.
            debug_assert_eq!(next, NIL);
            st.top = start;
            let pred = if start == prev { pprev } else { prev };
            // SAFETY: `pred` is NIL or on the free list.
            unsafe { self.link(st, pred, NIL) };
        } else {
            // SAFETY: `start` is a merged free range of at least `UNIT` bytes.
            unsafe { self.write(start, [len, next]) };
            if start != prev {
                // SAFETY: `prev` is NIL or on the free list.
                unsafe { self.link(st, prev, start) };
            }
        }
    }
}

/// `size` rounded up to a non-zero multiple of [`UNIT`].
fn granules(size: usize) -> Option<usize> {
    size.max(1).checked_next_multiple_of(UNIT)
}

// SAFETY: blocks are carved from free list entries or from above `top`,
// so they are disjoint, in bounds of the pool and aligned as requested; a
// block only returns to the free list (or below `top`) in `dealloc`.
unsafe impl<const N: usize> GlobalAlloc for FreeListAlloc<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(size) = granules(layout.size()) else {
            return ptr::null_mut();
        };
        let align = layout.align();
        // SAFETY: single execution context (see `new`), so this is the only
        // reference to the state for the duration of the call.
        let st = unsafe { &mut *self.state.get() };

        // First fit. The aligned start and the split points stay multiples
        // of `UNIT`: the pool is 16-byte aligned and `align` is a power of
        // two, so aligning up never leaves a sub-granule fragment.
        let (mut prev, mut cur) = (NIL, st.head());
        while cur != NIL {
            // SAFETY: `cur` is on the free list.
            let [len, next] = unsafe { self.read(cur) };
            if let Some(start) = self.align_up(cur, align)
                && let Some(end) = start.checked_add(size)
                && end <= cur + len
            {
                let tail = cur + len - end;
                let rest = if tail > 0 {
                    // SAFETY: `[end, end + tail)` is the free remainder.
                    unsafe { self.write(end, [tail, next]) };
                    end
                } else {
                    next
                };
                if start > cur {
                    // SAFETY: `[cur, start)` stays free.
                    unsafe { self.write(cur, [start - cur, rest]) };
                } else {
                    // SAFETY: `prev` is NIL or on the free list.
                    unsafe { self.link(st, prev, rest) };
                }
                // SAFETY: `start < N`, in bounds of the pool.
                return unsafe { self.base().add(start) };
            }
            prev = cur;
            cur = next;
        }

        // Nothing fits: carve from the unused end.
        let old_top = st.top;
        let Some(start) = self.align_up(old_top, align) else {
            return ptr::null_mut();
        };
        let Some(end) = start.checked_add(size) else {
            return ptr::null_mut();
        };
        if end > N {
            return ptr::null_mut();
        }
        st.top = end;
        if start > old_top {
            // SAFETY: the alignment gap `[old_top, start)` is unused and
            // below the new `top`.
            unsafe { self.free_range(st, old_top, start - old_top) };
        }
        // SAFETY: `start < N`, in bounds of the pool.
        unsafe { self.base().add(start) }
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        // The caller passes the layout `alloc` succeeded with, so this
        // rounding cannot overflow.
        let size = granules(layout.size()).unwrap_or(UNIT);
        let at = block.addr() - self.base().addr();
        // SAFETY: single execution context (see `new`).
        let st = unsafe { &mut *self.state.get() };
        // SAFETY: `[at, at + size)` is the block `alloc` handed out.
        unsafe { self.free_range(st, at, size) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::vec::Vec;

    /// A fresh 4 KiB allocator on the heap.
    fn heap() -> Box<FreeListAlloc<4096>> {
        // SAFETY: each test uses its allocator from its own thread only.
        Box::new(unsafe { FreeListAlloc::new() })
    }

    fn alloc(h: &FreeListAlloc<4096>, size: usize, align: usize) -> *mut u8 {
        // SAFETY: a valid, non-zero-size layout.
        let p = unsafe { h.alloc(Layout::from_size_align(size, align).unwrap()) };
        assert!(!p.is_null(), "out of memory for {size}/{align}");
        assert_eq!(p.addr() % align, 0);
        p
    }

    fn free(h: &FreeListAlloc<4096>, p: *mut u8, size: usize, align: usize) {
        // SAFETY: the tests only free blocks `alloc` returned for this layout.
        unsafe { h.dealloc(p, Layout::from_size_align(size, align).unwrap()) };
    }

    #[test]
    fn non_lifo_frees_are_reused() {
        let h = heap();
        // One long-lived block, then a repeated "refresh" pattern that frees
        // in allocation (FIFO) order: a bump allocator leaks every round.
        let keep = alloc(&h, 100, 4);
        let mut top = None;
        for round in 0..1000 {
            let a = alloc(&h, 24, 4);
            let b = alloc(&h, 200, 8);
            let c = alloc(&h, 7, 1);
            free(&h, a, 24, 4);
            let d = alloc(&h, 64, 4);
            free(&h, b, 200, 8);
            free(&h, c, 7, 1);
            free(&h, d, 64, 4);
            if round == 1 {
                top = Some(h.top());
            }
        }
        assert_eq!(Some(h.top()), top);
        free(&h, keep, 100, 4);
        assert_eq!(h.top(), 0);
        assert_eq!(h.free_listed(), 0);
    }

    #[test]
    fn neighbours_coalesce() {
        let h = heap();
        let blocks: Vec<_> = (0..8).map(|_| alloc(&h, 3 * UNIT, 4)).collect();
        let fence = alloc(&h, 1, 1);
        // Free every other block, then the rest: all must merge into one.
        for i in (0..8).step_by(2) {
            free(&h, blocks[i], 3 * UNIT, 4);
        }
        assert_eq!(h.free_listed(), 4 * 3 * UNIT);
        for i in (1..8).step_by(2) {
            free(&h, blocks[i], 3 * UNIT, 4);
        }
        assert_eq!(h.free_listed(), 8 * 3 * UNIT);
        // The merged block serves one large request at the pool start.
        let big = alloc(&h, 8 * 3 * UNIT, 4);
        assert_eq!(big, blocks[0]);
        assert_eq!(h.free_listed(), 0);
        free(&h, big, 8 * 3 * UNIT, 4);
        free(&h, fence, 1, 1);
        assert_eq!((h.top(), h.free_listed()), (0, 0));
    }

    #[test]
    fn over_aligned_blocks_and_exhaustion() {
        let h = heap();
        let small = alloc(&h, 1, 1);
        let wide = alloc(&h, 64, 256);
        free(&h, small, 1, 1);
        // The alignment gap below `wide` is free and reusable.
        assert!(h.free_listed() > 0);
        let again = alloc(&h, 1, 1);
        assert_eq!(again, small);
        free(&h, again, 1, 1);
        free(&h, wide, 64, 256);
        assert_eq!((h.top(), h.free_listed()), (0, 0));
        // SAFETY: valid, non-zero-size layouts.
        assert!(unsafe { h.alloc(Layout::from_size_align(4097, 1).unwrap()) }.is_null());
        let all = alloc(&h, 4096, 1);
        // SAFETY: as above.
        assert!(unsafe { h.alloc(Layout::from_size_align(1, 1).unwrap()) }.is_null());
        free(&h, all, 4096, 1);
    }

    #[test]
    fn realloc_moves_and_keeps_contents() {
        let h = heap();
        let l = Layout::from_size_align(4, 4).unwrap();
        let p = alloc(&h, 4, 4);
        // SAFETY: `p` is a live, 4-byte aligned block of 4 bytes.
        unsafe { p.cast::<u32>().write(0xdead_beef) };
        let _fence = alloc(&h, 1, 1);
        // SAFETY: `p` was allocated with `l`; 100 bytes is a valid size.
        let q = unsafe { h.realloc(p, l, 100) };
        assert!(!q.is_null());
        // SAFETY: `q` is a live 100-byte block whose first word was copied.
        assert_eq!(unsafe { q.cast::<u32>().read() }, 0xdead_beef);
        // The old block went back to the free list.
        assert_eq!(h.free_listed(), granules(4).unwrap());
    }
}
