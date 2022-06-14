//! Storage implementations which store their items in pre-set static memory regions.
//!
//! # Advantages
//! - No need for allocation
//! - Doesn't blow up the stack
//!
//! # Disadvantages
//! - Increases binary size
//! - Not much less rigorous than inline storage

mod cell;
mod traits;

mod multi;
mod single;

pub use cell::StorageCell;

pub use multi::MultiStatic;
pub use single::SingleStatic;
