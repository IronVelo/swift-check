#![allow(clippy::missing_safety_doc)]

use core::arch::x86_64::{
    __m128i,
    _mm_and_si128, _mm_cmpeq_epi8, _mm_cmpgt_epi8, _mm_cmplt_epi8, _mm_load_si128, _mm_loadu_si128,
    _mm_movemask_epi8, _mm_or_si128, _mm_set1_epi8, _mm_xor_si128, _mm_setzero_si128
};

#[cfg(not(feature = "verify"))]
use mirai_annotations::{precondition};
#[cfg(feature = "verify")]
use mirai_annotations::{checked_precondition};

pub type Vector = __m128i;
pub type Ptr = Vector;
pub const STEP: usize = 1;
pub const STEP_SIZE: usize = 16;
#[repr(transparent)]
pub struct MoveMask(u32);
impl MoveMask {
    pub const MAX_TRAIL: u32 = 32;

    #[inline(always)] #[must_use]
    pub unsafe fn new(input: Vector) -> Self {
        Self(_mm_movemask_epi8(input) as u32)
    }

    #[inline(always)] #[must_use]
    pub const fn all_bits_set(&self) -> bool {
        self.0 == 0xFFFF
    }

    #[inline(always)] #[must_use]
    pub const fn trailing_zeros(&self) -> u32 {
        self.0.trailing_zeros()
    }

    #[inline(always)] #[must_use]
    pub const fn trailing_ones(&self) -> u32 {
        self.0.trailing_ones()
    }
}

#[inline(always)] #[must_use]
pub unsafe fn eq(a: Vector, b: Vector) -> Vector { _mm_cmpeq_epi8(a, b) }

// Bitwise XOR with all bits set to simulate NOT
#[inline(always)] #[must_use]
pub unsafe fn not(a: Vector) -> Vector { xor(a, _mm_set1_epi8(-1)) }

#[inline(always)] #[must_use]
pub unsafe fn xor(a: Vector, b: Vector) -> Vector { _mm_xor_si128(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn or(a: Vector, b: Vector) -> Vector { _mm_or_si128(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn and(a: Vector, b: Vector) -> Vector { _mm_and_si128(a, b) }

// compute via compliment as sse lacks gt eq
#[inline(always)] #[must_use]
pub unsafe fn greater_than_or_eq(a: Vector, b: Vector) -> Vector { not(less_than(a, b)) }

#[inline(always)] #[must_use]
pub unsafe fn greater_than(a: Vector, b: Vector) -> Vector { _mm_cmpgt_epi8(a, b) }

// compute via compliment as sse lacks lt eq
#[inline(always)] #[must_use]
pub unsafe fn less_than_or_eq(a: Vector, b: Vector) -> Vector { not(greater_than(a, b)) }

#[inline(always)] #[must_use]
pub unsafe fn less_than(a: Vector, b: Vector) -> Vector { _mm_cmplt_epi8(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn splat(a: u8) -> Vector { _mm_set1_epi8(a as i8) }

#[inline(always)] #[must_use]
pub unsafe fn load_unchecked(ptr: *const Ptr) -> Vector {
    _mm_loadu_si128(ptr)
}

/// # Safety
///
/// The pointer must be aligned to the register width.
#[inline(always)] #[must_use]
#[contracts::requires(byte_ptr(ptr).align_offset(super::WIDTH) == 0)]
pub unsafe fn load_aligned(ptr: *const Ptr) -> Vector {
    _mm_load_si128(ptr)
}

#[inline(always)] #[must_use]
pub fn load(data: &[u8; 16]) -> Vector {
    let ptr = data.as_ptr();
    if ptr.align_offset(16) == 0 {
        unsafe { load_aligned(ptr.cast()) }
    } else {
        unsafe { load_unchecked(ptr.cast()) }
    }
}

#[inline(always)] #[must_use]
pub const fn byte_ptr(ptr: *const Ptr) -> *const u8 {
    ptr.cast()
}

#[inline(always)] #[must_use]
pub const fn simd_ptr(ptr: *const u8) -> *const Ptr {
    ptr.cast()
}

#[cfg(not(target_feature = "sse4.1"))]
macro_rules! set_sse_lane {
    ($data:ident, $reg:ident, $lane:expr, $count:expr) => {
        if $lane >= $count {
            return $reg;
        }
        $reg = _mm_or_si128(
            core::arch::x86_64::_mm_slli_si128::<{$lane}>(
                _mm_set1_epi8(*$data.as_ptr().add($lane) as i8)
            ),
            core::arch::x86_64::_mm_andnot_si128(
                core::arch::x86_64::_mm_slli_si128::<{$lane}>(splat(0xFF)),
                $reg
            )
        );
    };
}

#[cfg(target_feature = "sse4.1")]
macro_rules! set_sse_lane {
    ($data:ident, $reg:ident, $lane:expr, $count:expr) => {
        if $lane >= $count {
            return $reg;
        }
        $reg = core::arch::x86_64::_mm_insert_epi8::<{$lane}>($reg, *$data.as_ptr().add($lane) as i32);
    };
}

macro_rules! set_4_lanes {
    ($data:ident, $reg:ident, $start_lane:literal, $count:expr) => {{
        set_sse_lane!($data, $reg, $start_lane, $count);
        set_sse_lane!($data, $reg, $start_lane + 1, $count);
        set_sse_lane!($data, $reg, $start_lane + 2, $count);
        set_sse_lane!($data, $reg, $start_lane + 3, $count);
    }};
}

/// Load under 16 bytes into a SIMD register
///
/// This initializes the register with zeroes, and on sse4.1 it sets however many bytes were passed
/// (max 16), for sse2 it uses bitwise operations (slower)
///
/// # Performance
///
/// This is of course significantly slower than `load` or `load_unchecked`.
/// With sse4.1 available it is around 45% faster. On ARM, it is significantly more efficient, if
/// AVX support comes around that would be most efficient.
///
/// # Safety
///
/// If the count is greater than the data's length you'll CVE 125 yourself.
#[inline] #[must_use]
pub unsafe fn load_partial(data: &[u8], count: usize) -> Vector {
    debug_assert_eq!(data.len(), count);
    debug_assert!(count <= 16);

    let mut reg = _mm_setzero_si128(); // Create a register filled with zeros

    // isolate each lane and add our byte
    set_4_lanes!(data, reg, 0, count);
    set_4_lanes!(data, reg, 4, count);
    set_4_lanes!(data, reg, 8, count);
    set_4_lanes!(data, reg, 12, count);

    reg
}