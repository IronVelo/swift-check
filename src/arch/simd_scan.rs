use crate::{arch};
use crate::arch::{byte_ptr, simd_ptr, Vector};
use mirai_annotations as contract;
#[cfg(not(feature = "verify"))]
use mirai_annotations::{precondition, postcondition};
#[cfg(feature = "verify")]
use mirai_annotations::{checked_precondition, checked_postcondition};

macro_rules! verify_ptr_width {
    ($ptr:expr, $end_ptr:expr) => {
        contract::debug_checked_verify!(
            incr_ptr($ptr) <= $end_ptr, "Insufficient space after cur"
        );
    };
}

#[contracts::requires(x >= 0)]
#[contracts::ensures(x as usize == ret)]
#[inline(always)]
fn remove_sign(x: isize) -> usize {
    x as usize
}

#[contracts::requires(l >= r)]
#[inline(always)]
unsafe fn offset_from(l: *const u8, r: *const u8) -> isize {
    let ret = l.offset_from(r);
    // `l` being greater than 'r' is a precondition, therefore the offset will always be positive.
    contract::assumed_postcondition!(ret >= 0);
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
    "If `dist` is the byte offset of `_end` to `cur` then incr_ptr(simd_ptr(ret)) equates to `_end`"
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
            contract::checked_assume!(dist <= arch::WIDTH);
            Some(simd_ptr(make_space(byte_ptr(cur), dist, end)))
        }
    }
}

#[contracts::ensures(ptr == decr_ptr(ret))]
#[contracts::ensures(ptr.add(arch::STEP) == ret)]
#[contracts::ensures(byte_ptr(decr_ptr(ret)) == byte_ptr(ptr))]
#[contracts::ensures(ptr.align_offset(arch::WIDTH) == 0 -> ret.align_offset(arch::WIDTH) == 0)]
#[inline(always)]
unsafe fn incr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.add(arch::STEP)
}

#[contracts::ensures(incr_ptr(ret) == ptr)]
#[contracts::ensures(ptr.align_offset(arch::WIDTH) == 0 -> ret.align_offset(arch::WIDTH) == 0)]
#[inline(always)]
unsafe fn decr_ptr(ptr: *const arch::Ptr) -> *const arch::Ptr {
    ptr.sub(arch::STEP)
}

#[contracts::ensures(
    ptr.align_offset(arch::WIDTH) != 0
        -> ret.cast::<u8>().offset_from(ptr) as usize == ptr.align_offset(arch::WIDTH)
)]
#[contracts::ensures(
    old(ptr).align_offset(arch::WIDTH) == 0 -> ret == incr_ptr(simd_ptr(ptr))
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

#[contracts::ensures(cur.align_offset(arch::WIDTH) == 0 -> cur.align_offset(arch::WIDTH) == 0)]
#[contracts::ensures(incr_ptr(cur) <= end -> true)]
#[inline(always)]
unsafe fn can_proceed(cur: *const arch::Ptr, end: *const arch::Ptr) -> bool {
    cur <= end.sub(arch::STEP)
}

macro_rules! scan_all {
    (
        $data:ident,
        |$ptr:ident| => $do:expr,
        unaligned => $handle_partial:expr
    ) => {{
        contract::precondition!($data.len() >= 16);

        let mut $ptr = arch::simd_ptr($data.as_ptr());
        $handle_partial;

        $ptr = align_ptr_or_incr($data.as_ptr());

        let __end = arch::simd_ptr($data.as_ptr().add($data.len()));

        while can_proceed($ptr, __end) {
            verify_ptr_width!($ptr, __end);
            $do;
            $ptr = incr_ptr($ptr);
        }
        if let Some($ptr) = adjust_ptr($ptr, __end) {
            $handle_partial
        }
    }};
}

#[contracts::requires(data.len() >= arch::WIDTH)]
#[inline(always)]
pub unsafe fn for_all_ensure_ct(data: &[u8], cond: impl Fn(Vector) -> Vector, res: &mut bool) {
    scan_all!(
        data,
        |cur| => *res &= crate::ensure!(super::load_aligned(cur), cond),
        unaligned => *res &= crate::ensure!(super::load_unchecked(cur), cond)
    );
}

#[contracts::requires(data.len() >= arch::WIDTH)]
#[inline(always)]
pub unsafe fn for_all_ensure(data: &[u8], cond: impl Fn(Vector) -> Vector) -> bool {
    scan_all!(
        data,
        |cur| => if !crate::ensure!(super::load_aligned(cur), cond) { return false },
        unaligned => if !crate::ensure!(super::load_unchecked(cur), cond) { return false }
    );

    true
}

#[contracts::requires(data.len() >= arch::WIDTH)]
#[inline(always)]
pub unsafe fn search(data: &[u8], cond: impl Fn(Vector) -> Vector) -> Option<usize> {
    let len = arch::MoveMask::new(cond(arch::load_unchecked(simd_ptr(data.as_ptr()))))
        .trailing_zeros();
    if len < arch::WIDTH as u32 && len < data.len() as u32 {
        let ret = Some(len as usize);
        contract::assumed_postcondition!(ret.unwrap() < data.len());
        return ret;
    } else if data.len() == arch::WIDTH {
        return None;
    }

    // We have already checked the first arch::WIDTH of data. Rather than just continuing we align
    // the current pointer. If it was unaligned we will search an overlapping space as the initial
    // check.
    let mut cur  = align_ptr_or_incr(data.as_ptr());
    let end = simd_ptr(data.as_ptr().add(data.len()));

    while can_proceed(cur, end) {
        // We aligned `cur` at `align_ptr_or_incr`, according to the post conditions of
        // `can_proceed` and `incr_ptr` we will remain aligned until `can_proceed` yields false.
        let len = arch::MoveMask::new(cond(arch::load_aligned(cur))).trailing_zeros();

        if len < arch::WIDTH as u32 {
            // The first postcondition of `can_proceed` states that if true (meaning we are here)
            // then there is at least arch::WIDTH from `cur` to `end`, meaning that as long as
            // len is less than arch::WIDTH adding the length with distance from `cur` to `data`
            // will always be in bounds.
            let ret = Some(len as usize + distance(byte_ptr(cur), data.as_ptr()));
            contract::assumed_postcondition!(ret.unwrap() < data.len());
            return ret;
        }

        cur = incr_ptr(cur);
    }

    adjust_ptr(cur, end).and_then(|ptr| {
        verify_ptr_width!(ptr, end);

        let len = arch::MoveMask::new(cond(arch::load_unchecked(ptr))).trailing_zeros();
        if len < arch::WIDTH as u32 {
            // According to the postconditions of `adjust_ptr` if Some is returned the wrapped
            // pointer is exactly arch::WIDTH from `end`. Therefore, as long as the `len` is below
            // arch::WIDTH the sum of the `len` with the distance from `ptr` to `data` will always
            // be in bounds.
            let ret = Some(len as usize + distance(byte_ptr(ptr), data.as_ptr()));
            contract::assumed_postcondition!(
                match ret {
                    Some(d) if d < data.len() => true,
                    Some(_) => false,
                    _ => true
                }
            );
            ret
        } else {
            None
        }
    })
}