//! A storage-based implementation of [`std::string`]

use core::ops::Deref;

use crate::base::SingleRangeStorage;
use crate::collections::Vec;
use crate::error::Result;

/// Storage based implementation of [`String`](std::string::String)
pub struct String<S>
where
    S: SingleRangeStorage,
{
    inner: Vec<u8, S>,
}

impl<S> String<S>
where
    S: SingleRangeStorage + Default,
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
    S: SingleRangeStorage,
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

impl<S> Default for String<S>
where
    S: SingleRangeStorage + Default,
{
    fn default() -> String<S> {
        String::new()
    }
}

impl<S> From<&str> for String<S>
where
    S: SingleRangeStorage + Default,
{
    fn from(str: &str) -> Self {
        String {
            inner: Vec::from(str.as_bytes()),
        }
    }
}

impl<S> From<(&str, S)> for String<S>
where
    S: SingleRangeStorage,
{
    fn from(pair: (&str, S)) -> Self {
        String {
            inner: Vec::from((pair.0.as_bytes(), pair.1)),
        }
    }
}

impl<S> Deref for String<S>
where
    S: SingleRangeStorage,
{
    type Target = str;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Invariant of String that the inner vec is valid utf8
        unsafe { core::str::from_utf8_unchecked(&*self.inner) }
    }
}
