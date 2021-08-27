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

mod multi_elem;
mod single_elem;

mod multi_range;
mod single_range;

mod static_heap;

pub use cell::StorageCell;

pub use multi_elem::MultiElement;
pub use single_elem::SingleElement;

pub use multi_range::MultiRange;
pub use single_range::SingleRange;

pub use static_heap::StaticHeap;
