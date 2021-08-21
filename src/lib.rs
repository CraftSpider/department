#![feature(unsize, ptr_metadata, generic_associated_types)]
#![feature(layout_for_ptr)]
#![feature(coerce_unsized)]
#![cfg_attr(not(feature = "std"), no_std)]

mod utils;

pub mod traits;

pub mod boxed;
pub mod collections;

#[cfg(feature = "alloc")]
pub mod alloc;
pub mod inline;
