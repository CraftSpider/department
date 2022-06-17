//! Implementations of some common collection types, using storages for memory.

#[cfg(feature = "linked")]
mod linked_list;
#[cfg(feature = "vec")]
mod vec;

#[cfg(feature = "linked")]
pub use linked_list::LinkedList;
#[cfg(feature = "vec")]
pub use vec::Vec;
