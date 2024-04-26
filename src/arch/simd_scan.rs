#![allow(clippy::let_and_return)] // the contracts require this and without the `verify` feature
                                  // these bindings will cause warnings.

use crate::arch::{self, byte_ptr, simd_ptr, Vector};

cfg_verify!(
    use crate::arch::is_aligned;
    use mirai_annotations::{checked_precondition, checked_postcondition};
);

mod end_ptr {
    cfg_verify!(use super::checked_postcondition;);
    use crate::arch::Ptr;

    /// An immutable representation of the `data`'s upper bound
    #[derive(Copy, Clone)]
    #[repr(transparent)]
    pub struct EndPtr(*const Ptr);

    impl EndPtr {
        #[inline(always)] #[must_use]
        pub const unsafe fn new(data: &[u8]) -> Self {
            Self ( (data.as_ptr().add(data.len())).cast() )
        }
    }

    impl EndPtr {
        #[cfg_attr(feature = "verify", contracts::ensures(self.0 == ret))]
        #[cfg_attr(feature = "verify", contracts::ensures(self.0 == old(self.0)))]
        #[inline(always)] #[must_use]
        pub fn get(&self) -> *const Ptr {
            self.0
        }

        /// Checks that the underlying pointer has not changed, this is less ensuring correctness
        /// of the program, more ensuring that no future changes violate the immutability invariant
        /// via adjustments to the contracts & this type.
        #[cfg(feature = "verify")]
        pub unsafe fn check(&self, data: &[u8]) -> bool {
            super::byte_ptr(self.get()) == super::byte_ptr(Self::new(data).get())
        }
    }
}

macro_rules! check_end_ptr {
    ($end_ptr:expr, $data:expr) => {
        #[cfg(feature = "verify")]
        assert!($end_ptr.check($data))
    };
}

use end_ptr::EndPtr;

#[cfg_attr(feature = "verify", contracts::ensures(x >= 0 -> x as usize == ret))]
#[inline(always)] #[must_use]
unsafe fn remove_sign(x: isize) -> usize {
    x as usize
}

#[cfg_attr(feature = "verify", contracts::requires(l >= r))]
#[inline(always)] #[must_use]
unsafe fn offset_from(l: *const u8, r: *const u8) -> isize {
    let ret = l.offset_from(r);
    // `l` being greater than 'r' is a precondition, therefore the offset will always be positive.
    contract!(assumed_postcondition!(ret >= 0));
    ret
}

#[cfg_attr(feature = "verify", contracts::requires(l >= r))]
#[cfg_attr(feature = "verify", contracts::ensures(ret == remove_sign(offset_from(l, r))))]
#[inline(always)] #[must_use]
unsafe fn distance(l: *const u8, r: *const u8) -> usize {
    remove_sign(offset_from(l, r))
}

#[cfg_attr(feature = "verify", contracts::requires(l >= r))]
#[cfg_attr(feature = "verify", contracts::requires(byte_ptr(l) >= byte_ptr(r)))]
#[cfg_attr(feature = "verify", contracts::ensures(ret == distance(byte_ptr(l), byte_ptr(r))))]
#[inline(always)] #[must_use]
unsafe fn simd_distance(l: *const arch::Ptr, r: *const arch::Ptr) -> usize {
    distance(byte_ptr(l), byte_ptr(r))
}

#[cfg_attr(feature = "verify", contracts::requires(dist <= arch::WIDTH))]
#[cfg_attr(feature = "verify", contracts::ensures(
    dist == distance(byte_ptr(_end.get()), cur) -> incr_ptr(simd_ptr(ret)) == _end.get(),
    "If `dist` is the byte offset of `_end` to `cur` then incr_ptr(simd_ptr(ret)) equates to \
    `_end`"
))]
#[inline(always)] #[must_use]
unsafe fn make_space(cur: *const u8, dist: usize, _end: EndPtr) -> *const u8 {
    cur.sub(arch::WIDTH - dist)
}

#[cfg_attr(feature = "verify", contracts::ensures(ptr == decr_ptr(ret)))]
#[cfg_attr(feature = "verify", contracts::ensures(ptr.add(arch::STEP) == ret))]
#[cfg_attr(feature = "verify", contracts::ensures(byte_ptr(decr_ptr(ret)) == byte_ptr(ptr)))]
#[cfg_attr(feature = "verify", contracts::ensures(is_aligned(ptr) -> is_aligned(ret)))]
#[inline(always)] #[must_use]
unsafe fn incr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.add(arch::STEP)
}

#[cfg_attr(feature = "verify", contracts::ensures(incr_ptr(ret) == ptr))]
#[cfg_attr(feature = "verify", contracts::ensures(is_aligned(ptr) -> is_aligned(ret)))]
#[inline(always)] #[must_use]
unsafe fn decr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.sub(arch::STEP)
}

#[cfg_attr(feature = "verify", contracts::ensures(
    ptr.align_offset(arch::WIDTH) != 0
        -> ret.1.cast::<u8>().offset_from(ptr) as usize == ptr.align_offset(arch::WIDTH)
))]
#[cfg_attr(feature = "verify", contracts::ensures(
    ptr.align_offset(arch::WIDTH) == 0 -> ret.1 == incr_ptr(simd_ptr(ptr))
))]
#[inline(always)] #[must_use]
unsafe fn align_ptr_or_incr(ptr: *const u8) -> (Vector, *const arch::Ptr) {
    match ptr.align_offset(arch::WIDTH) {
        0 => {
            let simd_ptr = simd_ptr(ptr);
            // When the pointer is already aligned, increment it by `arch::STEP`
            (arch::load_aligned(simd_ptr), incr_ptr(simd_ptr))
        },
        offset => {
            // When the pointer is not aligned, adjust it to the next alignment boundary
            (arch::load_unchecked(simd_ptr(ptr)), simd_ptr(ptr.add(offset)))
        }
    }
}

#[cfg_attr(feature = "verify", contracts::requires(end >= cur))]
#[cfg_attr(feature = "verify", contracts::ensures(is_aligned(cur) -> is_aligned(cur)))]
#[cfg_attr(feature = "verify", contracts::ensures(ret -> incr_ptr(cur) <= end))]
#[inline(always)] #[must_use]
unsafe fn can_proceed(cur: *const arch::Ptr, end: *const arch::Ptr) -> bool {
    cur <= decr_ptr(end)
}

mod sealed {
    use super::*;
    cfg_verify!(use contracts::invariant;);

    /// Initiate the scanning process
    ///
    /// # Returns
    ///
    /// 0. The first `Vector` associated with `data` which most likely will not be included
    ///    in the `AlignedIter`, so it must be operated on independently. This is to align the
    ///    pointer, enabling aligned loads for a performance enhancement.
    /// 1. The `AlignedIter` which will handle loading all data after the initial `Vector`
    ///    (`0`).
    #[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
    #[inline(always)] #[must_use]
    pub unsafe fn init_scan(data: &[u8]) -> (Vector, AlignedIter) {
        let (vector, aligned_ptr) = align_ptr_or_incr(data.as_ptr());
        (
            vector,
            AlignedIter::after_first(aligned_ptr, data)
        )
    }

    pub struct AlignedIter {
        cur: *const arch::Ptr,
        // `EndPtr` cannot be mutated so is it safe to expose.
        pub end: EndPtr,
    }

    pub type Remainder = (Vector, *const arch::Ptr);

    pub enum Pointer {
        Aligned((Vector, *const arch::Ptr)),
        End(Option<Remainder>)
    }

    #[cfg(feature = "verify")]
    impl Pointer {
        #[must_use]
        pub const fn is_aligned(&self) -> bool {
            matches!(self, Self::Aligned(_))
        }
        #[must_use]
        pub const fn is_end_with_remaining(&self) -> bool {
            matches!(self, Self::End(Some(_)))
        }
        #[must_use]
        fn remaining_end_ptr(&self) -> *const arch::Ptr {
            let Self::End(Some((_, ptr))) = self else {
                unreachable!(
                    "`remaining_end_ptr` called when state was not `End` with `Some` remainder"
                );
            };

            *ptr
        }
    }

    #[cfg_attr(feature = "verify", invariant(incr_ptr(self.cur) <= self.end.get()))]
    impl AlignedIter {
        #[cfg_attr(feature = "verify", contracts::requires(
            is_aligned(aligned_ptr),
            "To create an `AlignedIter` the `cur` pointer must be aligned to the `arch::WIDTH`"
        ))]
        #[inline(always)] #[must_use]
        unsafe fn after_first(aligned_ptr: *const arch::Ptr, data: &[u8]) -> Self {
            Self { cur: aligned_ptr, end: EndPtr::new(data) }
        }

        #[cfg_attr(feature = "verify", contracts::ensures(is_aligned(ret)))]
        #[cfg_attr(feature = "verify", contracts::ensures(incr_ptr(ret) <= self.end.get()))]
        #[inline(always)] #[must_use]
        pub unsafe fn snap(&self) -> *const arch::Ptr {
            self.cur
        }

        #[cfg_attr(feature = "verify", contracts::ensures(is_aligned(ret)))]
        #[cfg_attr(feature = "verify", contracts::ensures(incr_ptr(ret) <= self.end.get()))]
        #[inline(always)] #[must_use]
        pub unsafe fn snap_and_incr(&mut self) -> *const arch::Ptr {
            let ret = self.snap();
            self.cur = incr_ptr(ret);
            ret
        }
    }

    #[cfg_attr(feature = "verify", invariant(self.end.get() >= self.cur))]
    impl AlignedIter {
        #[cfg_attr(feature = "verify", contracts::ensures(
            ret.is_aligned() -> incr_ptr(self.cur) <= self.end.get()
        ))]
        #[cfg_attr(feature = "verify", contracts::ensures(
            ret.is_end_with_remaining()
                -> incr_ptr(ret.remaining_end_ptr()) == self.end.get()
        ))]
        #[inline(always)] #[must_use]
        pub unsafe fn next(&mut self) -> Pointer {
            if can_proceed(self.cur, self.end.get()) {
                Pointer::Aligned({
                    let ptr = self.snap_and_incr();
                    (arch::load_aligned(ptr), ptr)
                })
            } else {
                // As `can_proceed` failed and our invariant requires `end` to be greater than
                // `cur` we know `distance(byte_ptr(self.end), byte_ptr(self.cur))` is less than
                // `arch::WIDTH`
                Pointer::End(self.end())
            }
        }

        /// Handle the potential unaligned remaining bytes of `data`
        ///
        /// # Unchecked Precondition
        ///
        /// The [`AlignedIter::next`] must be mutable, unfortunately this complicates / makes the
        /// expression of the precondition infeasible. That is why this method **cannot be exposed**
        /// outside the `sealed` module.
        ///
        /// <br>
        ///
        /// This method is only used within [`AlignedIter::next`] where [`can_proceed`] fails.
        /// [`can_proceed`]'s postcondition states by contradiction that if false then the distance
        /// between `cur` and `end` is less than `arch::WIDTH`, this postcondition in conjunction
        /// with the invariant `end >= cur` guarantees that the distance between `end` and `cur` is
        /// less than `arch::WIDTH`, ensuring that the preconditions of [`make_space`] hold.
        ///
        /// # Returns
        ///
        /// * `Some(Remainder)` - Indicating that it was impossible to scan all of `data` with
        ///   aligned loads and that there was a remainder. The pointer in `Remainder` is guaranteed
        ///   to be exactly `arch::WIDTH` less than `end`, with the vector representing the final 16
        ///   bytes of `data`
        /// * `None` - There was no remainder and the scan can be considered completed.
        ///
        /// ### Note
        ///
        /// As this does not mutate the iterator it is safe to be called multiple times as long as
        /// the invariants of the encapsulated pointers are not violated. Though there is no reason
        /// to do this nor should it be done.
        #[cfg_attr(feature = "verify", contracts::ensures(
            ret.is_some() -> incr_ptr(ret.unwrap().1) == self.end.get()
        ))]
        #[inline(always)]
        unsafe fn end(&self) -> Option<Remainder> {
            match simd_distance(self.end.get(), self.cur) {
                0 => None,
                dist => {
                    contract!(checked_assume!(dist <= arch::WIDTH));
                    let ptr = simd_ptr(make_space(
                        byte_ptr(self.cur), dist, self.end
                    ));
                    Some((arch::load_unchecked(ptr), ptr))
                }
            }
        }
    }
}

#[cfg_attr(feature = "verify", contracts::ensures(ret -> len < arch::WIDTH as u32))]
#[cfg_attr(feature = "verify", contracts::ensures(!ret -> len >= arch::WIDTH as u32))]
#[inline(always)] #[must_use]
fn valid_len(len: u32) -> bool {
    len < arch::WIDTH as u32
}

#[cfg_attr(feature = "verify", contracts::ensures(x == ret as u32))]
#[cfg_attr(feature = "verify", contracts::ensures(ret == x as usize))]
#[inline(always)] #[must_use]
fn u32_as_usize(x: u32) -> usize {
    x as usize
}

/// Post-condition: As long as the preconditions are respected the returned value will always be
/// less than `data.len()`
#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[cfg_attr(feature = "verify", contracts::requires(
    valid_len(len),
    "The length must be below the SIMD register width, it being outside of this range denotes that \
     find operation did not succeed."
))]
#[cfg_attr(feature = "verify", contracts::requires(
    cur >= data.as_ptr(),
    "The `cur` pointer must not have moved backwards beyond the start of `data`"
))]
#[cfg_attr(feature = "verify", contracts::requires(
    u32_as_usize(len) < usize::MAX - distance(cur, data.as_ptr()),
    "The length + the distance from `cur` to `data` must not be able to overflow."
))]
#[cfg_attr(feature = "verify", contracts::requires(
    incr_ptr(simd_ptr(cur)) <= _end.get(),
    "The distance between `cur` and `data` must be less than the data's length subtracted by the \
     SIMD register width."
))]
#[inline(always)] #[must_use]
unsafe fn final_length(len: u32, cur: *const u8, data: &[u8], _end: EndPtr) -> usize {
    let ret = u32_as_usize(len).wrapping_add(distance(cur, data.as_ptr()));
    // Relevant Preconditions:
    //
    // P(1) `incr_ptr(simd_ptr(cur)) <= _end.get()`: Guarantees that the distance between `cur`
    //   and `data.as_ptr()` is less than `arch::WIDTH` as `EndPtr` is an immutable representation
    //   of `data`'s upper bound.
    // P(2) `valid_len(len)`: Guarantees that the `len` is less than `arch::WIDTH`
    //
    // Therefore the sum of `len` and `distance(cur, data.as_ptr)` is guaranteed less than the
    // data's length.
    contract!(assumed_postcondition!(ret < data.len()));
    ret
}

macro_rules! valid_len_then {
    ($len:ident, $do:expr $(, $otherwise:expr)?) => {
        if valid_len($len) {
            // Re-emphasize postcondition of `valid_len`
            contract!(debug_checked_assume!(valid_len($len)));
            $do
        } $( else {
            $otherwise
        })?
    };
}

#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[cfg_attr(feature = "verify", contracts::ensures(ret.is_some() -> ret.unwrap() < data.len()))]
#[inline(always)]
pub unsafe fn search<F: Fn(Vector) -> Vector>(data: &[u8], cond: F) -> Option<usize> {
    let (vector, mut iter) = sealed::init_scan(data);

    let len = arch::MoveMask::new(cond(vector)).trailing_zeros();
    if valid_len(len) { return Some(len as usize); }

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, ptr)) => {
                check_end_ptr!(iter.end, data);
                let len = arch::MoveMask::new(cond(vector)).trailing_zeros();
                valid_len_then!(
                    len,
                    break Some(final_length(len, byte_ptr(ptr), data, iter.end))
                );
            },
            sealed::Pointer::End(Some((vector, ptr))) => {
                check_end_ptr!(iter.end, data);
                let len = arch::MoveMask::new(cond(vector)).trailing_zeros();
                break valid_len_then!(
                    len,
                    Some(final_length(len, byte_ptr(ptr), data, iter.end)),
                    None
                );
            },
            sealed::Pointer::End(None) => {
                check_end_ptr!(iter.end, data);
                break None;
            }
        }
    }
}

#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[inline(always)]
pub unsafe fn for_all_ensure_ct<F: Fn(Vector) -> Vector>(data: &[u8], cond: F, res: &mut bool) {
    let (vector, mut iter) = sealed::init_scan(data);
    *res &= crate::ensure!(vector, cond);

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, _)) => {
                check_end_ptr!(iter.end, data);
                *res &= crate::ensure!(vector, cond);
            },
            sealed::Pointer::End(Some((vector, _))) => {
                check_end_ptr!(iter.end, data);
                *res &= crate::ensure!(vector, cond);
                break;
            },
            sealed::Pointer::End(None) => {
                check_end_ptr!(iter.end, data);
                break;
            }
        }
    }
}

#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[inline(always)] #[must_use]
pub unsafe fn for_all_ensure<F: Fn(Vector) -> Vector>(data: &[u8], cond: F) -> bool {
    let (vector, mut iter) = sealed::init_scan(data);
    if !crate::ensure!(vector, cond) { return false; }

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, _)) => {
                check_end_ptr!(iter.end, data);
                if !crate::ensure!(vector, cond) { break false; }
            },
            sealed::Pointer::End(Some((vector, _))) => {
                check_end_ptr!(iter.end, data);
                break crate::ensure!(vector, cond);
            },
            sealed::Pointer::End(None) => {
                check_end_ptr!(iter.end, data);
                break true;
            }
        }
    }
}

#[cfg(feature = "require")]
#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[inline(always)]
pub unsafe fn ensure_requirements<R: crate::require::Requirement>(data: &[u8], mut req: R) -> R {
    let (vector, mut iter) = sealed::init_scan(data);
    req.check(vector);

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, _)) => {
                check_end_ptr!(iter.end, data);
                req.check(vector);
            },
            sealed::Pointer::End(Some((vector, _))) => {
                check_end_ptr!(iter.end, data);
                req.check(vector);
                break req;
            },
            sealed::Pointer::End(None) => {
                check_end_ptr!(iter.end, data);
                break req;
            }
        }
    }
}
