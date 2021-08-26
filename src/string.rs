use core::ops::Deref;

use crate::collections::Vec;
use crate::error::Result;
use crate::traits::SingleRangeStorage;

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
    pub fn new() -> String<S> {
        String { inner: Vec::new() }
    }

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
    pub fn new_in(storage: S) -> String<S> {
        String {
            inner: Vec::new_in(storage),
        }
    }

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
