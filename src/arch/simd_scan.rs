use crate::arch::{self, byte_ptr, simd_ptr, Vector};

cfg_verify!(
    use crate::arch::is_aligned;
    use mirai_annotations::{checked_precondition, checked_postcondition};
);

use crate::arch::scan::end_ptr::EndPtr;

#[cfg_attr(feature = "verify", contracts::ensures(x >= 0 -> x as usize == ret))]
#[inline(always)]
unsafe fn remove_sign(x: isize) -> usize {
    x as usize
}

#[cfg_attr(feature = "verify", contracts::requires(l >= r))]
#[inline(always)]
unsafe fn offset_from(l: *const u8, r: *const u8) -> isize {
    let ret = l.offset_from(r);
    // `l` being greater than 'r' is a precondition, therefore the offset will always be positive.
    contract!(assumed_postcondition!(ret >= 0));
    ret
}

#[cfg_attr(feature = "verify", contracts::requires(l >= r))]
#[cfg_attr(feature = "verify", contracts::ensures(ret == remove_sign(offset_from(l, r))))]
#[inline(always)]
unsafe fn distance(l: *const u8, r: *const u8) -> usize {
    remove_sign(offset_from(l, r))
}

#[cfg_attr(feature = "verify", contracts::requires(l >= r))]
#[cfg_attr(feature = "verify", contracts::requires(byte_ptr(l) >= byte_ptr(r)))]
#[cfg_attr(feature = "verify", contracts::ensures(ret == distance(byte_ptr(l), byte_ptr(r))))]
#[inline(always)]
unsafe fn simd_distance(l: *const arch::Ptr, r: *const arch::Ptr) -> usize {
    distance(byte_ptr(l), byte_ptr(r))
}

#[cfg_attr(feature = "verify", contracts::requires(dist <= arch::WIDTH))]
#[cfg_attr(feature = "verify", contracts::ensures(
    dist == distance(byte_ptr(_end), cur) -> incr_ptr(simd_ptr(ret)) == _end,
    "If `dist` is the byte offset of `_end` to `cur` then incr_ptr(simd_ptr(ret)) equates to \
    `_end`"
))]
#[inline(always)]
unsafe fn make_space(cur: *const u8, dist: usize, _end: *const arch::Ptr) -> *const u8 {
    cur.sub(arch::WIDTH - dist)
}

#[cfg_attr(feature = "verify", contracts::ensures(ptr == decr_ptr(ret)))]
#[cfg_attr(feature = "verify", contracts::ensures(ptr.add(arch::STEP) == ret))]
#[cfg_attr(feature = "verify", contracts::ensures(byte_ptr(decr_ptr(ret)) == byte_ptr(ptr)))]
#[cfg_attr(feature = "verify", contracts::ensures(is_aligned(ptr) -> is_aligned(ret)))]
#[inline(always)]
unsafe fn incr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.add(arch::STEP)
}

#[cfg_attr(feature = "verify", contracts::ensures(incr_ptr(ret) == ptr))]
#[cfg_attr(feature = "verify", contracts::ensures(is_aligned(ptr) -> is_aligned(ret)))]
#[inline(always)]
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
    let offset = ptr.align_offset(arch::WIDTH);
    if offset == 0 {
        let simd_ptr = simd_ptr(ptr);
        // When the pointer is already aligned, increment it by `arch::STEP`
        (arch::load_aligned(simd_ptr), incr_ptr(simd_ptr))
    } else {
        // When the pointer is not aligned, adjust it to the next alignment boundary
        (arch::load_unchecked(simd_ptr(ptr)), simd_ptr(ptr.add(offset)))
    }
}

#[cfg_attr(feature = "verify", contracts::requires(end >= cur))]
#[cfg_attr(feature = "verify", contracts::ensures(is_aligned(cur) -> is_aligned(cur)))]
#[cfg_attr(feature = "verify", contracts::ensures(ret -> incr_ptr(cur) <= end))]
#[inline(always)]
unsafe fn can_proceed(cur: *const arch::Ptr, end: *const arch::Ptr) -> bool {
    cur <= decr_ptr(end)
}

#[cfg_attr(feature = "verify", contracts::ensures(x == ret as u32))]
#[cfg_attr(feature = "verify", contracts::ensures(ret == x as usize))]
#[inline(always)]
fn u32_as_usize(x: u32) -> usize {
    x as usize
}

#[cfg_attr(feature = "verify", contracts::ensures(ret -> len < arch::WIDTH as u32))]
#[cfg_attr(feature = "verify", contracts::ensures(!ret -> len >= arch::WIDTH as u32))]
#[inline(always)]
fn valid_len(len: u32) -> bool {
    len < arch::WIDTH as u32
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
#[inline(always)]
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

mod end_ptr {
    use super::*;

    #[repr(transparent)]
    pub struct EndPtr(*const arch::Ptr);

    impl EndPtr {
        #[inline(always)]
        pub const fn new(end: *const arch::Ptr) -> Self {
            Self (end)
        }
    }

    impl EndPtr {
        #[cfg_attr(feature = "verify", contracts::ensures(self.0 == ret))]
        #[inline(always)]
        pub fn get(&self) -> *const arch::Ptr {
            self.0
        }
    }
}

mod sealed {
    use super::*;
    #[cfg(feature = "verify")]
    use contracts::invariant;
    use crate::arch::scan::end_ptr::EndPtr;

    pub struct Scanner;

    impl Scanner {
        #[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
        #[must_use] #[inline(always)]
        pub unsafe fn new(data: &[u8]) -> (Vector, AlignedIter) {
            let (vector, aligned_ptr) = align_ptr_or_incr(data.as_ptr());
            (
                vector,
                AlignedIter::after_first(aligned_ptr, data)
            )
        }
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
        pub const fn is_aligned(&self) -> bool {
            matches!(self, Self::Aligned(_))
        }
        pub const fn is_end_with_remaining(&self) -> bool {
            matches!(self, Self::End(Some(_)))
        }
        /// Only to be used in contracts
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
        /// Create an `AlignedIter`
        ///
        /// You must have already checked the initial `arch::WIDTH` of data as this will skip over
        /// part of it in the alignment process.
        #[inline(always)] #[must_use]
        unsafe fn after_first(aligned_ptr: *const arch::Ptr, data: &[u8]) -> Self {
            Self {
                cur: aligned_ptr,
                end: EndPtr::new(simd_ptr(data.as_ptr().add(data.len()))),
            }
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
        #[inline(always)]
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
                        byte_ptr(self.cur), dist, self.end.get()
                    ));
                    Some((arch::load_unchecked(ptr), ptr))
                }
            }
        }
    }
}

#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[cfg_attr(feature = "verify", contracts::ensures(ret.is_some() -> ret.unwrap() < data.len()))]
#[inline(always)]
pub unsafe fn search<F: Fn(Vector) -> Vector>(data: &[u8], cond: F) -> Option<usize> {
    let (first, mut iter) = sealed::Scanner::new(data);

    let len = arch::MoveMask::new(cond(first)).trailing_zeros();
    if valid_len(len) { return Some(len as usize); }

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, ptr)) => {
                let len = arch::MoveMask::new(cond(vector)).trailing_zeros();
                valid_len_then!(
                    len,
                    break Some(final_length(len, byte_ptr(ptr), data, iter.end))
                );
            },
            sealed::Pointer::End(Some((vector, ptr))) => {
                let len = arch::MoveMask::new(cond(vector)).trailing_zeros();
                break valid_len_then!(
                    len,
                    Some(final_length(len, byte_ptr(ptr), data, iter.end)),
                    None
                );
            },
            sealed::Pointer::End(None) => { break None; }
        }
    }
}

#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[inline(always)]
pub unsafe fn for_all_ensure_ct<F: Fn(Vector) -> Vector>(data: &[u8], cond: F, res: &mut bool) {
    let (vector, mut iter) = sealed::Scanner::new(data);
    *res &= crate::ensure!(vector, cond);

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, _)) => {
                *res &= crate::ensure!(vector, cond);
            },
            sealed::Pointer::End(Some((vector, _))) => {
                *res &= crate::ensure!(vector, cond);
                break;
            },
            sealed::Pointer::End(None) => { break; }
        }
    }
}

#[cfg_attr(feature = "verify", contracts::requires(data.len() >= arch::WIDTH))]
#[inline(always)]
pub unsafe fn for_all_ensure<F: Fn(Vector) -> Vector>(data: &[u8], cond: F) -> bool {
    let (vector, mut iter) = sealed::Scanner::new(data);
    if !crate::ensure!(vector, cond) { return false; }

    loop {
        match iter.next() {
            sealed::Pointer::Aligned((vector, _)) => {
                if !crate::ensure!(vector, cond) { break false; }
            },
            sealed::Pointer::End(Some((vector, _))) => {
                break crate::ensure!(vector, cond);
            },
            sealed::Pointer::End(None) => { break true; }
        }
    }
}
