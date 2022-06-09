//! The base of the whole API, the storage traits themselves.
//!
//! These traits represent the possible distinct use-cases for a storage.
//! They are separated to allow implementations to be as specific or general as they wish in
//! what they support.

use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};
use core::{fmt, ptr};

use crate::error;
use crate::error::StorageError;

/// A collection of types safe to be used with inline or static storages.
///
/// # Safety
///
/// Any type implementing this trait should contain no padding or other possible
/// 'UB-to-read' sections. The storage may slice over an array of this type, ignoring
/// normal boundaries.
pub unsafe trait StorageSafe: Copy + fmt::Debug {}

unsafe impl StorageSafe for u8 {}
unsafe impl StorageSafe for u16 {}
unsafe impl StorageSafe for u32 {}
unsafe impl StorageSafe for u64 {}
unsafe impl StorageSafe for u128 {}
unsafe impl StorageSafe for usize {}

unsafe impl<T: StorageSafe, const N: usize> StorageSafe for [T; N] {}

// TODO: These should probably be unsafe traits, like the `Allocator` trait, as impls must uphold
//       certain guarantees

/// Storages supporting single, possibly unsized, elements.
pub trait ElementStorage {
    /// The type of 'handles' given out by this storage.
    ///
    /// These not always being pointers allows a storage to possibly be moved or otherwise altered,
    /// without invalidating handles it has given out.
    ///
    /// # Validity
    ///
    /// Multiple functions may require a 'valid handle'. For a handle to be valid, these conditions
    /// must be upheld:
    /// - The handle must have been provided by the same instance of `ElementStorage` as
    ///   the method is being called on.
    /// - [`MultiElementStorage::deallocate`] or [`SingleElementStorage::deallocate_single`] must
    ///   not have been called with the handle.
    type Handle<T: ?Sized + Pointee>: Clone + Copy;

    /// Convert a handle into a raw pointer.
    ///
    /// # Safety
    ///
    /// The returned pointer, in general, is only valid as long as the following conditions are
    /// upheld:
    /// - The handle must be valid. See [`Self::Handle`].
    /// - This storage is not moved while the pointer is in use
    /// - The handle must not be deallocated while the pointer is in use
    ///
    /// Specific implementations *may* loosen these requirements.
    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T>;

    /// Convert unsizing on a handle. This function is a temporary solution until Rust
    /// supports better custom unsizing.
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`].
    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U>;
}

/// An element storage supporting only a single element at once
pub trait SingleElementStorage: ElementStorage {
    /// Attempt to allocate an element into this storage, returning a [`StorageError`] on failure.
    ///
    /// If an element has already been allocated, this *may* overwrite the existing item, or return
    /// `Err(`[`StorageError::NoSlots`]`)`, at the discretion of the implementation.
    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>>;

    /// Deallocate a previously allocated element
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`](`ElementStorage::Handle`).
    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>);

    /// Attempt to allocate an element into this storage, and initialize it with
    /// the provided `T`.
    fn create_single<T: Pointee>(
        &mut self,
        value: T,
    ) -> core::result::Result<Self::Handle<T>, (StorageError, T)> {
        // Meta is always `()` for sized types
        let handle = match self.allocate_single(()) {
            Ok(handle) => handle,
            Err(e) => return Err((e, value)),
        };

        // SAFETY: `handle` is valid, as allocate just succeeded.
        let pointer = unsafe { self.get(handle) };

        // SAFETY: `pointer` points to a suitable memory area for `T` by impl guarantees.
        unsafe { ptr::write(pointer.as_ptr(), value) };

        Ok(handle)
    }

    /// Deallocate an element from this storage, dropping the existing item.
    ///
    /// # Safety
    ///
    /// All the caveats of [`SingleElementStorage::deallocate_single`], as well as
    /// the requirement that the handle must contain a valid instance of `T`.
    unsafe fn drop_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: `handle` is valid by safety requirements.
        let element = self.get(handle);

        // SAFETY: `element` is valid by safety requirements.
        ptr::drop_in_place(element.as_ptr());

        self.deallocate_single(handle);
    }
}

/// An element storage supporting multiple elements at once
pub trait MultiElementStorage: ElementStorage {
    /// Attempt to allocate an element into this storage, returning [`StorageError`] on failure.
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>>;

    /// Deallocate a previously allocated element
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`](`ElementStorage::Handle`).
    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>);

    /// Attempt to allocate an element into this storage, and initialize it with
    /// the provided `T`.
    fn create<T: Pointee>(
        &mut self,
        value: T,
    ) -> core::result::Result<Self::Handle<T>, (StorageError, T)> {
        // Meta is always `()` for sized types
        let handle = match self.allocate(()) {
            Ok(handle) => handle,
            Err(e) => return Err((e, value)),
        };

        // SAFETY: `handle` is valid, as allocate succeeded.
        let pointer = unsafe { self.get(handle) };

        // SAFETY: `pointer` points to a suitable memory area for `T` by impl guarantees.
        unsafe { ptr::write(pointer.as_ptr(), value) };

        Ok(handle)
    }

    /// Deallocate an element from this storage, dropping the existing item.
    ///
    /// # Safety
    ///
    /// All the caveats of [`SingleElementStorage::deallocate_single`], as well as
    /// the requirement that the handle must contain a valid instance of `T`.
    unsafe fn drop<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: `handle` is valid by safety requirements.
        let element = self.get(handle);

        // SAFETY: `element` is valid by safety requirements.
        ptr::drop_in_place(element.as_ptr());

        self.deallocate(handle);
    }
}

/// Storages supporting contiguous ranges of multiple sized elements.
pub trait RangeStorage {
    /// The type of 'handles' given out by this storage.
    ///
    /// These not always being pointers allows a storage to possibly be moved or otherwise altered,
    /// without invalidating handles it has given out.
    ///
    /// # Validity
    ///
    /// Multiple functions may require a 'valid handle'. For a handle to be valid, these conditions
    /// must be upheld:
    /// - The handle must have been provided by the same instance of `RangeStorage` as
    ///   the method is being called on.
    /// - [`MultiRangeStorage::deallocate`] or [`SingleRangeStorage::deallocate_single`] must
    ///   not have been called with the handle.
    type Handle<T>: Clone + Copy;

    /// Get the maximum capacity number of contiguous elements of `T` this storage can support
    /// in a single range.
    ///
    /// Just because a range doesn't exceed the maximum capacity, does not guarantee a successful
    /// allocation.
    fn maximum_capacity<T>(&self) -> usize;

    /// Convert a handle into a raw pointer to the backing range slice.
    ///
    /// # Safety
    ///
    /// The returned pointer, in general, is only valid as long as the following conditions are
    /// upheld:
    /// - The handle must be valid. See [`Self::Handle`].
    /// - This storage is not moved while the pointer is in use
    /// - The handle must not be deallocated while the pointer is in use
    ///
    /// Specific implementations *may* loosen these requirements.
    unsafe fn get<T>(&self, handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]>;

    /// Attempt to grow a previously allocated range up to the size of `capacity`.
    ///
    /// # Safety
    ///
    /// The following conditions must be upheld:
    /// - The provided handle must be valid. See [`Self::Handle`]
    /// - `capacity` must be greater than or equal to the allocation's current length
    /// - `capacity` must not exceed [`RangeStorage::maximum_capacity()`]
    #[allow(unused_variables)]
    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> error::Result<Self::Handle<T>> {
        Err(StorageError::Unimplemented)
    }

    /// Attempt to shrink a previously allocated range down to the size of `capacity`
    ///
    /// # Safety
    ///
    /// - The provided handle must be valid. See [`Self::Handle`]
    /// - `capacity` must be less than or equal to the allocation's current length
    #[allow(unused_variables)]
    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> error::Result<Self::Handle<T>> {
        Err(StorageError::Unimplemented)
    }
}

/// A range storage supporting only a single range at once
pub trait SingleRangeStorage: RangeStorage {
    /// Attempt to allocate a range into this storage, returning a [`StorageError`] on failure.
    ///
    /// If a range has already been allocated, this *may* overwrite the existing item, or return
    /// `Err(`[`StorageError::NoSlots`]`)`, at the discretion of the implementation.
    fn allocate_single<T>(&mut self, capacity: usize) -> error::Result<Self::Handle<T>>;

    /// Deallocate a previously allocated range
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`](`RangeStorage::Handle`).
    unsafe fn deallocate_single<T>(&mut self, handle: Self::Handle<T>);

    /// Attempt to allocate a range into this storage, and initialize it with
    /// the provided array `[T; N]`.
    fn create_single<T, const N: usize>(
        &mut self,
        arr: [T; N],
    ) -> core::result::Result<Self::Handle<T>, (StorageError, [T; N])> {
        let handle = match self.allocate_single(N) {
            Ok(handle) => handle,
            Err(e) => return Err((e, arr)),
        };

        // SAFETY: `handle` is valid as allocate succeeded.
        let mut pointer: NonNull<[MaybeUninit<T>]> = unsafe { self.get(handle) };

        // SAFETY: `pointer` points to a suitable memory area for `T` by impl guarantee.
        for (idx, val) in arr.into_iter().enumerate() {
            unsafe { pointer.as_mut()[idx].write(val) };
        }

        Ok(handle)
    }
}

/// A range storage supporting multiple ranges at once
pub trait MultiRangeStorage: RangeStorage {
    /// Attempt to allocate a range into this storage, returning [`StorageError`] on failure.
    fn allocate<T>(&mut self, capacity: usize) -> error::Result<Self::Handle<T>>;

    /// Deallocate a previously allocated range
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`](`RangeStorage::Handle`).
    unsafe fn deallocate<T>(&mut self, handle: Self::Handle<T>);

    /// Attempt to allocate a range into this storage, and initialize it with
    /// the provided array `[T; N]`.
    fn create<T, const N: usize>(
        &mut self,
        arr: [T; N],
    ) -> core::result::Result<Self::Handle<T>, (StorageError, [T; N])> {
        let handle = match self.allocate(N) {
            Ok(handle) => handle,
            Err(e) => return Err((e, arr)),
        };

        // SAFETY: `handle` is valid, as allocate succeeded.
        let mut pointer: NonNull<[MaybeUninit<T>]> = unsafe { self.get(handle) };

        // SAFETY: `pointer` points to a suitable memory area for `T` by impl guarantee.
        for (idx, val) in arr.into_iter().enumerate() {
            unsafe { pointer.as_mut()[idx].write(val) };
        }

        Ok(handle)
    }
}
