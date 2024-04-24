#![allow(clippy::missing_safety_doc)]

use core::arch::aarch64::{
    uint8x16_t,
    vandq_u8, vceqq_u8, vcgeq_u8, vcgtq_u8, vcleq_u8, vcltq_u8, vdupq_n_u8, veorq_u8, vget_lane_u64,
    vld1q_u8, vmvnq_u8, vorrq_u8, vreinterpret_u64_u8, vreinterpretq_u16_u8, vshrn_n_u16
};
use core::arch::aarch64::vld1q_lane_u8;

pub type Vector = uint8x16_t;
pub type Ptr = u8;
pub const STEP: usize = 16;
pub const STEP_SIZE: usize = 1;

#[repr(transparent)]
pub struct MoveMask(u64);

impl MoveMask {
    pub const MAX_TRAIL: u32 = 16;
    #[inline(always)] #[must_use]
    pub unsafe fn new(input: Vector) -> Self {
        let asu16s = vreinterpretq_u16_u8(input);
        let mask = vshrn_n_u16::<4>(asu16s);
        let asu64 = vreinterpret_u64_u8(mask);
        let scalar64 = vget_lane_u64::<0>(asu64);

        Self(scalar64)
    }

    // Check if all the high bits are set
    #[inline(always)] #[must_use]
    pub const fn all_bits_set(&self) -> bool {
        self.0 == 0xFFFF_FFFF_FFFF_FFFF
    }

    #[inline(always)] #[must_use]
    pub const fn trailing_zeros(&self) -> u32 {
        self.0.trailing_zeros() >> 2
    }

    #[inline(always)] #[must_use]
    pub const fn trailing_ones(&self) -> u32 {
        self.0.trailing_ones() >> 2
    }
}

#[inline(always)] #[must_use]
pub unsafe fn eq(a: Vector, b: Vector) -> Vector {
    vceqq_u8(a, b)
}

#[inline(always)] #[must_use]
pub unsafe fn not(a: Vector) -> Vector {
    vmvnq_u8(a)
}

#[inline(always)] #[must_use]
pub unsafe fn xor(a: Vector, b: Vector) -> Vector { veorq_u8(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn or(a: Vector, b: Vector) -> Vector {
    vorrq_u8(a, b)
}

#[inline(always)] #[must_use]
pub unsafe fn and(a: Vector, b: Vector) -> Vector {
    vandq_u8(a, b)
}

#[inline(always)] #[must_use]
pub unsafe fn greater_than_or_eq(a: Vector, b: Vector) -> Vector {
    vcgeq_u8(a, b)
}

#[inline(always)] #[must_use]
pub unsafe fn greater_than(a: Vector, b: Vector) -> Vector {
    vcgtq_u8(a, b)
}

#[inline(always)] #[must_use]
pub unsafe fn less_than_or_eq(a: Vector, b: Vector) -> Vector { vcleq_u8(a, b) }

#[inline(always)] #[must_use]
pub unsafe fn less_than(a: Vector, b: Vector) -> Vector {
    vcltq_u8(a, b)
}

#[inline(always)] #[must_use]
pub unsafe fn splat(a: u8) -> Vector { vdupq_n_u8(a) }

#[inline(always)] #[must_use]
pub unsafe fn load_unchecked(data: *const Ptr) -> Vector {
    vld1q_u8(data)
}

#[inline(always)] #[must_use]
pub unsafe fn load_aligned(ptr: *const Ptr) -> Vector { load_unchecked(ptr) }

#[inline(always)] #[must_use]
pub fn load(data: &[u8; 16]) -> Vector { unsafe { load_unchecked(data.as_ptr()) } }

#[inline(always)]
pub const fn byte_ptr(ptr: *const Ptr) -> *const u8 {
    ptr
}

#[inline(always)]
pub const fn simd_ptr(ptr: *const u8) -> *const Ptr {
    ptr
}

macro_rules! load_neon_lane {
    ($data:ident, $reg:ident, $lane:expr, $count:expr) => {
        if $lane < $count {
            $reg = vld1q_lane_u8::<{$lane}>($data.as_ptr().add($lane), $reg);
        }
    };
}

macro_rules! load_4_lanes {
    ($data:ident, $reg:ident, $start_lane:literal, $count:expr) => {{
        load_neon_lane!($data, $reg, $start_lane, $count);
        load_neon_lane!($data, $reg, $start_lane + 1, $count);
        load_neon_lane!($data, $reg, $start_lane + 2, $count);
        load_neon_lane!($data, $reg, $start_lane + 3, $count);
    }};
}

#[inline]
pub unsafe fn load_partial(data: &[u8], count: usize) -> Vector {
    let mut reg = vdupq_n_u8(0); // Create a register filled with zeros

    // isolate each lane and add our byte
    load_4_lanes!(data, reg, 0, count);
    load_4_lanes!(data, reg, 4, count);
    load_4_lanes!(data, reg, 8, count);
    load_4_lanes!(data, reg, 12, count);

    reg
}