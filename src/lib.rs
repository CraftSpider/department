#![feature(unsize)]
#![feature(generic_associated_types)]
#![feature(ptr_metadata)]
#![feature(layout_for_ptr)]
#![feature(coerce_unsized)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc", feature(allocator_api))]
#![cfg(feature = "alloc")]
extern crate alloc as core_alloc;

mod utils;

pub mod traits;

pub mod boxed;
pub mod collections;

#[cfg(feature = "alloc")]
pub mod alloc;
pub mod inline;
