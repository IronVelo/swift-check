use crate::{arch, find};
use crate::arch::Vector;

#[cfg(debug_assertions)]
macro_rules! ensure_width {
    ($ptr:expr, $end_ptr:ident) => {
        if $ptr.add($crate::arch::STEP) <= $end_ptr {
            $ptr
        } else {
            panic!("Insufficient space between ptr and end at {}:{}", file!(), line!());
        }
    };
}

#[cfg(not(debug_assertions))]
macro_rules! ensure_width {
    ($ptr:expr, $end_ptr:ident) => { $ptr };
}

macro_rules! dbg_assert_ptr_width {
    ($ptr:expr, $end_ptr:expr) => {
        debug_assert!($ptr.add($crate::arch::STEP) <= $end_ptr, "Insufficient space after cur");
    };
}

/// Adjusts the pointer to handle trailing bytes in the input that do not fill the SIMD
/// register width.
///
/// # Returns
///
/// - `Some(ptr)`: Returns a pointer adjusted to handle the remaining bytes unaligned to SIMD
/// boundaries. This pointer points to a position where the distance to `$end` is exactly the
/// register width.
/// - `None`: Indicates that the end of the input has been reached and no adjustment is necessary.
///
/// # Safety
///
/// - The returned pointer does not guarantee alignment suitable for aligned SIMD loads.
/// - The operation requires that it must be safe to decrement the pointer by up to the SIMD
///   register width, implying the entire buffer length must be at least as large as `WIDTH`.
/// - All standard pointer safety invariants must be upheld, ensuring the pointer remains valid,
///   does not cause aliasing issues, and does not overlap with other memory regions being
///   manipulated.
macro_rules! adjust_ptr {
    ($start:ident, $end:ident) => {{
        let c_cast = super::byte_ptr($start);
        match super::byte_ptr($end).offset_from(c_cast) as usize {
            0 => None,
            dist => Some(ensure_width!(super::simd_ptr(c_cast.sub(super::WIDTH - dist)), $end))
        }
    }};
}

macro_rules! align_ptr {
    ($ptr:ident, $expr:expr) => {{
        let byte_ptr = arch::byte_ptr($ptr);
        let offset = byte_ptr.align_offset(arch::WIDTH);
        if offset != 0 {
            // we compute whatever expr, and return the new, now aligned, pointer (which will
            // overlap what we have already computed)
            $expr;
            arch::simd_ptr(byte_ptr.add(offset))
        } else {
            $ptr
        }
    }};
}

/// Compute the distance between `$ptr` from `$data`
///
/// This is used in searching, as it tells you how many bytes have we covered to this point.
///
/// # Safety
///
/// The `$ptr` must be greater than the `data` pointer
macro_rules! dist {
    ($ptr:ident from $data:ident) => {
        arch::byte_ptr($ptr).offset_from($data.as_ptr()) as usize
    };
}

macro_rules! scan_all {
    (
        $data:ident,
        |$ptr:ident| => $do:expr,
        unaligned => $handle_partial:expr
    ) => {{
        debug_assert!($data.len() >= arch::WIDTH);
        let mut $ptr = arch::simd_ptr($data.as_ptr());

        $ptr = align_ptr!($ptr, $handle_partial);

        let __end = arch::simd_ptr($data.as_ptr().add($data.len()));
        while $ptr < __end.sub(arch::STEP) {
            dbg_assert_ptr_width!($ptr, __end);
            $do;
            $ptr = $ptr.add(arch::STEP);
        }
        if let Some($ptr) = adjust_ptr!($ptr, __end) {
            $handle_partial
        }
    }};
}

/// # Safety
///
/// This function makes the assumption that the data is greater than or equal to the SIMD register
/// width. It is the responsibility of the caller to ensure this invariant is upheld.
#[inline(always)]
pub unsafe fn for_all_ensure_ct(data: &[u8], cond: impl Fn(Vector) -> Vector, res: &mut bool) {
    scan_all!(
        data,
        |cur| => *res &= crate::ensure!(super::load_aligned(cur), cond),
        unaligned => *res &= crate::ensure!(super::load_unchecked(cur), cond)
    );
}

/// # Safety
///
/// This function makes the assumption that the data is greater than or equal to the SIMD register
/// width. It is the responsibility of the caller to ensure this invariant is upheld.
#[inline(always)]
pub unsafe fn for_all_ensure(data: &[u8], cond: impl Fn(Vector) -> Vector) -> bool {
    scan_all!(
        data,
        |cur| => if !crate::ensure!(super::load_aligned(cur), cond) { return false },
        unaligned => if !crate::ensure!(super::load_unchecked(cur), cond) { return false }
    );

    true
}

/// # Safety
///
/// This function makes the assumption that the data is greater than or equal to the SIMD register
/// width. It is the responsibility of the caller to ensure this invariant is upheld.
#[inline(always)]
pub unsafe fn search(data: &[u8], cond: impl Fn(Vector) -> Vector) -> Option<usize> {
    debug_assert!(data.len() >= arch::WIDTH);
    let mut cur = arch::simd_ptr(data.as_ptr());

    // align the cur ptr, if we find the true cond in the alignment process return that
    cur = align_ptr!(
        cur,
        if let Some(position) = find!(arch::load_unchecked(cur), cond) {
            return Some(position as usize)
        }
    );

    let end = arch::simd_ptr(data.as_ptr().add(data.len()));

    while cur <= end.sub(arch::STEP) {
        dbg_assert_ptr_width!(cur, end);
        if let Some(position) = find!(arch::load_aligned(cur), cond) {
            return Some(
                position as usize + dist!(cur from data)
            )
        }
        cur = cur.add(arch::STEP);
    }

    adjust_ptr!(cur, end).and_then(|ptr| {
        dbg_assert_ptr_width!(ptr, end);
        find!(arch::load_unchecked(ptr), cond)
            .map(|pos| pos as usize + dist!(ptr from data))
    })
}