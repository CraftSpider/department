//! Storage implementations which store their items in pre-set static memory regions.
//!
//! # Advantages
//! - No need for allocation
//! - Can provide 'heaps' which support any type of storage item (elements, ranges, etc)
//!
//! # Disadvantages
//! - Increase binary size

mod cell;
mod traits;

mod multi;
mod single;

pub use cell::StorageCell;

pub use multi::MultiStatic;
pub use single::SingleStatic;
