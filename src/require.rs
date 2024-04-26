//! Ensure each requirement was met
//!
//! **Note**:
//! This module is experimental, other parts of this crate for the most part should have a stable
//! api, this is an exception.
//!
//! The idea behind this part of the crate is you can define requirements once and reuse them
//! across your project. A requirement is a higher-level concept than a condition in the context
//! of this crate. Unlike the condition combinatorics `swift-check` exposes, requirements must have
//! been fulfilled at least once in the input, or they raise the error associated with them. This is
//! valuable for things like specifying password rules in a highly maintainable fashion.
//!
//! # Example
//!
//! ```
//! use swift_check::{require::{Requirement, check}, requirement, requirements, eq};
//!
//! enum MyError {
//!     Space,
//!     Seven
//! }
//!
//! requirement!(
//!     /// The input must include a space
//!     pub space => eq(b' ') =>! MyError::Space
//! );
//! requirement!(
//!     /// The input must include a seven
//!     pub seven => eq(b'7') =>! MyError::Seven
//! );
//!
//! // (you can use any condition exposed by swift-check, not just eq)
//!
//! // now check if all the requirements were fulfilled, and that each byte in the input fell into
//! // at least one requirement (for validation)
//! let (valid, res) = check(
//!     b"example input 7",
//!     // as long as each requirement's error implements `Into<MyError>` you can use it.
//!     requirements!(MyError, [space, seven])
//! ).result(); // fails fast, there's also `results()` which allows you to iterate over the results
//!
//! // not all the example input was 7 or space, so valid is false
//! assert!(!valid);
//!
//! // there was a space and 7 in the input so the res is Ok
//! assert!(res.is_ok())
//! ```
//!
//! # Performance
//!
//! This is the slowest part of the api due to the added complexity to facilitate the desired
//! functionality. It is not slow, but it is not swift, there's a lot of room for optimization so if
//! `require` remains a part of the api it will overtime become more and more performant.

use crate::arch;
use crate::arch::{Vector, MoveMask};

/// The default error type meant for when you're feeling lazy.
#[repr(transparent)]
pub struct ErrMsg {
    pub msg: &'static str
}

impl ErrMsg {
    #[inline] #[must_use]
    pub const fn new(msg: &'static str) -> Self {
        Self { msg }
    }
}

impl From<&'static str> for ErrMsg {
    #[inline]
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

impl core::fmt::Display for ErrMsg {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.write_str(self.msg)
    }
}

impl core::fmt::Debug for ErrMsg {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Unsatisfied Requirement: {}", self.msg)
    }
}

impl core::ops::Deref for ErrMsg {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.msg
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ErrMsg {}

/// A `Condition` is used internally by the [`Requirement`] trait. `requirement!` will expand into
/// a constant function which returns a type that implements this trait.
pub trait Condition {
    /// The error type associated with the requirement. When used in `requirements` all the error
    /// types must implement `Into` to a common type.
    type Error;
    /// Used internally by the `requirements!` macro.
    #[must_use]
    fn check(&mut self, vector: Vector) -> MoveMask;
    /// Check that the condition was met at least once, used internally by the `requirements!` macro
    fn ok(self) -> Result<(), Self::Error>;
}

/// A trait representing a collection of conditions which must be met at least once.
///
/// # Methods
///
/// * [`result`] - Check if all requirements were met, fails fast
/// * [`results`] - Iterate over each requirement's result
///

/// [`result`]: Requirement::result
/// [`results`]: Requirement::results
pub trait Requirement {
    /// The common error of each `requirement!`, each error associated with the requirement should
    /// either be this error type or implement `Into` for this error type.
    type Error;
    /// Used internally by the [`check`] function
    fn check(&mut self, vector: Vector);
    /// Used internally by the [`check`] function
    ///
    /// This operates similarly to `Requirement::check` but for handling partial loads.
    ///
    /// The reason this is necessary is that partial loads pad the register with zeroes to ensure
    /// safety, if these zeroes to not meet any requirement then the validator would consider there
    /// to be illegal input and flag it as such.
    ///
    /// # Arguments
    ///
    /// * `vector` - The vector to check.
    /// * `len`    - The length of the data so that the validator knows what to check.
    fn check_partial(&mut self, vector: Vector, len: u32);
    /// # Result
    ///
    /// Get the result of the requirement check. This will return the first error caught in order
    /// of the requirements. If you need to know each unfulfilled requirement see [`results`]
    ///
    /// # Returns
    ///
    /// 0. A `bool` denoting if all bytes met at least one of the requirements.
    /// 1. `Ok(())` if all requirements were met, or the first error (in order of requirements)
    ///    caught.
    ///

    /// [`results`]: Requirement::results
    fn result(self) -> (bool, Result<(), Self::Error>);
    /// # Results
    ///
    /// Get an iterator over each requirement result. If you only need to know if the
    /// requirements were fulfilled see [`result`].
    ///
    /// # Returns
    ///
    /// 0. A `bool` denoting if all bytes met at least one of the requirements.
    /// 1. An iterator over results, in order of the provided requirements.
    ///

    /// [`result`]: Requirement::result
    fn results(self) -> (bool, impl Iterator<Item = Result<(), Self::Error>>);
}

/// `Requires` is the final representation of a requirement, usable in the `requirements!` macro.
///
/// When you use the `requirement!` macro it will expand into a constant function which returns
/// a `Requires` instance.
///
/// # Generics
///
/// - `C`: The required condition
/// - `Raise`: If the requirement was not fulfilled this is invoked to raise the corresponding `Err`
/// - `Err`: The error to `Raise` if the requirement was not fulfilled
pub struct Requires<C, Raise, Err>
    where
        C: Fn(Vector) -> Vector,
        Raise: FnOnce() -> Err
{
    /// The required condition
    pub cond: C,
    /// Raise the error if the requirement was not met
    raise: Raise,
    /// Track if the requirement has been met
    seen: bool
}

impl<C, Raise, Err> Requires<C, Raise, Err>
    where
        C: Fn(Vector) -> Vector,
        Raise: FnOnce() -> Err
{
    /// Create a new `Requires` instance
    #[inline] #[must_use]
    pub const fn new(cond: C, raise: Raise) -> Self {
        Self { cond, raise, seen: false }
    }
}

impl<C, Raise, Err> Condition for Requires<C, Raise, Err>
    where
        C: Fn(Vector) -> Vector,
        Raise: FnOnce() -> Err
{
    type Error = Err;

    /// Compute the condition over the vector, if any bit was set update seen to true.
    ///
    /// # Returns
    ///
    /// The `MoveMask` used to extract the condition result, used to check validity of the input,
    /// ensuring each byte fulfilled at least one condition.
    #[inline] #[must_use]
    fn check(&mut self, vector: Vector) -> MoveMask {
        let mask = unsafe { MoveMask::new((self.cond)(vector)) };
        self.seen |= mask.any_bit_set();
        mask
    }

    #[inline(always)]
    fn ok(self) -> Result<(), Self::Error> {
        if self.seen { Ok(()) } else { Err((self.raise)()) }
    }
}

/// Define a Requirement
///
/// Requirements can easily be composed in the `requirements!` macro, allowing you to create
/// highly maintainable and robust validators with decent performance.
///
/// # Example
///
/// ```
/// use swift_check::{
///     require::{Requirement, check},
///     requirement, requirements,
///     range, eq
/// };
///
/// enum Errors {
///     Space,
///     F,
///     Number,
/// }
///
/// requirement!(
///     /// The input must include a space
///     space => eq(b' ') =>! Errors::Space
/// );
///
/// requirement!(
///     /// The input must include `F`
///     f => eq(b'f') =>! Errors::F
/// );
///
/// requirement!(
///     /// The input must include a number
///     number => range!(b'0'..=b'9') =>! Errors::Number
/// );
///
/// // now we can use each requirement together in the requirements! macro, as long as the errors
/// // of each requirement impl Into<requirements! error type> then you're good to go
///
/// let (valid, res_iter) = check(
///     b"example input",
///     requirements!(Errors, [space, f, number])
/// ).results();
///
/// // valid denotes each byte met at least one of the requirements, in this case it should be
/// // false
/// assert!(!valid);
///
/// for result in res_iter {
///     match result {
///         Err(Errors::F) => {/* There was not an F in the input */},
///         Err(Errors::Number) => {/* There was not a number in the input */},
///         Err(Errors::Space) => unreachable!("There was a space in the input"),
///         _ => {}
///     }
/// }
/// ```
///
/// Or, if you're feeling lazy you can use literals as your error, these use the [`ErrMsg`] type.
///
/// ```
/// # use swift_check::{
/// #     require::{Requirement, check},
/// #     requirement, requirements,
/// #     range, eq
/// # };
/// #
/// requirement!(pub space => eq(b' ') =>! "There needs to be a space!");
/// ```
///
/// # Syntax
///
/// ```txt
/// #[attributes]
/// visibility identifier => condition =>! error
/// ```
///
/// **Syntax Limitation**: Right now you cannot use errors within a module, so you must import them.
#[macro_export]
macro_rules! requirement {
    // when implemented as an accumulator it didn't work well with rust analyzer / rust rover
    (
        $(#[$attr:meta])*
        $vis:vis $req_name:ident => $cond:expr =>! $error_message:literal
    ) => {
        $(#[$attr])*
        #[must_use]
        $vis const fn $req_name () -> impl $crate::require::Condition<Error = $crate::require::ErrMsg> {
            let res = $crate::require::Requires::new($cond, || { $crate::require::ErrMsg::new($error_message) });
            res
        }
    };
    (
        $(#[$attr:meta])*
        $vis:vis $req_name:ident => $cond:expr =>! $create_err:expr => $err_ty:ty
    ) => {
        $(#[$attr])*
        #[must_use]
        $vis const fn $req_name () -> impl $crate::require::Condition<Error = $err_ty> {
            let res = $crate::require::Requires::new($cond, || { $create_err });
            res
        }
    };
    (
        $(#[$attr:meta])*
        $vis:vis $req_name:ident => $cond:expr =>! $err:ident ($($args:expr),* $(,)?)
    ) => {
        $(#[$attr])*
        #[must_use]
        $vis const fn $req_name () -> impl $crate::require::Condition<Error = $err> {
            let res = $crate::require::Requires::new($cond, || { $err ($($args),*) });
            res
        }
    };
    (
        $(#[$attr:meta])*
        $vis:vis $req_name:ident => $cond:expr =>! $err:ident :: $func:ident ($($args:expr),* $(,)?)
    ) => {
        $(#[$attr])*
        #[must_use]
        $vis const fn $req_name () -> impl $crate::require::Condition<Error = $err> {
            let res = $crate::require::Requires::new($cond, || { $err :: $func ($($args),*) });
            res
        }
    };
    (
        $(#[$attr:meta])*
        $vis:vis $req_name:ident => $cond:expr =>! $err:ident :: $variant:ident
    ) => {
        $(#[$attr])*
        #[must_use]
        $vis const fn $req_name () -> impl $crate::require::Condition<Error = $err> {
            let res = $crate::require::Requires::new($cond, || { $err :: $variant });
            res
        }
    };
}

/// Check multiple `requirement!`s
///
/// # Example
///
/// ```
/// use swift_check::{
///     require::{Requirement, check},
///     requirement, requirements,
///     eq
/// };
///
/// requirement!(pub space => eq(b' ') =>! "There needs to be a space!");
/// requirement!(pub seven => eq(b'7') =>! "There needs to be a seven!");
///
/// let (valid, res) = check(
///     b"example input 7",
///     requirements!([space, seven])
/// ).results();
///
/// // The first element of result denotes if all bytes met at least one of the conditions, in this
/// // case this is false.
/// assert!(!valid);
///
/// // the res is an iterator over each requirement! result. You also can use `result` instead of
/// // `results` if you only need to know all requirements were met and care less about granular
/// // error reporting.
/// ```
///
/// # Error Handling
///
/// The individual requirements do not need to share the same error type, but they do all need to
/// implement either `From` or `Into` the `requirements!` error type.
///
/// # Syntax
///
/// ```txt
/// Error Type, [Requirements, ...]
/// ```
///
/// or for when you're feeling lazy, the default error is `ErrMsg`, the above example uses this.
///
/// ```txt
/// [Requirements, ...]
/// ```
#[macro_export]
macro_rules! requirements {
    ([$($requirement:ident),* $(,)?] $(,)?) => {
        $crate::requirements!($crate::require::ErrMsg, [$($requirement),*])
    };
    ($error:ty, [$($requirement:ident),* $(,)?] $(,)?) => {{
        #[allow(non_camel_case_types)]
        struct Requirements<$($requirement: $crate::require::Condition),*> {
            __valid: bool,
            $($requirement: $requirement),*
        }
        #[allow(non_camel_case_types)]
        impl<$($requirement),*> $crate::require::Requirement for Requirements<$($requirement),*>
            where
                $($requirement: $crate::require::Condition,
                <$requirement as $crate::require::Condition>::Error: Into<$error>),*
        {
            type Error = $error;
            #[inline]
            fn check(&mut self, vector: $crate::arch::Vector) {
                #[allow(unused_imports)]
                use $crate::require::Condition as _;
                self.__valid &= ($(self.$requirement.check(vector) )|*).all_bits_set();
            }
            #[inline]
            fn check_partial(&mut self, vector: $crate::arch::Vector, len: u32) {
                #[allow(unused_imports)]
                use $crate::require::Condition as _;
                self.__valid &= ($(self.$requirement.check(vector) )|*)
                    .trailing_ones() >= len;
            }
            #[inline]
            fn result(self) -> (bool, Result<(), Self::Error>) {
                $(
                    if let Err(err) = self.$requirement.ok() {
                        return (self.__valid, Err(err.into()));
                    };
                )*
                (self.__valid, Ok(()))
            }
            #[inline]
            fn results(self) -> (bool, impl Iterator<Item=Result<(), Self::Error>>) {
                (
                    self.__valid,
                    [$(self.$requirement.ok().map_err(|err| -> Self::Error {err.into()})),*]
                        .into_iter()
                )
            }
        }

        Requirements {
            __valid: true,
            $($requirement: $requirement ()),*
        }
    }};
}

/// Check that all `requirement!`s are fulfilled
///
/// # Arguments
///
/// * `data` - The data to validate
/// * `req` - The requirements to check against
///
/// # Result
///
/// An implementor of [`Requirement`] at the final state
///
/// # Example
///
/// ```
/// use swift_check::{
///     require::{Requirement, check},
///     requirement, requirements,
///     eq
/// };
///
/// requirement!(pub space => eq(b' ') =>! "There needs to be a space!");
/// requirement!(pub seven => eq(b'7') =>! "There needs to be a seven!");
///
/// let (valid, res) = check(
///     b"example input 7",
///     requirements!([space, seven])
/// ).results();
///
/// // The first element of result denotes if all bytes met at least one of the conditions, in this
/// // case this is false.
/// assert!(!valid);
/// ```
#[inline]
pub fn check<R: Requirement>(data: &[u8], mut req: R) -> R {
    if data.len() >= arch::WIDTH {
        unsafe { arch::scan::ensure_requirements(data, req) }
    } else {
        let len = data.len();
        req.check_partial(unsafe { arch::load_partial(data, len) }, len as u32);
        req
    }
}

#[cfg(test)]
#[test]
fn test() {
    use crate::{eq, range};
    struct SpecialError(&'static str);
    impl From<SpecialError> for ErrMsg {
        fn from(value: SpecialError) -> Self {
            ErrMsg::new(value.0)
        }
    }
    impl SpecialError {
        fn new(msg: &'static str) -> Self {
            Self ( msg )
        }
    }

    // no std, no alloc, requirement checks / validator with SIMD on aarch64, x86, and WASM32

    // if you're lazy you can use literals as your error
    requirement!(
        /// There should be at least one uppercase letter
        #[inline] pub uppercase => range!(b'A'..=b'Z') =>! "needs uppercase!"
    );
    requirement!(
        /// There should be at least one lowercase letter
        #[inline] pub lowercase => range!(b'a'..=b'z') =>! "needs lowercase!"
    );
    // you can use multiple different error types as long as they impl From to the requirements! err
    requirement!(
        /// There needs to be at least one number
        #[inline] pub numeric => range!(b'0'..=b'9') =>! SpecialError("needs number!")
    );
    requirement!(
        /// Question marks are a requirement
        #[inline] pub question_mark => eq(b'?') =>! SpecialError::new("needs a question mark!")
    );

    let res = check(
        b"hello world 12345678910",
        requirements!([uppercase, lowercase, numeric, question_mark])
    );

    // you can iterate through the errors
    for res in res.results().1 {
        if let Err(err) = res {
            println!("{err:?}");
        }
    }

    let res = check(
        b"hello world 12345678910",
        requirements!([uppercase, lowercase, numeric, question_mark])
    );

    // or if you just want to know if an err took place you can use ok
    println!("{:?}", res.result().1.unwrap_err());
}