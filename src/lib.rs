//! An implementation of the proposed Storages API, including both Storage implementations
//! as well as common collections built on top of it.

// Needed to implement the custom unsizing of `Box` and similar
#![feature(unsize)]
// Needed for `::Handle<T>`, which is a basis of the whole interface
#![feature(generic_associated_types)]
// Needed to (de)construct unsized pointers and store their metadata safely
#![feature(ptr_metadata)]
// Needed to get a layout from just a type and metadata in `utils::layout_of`
#![feature(layout_for_ptr)]
// Needed to implement unsizing coercion via `Box`
#![feature(coerce_unsized)]
// Needed so we can avoid providing static storages on non-atomic platforms
#![feature(cfg_target_has_atomic)]
// A helper for initializing arrays. Could be replaced, but low priority compared to above
// requirements.
#![feature(maybe_uninit_uninit_array)]
#![warn(
    missing_docs,
    elided_lifetimes_in_paths,
    explicit_outlives_requirements,
    missing_abi,
    noop_method_call,
    pointer_structural_match,
    semicolon_in_expressions_from_macros,
    unused_import_braces,
    unused_lifetimes,
    clippy::cargo,
    clippy::missing_panics_doc,
    clippy::doc_markdown,
    clippy::ptr_as_ptr,
    clippy::cloned_instead_of_copied,
    clippy::unreadable_literal
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc", feature(allocator_api))]

#[cfg(feature = "alloc")]
extern crate alloc as rs_alloc;

mod utils;

pub mod base;

// TODO: Default to allocation?
pub mod boxed;
pub mod collections;
pub mod string;

#[cfg(feature = "alloc")]
pub mod alloc;
pub mod error;
pub mod inline;
#[cfg(target_has_atomic = "8")]
pub mod statics;
