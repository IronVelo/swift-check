pub(crate) const WIDTH: usize = 16;
#[macro_use]
mod cfg_macros;

// Basic predicate
#[allow(dead_code)]
pub(crate) fn is_aligned(ptr: *const Ptr) -> bool {
    byte_ptr(ptr).align_offset(WIDTH) == 0
}

cfg_verify!(
    macro_rules! contract {
        ($kind:ident!($args:expr)) => {
            mirai_annotations::$kind!($args)
        };
        ($expr:expr) => {
            $expr
        };
    }
);

cfg_runtime!(
    #[allow(unused_macros)]
    macro_rules! contract {
        ($kind:ident!($args:expr)) => {};
        ($expr:expr) => {};
    }
);

cfg_neon!(
    pub mod aarch64;
    pub use aarch64 as arch;
);

cfg_sse!(
    pub mod x86_64;
    pub use x86_64 as arch;
);

cfg_simd128!(
    pub mod wasm;
    pub use wasm as arch;
);

cfg_fallback!(
    pub mod fallback;
    pub use fallback as arch;
);

#[doc(hidden)]
pub use arch::{
    eq, not, xor, or, and, splat, byte_ptr, simd_ptr, load_partial, load_aligned, maybe_aligned_load
};

#[doc(hidden)]
pub use arch::{MoveMask, Ptr, STEP, STEP_SIZE};

pub use arch::Vector;
pub use arch::{load, load_unchecked};

cfg_simd!(
    #[doc(hidden)]
    pub mod simd_scan;
    #[doc(hidden)]
    pub use simd_scan as scan;
);

cfg_fallback!(
    #[doc(hidden)]
    pub mod fallback_scan;
    #[doc(hidden)]
    pub use fallback_scan as scan;
);

cfg_i8!(
    macro_rules! impl_cmp {
        (
            $CONST:ident,
            overflow => $handle_overflow:expr,
            default => $handle_non_overflow:expr
            $(, max => $handle_max:expr)?
            $(, min => $handle_min:expr)?
            $(,)?
        ) => {
            match $CONST {
                $(255 => $handle_max,)?
                128..=255 => $handle_overflow,
                $(0 => $handle_min,)?
                _ => $handle_non_overflow
            }
        };
    }

    macro_rules! impl_gt {
        ($MIN:ident, $gt:ident, $handle_max:expr $(, $handle_min:expr)? $(,)?) => {
            impl_cmp!(
                $MIN,
                overflow => {
                    // ensure that the value is less than 0 and greater than MAX
                    move |data| {
                        and(
                            arch::less_than(data, splat(0)),
                            arch::$gt(data, splat($MIN))
                        )
                    }
                },
                default => {
                    // everything else we need to check that the value is either below 0 or greater
                    // than max
                    move |data| {
                        or(
                            arch::less_than(data, splat(0)),
                            arch::$gt(data, splat($MIN))
                        )
                    }
                },
                max => $handle_max
                $(, min => $handle_min)?
            )
        };
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn greater_than<const MIN: u8>() -> impl Fn(Vector) -> Vector {
        unsafe { impl_gt!(
            MIN,
            greater_than,
            move |_| { splat(0) }
        ) }
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn greater_than_or_eq<const MIN: u8>() -> impl Fn(Vector) -> Vector {
        unsafe { impl_gt!(
            MIN,
            greater_than_or_eq,
            move |data| { eq(data, splat(255)) },
            move |_| {
                // gt or eq 0 would always be true, splat all ones
                splat(0xFF)
            }
        ) }
    }

    macro_rules! impl_lt {
        ($MAX:ident, $lt:ident, $handle_min:expr $(, $handle_max:expr)? $(,)?) => {
            impl_cmp!(
                $MAX,
                overflow => move |data| unsafe {
                    // overflow, so less than 127 OR MIN
                    or(
                        arch::greater_than_or_eq(data, splat(0)),
                        arch::$lt(data, splat($MAX))
                    )
                },
                default => move |data| unsafe {
                    // no overflow, but data coming in could have, so greater than 0 and less than
                    // MIN
                    and(
                        arch::greater_than_or_eq(data, splat(0)),
                        arch::$lt(data, splat(MAX))
                    )
                },
                $(max => $handle_max,)?
                min => $handle_min
            )
        };
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn less_than<const MAX: u8>() -> impl Fn(Vector) -> Vector {
        impl_lt!(
            MAX,
            less_than,
            move |_| unsafe {
                // always fails
                splat(0)
            }
        )
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn less_than_or_eq<const MAX: u8>() -> impl Fn(Vector) -> Vector {
        impl_lt!(
            MAX,
            less_than_or_eq,
            move |data| unsafe {
                // lt or eq 0 is practically just eq 0
                eq(data, splat(0))
            },
            move |_| unsafe {
                // lt or eq maximum value is always true
                splat(0xFF)
            }
        )
    }

    macro_rules! impl_range_cast {
        ($MIN:ident, $MAX:ident, $gt:ident, $lt:ident, $eq:expr, $max_128:expr) => {
            match ($MIN, $MAX) {
                _ if $MIN == $MAX => $eq,
                (0..=127, 129..=255) => {
                    // gt min OR lt MAX
                    move |data| unsafe {
                        or(
                            arch::$gt(data, splat($MIN)),
                            arch::$lt(data, splat($MAX))
                        )
                    }
                },
                (0..=127, 128) => $max_128,
                _ => {
                    // gte min AND lte MAX
                    move |data| unsafe {
                        and(
                            arch::$gt(data, splat($MIN)),
                            arch::$lt(data, splat($MAX))
                        )
                    }
                }
            }
        };
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn range<const MIN: u8, const MAX: u8>() -> impl Fn(Vector) -> Vector {
        impl_range_cast!(MIN, MAX, greater_than_or_eq, less_than_or_eq,
            move |data| unsafe { eq(data, splat(MIN)) },
            move |data| unsafe {
                or(
                    arch::greater_than_or_eq(data, splat(MIN)),
                    eq(data, splat(MAX))
                )
            }
        )
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn exclusive_range<const MIN: u8, const MAX: u8>() -> impl Fn(Vector) -> Vector {
        if MIN.abs_diff(MAX) == 1 {
            move |_data: Vector| -> Vector { unsafe { splat(0) } }
        } else {
            impl_range_cast!(
                MIN, MAX, greater_than, less_than, move |_| unsafe { splat(0) },
                move |data| unsafe { arch::greater_than(data, splat(MIN)) }
            )
        }
    }
);

cfg_u8!(
    #[doc(hidden)] #[inline(always)]
    pub const fn less_than<const MAX: u8>() -> impl Fn(Vector) -> Vector {
        match MAX {
            0 => move |_| unsafe {
                // always false
                splat(0)
            },
            _ => move |data| unsafe {
                arch::less_than(data, splat(MAX))
            }
        }
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn less_than_or_eq<const MAX: u8>() -> impl Fn(Vector) -> Vector {
        match MAX {
            255 => move |_| unsafe {
                // always true
                splat(0xFF)
            },
            _ => move |data| unsafe {
                arch::less_than_or_eq(data, splat(MAX))
            }
        }
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn greater_than<const MIN: u8>() -> impl Fn(Vector) -> Vector {
        match MIN {
            255 => move |_| unsafe {
                // always false
                splat(0)
            },
            _ => move |data| unsafe {
                arch::greater_than(data, splat(MIN))
            }
        }
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn greater_than_or_eq<const MIN: u8>() -> impl Fn(Vector) -> Vector {
        match MIN {
            0 => move |_| unsafe {
                // always true
                splat(0xFF)
            },
            _ => move |data| unsafe {
                arch::greater_than_or_eq(data, splat(MIN))
            }
        }
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn range<const MIN: u8, const MAX: u8>() -> impl Fn(Vector) -> Vector {
        match (MIN, MAX) {
            _ if MIN == MAX => move |data| unsafe { eq(data, splat(MIN)) },
            _ => move |data| unsafe {
                and(
                    arch::greater_than_or_eq(data, splat(MIN)),
                    arch::less_than_or_eq(data, splat(MAX))
                )
            }
        }
    }

    #[doc(hidden)] #[inline(always)]
    pub const fn exclusive_range<const MIN: u8, const MAX: u8>() -> impl Fn(Vector) -> Vector {
        match (MIN, MAX, MIN.abs_diff(MAX)) {
            (_, _, 1) |
            (_, _, _) if MIN == MAX => move |_| unsafe { splat(0) },
            _ => move |data| unsafe {
                and(
                    arch::greater_than(data, splat(MIN)),
                    arch::less_than(data, splat(MAX))
                )
            }
        }
    }
);