//! ```
//! use swift_check::{search, range, one_of, any, all, eq, not};
//!
//! let input = b"
//!     swift-check is a high performance library for searching or validating data based on \
//!     expressive conditions
//! ";
//!
//! let cond = all!(
//!     all!(any!(range!(b'a'..=b'z'), range!(b'A'..=b'Z'), eq(b' ')), not(range!(b'0'..=b'9'))),
//!     one_of!(eq(b' '), range!(b'a'..=b'z'), range!(b'a'..=b'z'))
//! );
//!
//! let Some(first_space) = search(input, cond) else {
//!     unreachable!("There's a space!")
//! };
//!
//! assert_eq!(input[first_space], b' ');
//!
//! // or, more simply
//!
//! let Some(first_space2) = search(input, eq(b' ')) else {
//!     unreachable!("There's a space!")
//! };
//!
//! assert_eq!(input[first_space2], b' ');
//! assert_eq!(first_space2, first_space);
//! ```
#![allow(unused_unsafe, unused_parens)] // fallback
// #![cfg_attr(not(test), no_std)]
// #![cfg_attr(not(test), no_builtins)]
//
pub mod arch;
use arch::Vector;

/// Check that the condition holds for all bytes
///
/// # Arguments
///
/// * `data` - The `Vector` to check each element of
/// * `cond` - The condition to check
///
/// # Example
///
/// ```
/// use swift_check::{ensure, any, eq, arch::load};
///
/// let input = b"2112111211211211";
/// let data = load(input);
///
/// let two_or_one = ensure!(data, any!(eq(b'1'), eq(b'2')));
/// assert!(two_or_one);
///
/// let should_fail = ensure!(data, eq(b'1'));
/// assert!(!should_fail);
/// ```
///
/// **Note**: This is part of the lower level api, for better ergonomics see [`for_all_ensure`].
#[macro_export]
macro_rules! ensure {
    ($data:expr, $cond:expr) => {
        unsafe { $crate::arch::MoveMask::new($cond($data)).all_bits_set() }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __is_found {
    ($cond_eval:expr, |$len:ident| $then:expr, || $otherwise:expr) => { unsafe {
        let $len = $crate::arch::MoveMask::new($cond_eval).trailing_zeros();
        if $len == $crate::arch::MoveMask::MAX_TRAIL { $otherwise } else { $then }
    }};
}

/// Find the first circumstance of the condition being met
///
/// # Arguments
///
/// * `data` - The `Vector` to search
/// * `cond` - The condition to find
///
/// # Example
///
/// ```
/// use swift_check::{arch::load, find, eq};
///
/// let input = b"aaaaaaaaaaaaaaaB";
/// let data = load(input);
///
/// if let Some(position) = find(data, eq(b'B')) {
///     assert_eq!(position as usize, input.len() - 1);
/// } else {
///     unreachable!("B is within the data");
/// }
/// ```
///
/// **Note**: This is part of the lower level api, for better ergonomics see [`search`].
#[inline(always)]
pub fn find(data: Vector, cond: impl Fn(Vector) -> Vector) -> Option<u32> {
    let len = unsafe { arch::MoveMask::new(cond(data)).trailing_zeros() };
    if len >= arch::MoveMask::MAX_TRAIL { None } else { Some(len) }
}

#[macro_export]
macro_rules! find {
    ($data:expr, $cond:expr) => {
        $crate::__is_found!($cond($data), |__len| Some(__len), || None)
    };
}

/// For all the bytes, ensure that the condition holds
///
/// # Security
///
/// Unlike [`for_all_ensure`] this will continue scanning even if the condition has failed, this
/// is to reduce information leakage. While this doesn't branch on the data, we cannot assure it is
/// perfect constant time for all architectures, so if you're using this on secrets please use tools
/// such as `dudect` to verify that said usage is acceptable.
///
/// # Arguments
///
/// * `data` - The data to validate
/// * `cond` - The condition to validate with, this should be some composition of the conditions
///            exposed within this crate.
///
/// # Performance
///
/// For anything less than 16 bytes you will not benefit from using this, in fact for simple
/// conditions a branchless search will outperform this by a good amount.
///
/// # Example
///
/// ```
/// use swift_check::{for_all_ensure_ct, any, range, eq};
///
/// let input = b"hello world I am valid input";
/// // everything must be a lowercase character outside I and space
/// let res = for_all_ensure_ct(input, any!(range!(b'a'..=b'z'), eq(b'I'), eq(b' ')));
/// assert!(res);
///
/// let input = b"Hello world I am invalid input";
/// // has capital H, will fail
/// let should_fail = for_all_ensure_ct(input, any!(range!(b'a'..=b'z'), eq(b'I'), eq(b' ')));
/// assert!(!should_fail);
/// ```
#[inline]
pub fn for_all_ensure_ct(data: &[u8], cond: impl Fn(Vector) -> Vector) -> bool {
    let mut valid = true;
    if data.len() >= arch::WIDTH {
        unsafe { arch::scan::for_all_ensure_ct(data, cond, &mut valid) }
    } else {
        // This is a temporary solution, in the future conditions will support both simd and byte
        // by byte checks
        valid &= unsafe {
            arch::MoveMask::new(cond(arch::load_partial(data, data.len())))
                .trailing_ones() >= data.len() as u32
        };
    }

    valid
}

/// For all the bytes, ensure that the condition holds
///
/// # Arguments
///
/// * `data` - The data to validate
/// * `cond` - The condition to validate with, this should be some composition of the conditions
///            exposed within this crate.
///
/// # Performance
///
/// For anything less than 16 bytes you will not benefit from using this, in fact for simple
/// conditions a branchless search will outperform this by a good amount.
///
/// ```
/// use swift_check::{for_all_ensure, any, range, eq};
///
/// let input = b"hello world I am valid input";
/// // everything must be a lowercase character outside I and space
/// let res = for_all_ensure(input, any!(range!(b'a'..=b'z'), eq(b'I'), eq(b' ')));
/// assert!(res);
///
/// let input = b"Hello world I am invalid input";
/// // has capital H, will fail
/// let should_fail = for_all_ensure(input, any!(range!(b'a'..=b'z'), eq(b'I'), eq(b' ')));
/// assert!(!should_fail);
/// ```
#[inline]
pub fn for_all_ensure(data: &[u8], cond: impl Fn(Vector) -> Vector) -> bool {
    if data.len() >= arch::WIDTH {
        unsafe { arch::scan::for_all_ensure(data, cond) }
    } else {
        unsafe {
            arch::MoveMask::new(cond(arch::load_partial(data, data.len())))
                .trailing_ones() >= data.len() as u32
        }
    }
}

/// Find the first byte that meets the `cond`
///
/// # Arguments
///
/// * `data` - The haystack to search
/// * `cond` - The condition to find the first occurrence of
///
/// # Example
///
/// ```
/// use swift_check::{search, eq};
///
/// let input = b"some data with a 5 burger 383294 hello world blah blah blah";
/// if let Some(pos) = search(input, eq(b'5')) {
///     assert_eq!(input[pos], b'5');
/// } else {
///     panic!("input contained a 5");
/// }
/// ```
#[inline]
pub fn search(data: &[u8], cond: impl Fn(Vector) -> Vector) -> Option<usize> {
    if data.len() >= arch::WIDTH {
        unsafe { arch::scan::search(data, cond) }
    } else {
        match unsafe { arch::MoveMask::new(cond(arch::load_partial(data, data.len()))).trailing_zeros() } {
            offset if offset < data.len() as u32 => Some(offset as usize),
            _ => None
        }
    }
}

/// Ensure min is less than max at compile time
#[doc(hidden)] #[macro_export]
macro_rules! comp_check_rng {
    ($min:literal, $max:literal, $do:expr) => {
        match ($min, $max) {
            ($min..=$max, $min..=$max) | _ => $do
        }
    };
}

/// Check that the value is within the specified range
///
/// # Between
///
/// Inclusive
/// ```no
/// range!(5..=20)
/// ```
/// Exclusive
/// ```no
/// range!(5..20)
/// ```
///
/// ### Example
///
/// ```
/// use swift_check::{ensure, range, arch::load};
///
/// let input = b"abcdefghijklmnop";
/// let data = load(input);
///
/// let is_lowercase_alphabet = ensure!(data, range!(b'a'..=b'z'));
/// assert!(is_lowercase_alphabet);
///
/// let input = b"Abcdefghijklmnop";
/// let data = load(input);
///
/// let is_lowercase_alphabet = ensure!(data, range!(b'a'..=b'z'));
/// assert!(!is_lowercase_alphabet);
///
/// let data = load(b"bbcdefghijklmnop");
/// // exclusive range
/// let is_not_a_or_z = ensure!(data, range!(b'a'..b'z'));
/// assert!(is_not_a_or_z);
///
/// let data = load(b"abbbbbbbbbbbbbbz");
/// let should_fail = ensure!(data, range!(b'a'..b'z'));
///
/// assert!(!should_fail);
/// ```
///
/// # Less or Greater Than
///
/// Less than
/// ```no
/// range!(< 20)
/// ```
/// Less than or eq
/// ```no
/// range!(<= 20)
/// ```
/// Greater than
/// ```no
/// range!(> 20)
/// ```
/// Greater than or eq
/// ```no
/// range!(>= 20)
/// ```
#[macro_export]
macro_rules! range {
    ($min:literal..=$max:literal) => {
        $crate::comp_check_rng!($min, $max, $crate::arch::range::<$min, $max>())
    };
    ($min:literal..$max:literal) => {
        $crate::comp_check_rng!($min, $max, $crate::arch::exclusive_range::<$min, $max>())
    };
    (<= $max:literal) => {
        $crate::arch::less_than_or_eq::<$max>()
    };
    (< $max:literal) => {
        $crate::arch::less_than::<$max>()
    };
    (>= $min:literal) => {
        $crate::arch::greater_than_or_eq::<$min>()
    };
    (> $min:literal) => {
        $crate::arch::greater_than::<$min>()
    };
}

/// Check if the bytes are equal to `expected`
///
/// # Arguments
///
/// * `expected` - The value you expect
///
/// # Example
///
/// ```
/// use swift_check::{ensure, any, eq, arch::load};
///
/// let input = b"1111111111111111";
/// let data = load(input);
///
/// let has_one = ensure!(data, eq(b'1'));
/// assert!(has_one);
///
/// let input = b"2112111211211211";
/// let data = load(input);
///
/// let has_one_or_two = ensure!(data, any!(eq(b'1'), eq(b'2')));
/// assert!(has_one_or_two);
///
/// let input = b"3452111211211211";
/// let data = load(input);
///
/// let should_fail = ensure!(data, any!(eq(b'1'), eq(b'2')));
/// assert!(!should_fail);
/// ```
#[inline(always)]
pub const fn eq(expected: u8) -> impl Fn(Vector) -> Vector {
    move |data| unsafe { arch::eq(data, arch::splat(expected)) }
}

/// Negate a condition
///
/// # Arguments
///
/// * `cond` - The condition to negate
///
/// # Example
///
/// ```
/// use swift_check::{ensure, range, not, arch::load};
///
/// let input = b"abcdefghijklmnop";
/// let data = load(input);
///
/// let no_numbers = ensure!(data, not(range!(b'0'..=b'9')));
/// assert!(no_numbers);
///
/// let input = b"abcdefghijklmno1";
/// let data = load(input);
///
/// let should_fail = ensure!(data, not(range!(b'0'..=b'9')));
/// assert!(!should_fail);
/// ```
#[inline(always)]
pub const fn not(cond: impl Fn(Vector) -> Vector) -> impl Fn(Vector) -> Vector {
    move |data| unsafe { arch::not(cond(data)) }
}

/// Combine two conditions
///
/// # Arguments
///
/// * `a` - The lhs condition
/// * `b` - The rhs condition
///
/// # Example
///
/// ```
/// use swift_check::{ensure, range, eq, not, and, arch::load};
///
/// let input = b"1112221111111111";
/// let data = load(input);
///
/// let is_num_but_not_5 = ensure!(data, and(range!(b'0'..=b'9'), not(eq(b'5'))));
/// assert!(is_num_but_not_5);
///
/// let input = b"5112221111111111";
/// let data = load(input);
///
/// let should_fail = ensure!(data, and(range!(b'0'..=b'9'), not(eq(b'5'))));
/// assert!(!should_fail);
/// ```
#[inline(always)]
pub const fn and(
    a: impl Fn(Vector) -> Vector,
    b: impl Fn(Vector) -> Vector
) -> impl Fn(Vector) -> Vector {
    move |data| unsafe { arch::and(a(data), b(data)) }
}

/// Check that either condition is met
///
/// # Arguments
///
/// * `a` - The lhs condition
/// * `b` - The rhs condition
///
/// # Example
///
/// ```
/// use swift_check::{ensure, or, eq, arch::load};
///
/// let input = b"1113331111111111";
/// let data = load(input);
///
/// let is_1_or_3 = ensure!(data, or(eq(b'1'), eq(b'3')));
/// assert!(is_1_or_3);
///
/// let input = b"!113331111111111";
/// let data = load(input);
///
/// let should_fail = ensure!(data, or(eq(b'1'), eq(b'3')));
/// assert!(!should_fail);
/// ```
#[inline(always)]
pub const fn or(
    a: impl Fn(Vector) -> Vector,
    b: impl Fn(Vector) -> Vector
) -> impl Fn(Vector) -> Vector {
    move |data| unsafe { arch::or(a(data), b(data)) }
}

/// Check that only one of the conditions are met
///
/// # Arguments
///
/// * `a` - The lhs condition
/// * `b` - The rhs condition
///
/// # Example
///
/// ```
/// use swift_check::{ensure, xor, range, arch::load};
///
/// let input = b"3333333377777777";
/// let data = load(input);
///
/// let not_five = ensure!(data, xor(range!(b'0'..=b'5'), range!(b'5'..=b'9')));
/// assert!(not_five);
///
/// let input = b"3333333557777777";
/// let data = load(input);
///
/// let should_fail = ensure!(data, xor(range!(b'0'..=b'5'), range!(b'5'..=b'9')));
/// assert!(!should_fail);
/// ```
#[inline(always)]
pub const fn xor(
    a: impl Fn(Vector) -> Vector,
    b: impl Fn(Vector) -> Vector
) -> impl Fn(Vector) -> Vector {
    move |data| unsafe { arch::xor(a(data), b(data)) }
}

#[macro_export]
#[doc(hidden)]
macro_rules! __all {
    ($left:expr $(,)?) => {
        $left
    };
    ($left:expr, $right:expr $(,)?) => {
        $crate::arch::and($left, $right)
    };
    ($left:expr, $right:expr, $($rest:expr),+ $(,)?) => {
        $crate::arch::and(
            $crate::__all!($left, $right),
            $crate::__all!($($rest),+)
        )
    };
}

/// Check that all the conditions hold
///
/// # Arguments
///
/// * `conditions` - The conditions to ensure all hold
///
/// # Example
///
/// ```
/// use swift_check::{ensure, all, range, not, eq, arch::load};
///
/// let input = b"3333333377777777";
/// let data = load(input);
///
/// let not_five = ensure!(data, all!(range!(b'0'..=b'9'), not(eq(b'5'))));
/// assert!(not_five);
///
/// let input = b"3333333557777777";
/// let data = load(input);
///
/// let should_fail = ensure!(data, all!(range!(b'0'..=b'9'), not(eq(b'5'))));
/// assert!(!should_fail);
/// ```
#[macro_export]
macro_rules! all {
    // Base case: if only one argument is provided, simply return it
    ($left:expr $(,)? ) => {
        $left
    };
    // Two arguments: directly apply AND between them
    ($left:expr, $right:expr $(,)?) => {
        |data: $crate::arch::Vector| -> $crate::arch::Vector {
            #[allow(unused_unsafe)]
            unsafe { $crate::__all!($left(data), $right(data)) }
        }
    };
    ($left:expr, $right:expr, $($rest:expr),+ $(,)?) => {
        |data: $crate::arch::Vector| -> $crate::arch::Vector {
            #[allow(unused_unsafe)]
            unsafe { $crate::__all!($left(data), $right(data), $($rest(data)),+) }
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __or {
    ($left:expr) => {
        // or against nothing, so just return left
        $left
    };
    ($left:expr, $right:expr $(,)?) => {
        $crate::arch::or($left, $right)
    };
    ($left:expr, $right:expr, $($rest:expr),+ $(,)?) => {
        $crate::arch::or(
            $crate::__or!($left, $right),
            $crate::__or!($($rest),+)
        )
    };
}

/// Check that any of the conditions hold
///
/// This is practically [`or`], just can handle any number of conditions
///
/// # Arguments
///
/// * `conditions` - The conditions where at least one must hold
///
/// # Example
///
/// ```
/// use swift_check::{ensure, any, eq, arch::load};
///
/// let input = b"3333333377777777";
/// let data = load(input);
///
/// let three_or_seven = ensure!(data, any!(eq(b'3'), eq(b'7')));
/// assert!(three_or_seven);
///
/// let input = b"3333333557777777";
/// let data = load(input);
///
/// let should_fail = ensure!(data, any!(eq(b'3'), eq(b'7')));
/// assert!(!should_fail);
/// ```
#[macro_export]
macro_rules! any {
    ($left:expr $(,)?) => {
        // any, all, etc are not leaf conditions, therefore we can just return left as it should
        // already be a higher order function
        $left
    };
    ($left:expr, $right:expr $(,)?) => {
        |data: $crate::arch::Vector| -> $crate::arch::Vector {
            #[allow(unused_unsafe)]
            unsafe { $crate::__or!($left(data), $right(data)) }
        }
    };
    ($left:expr, $right:expr, $($rest:expr),+ $(,)?) => {
        |data: $crate::arch::Vector| -> $crate::arch::Vector {
            #[allow(unused_unsafe)]
            unsafe { $crate::__or!($left(data), $right(data), $($rest(data)),+) }
        }
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! __xor {
    ($left:expr $(,)?) => {
        $left
    };
    ($left:expr, $right:expr $(,)?) => {
        $crate::arch::xor($left, $right)
    };
    ($left:expr, $right:expr, $($rest:expr),+ $(,)?) => {
        $crate::arch::xor(
            $crate::__xor!($left, $right),
            $crate::__xor!($($rest),+)
        )
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __one_of {
    ($l_i:ident: $left:expr, $r_i:ident: $right:expr, $($rest:ident: $cond:expr),* $(,)?) => {
        |data: $crate::arch::Vector| -> $crate::arch::Vector {
            // I had tried from the obvious shift conds into place and compute the
            // hamming weight, perf degraded and less flexible. May be worth revisiting in the
            // future. Yes, this approach is flawed and does not scale to large numbers of args
            #[allow(unused_unsafe)]
            unsafe {
                let ($l_i, $r_i, $($rest),+) = ($left(data), $right(data), $($cond(data)),+);
                // combine xor and nand to ensure only one cond held
                $crate::arch::and(
                    $crate::__xor!($l_i, $r_i, $($rest),+),
                    $crate::arch::not($crate::__all!($l_i, $r_i, $($rest),+))
                )
            }
        }
    };
}

/// Ensure only one of the conditions are true
///
/// # Arguments
///
/// * `condition`, ... - The conditions to check, only allowing one to hold (up to 4)
///
/// # Example
///
/// ```
/// use swift_check::{one_of, for_all_ensure, eq, range};
///
/// let input = b"123456789";
/// let char_or_num = for_all_ensure(
///     input, one_of!(range!(b'0'..=b'9'), range!(b'a'..=b'z'), range!(b'A'..=b'Z'))
/// );
/// assert!(char_or_num);
///
/// let should_fail = for_all_ensure(
///     input,
///     one_of!(range!(b'0'..=b'9'), range!(b'0'..=b'9'), range!(b'0'..=b'9'))
/// );
/// assert!(!should_fail)
/// ```
#[macro_export]
macro_rules! one_of {
    ($left:expr $(,)?) => {
        $left
    };
    ($left:expr, $right:expr $(,)?) => {
        |data: $crate::arch::Vector| -> $crate::arch::Vector {
            #[allow(unused_unsafe)]
            unsafe { $crate::__xor!($left(data), $right(data)) }
        }
    };
    ($first:expr, $second:expr, $third:expr $(,)?) => {
        $crate::__one_of!(first: $first, second: $second, third: $third)
    };
    ($first:expr, $second:expr, $third:expr, $fourth:expr $(,)?) => {
        $crate::__one_of!(first: $first, second: $second, third: $third, fourth: $fourth)
    };
}

#[cfg(all(test, not(mirai)))]
mod tests {
    use super::*;
    use quickcheck::quickcheck;

    extern crate alloc;
    use alloc::string::String;
    use alloc::vec::Vec;

    #[test]
    fn for_all_ensure_range_is_inclusive() {
        let input = b"hello world";
        let res = for_all_ensure(input, any!(range!(0..=127), eq(b' ')));
        assert!(res);
    }

    macro_rules! one_of_eq {
        ($($lit:literal),* $(,)?) => {
            one_of!($(eq($lit)),*)
        };
    }

    macro_rules! ensure_one_of {
        ($input:ident, $($lit:literal),* $(,)?) => {
            ensure!($input, one_of_eq!($($lit),*))
        };
    }

    macro_rules! failure_perms {
        ($input:ident, $f_val:literal, $a_val:literal) => {{
            assert!(!ensure_one_of!($input, $f_val, $f_val));
            assert!(!ensure_one_of!($input, $f_val, $f_val, $f_val));
            assert!(!ensure_one_of!($input, $f_val, $f_val, $f_val, $f_val));

            assert!(!ensure_one_of!($input, $f_val, $f_val, $a_val, $a_val));
            assert!(!ensure_one_of!($input, $a_val, $a_val, $f_val, $f_val));

            assert!(!ensure_one_of!($input, $a_val, $f_val, $a_val, $f_val));
            assert!(!ensure_one_of!($input, $f_val, $a_val, $f_val, $a_val));

            assert!(!ensure_one_of!($input, $f_val, $a_val, $a_val, $f_val));
            assert!(!ensure_one_of!($input, $a_val, $f_val, $f_val, $a_val));
        }};
    }

    #[test]
    fn one_of_permutations() {
        let input = arch::load(&[0u8; 16]);
        failure_perms!(input, 0, 0);
        failure_perms!(input, 0, 1);
        failure_perms!(input, 1, 0);

        assert!(ensure_one_of!(input, 0, 1, 1, 1));
        assert!(ensure_one_of!(input, 1, 0, 1, 1));
        assert!(ensure_one_of!(input, 1, 1, 0, 1));
        assert!(ensure_one_of!(input, 1, 1, 1, 0));
        assert!(ensure_one_of!(input, 0, 1, 1));
        assert!(ensure_one_of!(input, 1, 0, 1));
        assert!(ensure_one_of!(input, 1, 1, 0));
        assert!(ensure_one_of!(input, 0, 1));
        assert!(ensure_one_of!(input, 1, 0));
        assert!(ensure_one_of!(input, 0));
        assert!(!ensure_one_of!(input, 1));

        // none true

        assert!(!ensure_one_of!(input, 1, 1, 1, 1));
        assert!(!ensure_one_of!(input, 1, 1, 1));
        assert!(!ensure_one_of!(input, 1, 1));
        assert!(!ensure_one_of!(input, 1));
    }

    macro_rules! one_of_nest {
        ($($($f_lit:literal),*);* $(;)?) => {
            one_of!(
                $(one_of_eq!($($f_lit),*)),+
            )
        };
    }

    #[test]
    fn one_of_nesting() {
        let input = arch::load(&[1u8; 16]);

        assert!(ensure!(input, one_of_nest!(
            1, 0, 0, 0;
            0, 0, 0, 0;
            0, 0, 0, 0;
            0, 0, 0, 0
        )));

        assert!(!ensure!(input, one_of_nest!(1; 1)));

        assert!(ensure!(input, one_of_nest!(
            0, 0, 0, 0; // fails
            1, 1, 1, 1; // fails
            1, 0, 0, 0  // succeeds therefore true
        )));
    }

    macro_rules! ensure_test {
        (
            $input:expr, $condition:expr,
            |$success_byte:ident| $validate_positive:expr,
            |$failure_byte:ident| $validate_negative:expr
        ) => {
            if for_all_ensure($input, $condition) {
                let mut true_success = true;
                for $success_byte in $input {
                    true_success &= $validate_positive;
                }
                true_success
            } else {
                let mut should_have_failed = false;
                for $failure_byte in $input {
                    should_have_failed |= $validate_negative
                }
                should_have_failed
            }
        };
    }

    macro_rules! search_test {
        (
            $input:expr,
            $condition:expr, |$pos:ident| $cond_met_assertion:expr,
            |$byte:ident| $cond_failed_assertion:expr
        ) => {{
            if let Some($pos) = search($input, $condition) {
                if $pos >= $input.len() {
                    panic!(
                        "search should never return greater than or eq len. \n{:?}\t{:?} >= {}\t{}",
                        $input, $pos, $input.len(), stringify!($condition)
                    );
                }
                $cond_met_assertion
            } else {
                let mut actually_failed = true;
                for $byte in $input {
                    actually_failed &= $cond_failed_assertion;
                }
                actually_failed
            }
        }};
    }

    macro_rules! cmp_test {
        ($cmp:expr, $assert:pat, $input:ident) => {
            search_test!(
                $input.as_slice(),
                $cmp, |pos| matches!($input[pos], $assert),
                |byte| !matches!(byte, $assert)
            )
        };
    }

    macro_rules! range_test {
        ($min:literal..=$max:literal, $input:ident) => {
            cmp_test!(
                range!($min..=$max), $min..=$max, $input
            )
        };
    }

    macro_rules! excl_r_test {
        ($min:literal..$max:literal, $input:ident) => {
            search_test!(
                $input.as_slice(),
                range!($min..$max), |pos| $input[pos] > $min && $input[pos] < $max,
                |byte| !(byte > &$min && byte < &$max)
            )
        };
    }

    macro_rules! check {
        ($test:expr, $message:expr) => {
            if !$test {
                println!("[{}:{}] Test failed: {}", file!(), line!(), $message);
                return false;
            }
        };
        ($test:expr $(,)?) => {
            check!($test, stringify!($test))
        };
    }

    macro_rules! checks {
        ($($test:expr),+ $(,)?) => {{
            $(check!($test);)*
            true
        }};
    }

    quickcheck! {
        fn search_for_needle(s: String) -> bool {
            if let Some(pos) = search(s.as_bytes(), eq(b'a')) {
                s.as_bytes()[pos] == b'a'
            } else {
                !s.contains("a")
            }
        }
        fn search_for_number(s: String) -> bool {
            if let Some(pos) = search(s.as_bytes(), range!(b'0'..=b'9')) {
                s.as_bytes()[pos].is_ascii_digit()
            } else {
                let mut has_digits = false;
                for byte in s.as_bytes() {
                    has_digits |= byte.is_ascii_digit()
                }
                !has_digits
            }
        }
        fn always_holds(s: String) -> bool {
            for_all_ensure(s.as_bytes(), range!(0..=255))
        }
        fn range_large(s: Vec<u8>) -> bool {
            checks!(
                range_test!(10..=244, s),
                range_test!(1..=254, s),
                range_test!(54..=200, s),
                range_test!(100..=200, s),
                range_test!(0..=128, s),
                range_test!(0..=129, s)
            )
        }
        fn range_mid(s: Vec<u8>) -> bool {
            checks!(
                range_test!(100..=150, s),
                range_test!(127..=129, s),
                range_test!(127..=128, s),
                range_test!(128..=129, s),
                range_test!(120..=130, s)
            )
        }
        fn range_small(s: Vec<u8>) -> bool {
            checks!(
                range_test!(200..=255, s),
                range_test!(240..=255, s),
                range_test!(255..=255, s),
                range_test!(254..=255, s),
                range_test!(130..=255, s),
                range_test!(160..=255, s),
                range_test!(1..=5, s),
                range_test!(0..=1, s),
                range_test!(0..=13, s),
                range_test!(3..=60, s),
                range_test!(30..=31, s)
            )
        }
        fn excl_range_large(s: Vec<u8>) -> bool {
            checks!(
                excl_r_test!(10..245, s),
                excl_r_test!(0..255, s),
                excl_r_test!(200..254, s),
                excl_r_test!(100..200, s),
                excl_r_test!(60..160, s),
                excl_r_test!(1..254, s)
            )
        }
        fn excl_range_mid(s: Vec<u8>) -> bool {
            checks!(
                excl_r_test!(125..135, s),
                excl_r_test!(110..140, s),
                excl_r_test!(127..128, s),
                excl_r_test!(128..129, s),
                excl_r_test!(127..129, s),
                excl_r_test!(126..129, s),
                excl_r_test!(126..128, s),
                excl_r_test!(128..130, s)
            )
        }
        fn excl_range_small(s: Vec<u8>) -> bool {
            checks!(
                excl_r_test!(0..1, s),
                excl_r_test!(0..2, s),
                excl_r_test!(0..5, s),
                excl_r_test!(254..255, s),
                excl_r_test!(253..255, s),
                excl_r_test!(250..255, s),
                excl_r_test!(1..5, s),
                excl_r_test!(2..5, s),
                excl_r_test!(4..5, s),
            )
        }
        fn less_than_always_false(s: Vec<u8>) -> bool {
            search_test!(
                s.as_slice(),
                range!(< 0), |_cannot_happen| false,
                |_na| true
            )
        }
        fn less_than_large(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(< 255), 0..=254, s),
                cmp_test!(range!(< 240), 0..=239, s),
                cmp_test!(range!(< 254), 0..=253, s),
                cmp_test!(range!(< 200), 0..=199, s),
                cmp_test!(range!(< 140), 0..=139, s)
            )
        }
        fn less_than_mid(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(< 128), 0..=127, s),
                cmp_test!(range!(< 129), 0..=128, s),
                cmp_test!(range!(< 128), 0..=127, s),
                cmp_test!(range!(< 130), 0..=129, s)
            )
        }
        fn less_than_small(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(< 30), 0..=29, s),
                cmp_test!(range!(< 1), 0, s),
                cmp_test!(range!(< 64), 0..=63, s),
                cmp_test!(range!(< 120), 0..=119, s)
            )
        }
        fn less_than_or_eq_small(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(<= 30), 0..=30, s),
                cmp_test!(range!(<= 0), 0, s),
                cmp_test!(range!(<= 1), 0..=1, s),
                cmp_test!(range!(<= 2), 0..=2, s)
            )
        }
        fn less_than_or_eq_mid(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(<= 128), 0..=128, s),
                cmp_test!(range!(<= 129), 0..=129, s),
                cmp_test!(range!(<= 127), 0..=127, s),
                cmp_test!(range!(<= 130), 0..=130, s)
            )
        }
        fn less_than_or_eq_large(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(<= 254), 0..=254, s),
                cmp_test!(range!(<= 250), 0..=250, s),
                cmp_test!(range!(<= 200), 0..=200, s),
                cmp_test!(range!(<= 160), 0..=160, s)
            )
        }
        fn less_than_or_eq_always_true(s: Vec<u8>) -> bool {
            search_test!(
                s.as_slice(),
                range!(<= 255), |_always| true,
                |_never| false
            )
        }
        fn greater_than_always_false(s: Vec<u8>) -> bool {
            search_test!(
                s.as_slice(),
                range!(> 255), |_never| false,
                |_always| true
            )
        }
        fn greater_than_small(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(> 254), 255, s),
                cmp_test!(range!(> 250), 251..=255, s),
                cmp_test!(range!(> 253), 254..=255, s),
                cmp_test!(range!(> 230), 231..=255, s),
                cmp_test!(range!(> 190), 191..=255, s)
            )
        }
        fn greater_than_mid(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(> 128), 129..=255, s),
                cmp_test!(range!(> 127), 128..=255, s),
                cmp_test!(range!(> 129), 130..=255, s),
                cmp_test!(range!(> 125), 126..=255, s)
            )
        }
        fn greater_than_large(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(> 0), 1..=255, s),
                cmp_test!(range!(> 50), 51..=255, s),
                cmp_test!(range!(> 90), 91..=255, s),
                cmp_test!(range!(> 1), 2..=255, s)
            )
        }
        fn greater_than_or_eq_always_true(s: Vec<u8>) -> bool {
            search_test!(
                s.as_slice(),
                range!(>= 0), |_always| true,
                |_never| false
            )
        }
        fn greater_than_or_eq_small(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(>= 255), 255, s),
                cmp_test!(range!(>= 254), 254..=255, s),
                cmp_test!(range!(>= 250), 250..=255, s),
                cmp_test!(range!(>= 230), 230..=255, s),
                cmp_test!(range!(>= 200), 200..=255, s)
            )
        }
        fn greater_than_or_eq_mid(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(>= 128), 128..=255, s),
                cmp_test!(range!(>= 127), 127..=255, s),
                cmp_test!(range!(>= 129), 129..=255, s),
                cmp_test!(range!(>= 126), 126..=255, s),
                cmp_test!(range!(>= 130), 130..=255, s)
            )
        }
        fn greater_than_or_eq_large(s: Vec<u8>) -> bool {
            checks!(
                cmp_test!(range!(>= 1), 1..=255, s),
                cmp_test!(range!(>= 3), 3..=255, s),
                cmp_test!(range!(>= 64), 64..=255, s),
                cmp_test!(range!(>= 90), 90..=255, s),
                cmp_test!(range!(>= 120), 120..=255, s)
            )
        }

        fn basic_for_all_ensure(s: Vec<u8>) -> bool {
            checks!(
                ensure_test!(
                    s.as_slice(), eq(0),
                    |succ| succ == &0,
                    |fail| fail != &0
                ),
                ensure_test!(
                    s.as_slice(), range!(0..=250),
                    |succ| matches!(succ, 0..=250),
                    |fail| matches!(fail, 251..=255)
                ),
                ensure_test!(
                    s.as_slice(), not(eq(0)),
                    |succ| succ != &0,
                    |fail| fail == &0
                ),
                ensure_test!(
                    s.as_slice(), any!(eq(0), eq(1)),
                    |succ| matches!(succ, 0..=1),
                    |fail| matches!(fail, 2..=255)
                ),
                ensure_test!(
                    s.as_slice(), all!(eq(0), eq(0)),
                    |succ| succ == &0,
                    |fail| fail != &0
                ),
                ensure_test!(
                    s.as_slice(), all!(eq(0), eq(1)),
                    |_succ| false,
                    |_fail| true
                ),
                ensure_test!(
                    s.as_slice(), any!(eq(0), not(eq(0))),
                    |_succ| true,
                    |_fail| false
                )
            )
        }
    }
}

#[cfg(all(test))]
mod mirai_tests {
    use crate::{eq, search};

    #[test]
    fn simple_search() {
        let input = b"I am a simple input to evaluate the correctness of the search";

        let res = search(input, eq(b'h')).unwrap();
        assert_eq!(input[res], b'h');
    }
}