//! Ported from lvgl v9.6.0: `src/draw/sw/blend/lv_draw_sw_blend_private.h`
//! (`lv_draw_sw_word_t`, the word-at-a-time access used by the v9.6 fills).
//!
//! Byte slice <-> `u16` / `u32` word views for the blend kernels.
//!
//! This is the only module of the crate (and one of two in the workspace)
//! allowed to contain `unsafe`. Every view is handed out only when the
//! bytes are suitably aligned; callers fall back to a byte path otherwise
//! (upstream does the same with its "lead in until word aligned" loops).
//! The words are in native byte order: the kernels convert with
//! `u32::from_le` / `to_le` so that the memory layout is little-endian on
//! every target, like the upstream `LV_BIG_ENDIAN_SYSTEM == 0` paths.
//! Everything else in the renderer uses `chunks_exact` and `from_le_bytes`.

#![allow(unsafe_code)]

use core::mem::{align_of, size_of, size_of_val};

/// Number of leading bytes of `buf` before the first address aligned to
/// `align` (clamped to `buf.len()`).
#[inline]
fn lead_len(buf: &[u8], align: usize) -> usize {
    let addr = buf.as_ptr() as usize;
    let lead = (align - (addr & (align - 1))) & (align - 1);
    lead.min(buf.len())
}

/// `buf` starts at an address aligned for `T` and holds a whole number of
/// `T`s: the precondition of every checked view below.
#[inline]
fn fits<T>(buf: &[u8]) -> bool {
    (buf.as_ptr() as usize).is_multiple_of(align_of::<T>()) && buf.len().is_multiple_of(size_of::<T>())
}

/// Split `buf` into `(head, words, tail)`: `head` is the unaligned lead-in
/// (at most 3 bytes), `words` the 4-byte aligned middle viewed as `u32`
/// and `tail` the remaining 0..=3 bytes. Mirrors the upstream
/// `while(dest < dest_end && ((lv_uintptr_t)dest & 0x3))` lead-in followed
/// by `lv_draw_sw_word_t` stores.
#[must_use]
pub fn split_u32_mut(buf: &mut [u8]) -> (&mut [u8], &mut [u32], &mut [u8]) {
    let lead = lead_len(buf, align_of::<u32>());
    let (head, rest) = buf.split_at_mut(lead);
    let n = rest.len() / size_of::<u32>();
    let (mid, tail) = rest.split_at_mut(n * size_of::<u32>());
    // With `n > 0` the lead-in was not clamped, so `mid` is aligned and the
    // checked view always succeeds. With `n == 0` `mid` is empty (and maybe
    // unaligned, which `from_raw_parts` would not accept): no words.
    let words: &mut [u32] = as_u32_mut(mid).unwrap_or_default();
    debug_assert_eq!(words.len(), n);
    (head, words, tail)
}

/// Split `buf` into `(head, words, tail)` like [`split_u32_mut`] but for
/// 2-byte `u16` words (`head` and `tail` are at most 1 byte each).
#[must_use]
pub fn split_u16_mut(buf: &mut [u8]) -> (&mut [u8], &mut [u16], &mut [u8]) {
    let lead = lead_len(buf, align_of::<u16>());
    let (head, rest) = buf.split_at_mut(lead);
    let n = rest.len() / size_of::<u16>();
    let (mid, tail) = rest.split_at_mut(n * size_of::<u16>());
    // See `split_u32_mut`: aligned whenever it is not empty.
    let words: &mut [u16] = as_u16_mut(mid).unwrap_or_default();
    debug_assert_eq!(words.len(), n);
    (head, words, tail)
}

/// View `buf` as `u32` words if it is 4-byte aligned and its length is a
/// multiple of 4, `None` otherwise (callers then use a byte path).
#[must_use]
pub fn as_u32_mut(buf: &mut [u8]) -> Option<&mut [u32]> {
    if !fits::<u32>(buf) {
        return None;
    }
    let n = buf.len() / size_of::<u32>();
    // SAFETY: alignment and length were checked just above, every bit
    // pattern is a valid `u32` and the result exclusively reborrows `buf`.
    Some(unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u32>(), n) })
}

/// Read-only variant of [`as_u32_mut`].
#[must_use]
pub fn as_u32(buf: &[u8]) -> Option<&[u32]> {
    if !fits::<u32>(buf) {
        return None;
    }
    let n = buf.len() / size_of::<u32>();
    // SAFETY: alignment and length were checked just above, every bit
    // pattern is a valid `u32` and the result shares the borrow of `buf`.
    Some(unsafe { core::slice::from_raw_parts(buf.as_ptr().cast::<u32>(), n) })
}

/// View `buf` as `u16` words if it is 2-byte aligned and of even length,
/// `None` otherwise.
#[must_use]
pub fn as_u16_mut(buf: &mut [u8]) -> Option<&mut [u16]> {
    if !fits::<u16>(buf) {
        return None;
    }
    let n = buf.len() / size_of::<u16>();
    // SAFETY: alignment and length were checked just above, every bit
    // pattern is a valid `u16` and the result exclusively reborrows `buf`.
    Some(unsafe { core::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u16>(), n) })
}

/// Read-only variant of [`as_u16_mut`].
#[must_use]
pub fn as_u16(buf: &[u8]) -> Option<&[u16]> {
    if !fits::<u16>(buf) {
        return None;
    }
    let n = buf.len() / size_of::<u16>();
    // SAFETY: alignment and length were checked just above, every bit
    // pattern is a valid `u16` and the result shares the borrow of `buf`.
    Some(unsafe { core::slice::from_raw_parts(buf.as_ptr().cast::<u16>(), n) })
}

/// View `u32` words as their bytes (always possible: `u8` has alignment 1).
#[must_use]
pub fn u32_as_bytes_mut(words: &mut [u32]) -> &mut [u8] {
    let n = size_of_val(words);
    // SAFETY: `u8` has alignment 1 and no invalid bit patterns, the byte
    // length covers exactly the memory of `words`, and the result
    // exclusively reborrows `words`, so writes stay valid `u32`s.
    unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), n) }
}

/// Read-only variant of [`u32_as_bytes_mut`].
#[must_use]
pub fn u32_as_bytes(words: &[u32]) -> &[u8] {
    let n = size_of_val(words);
    // SAFETY: `u8` has alignment 1, the length covers exactly the memory of
    // `words`, and the result shares the borrow of `words`.
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), n) }
}

/// Store `v` (little-endian in memory) into every 4-byte group of `buf`,
/// with aligned word stores when possible and a byte path otherwise.
/// `buf.len()` must be a multiple of 4 (a trailing partial group is left
/// untouched).
pub fn fill_u32_le(buf: &mut [u8], v: u32) {
    if let Some(words) = as_u32_mut(buf) {
        let v = v.to_le();
        for w in words.iter_mut() {
            *w = v;
        }
    } else {
        let bytes = v.to_le_bytes();
        for c in buf.as_chunks_mut::<4>().0 {
            c.copy_from_slice(&bytes);
        }
    }
}

/// Replace every 4-byte little-endian group `d` of `buf` with `f(d)`, using
/// aligned word accesses when possible and a byte path otherwise. A
/// trailing partial group is left untouched.
pub fn map_u32_le(buf: &mut [u8], mut f: impl FnMut(u32) -> u32) {
    if let Some(words) = as_u32_mut(buf) {
        for w in words.iter_mut() {
            *w = f(u32::from_le(*w)).to_le();
        }
    } else {
        for c in buf.as_chunks_mut::<4>().0 {
            let d = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            c.copy_from_slice(&f(d).to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// A `u32` backed buffer so that byte offsets have known alignment.
    fn aligned(n_words: usize) -> Vec<u32> {
        vec![0u32; n_words]
    }

    #[test]
    fn split_u32_every_offset_and_len() {
        let mut backing = aligned(8);
        let bytes = u32_as_bytes_mut(&mut backing);
        for ofs in 0..4 {
            for len in 0..(bytes.len() - ofs) {
                let s = &mut bytes[ofs..ofs + len];
                let (head, words, tail) = split_u32_mut(s);
                let expect_head = ((4 - ofs) % 4).min(len);
                assert_eq!(head.len(), expect_head);
                assert_eq!(words.len(), (len - expect_head) / 4);
                assert_eq!(tail.len(), (len - expect_head) % 4);
                assert_eq!(words.as_ptr() as usize % 4, 0);
            }
        }
    }

    #[test]
    fn split_u32_writes_land_in_place() {
        let mut backing = aligned(4);
        let bytes = u32_as_bytes_mut(&mut backing);
        let s = &mut bytes[1..15];
        let (head, words, tail) = split_u32_mut(s);
        for b in head.iter_mut() {
            *b = 0xAA;
        }
        for w in words.iter_mut() {
            *w = 0x0403_0201u32.to_le();
        }
        for b in tail.iter_mut() {
            *b = 0xBB;
        }
        assert_eq!(&bytes[0..4], &[0, 0xAA, 0xAA, 0xAA]);
        assert_eq!(&bytes[4..12], &[1, 2, 3, 4, 1, 2, 3, 4]);
        assert_eq!(&bytes[12..16], &[0xBB, 0xBB, 0xBB, 0]);
    }

    #[test]
    fn split_u16_every_offset() {
        let mut backing = aligned(4);
        let bytes = u32_as_bytes_mut(&mut backing);
        for ofs in 0..2 {
            for len in 0..(bytes.len() - ofs) {
                let (head, words, tail) = split_u16_mut(&mut bytes[ofs..ofs + len]);
                let expect_head = (ofs % 2).min(len);
                assert_eq!(head.len(), expect_head);
                assert_eq!(words.len(), (len - expect_head) / 2);
                assert_eq!(tail.len(), (len - expect_head) % 2);
                for w in words.iter_mut() {
                    *w = 0xBEEF;
                }
            }
        }
    }

    #[test]
    fn checked_views() {
        let mut backing = aligned(3);
        let bytes = u32_as_bytes_mut(&mut backing);
        assert!(as_u32_mut(&mut bytes[1..5]).is_none());
        assert!(as_u32_mut(&mut bytes[0..5]).is_none());
        {
            let w = as_u32_mut(&mut bytes[4..12]).unwrap();
            assert_eq!(w.len(), 2);
            w[1] = u32::from_le_bytes([9, 8, 7, 6]);
        }
        assert_eq!(&bytes[8..12], &[9, 8, 7, 6]);
        assert_eq!(
            as_u32(&bytes[8..12]).unwrap()[0],
            u32::from_le_bytes([9, 8, 7, 6])
        );
        assert!(as_u32(&bytes[2..6]).is_none());
        assert!(as_u16_mut(&mut bytes[1..3]).is_none());
        assert!(as_u16_mut(&mut bytes[0..3]).is_none());
        as_u16_mut(&mut bytes[2..4]).unwrap()[0] = u16::from_le_bytes([5, 6]);
        assert_eq!(&bytes[2..4], &[5, 6]);
        assert_eq!(as_u16(&bytes[2..4]).unwrap()[0], u16::from_le_bytes([5, 6]));
        assert!(as_u16(&bytes[1..3]).is_none());
        assert_eq!(u32_as_bytes(&backing).len(), 12);
    }

    #[test]
    fn fill_and_map_aligned_and_unaligned_agree() {
        let mut a = aligned(5);
        let mut b = aligned(5);
        let ab = u32_as_bytes_mut(&mut a);
        let bb = u32_as_bytes_mut(&mut b);
        for (i, (x, y)) in ab.iter_mut().zip(bb.iter_mut()).enumerate() {
            *x = i as u8;
            *y = i as u8;
        }
        // Aligned path vs unaligned (byte) path over the same values.
        let f = |d: u32| d.rotate_left(8) ^ 0x1234_5678;
        map_u32_le(&mut ab[0..16], f);
        let mut tmp = [0u8; 17];
        tmp[1..17].copy_from_slice(&bb[0..16]);
        map_u32_le(&mut tmp[1..17], f);
        assert_eq!(&ab[0..16], &tmp[1..17]);

        fill_u32_le(&mut ab[4..12], 0xFF11_2233);
        assert_eq!(&ab[4..12], &[0x33, 0x22, 0x11, 0xFF, 0x33, 0x22, 0x11, 0xFF]);
        fill_u32_le(&mut tmp[1..9], 0xFF11_2233);
        assert_eq!(&tmp[1..9], &ab[4..12]);
    }
}
