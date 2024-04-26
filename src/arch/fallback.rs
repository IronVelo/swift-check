#![allow(clippy::missing_safety_doc)]

const BYTE_MASK: u128 = 0x00FF;
pub type Vector = u128;
pub type Ptr = [u8; 16];
pub const STEP: usize = 16;
pub const STEP_SIZE: usize = 1;

#[repr(transparent)]
pub struct MoveMask(u16);

#[cfg(feature = "ensure-simd")]
compile_error!("Attempted to use fallback implementation with the `ensure-simd` feature enabled");

macro_rules! for_each_byte {
    ($shift:ident, |$($ident:ident),* $(,)?| $do:expr) => {{
        let mut $shift = 0;
        loop {
            let ($($ident),*) = ($(($ident >> $shift) & BYTE_MASK),*);
            $do;
            $shift += 8;
            if $shift == 128 { break }
        }
    }};
}

impl MoveMask {
    pub const MAX_TRAIL: u32 = 16;
    #[inline(always)] #[must_use]
    pub const unsafe fn new(input: Vector) -> Self {
        let mut result: u16 = 0;

        let mut shift = 0;
        let mut i = 0;

        loop {
            result |= (((input & (1 << (shift))) >> (shift)) as u16) << i;
            shift += 8;
            i += 1;
            if i == 16 { break }
        }

        Self(result)
    }

    #[inline(always)] #[must_use]
    pub const fn all_bits_set(&self) -> bool {
        self.0 == 0xFFFF
    }
    #[inline(always)] #[must_use]
    pub const fn any_bit_set(&self) -> bool { self.0 > 0 }
    #[inline(always)] #[must_use]
    pub const fn trailing_zeros(&self) -> u32 {
        self.0.trailing_zeros()
    }
    #[inline(always)] #[must_use]
    pub const fn trailing_ones(&self) -> u32 {
        self.0.trailing_ones()
    }
}

impl_bit_ops!(MoveMask);

#[inline] #[must_use]
pub const fn eq(a: Vector, b: Vector) -> Vector {
    let mut result = 0;
    for_each_byte!(shift, |a, b| {
        result |= ((a == b) as u128) << shift;
    });
    result
}

#[inline(always)] #[must_use]
pub const fn not(a: Vector) -> Vector {
    !a
}

#[inline(always)] #[must_use]
pub const fn xor(a: Vector, b: Vector) -> Vector {
    a ^ b
}

#[inline(always)] #[must_use]
pub const fn or(a: Vector, b: Vector) -> Vector {
    a | b
}

#[inline(always)] #[must_use]
pub const fn and(a: Vector, b: Vector) -> Vector {
    a & b
}

#[inline] #[must_use]
pub const fn greater_than_or_eq(a: Vector, b: Vector) -> Vector {
    let mut result = 0;
    for_each_byte!(shift, |a, b| {
        result |= ((a >= b) as u128) << shift;
    });
    result
}

#[inline] #[must_use]
pub const fn greater_than(a: Vector, b: Vector) -> Vector {
    let mut result = 0;
    for_each_byte!(shift, |a, b| {
        result |= ((a > b) as u128) << shift;
    });
    result
}

#[inline] #[must_use]
pub const fn less_than_or_eq(a: Vector, b: Vector) -> Vector {
    let mut result = 0;
    for_each_byte!(shift, |a, b| {
        result |= ((a <= b) as u128) << shift;
    });
    result
}

#[inline] #[must_use]
pub const fn less_than(a: Vector, b: Vector) -> Vector {
    let mut result = 0;
    for_each_byte!(shift, |a, b| {
        result |= ((a < b) as u128) << shift;
    });
    result
}

#[inline] #[must_use]
pub const fn splat(a: u8) -> Vector {
    let mut result = a as u128;
    result |= result << 8;
    result |= result << 16;
    result |= result << 32;
    result |= result << 64;
    result
}

#[inline(always)] #[must_use]
pub const unsafe fn load_unchecked(data: &[u8]) -> Vector {
    u128::from_le_bytes(*data.as_ptr().cast())
}

#[inline(always)] #[must_use]
pub const fn load(data: &[u8; 16]) -> Vector {
    unsafe { load_unchecked(data) }
}

#[inline(always)] #[must_use]
pub const unsafe fn load_aligned(data: &[u8]) -> Vector {
    load_unchecked(data)
}

#[inline(always)] #[must_use]
pub unsafe fn maybe_aligned_load(data: &[u8]) -> crate::arch::Vector {
    load_unchecked(data)
}

#[inline(always)] #[must_use]
pub const fn byte_ptr(ptr: *const Ptr) -> *const u8 {
    ptr.cast()
}

#[inline(always)] #[must_use]
pub const fn simd_ptr(ptr: *const u8) -> *const Ptr {
    ptr.cast()
}

#[inline] #[must_use]
pub fn load_partial(data: &[u8], count: usize) -> Vector {
    let mut buf = [0u8; 16];
    buf[..count].copy_from_slice(&data[..count]);
    load(&buf)
}