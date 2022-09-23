//! A storage-based implementation of [`std::string`]

use core::borrow::Borrow;
use core::ops::Deref;
use core::{fmt, ops};

use crate::base::Storage;
use crate::collections::Vec;
use crate::error::Result;

/// Storage based implementation of [`String`](std::string::String)
pub struct String<S>
where
    S: Storage,
{
    inner: Vec<u8, S>,
}

impl<S> String<S>
where
    S: Storage + Default,
{
    /// Create a new, empty `String` with a default instance of the desired storage
    ///
    /// # Panics
    ///
    /// If the backing allocation fails for any reason
    pub fn new() -> String<S> {
        String { inner: Vec::new() }
    }

    /// Attempt to create a new, empty `String` with a default instance of the desired storage
    pub fn try_new() -> Result<String<S>> {
        Ok(String {
            inner: Vec::try_new()?,
        })
    }
}

impl<S> String<S>
where
    S: Storage,
{
    /// Create a new, empty `String` with the provided storage instance
    ///
    /// # Panics
    ///
    /// If the backing allocation fails for any reason
    pub fn new_in(storage: S) -> String<S> {
        String {
            inner: Vec::new_in(storage),
        }
    }

    /// Attempt to create a new, empty `String` with the provided storage instance
    pub fn try_new_in(storage: S) -> Result<String<S>> {
        Ok(String {
            inner: Vec::try_new_in(storage)?,
        })
    }
}

impl<S> fmt::Debug for String<S>
where
    S: Storage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &**self)
    }
}

impl<S> fmt::Display for String<S>
where
    S: Storage,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &**self)
    }
}

impl<S> PartialEq for String<S>
where
    S: Storage,
{
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<S> PartialEq<str> for String<S>
where
    S: Storage,
{
    fn eq(&self, other: &str) -> bool {
        **self == *other
    }
}

impl<S> Default for String<S>
where
    S: Storage + Default,
{
    fn default() -> String<S> {
        String::new()
    }
}

impl<S> From<&str> for String<S>
where
    S: Storage + Default,
{
    fn from(str: &str) -> Self {
        String {
            inner: Vec::from(str.as_bytes()),
        }
    }
}

impl<S> From<(&str, S)> for String<S>
where
    S: Storage,
{
    fn from(pair: (&str, S)) -> Self {
        String {
            inner: Vec::from((pair.0.as_bytes(), pair.1)),
        }
    }
}

impl<S> ops::Add<&str> for String<S>
where
    S: Storage,
{
    type Output = String<S>;

    fn add(mut self, rhs: &str) -> Self::Output {
        self.inner.extend(rhs.as_bytes().iter().copied());
        self
    }
}

impl<S> Deref for String<S>
where
    S: Storage,
{
    type Target = str;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Invariant of String that the inner vec is valid utf8
        unsafe { core::str::from_utf8_unchecked(&self.inner) }
    }
}

impl<S> AsRef<str> for String<S>
where
    S: Storage,
{
    fn as_ref(&self) -> &str {
        self
    }
}

impl<S> Borrow<str> for String<S>
where
    S: Storage,
{
    fn borrow(&self) -> &str {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline::SingleInline;

    #[test]
    fn test_add() {
        let s = String::<SingleInline<[u8; 20]>>::from("Hello") + " World!";

        assert_eq!(&s, "Hello World!");
    }
}
