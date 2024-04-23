use crate::arch::{self, byte_ptr, simd_ptr, Vector};
use mirai_annotations as contract;
use crate::arch::is_aligned;

#[allow(unused_imports)]
use mirai_annotations::{
    checked_precondition, checked_postcondition,
    precondition, postcondition
};


#[contracts::requires(x >= 0)]
#[contracts::ensures(x as usize == ret)]
#[inline(always)]
unsafe fn remove_sign(x: isize) -> usize {
    x as usize
}

#[contracts::requires(l >= r)]
#[inline(always)]
unsafe fn offset_from(l: *const u8, r: *const u8) -> isize {
    let ret = l.offset_from(r);
    // `l` being greater than 'r' is a precondition, therefore the offset will always be positive.
    contract!(assumed_postcondition!(ret >= 0));
    ret
}

#[contracts::requires(l >= r)]
#[contracts::ensures(ret == remove_sign(offset_from(l, r)))]
#[inline(always)]
unsafe fn distance(l: *const u8, r: *const u8) -> usize {
    remove_sign(offset_from(l, r))
}

#[contracts::requires(dist <= arch::WIDTH)]
#[contracts::ensures(
    dist == distance(byte_ptr(_end), cur) -> incr_ptr(simd_ptr(ret)) == _end,
    "If `dist` is the byte offset of `_end` to `cur` then incr_ptr(simd_ptr(ret)) equates to \
    `_end`"
)]
#[inline(always)]
unsafe fn make_space(cur: *const u8, dist: usize, _end: *const arch::Ptr) -> *const u8 {
    cur.sub(arch::WIDTH - dist)
}

#[contracts::requires(
    end >= cur,
    "The `end` ptr must be greater than or equal to `cur`"
)]
#[contracts::requires(
    cur >= decr_ptr(end) && distance(byte_ptr(end), byte_ptr(cur)) <= arch::WIDTH,
    "The distance between `end` and `cur` must be less than arch::WIDTH"
)]
#[contracts::ensures(
    distance(byte_ptr(end), byte_ptr(cur)) != 0 -> incr_ptr(ret.unwrap()) == end,
    "If there is distance between `end` and `cur` this will always return Some"
)]
#[contracts::ensures(
    ret.is_some() -> incr_ptr(ret.unwrap()) == end,
    "If Some is returned the wrapped ptr will always have a distance of arch::WIDTH to `end` \
    (no assurance of alignment)"
)]
#[inline(always)]
unsafe fn adjust_ptr(cur: *const arch::Ptr, end: *const arch::Ptr) -> Option<*const arch::Ptr> {
    match distance(byte_ptr(end), byte_ptr(cur)) {
        0 => None,
        dist => {
            // The first precondition states that the distance between byte_ptr(end) and
            // byte_ptr(cur) must be less than or equal to arch::WIDTH.
            contract!(checked_assume!(dist <= arch::WIDTH));
            Some(simd_ptr(make_space(byte_ptr(cur), dist, end)))
        }
    }
}

#[contracts::ensures(ptr == decr_ptr(ret))]
#[contracts::ensures(ptr.add(arch::STEP) == ret)]
#[contracts::ensures(byte_ptr(decr_ptr(ret)) == byte_ptr(ptr))]
#[contracts::ensures(is_aligned(ptr) -> is_aligned(ret))]
#[inline(always)]
unsafe fn incr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.add(arch::STEP)
}

#[contracts::ensures(incr_ptr(ret) == ptr)]
#[contracts::ensures(is_aligned(ptr) -> is_aligned(ret))]
#[inline(always)]
unsafe fn decr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.sub(arch::STEP)
}

#[contracts::ensures(
    ptr.align_offset(arch::WIDTH) != 0
        -> ret.cast::<u8>().offset_from(ptr) as usize == ptr.align_offset(arch::WIDTH)
)]
#[contracts::ensures(
    ptr.align_offset(arch::WIDTH) == 0 -> ret == incr_ptr(simd_ptr(ptr))
)]
#[inline(always)]
unsafe fn align_ptr_or_incr(ptr: *const u8) -> *const arch::Ptr {
    let offset = ptr.align_offset(arch::WIDTH);
    if offset == 0 {
        // When the pointer is already aligned, increment it by `arch::STEP`
        incr_ptr(simd_ptr(ptr))
    } else {
        // When the pointer is not aligned, adjust it to the next alignment boundary
        simd_ptr(ptr.add(offset))
    }
}

#[contracts::ensures(is_aligned(cur) -> is_aligned(cur))]
#[contracts::ensures(incr_ptr(cur) <= end -> true)]
#[inline(always)]
unsafe fn can_proceed(cur: *const arch::Ptr, end: *const arch::Ptr) -> bool {
    cur <= decr_ptr(end)
}

#[contracts::ensures(x == ret as u32)]
#[contracts::ensures(ret == x as usize)]
#[inline(always)]
fn cast_usize(x: u32) -> usize {
    x as usize
}

#[contracts::ensures(ret -> len < arch::WIDTH as u32)]
#[contracts::ensures(!ret -> len >= arch::WIDTH as u32)]
#[inline(always)]
fn valid_len(len: u32) -> bool {
    len < arch::WIDTH as u32
}

/// Post-condition: As long as the preconditions are respected the returned value will always be
/// less than `data.len()`
#[contracts::requires(data.len() >= arch::WIDTH)]
#[contracts::requires(
    valid_len(len),
    "The length must be below the SIMD register width, it being outside of this range denotes that \
     find operation did not succeed."
)]
#[contracts::requires(
    cur >= data.as_ptr(),
    "The `cur` pointer must not have moved backwards beyond the start of `data`"
)]
#[contracts::requires(
    cast_usize(len) < usize::MAX - distance(cur, data.as_ptr()),
    "The length + the distance from `cur` to `data` must not be able to overflow."
)]
#[contracts::requires(
    distance(cur, data.as_ptr()) < data.len() - arch::WIDTH,
    "The distance between `cur` and `data` must be less than the data's length subtracted by the \
     SIMD register width."
)]
#[inline(always)]
unsafe fn final_length(len: u32, cur: *const u8, data: &[u8]) -> usize {
    // Let:
    // - `len` be a 32-bit unsigned integer,
    // - `cur` be a pointer to a byte within `data`,
    // - `data` be a slice of bytes.
    //
    // Preconditions:
    // (P1) `data.len() >= arch::WIDTH`: Ensures sufficient length of `data` to safely perform SIMD
    //      operations.
    // (P2) `valid_len(len)`: Requires `len` to be less than the width of a SIMD register, signaling
    //      a successful find operation.
    // (P3) `cur >= data.as_ptr()`: Guarantees that the pointer `cur` does not retrogress beyond the
    //      starting point of `data`.
    // (P4) `cast_usize(len) + distance(cur, data.as_ptr()) < usize::MAX`: Prevents integer overflow
    //      by ensuring the sum of `len` and the byte offset from `cur` to `data` start is within
    //      usize limits.
    // (P5) `distance(cur, data.as_ptr()) < data.len() - arch::WIDTH`: Confirms that the offset from
    //      `cur` to `data` plus the SIMD register width remains within the bounds of `data`.
    //
    // Postconditions:
    // (Q1) `ret < data.len()`: Asserts that the computed result `ret` remains strictly less than
    //      the total length of `data`, preventing out-of-bounds indexing in further operations.
    //
    // Argument for Q1:
    // Given P(2) and P(5), we have:
    // - P(2) asserts that `len` is strictly less than `arch::WIDTH`.
    // - P(5) asserts that the offset from `cur` to `data` plus `arch::WIDTH` is less than
    //        `data.len()`.
    // Therefore, the sum of `len` and the distance from `cur` to `data` is guaranteed to be less
    // than `data.len()`, as:
    // `len` + distance <= `arch::WIDTH` + (data.len() - `arch::WIDTH`) = data.len().
    //
    // This function utilizes `wrapping_add` for adding `len` to the byte offset, under the
    // assumption derived from the preconditions that no inappropriate overflow will occur.

    let ret = cast_usize(len).wrapping_add(distance(cur, data.as_ptr()));
    // See argument for Q1 in function commentary.
    contract!(assumed_postcondition!(ret < data.len()));
    ret
}

macro_rules! valid_len_then {
    ($len:ident, $do:expr $(, $otherwise:expr)?) => {
        if valid_len($len) {
            // Re-emphasize postcondition of `valid_len`
            contract::debug_checked_assume!(valid_len($len));
            $do
        } $( else {
            $otherwise
        })?
    };
}

#[contracts::requires(data.len() >= arch::WIDTH)]
#[contracts::ensures(ret.is_some() -> ret.unwrap() < data.len())]
#[inline(always)]
pub unsafe fn search<F: Fn(Vector) -> Vector>(data: &[u8], cond: F) -> Option<usize> {
    let len = arch::MoveMask::new(cond(arch::load_unchecked(simd_ptr(data.as_ptr()))))
        .trailing_zeros();
    if valid_len(len) {
        // Due to the precondition we know len is in bounds.
        return Some(len as usize);
    }

    // We have already checked the first arch::WIDTH of data. Rather than just continuing we align
    // the current pointer. If it was unaligned we will search an overlapping space as the initial
    // check.
    let mut cur  = align_ptr_or_incr(data.as_ptr());
    let end = simd_ptr(data.as_ptr().add(data.len()));

    while can_proceed(cur, end) {
        mirai_annotations::debug_checked_verify!(is_aligned(cur));
        // We aligned `cur` at `align_ptr_or_incr`, according to the postconditions of
        // `can_proceed` and `incr_ptr` we will remain aligned until `can_proceed` yields false.
        let len = arch::MoveMask::new(cond(arch::load_aligned(cur))).trailing_zeros();

        valid_len_then!(
            len, return Some(final_length(len, byte_ptr(cur), data))
        );

        cur = incr_ptr(cur);
    }

    if let Some(ptr) = adjust_ptr(cur, end) {
        // According to the postconditions of `adjust_ptr` when the result is `Some` the wrapped
        // pointer is exactly `arch::WIDTH` from `end`. We can make no assumption regarding the
        // alignment of the pointer, use an unaligned load.

        let len = arch::MoveMask::new(cond(arch::load_unchecked(ptr))).trailing_zeros();

        valid_len_then!(
            len,
            Some(final_length(len, byte_ptr(ptr), data)),
            None
        )
    } else {
        None
    }
}

#[contracts::requires(data.len() >= arch::WIDTH)]
#[inline(always)]
pub unsafe fn for_all_ensure_ct(data: &[u8], cond: impl Fn(Vector) -> Vector, res: &mut bool) {
    *res &= crate::ensure!(super::load_unchecked(simd_ptr(data.as_ptr())), cond);

    // Align the pointer if it is not already, if it was already aligned it `incr_ptr`'s as we have
    // already checked this space. If the pointer was unaligned we will search over the same space
    // again.
    let mut cur = align_ptr_or_incr(data.as_ptr());
    let end = simd_ptr(data.as_ptr().add(data.len()));

    while can_proceed(cur, end) {
        mirai_annotations::debug_checked_verify!(is_aligned(cur));
        // Outside the loop we ensured `cur` is aligned at this point. According to the post
        // condition of `incr_ptr` if the pointer being incremented was initially aligned to the
        // register width, it will retain this alignment. Therefore, we know it is safe to do an
        // aligned load. `cur_proceed` also ensures that we are at least arch::WIDTH from `end`.

        *res &= crate::ensure!(arch::load_aligned(cur), cond);
        cur = incr_ptr(cur);
    }

    // There was less than arch::WIDTH from `cur` to `end`, we do not know that we have reached end
    // of input. If we have not reached the end, adjust the pointer so that the distance from `cur`
    // to `end` is exactly arch::WIDTH

    if let Some(ptr) = adjust_ptr(cur, end) {
        // We know the distance from `cur` to `end` is exactly arch::WIDTH due to `adjust_ptr`'s
        // postcondition. We have no assurance of alignment, so use an unaligned load.
        *res &= crate::ensure!(arch::load_unchecked(ptr), cond);
    }
}

#[contracts::requires(data.len() >= arch::WIDTH)]
#[inline(always)]
pub unsafe fn for_all_ensure(data: &[u8], cond: impl Fn(Vector) -> Vector) -> bool {
    if !crate::ensure!(arch::load_unchecked(simd_ptr(data.as_ptr())), cond) {
        return false;
    }

    // Align the pointer if it is not already, if it was already aligned it `incr_ptr`'s as we have
    // already checked this space. If the pointer was unaligned we will search over the same space
    // again.
    let mut cur = align_ptr_or_incr(data.as_ptr());
    let end = simd_ptr(data.as_ptr().add(data.len()));

    while can_proceed(cur, end) {
        mirai_annotations::debug_checked_verify!(is_aligned(cur));

        // Outside the loop we ensured `cur` is aligned at this point. According to the post
        // condition of `incr_ptr` if the pointer being incremented was initially aligned to the
        // register width, it will retain this alignment. Therefore, we know it is safe to do an
        // aligned load. `cur_proceed` also ensures that we are at least arch::WIDTH from `end`.

        if !crate::ensure!(arch::load_aligned(cur), cond) {
            return false;
        }
        cur = incr_ptr(cur);
    }

    // There was less than arch::WIDTH from `cur` to `end`, we do not know that we have reached end
    // of input. If we have not reached the end, adjust the pointer so that the distance from `cur`
    // to `end` is exactly arch::WIDTH

    if let Some(ptr) = adjust_ptr(cur, end) {
        // We know the distance from `cur` to `end` is exactly arch::WIDTH due to `adjust_ptr`'s
        // postcondition. We have no assurance of alignment, so use an unaligned load.
        crate::ensure!(arch::load_unchecked(ptr), cond)
    } else {
        true
    }
}
