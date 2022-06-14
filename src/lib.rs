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
#![cfg_attr(feature = "coerce-unsized", feature(coerce_unsized))]
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
    clippy::unreadable_literal,
    clippy::missing_safety_doc
)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "alloc", feature(allocator_api))]

#[cfg(any(feature = "alloc", feature = "debug"))]
extern crate alloc as rs_alloc;
extern crate core;

mod utils;

pub mod base;
pub mod error;

// Storage implementations

#[cfg(feature = "alloc")]
pub mod alloc;
#[cfg(feature = "debug")]
pub mod debug;
#[cfg(feature = "fallback")]
pub mod fallback;
#[cfg(feature = "inline")]
pub mod inline;
#[cfg(feature = "static")]
pub mod statics;

// Collection implementations

#[cfg(feature = "box")]
pub mod boxed;
#[cfg(any(feature = "vec", feature = "hash"))]
pub mod collections;
#[cfg(feature = "rc")]
pub mod rc;
#[cfg(feature = "string")]
pub mod string;
