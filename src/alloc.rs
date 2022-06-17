//! Storage implementation that is backed by an [`Allocator`]
//!
//! # Advantages
//! - Only takes up as much space at runtime as it actually needed
//! - Doesn't increase binary or stack sizes
//! - Handles are standard pointers
//!
//! # Disadvantages
//! - Unavailable on some embedded or 'bare-metal' platforms

use core::alloc::{Allocator, Layout};
#[cfg(feature = "unsize")]
use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};
use rs_alloc::alloc::Global;

use crate::base::{
    ClonesafeStorage, FromLeakedStorage, LeaksafeStorage, MultiItemStorage, Storage,
};
use crate::error::StorageError;
use crate::{error, utils};

/// An alias for a storage using the global allocator
pub type GlobalAlloc = Alloc<Global>;

/// Storage for using a standard `alloc::Allocator` as the backing
#[derive(Copy, Clone)]
pub struct Alloc<A: Allocator>(A);

impl<A: Allocator> Alloc<A> {
    /// Create a new [`Alloc`] from the provided allocator instance.
    pub fn new(alloc: A) -> Alloc<A> {
        Alloc(alloc)
    }
}

impl Alloc<Global> {
    /// Get a storage backed by the global allocator
    pub fn global() -> Alloc<Global> {
        Alloc(Global)
    }
}

impl<A: Allocator + Default> Default for Alloc<A> {
    fn default() -> Self {
        Alloc(A::default())
    }
}

// SAFETY: `Allocator` safety requirements are a superset of `Storage` currently
unsafe impl<A: Allocator> Storage for Alloc<A> {
    type Handle<T: ?Sized + Pointee> = NonNull<T>;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        handle
    }

    fn from_raw_parts<T: ?Sized + Pointee>(handle: Self::Handle<()>, meta: T::Metadata) -> Self::Handle<T> {
        <Self::Handle<T>>::from_raw_parts(handle, meta)
    }

    fn cast<T: ?Sized + Pointee, U>(handle: Self::Handle<T>) -> Self::Handle<U> {
        handle.cast::<U>()
    }

    fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let (ptr, meta) = handle.to_raw_parts();
        NonNull::from_raw_parts(ptr, meta)
    }

    #[cfg(feature = "unsize")]
    fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        handle
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        <Self as MultiItemStorage>::allocate(self, meta)
    }

    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        <Self as MultiItemStorage>::deallocate(self, handle)
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        let old_len = handle.as_ref().len();

        let old_layout = Layout::array::<T>(old_len).expect("Valid handle");
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        let new_ptr = self
            .0
            .grow(handle.cast(), old_layout, new_layout)
            // This may actually be unimplemented or other, but we're making an educated guess
            .map_err(|_| StorageError::InsufficientSpace {
                expected: new_layout.size(),
                available: None,
            })?;

        Ok(NonNull::from_raw_parts(new_ptr.cast(), capacity))
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        let old_len = handle.as_ref().len();

        let old_layout = Layout::array::<T>(old_len).expect("Valid handle");
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        let new_ptr = self
            .0
            .shrink(handle.cast(), old_layout, new_layout)
            // Should probably only fail if shrinking isn't supported
            .map_err(|_| StorageError::Unimplemented)?;

        Ok(NonNull::from_raw_parts(new_ptr.cast(), capacity))
    }
}

// SAFETY: Rust requires that implementors of `Allocator` are multi-item currently
unsafe impl<A: Allocator> MultiItemStorage for Alloc<A> {
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        let layout = utils::layout_of::<T>(meta);

        let allocated: NonNull<()> = self
            .0
            .allocate(layout)
            .map_err(|_| StorageError::InsufficientSpace {
                expected: layout.size(),
                available: None,
            })?
            .cast();

        Ok(NonNull::from_raw_parts(allocated, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        let layout = Layout::for_value(handle.as_ref());
        self.0.deallocate(handle.cast(), layout);
    }
}

// SAFETY: Rust requires that implementors of `Allocator` are clone-safe currently
unsafe impl<A: Allocator + Clone> ClonesafeStorage for Alloc<A> {}

// SAFETY: Rust requires that implementors of `Allocator` are leak-safe currently
unsafe impl<A: Allocator> LeaksafeStorage for Alloc<A> {}

// SAFETY: Rust `Allocator` uses a `NonNull` as its handle type, this works trivially
unsafe impl<A: Allocator + Clone> FromLeakedStorage for Alloc<A> {
    unsafe fn unleak_ptr<T: ?Sized>(&self, leaked: *mut T) -> Self::Handle<T> {
        NonNull::new(leaked).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::boxed::Box;
    use crate::collections::Vec;

    use super::*;

    #[test]
    fn test_box() {
        let b = Box::<_, Alloc<Global>>::new([1, 2, 3, 4]);
        let b = b.coerce::<[i32]>();

        assert_eq!(&*b, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_vec() {
        let mut v = Vec::<_, Alloc<Global>>::new();
        v.push(1);
        v.push(2);
        v.push(3);
        v.push(4);

        assert_eq!(&*v, &[1, 2, 3, 4]);
    }
}
