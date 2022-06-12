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

mod static_heap;

pub use cell::StorageCell;

pub use multi::MultiItem;
pub use single::SingleItem;

pub use static_heap::StaticHeap;
