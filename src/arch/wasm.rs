use core::arch::wasm32::*;

#[cfg(not(feature = "experimental"))]
compile_error!(
    "WASM SIMD128 has not received adequate testing / work, and may not even compile. To use this \
    you must enable the `experimental` feature flag."
);
pub struct MoveMask(u64);
pub type Vector = v128;

pub type Ptr = Vector;
pub const STEP: usize = 1;
pub const STEP_SIZE: usize = 16;

impl MoveMask {
    pub const MAX_TRAIL: u32 = 32;
    #[inline(always)] #[must_use]
    pub unsafe fn new(input: v128) -> Self {
        let mask = i8x16_shr(i16x8_shl(input, 7), 15);

        let packed_bits = i64x2_shr(mask, 7);
        let scalar64 = i64x2_extract_lane::<0>(packed_bits) as u64;

        Self(scalar64 & 0x8888888888888888)
    }

    #[inline(always)] #[must_use]
    pub const fn all_bits_set(&self) -> bool {
        self.0 == 0x8888888888888888
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
pub unsafe fn eq(a: Vector, b: Vector) -> Vector { u8x16_eq(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn not(a: Vector) -> Vector { v128_not(a) }

#[inline(always)] #[must_use]
pub unsafe fn xor(a: Vector, b: Vector) -> Vector { v128_xor(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn or(a: Vector, b: Vector) -> Vector { v128_or(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn and(a: Vector, b: Vector) -> Vector { v128_and(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn greater_than_or_eq(a: Vector, b: Vector) -> Vector { f32x4_ge(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn greater_than(a: Vector, b: Vector) -> Vector { f32x4_gt(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn less_than_or_eq(a: Vector, b: Vector) -> Vector { f32x4_le(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn less_than(a: Vector, b: Vector) -> Vector { f32x4_lt(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn splat(a: u8) -> Vector { i8x16_splat(a as i8) }

#[inline(always)] #[must_use]
pub unsafe fn load_unchecked(data: *const Ptr) -> Vector {
    v128_load(data)
}

#[inline(always)] #[must_use]
pub unsafe fn load_aligned(ptr: *const Ptr) -> Vector { load_unchecked(ptr) }

#[inline(always)] #[must_use]
pub unsafe fn maybe_aligned_load(ptr: *const u8) -> Vector { load_unchecked(ptr) }

#[inline(always)] #[must_use]
pub fn load(data: &[u8; 16]) -> Vector {
    unsafe {
        // Directly load the data as a SIMD vector.
        // WebAssembly handles alignment at the virtual machine level,
        // so manually checking alignment and choosing between aligned
        // and unaligned loads is typically not necessary.
        load_unchecked(simd_ptr(data.as_ptr()))
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

macro_rules! set_lane {
    ($data:ident, $reg:ident, $lane:expr, $count:expr) => {
        if $lane >= $count {
            return $reg;
        }
        $reg = v128_load8_lane::<{$lane}>($reg, $data.as_ptr().add($lane))
    };
}

macro_rules! set_4_lanes {
    ($data:ident, $reg:ident, $start_lane:expr, $count:expr) => {{
        set_lane!($data, $reg, $start_lane, $count);
        set_lane!($data, $reg, $start_lane + 1, $count);
        set_lane!($data, $reg, $start_lane + 2, $count);
        set_lane!($data, $reg, $start_lane + 3, $count);
    }};
}

#[inline]
pub unsafe fn load_partial(data: &[u8], count: usize) -> Vector {
    let mut reg = splat(0);

    set_4_lanes!(data, reg, 0, count);
    set_4_lanes!(data, reg, 4, count);
    set_4_lanes!(data, reg, 8, count);
    set_4_lanes!(data, reg, 12, count);

    reg
}