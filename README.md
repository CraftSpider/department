
# Department

[![crates.io](https://img.shields.io/crates/v/department.svg)](https://crates.io/crates/department)
[![Documentation](https://docs.rs/department/badge.svg)](https://docs.rs/department)
[![MIT/Apache-2 licensed](https://img.shields.io/crates/l/department.svg)](./LICENSE-APACHE)

A Rust library hosting a possible implementation of the proposed Storages API,
as well as several of the standard `std` collections implemented with it.

Inspired by [storage-poc](https://github.com/matthieu-m/storage-poc), re-implemented
and built upon to provide (hopefully) release-ready functionality.

## Features

By default, all features are turned on - but they can be disabled if you only
want specific storages and collections

- `std`: Whether to include std error support and other std-only features
- `all_storages`: Enable all storage features
  - `inline`: Inline on-the-stack storages
  - `heap`: Virtual heap-like storage, can be used on the stack or in a static
  - `static`: Storages backed by static memory, stored in the binary
  - `alloc`: Storages backed by a standard allocator. Requires the `alloc` crate to be available
  - `fallback`: Storage which attempts to store something in one, then falls back to a second storage
  - `debug`: Storage which wraps another, and provides a number of runtime checks which panic on certain forms of
             UB or incorrect usages.
- `all_collections`: Enable all collection types
  - `box`: Include the `Box` type
  - `rc`: Include the `Rc` and `Weak` types
  - `vec`: Include the `Vec` type
  - `string`: Include the `String` type, requires `vec`

## Future Work

In the future, more types of storages and collections need to be added, hopefully
up to `std` parity. Tests should be added for all storage types, with coverage for most
edge cases (ZST, alignment requirements, etc).

### Missing Collections

`btree`, an implementation of a `BTreeMap` and `BTreeSet`
`hash`, an implementation of a `HashMap` and `HashSet`
`arc`, implementation for atomically ref-counted items
