//! Storage implementations which store their items inline, on the stack.
//!
//! # Advantages
//! - No need for allocation
//! - Unlike static storages, do not blow up binary sizes
//!
//! # Disadvantages
//! - All types must be at least their storage size on the stack, increasing stack overflow risks
//! - Strictest `get` validity requirements of the provided implementations
//!

mod multi_elem;
mod single_elem;

mod multi_range;
mod single_range;

pub use multi_elem::MultiElement;
pub use single_elem::SingleElement;

pub use multi_range::MultiRange;
pub use single_range::SingleRange;
