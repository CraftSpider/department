//! An implementation of the proposed Storages API, including both Storage implementations
//! as well as common collections built on top of it.

#![feature(unsize)]
#![feature(generic_associated_types)]
#![feature(ptr_metadata)]
#![feature(layout_for_ptr)]
#![feature(coerce_unsized)]
#![feature(cfg_target_has_atomic)]
#![feature(maybe_uninit_uninit_array)]
#![warn(
    // missing_docs,
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

pub mod boxed;
pub mod collections;
pub mod string;

#[cfg(feature = "alloc")]
pub mod alloc;
pub mod error;
pub mod inline;
#[cfg(target_has_atomic = "8")]
pub mod statics;
