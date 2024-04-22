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

#[must_use]
pub unsafe fn hamming_weight(v: Vector) -> u32 {
    // Constants for bit-count reduction
    let mask1 = i8x16_splat(0x55); // 01010101
    let mask2 = i8x16_splat(0x33); // 00110011
    let mask4 = i8x16_splat(0x0F); // 00001111

    // Perform parallel bit count reduction
    let mut tmp = v128_and(v, mask1);
    tmp = v128_add(tmp, v128_and(v128_shr_u8(v, 1), mask1));
    tmp = v128_and(tmp, mask2);
    tmp = v128_add(tmp, v128_and(v128_shr_u8(tmp, 2), mask2));
    tmp = v128_and(tmp, mask4);
    tmp = v128_add(tmp, v128_shr_u8(tmp, 4));
    // Now each byte of tmp contains the number of bits set in the corresponding byte of the
    // original vector

    // Sum all bytes to get the total number of set bits
    // Horizontal add and extract the final sum, adjust the operations as per available intrinsics
    let sums = i8x16_shl(tmp, 4); // Shift to position for addition
    let result = i8x16_add(sums, tmp); // Horizontal add

    // Extract the sum from the first lane
    let scalar_sum = i8x16_extract_lane::<0>(result) as u32;

    scalar_sum
}

#[inline(always)] #[must_use]
pub unsafe fn load_unchecked(data: *const Ptr) -> Vector {
    v128_load(data)
}

#[inline(always)] #[must_use]
pub fn load(data: &[u8; 16]) -> Vector {
    unsafe {
        // Directly load the data as a SIMD vector.
        // WebAssembly handles alignment at the virtual machine level,
        // so manually checking alignment and choosing between aligned
        // and unaligned loads is typically not necessary.
        load_unchecked(data.as_ptr().cast())
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