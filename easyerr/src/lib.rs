#![doc = include_str!(concat!("../", core::env!("CARGO_PKG_README")))]
#![no_std]

pub use easyerr_derive::Error;

/// Prelude. Currently, just reexports everything (a grand total of... 4 items), but might change
/// in the future.
pub mod prelude {
    pub use crate::*;
}

/// Trait for types which can add context to some [`Error`] ([`Source`](Self::Source)), transforming
/// it into a new [`Error`] ([`Err`](Self::Err)).
///
/// [`Error`]: core::error::Error
pub trait ErrorContext {
    /// The source error type which this context can be added to.
    type Source: core::error::Error;
    /// The new error type after adding this context.
    type Err: core::error::Error;

    /// Add this context to the given error [`Source`](Self::Source), transforming it into
    /// [`Err`](Self::Err).
    fn add_to_source(self, source: Self::Source) -> Self::Err;
}

/// Extension trait for [`Result`] which adds useful methods to use [`ErrorContext`]s.
pub trait ResultExt<T, E1> {
    /// Add the given context to the error of this result.
    fn context<C, E2>(self, ctx: C) -> Result<T, E2>
    where
        C: ErrorContext<Err = E2, Source = E1>;

    /// Add the given context to the error of this result, lazily.
    fn with_context<C, E2, F>(self, f: F) -> Result<T, E2>
    where
        F: FnOnce(&E1) -> C,
        C: ErrorContext<Err = E2, Source = E1>;
}

impl<T, E1> ResultExt<T, E1> for Result<T, E1> {
    #[inline(always)]
    fn context<C, E2>(self, ctx: C) -> Result<T, E2>
    where
        C: ErrorContext<Err = E2, Source = E1>,
    {
        self.map_err(|e| ctx.add_to_source(e))
    }

    #[inline(always)]
    fn with_context<C, E2, F>(self, f: F) -> Result<T, E2>
    where
        F: FnOnce(&E1) -> C,
        C: ErrorContext<Err = E2, Source = E1>,
    {
        self.map_err(|e| f(&e).add_to_source(e))
    }
}

/// Macro that evaluates an expression and returns an error if it is not true.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $e:expr $(,)?) => {
        if !($cond) {
            return Err($e.into());
        }
    };
}
