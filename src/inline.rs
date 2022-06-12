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

mod multi;
mod single;

pub use multi::MultiInline;
pub use single::SingleInline;
