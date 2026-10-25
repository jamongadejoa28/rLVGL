//! Generational arena used for objects, displays, input devices, timers,
//! animations, styles and event callbacks.
//!
//! Every LVGL object is addressed through a `Copy` key. Removing a slot bumps
//! its generation so stale keys are detected instead of aliasing a new value,
//! which mirrors the `lv_obj_t *` discipline (parent owns, everyone else holds
//! a weak reference) without any `unsafe`.

use alloc::vec::Vec;
use core::fmt;
use core::num::NonZeroU32;

/// Index plus generation. `Option<Key>` is 8 bytes thanks to the `NonZeroU32`
/// niche.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key {
    index: u32,
    generation: NonZeroU32,
}

impl Key {
    /// Slot index (for debugging and deterministic ordering).
    #[inline]
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
    /// Generation of the slot when the key was issued.
    #[inline]
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation.get()
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}v{}", self.index, self.generation)
    }
}

#[derive(Debug)]
enum Slot<T> {
    Occupied {
        generation: NonZeroU32,
        value: T,
    },
    Vacant {
        next_generation: NonZeroU32,
        next_free: Option<u32>,
    },
}

/// A generational arena.
#[derive(Debug)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free_head: Option<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Arena<T> {
    /// Create an empty arena.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            len: 0,
        }
    }

    /// Create an arena with room for `cap` values.
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            slots: Vec::with_capacity(cap),
            free_head: None,
            len: 0,
        }
    }

    /// Number of live values.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if no value is stored.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Insert a value and return its key.
    #[must_use]
    pub fn insert(&mut self, value: T) -> Key {
        self.len += 1;
        if let Some(index) = self.free_head {
            let slot = &mut self.slots[index as usize];
            let Slot::Vacant {
                next_generation,
                next_free,
            } = *slot
            else {
                unreachable!("free list points at an occupied slot");
            };
            self.free_head = next_free;
            *slot = Slot::Occupied {
                generation: next_generation,
                value,
            };
            Key {
                index,
                generation: next_generation,
            }
        } else {
            let index = u32::try_from(self.slots.len()).expect("arena exceeds u32::MAX slots");
            let generation = NonZeroU32::MIN;
            self.slots.push(Slot::Occupied { generation, value });
            Key { index, generation }
        }
    }

    /// Remove and return the value behind `key`, if it is still alive.
    pub fn remove(&mut self, key: Key) -> Option<T> {
        let slot = self.slots.get_mut(key.index as usize)?;
        match slot {
            Slot::Occupied { generation, .. } if *generation == key.generation => {
                // Wrap the generation to 1 instead of 0 so the NonZero niche survives;
                // a 2^32 cycle on one slot is accepted as the usual generational caveat.
                let next_generation =
                    NonZeroU32::new(generation.get().wrapping_add(1)).unwrap_or(NonZeroU32::MIN);
                let old = core::mem::replace(
                    slot,
                    Slot::Vacant {
                        next_generation,
                        next_free: self.free_head,
                    },
                );
                self.free_head = Some(key.index);
                self.len -= 1;
                match old {
                    Slot::Occupied { value, .. } => Some(value),
                    Slot::Vacant { .. } => unreachable!(),
                }
            }
            _ => None,
        }
    }

    /// `true` if `key` refers to a live value.
    #[inline]
    #[must_use]
    pub fn contains(&self, key: Key) -> bool {
        self.get(key).is_some()
    }

    /// Borrow the value behind `key`.
    #[inline]
    #[must_use]
    pub fn get(&self, key: Key) -> Option<&T> {
        match self.slots.get(key.index as usize)? {
            Slot::Occupied { generation, value } if *generation == key.generation => Some(value),
            _ => None,
        }
    }

    /// Mutably borrow the value behind `key`.
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        match self.slots.get_mut(key.index as usize)? {
            Slot::Occupied { generation, value } if *generation == key.generation => Some(value),
            _ => None,
        }
    }

    /// Mutably borrow two distinct values at once (e.g. a parent and a child).
    ///
    /// Returns `None` if either key is dead or both keys are equal.
    #[must_use]
    pub fn get2_mut(&mut self, a: Key, b: Key) -> Option<(&mut T, &mut T)> {
        if a.index == b.index {
            return None;
        }
        if !self.contains(a) || !self.contains(b) {
            return None;
        }
        let (lo, hi, swapped) = if a.index < b.index {
            (a.index, b.index, false)
        } else {
            (b.index, a.index, true)
        };
        let (left, right) = self.slots.split_at_mut(hi as usize);
        let lo_val = match &mut left[lo as usize] {
            Slot::Occupied { value, .. } => value,
            Slot::Vacant { .. } => return None,
        };
        let hi_val = match &mut right[0] {
            Slot::Occupied { value, .. } => value,
            Slot::Vacant { .. } => return None,
        };
        Some(if swapped {
            (hi_val, lo_val)
        } else {
            (lo_val, hi_val)
        })
    }

    /// Iterate over live `(key, &value)` pairs in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (Key, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, slot)| match slot {
            Slot::Occupied { generation, value } => Some((
                Key {
                    index: i as u32,
                    generation: *generation,
                },
                value,
            )),
            Slot::Vacant { .. } => None,
        })
    }

    /// Iterate over live `(key, &mut value)` pairs in slot order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Key, &mut T)> + '_ {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(i, slot)| match slot {
                Slot::Occupied { generation, value } => Some((
                    Key {
                        index: i as u32,
                        generation: *generation,
                    },
                    value,
                )),
                Slot::Vacant { .. } => None,
            })
    }

    /// Keys of all live values, in slot order.
    pub fn keys(&self) -> impl Iterator<Item = Key> + '_ {
        self.iter().map(|(k, _)| k)
    }

    /// Remove every value.
    pub fn clear(&mut self) {
        self.slots.clear();
        self.free_head = None;
        self.len = 0;
    }
}

impl<T> core::ops::Index<Key> for Arena<T> {
    type Output = T;
    #[track_caller]
    fn index(&self, key: Key) -> &T {
        self.get(key).expect("stale or invalid arena key")
    }
}

impl<T> core::ops::IndexMut<Key> for Arena<T> {
    #[track_caller]
    fn index_mut(&mut self, key: Key) -> &mut T {
        self.get_mut(key).expect("stale or invalid arena key")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut a = Arena::new();
        let k1 = a.insert("one");
        let k2 = a.insert("two");
        assert_eq!(a.len(), 2);
        assert_eq!(a.get(k1), Some(&"one"));
        assert_eq!(a[k2], "two");
        assert_eq!(a.remove(k1), Some("one"));
        assert_eq!(a.remove(k1), None);
        assert!(!a.contains(k1));
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn stale_keys_are_rejected_after_reuse() {
        let mut a = Arena::new();
        let k1 = a.insert(1);
        a.remove(k1);
        let k2 = a.insert(2);
        assert_eq!(k1.index(), k2.index(), "slot is reused");
        assert_ne!(k1, k2);
        assert_eq!(a.get(k1), None);
        assert_eq!(a.get(k2), Some(&2));
    }

    #[test]
    fn get2_mut_disjoint() {
        let mut a = Arena::new();
        let k1 = a.insert(1);
        let k2 = a.insert(2);
        let (x, y) = a.get2_mut(k2, k1).unwrap();
        core::mem::swap(x, y);
        assert_eq!(a[k1], 2);
        assert_eq!(a[k2], 1);
        assert!(a.get2_mut(k1, k1).is_none());
    }

    #[test]
    fn iteration_is_in_slot_order() {
        let mut a = Arena::new();
        let keys: Vec<Key> = (0..5).map(|i| a.insert(i)).collect();
        a.remove(keys[1]);
        a.remove(keys[3]);
        let values: Vec<i32> = a.iter().map(|(_, v)| *v).collect();
        assert_eq!(values, [0, 2, 4]);
        let k = a.insert(9);
        assert_eq!(k.index(), 3, "most recently freed slot is reused first");
    }

    #[test]
    fn option_key_is_niche_optimised() {
        assert_eq!(size_of::<Option<Key>>(), 8);
    }
}
