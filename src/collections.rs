//! Implementations of some common collection types, using storages for memory.

#[cfg(feature = "vec")]
mod vec;

#[cfg(feature = "vec")]
pub use vec::Vec;
