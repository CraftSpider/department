//! The base of the whole API, the storage trait itself and its various extensions.
//!
//! These traits represent the allowed use-cases for a storage.
//! They are separated to allow implementations to be as specific or general as they wish in
//! what they support.

#[cfg(feature = "unsize")]
use core::marker::Unsize;
#[cfg(feature = "unsize")]
use core::ptr::DynMetadata;
use core::ptr::{NonNull, Pointee};
use core::{fmt, ptr};

use crate::error;
use crate::error::StorageError;

macro_rules! create_drop {
    ($create:ident, $create_range:ident, $create_dyn:ident, $drop:ident; $allocate:ident, $deallocate:ident) => {
        /// Attempt to allocate an item into this storage, and initialize it with the provided `T`.
        fn $create<T: Pointee>(
            &mut self,
            value: T,
        ) -> core::result::Result<Self::Handle<T>, (StorageError, T)> {
            // Meta is always `()` for sized types
            let handle = match self.$allocate(()) {
                Ok(handle) => handle,
                Err(e) => return Err((e, value)),
            };

            // SAFETY: `handle` is valid, as allocate just succeeded.
            let pointer = unsafe { self.get(handle) };

            // SAFETY: `pointer` points to a suitable memory area for `T` by impl guarantees.
            unsafe { ptr::write(pointer.as_ptr(), value) };

            Ok(handle)
        }

        /// Attempt to allocate a range into this storage, initializing it with the provided `T`
        #[cfg(feature = "unsize")]
        fn $create_range<U, T: Unsize<[U]>>(
            &mut self,
            value: T,
        ) -> error::Result<Self::Handle<[U]>> {
            let meta = ptr::metadata(&value as &[U]);
            let handle = self.$allocate(meta)?;

            // SAFETY: `handle` is valid, as allocate just succeeded
            let pointer: NonNull<[U]> = unsafe { self.get(handle) };

            // SAFETY: `pointer` points to a suitable location for `T` by impl guarantee
            unsafe { ptr::write(pointer.as_ptr().cast(), value) };

            Ok(handle)
        }

        /// Attempt to allocate a dyn into this storage, initializing it with the provided `T`
        #[cfg(feature = "unsize")]
        fn $create_dyn<Dyn: ?Sized + Pointee<Metadata = DynMetadata<Dyn>>, T: Unsize<Dyn>>(
            &mut self,
            value: T,
        ) -> error::Result<Self::Handle<Dyn>> {
            let meta = ptr::metadata(&value as &Dyn);
            let handle = self.$allocate(meta)?;

            // SAFETY: `handle` is valid, as allocate just succeeded
            let pointer: NonNull<Dyn> = unsafe { self.get(handle) };

            // SAFETY: `pointer` points to a suitable location for `T` by impl guarantee
            unsafe { ptr::write(pointer.as_ptr().cast(), value) };

            Ok(handle)
        }

        /// Deallocate an item from this storage, dropping the existing item.
        ///
        /// # Safety
        ///
        /// All the caveats of [`Storage::deallocate_single`], as well as
        /// the requirement that the handle must contain a valid instance of `T`.
        unsafe fn $drop<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
            // SAFETY: `handle` is valid by safety requirements.
            let element = self.get(handle);

            // SAFETY: `element` is valid by safety requirements.
            ptr::drop_in_place(element.as_ptr());

            self.$deallocate(handle);
        }
    };
}

/// A collection of types safe to be used with inline or static storages.
///
/// # Safety
///
/// Any type implementing this trait should contain no padding or other possible
/// 'UB-to-read' sections. The storage may slice over any bytes of this type, ignoring
/// normal boundaries.
pub unsafe trait StorageSafe: Sized + Copy + fmt::Debug {}

// SAFETY: `u8` contains no padding
unsafe impl StorageSafe for u8 {}
// SAFETY: `u16` contains no padding
unsafe impl StorageSafe for u16 {}
// SAFETY: `u32` contains no padding
unsafe impl StorageSafe for u32 {}
// SAFETY: `u64` contains no padding
unsafe impl StorageSafe for u64 {}
// SAFETY: `u128` contains no padding
unsafe impl StorageSafe for u128 {}
// SAFETY: `usize` contains no padding
unsafe impl StorageSafe for usize {}

// SAFETY: Arrays of items with no padding contain no padding, since size must be multiple of
//         alignment
unsafe impl<T: StorageSafe, const N: usize> StorageSafe for [T; N] {}

/// A storage, an abstraction of the idea of a location data can be placed. This may be on the
/// stack, on the heap, or even in more unusual places.
///
/// A baseline storage may only be able to store a single item at once.
///
/// # Safety
///
/// Implementations must not cause memory unsafety as long as the user follows the unsafe method
/// invariants documented on this trait. Valid handles must be returned from
/// [`Self::allocate_single`], valid pointers from [`Self::get`] when a valid handle is used,
/// UB must not be caused when [`Self::deallocate_single`] is called on a valid handle, etc.
pub unsafe trait Storage {
    /// The type of 'handles' given out by this storage
    ///
    /// These not always being pointers allows a storage to possibly be moved or otherwise altered,
    /// without invalidating handles it has given out.
    ///
    /// # Validity
    ///
    /// Multiple functions may require a 'valid handle'. For a handle to be valid, these conditions
    /// must be upheld:
    /// - The handle must have been provided by the same instance of `Storage` as
    ///   the method is being called on.
    /// - [`Storage::deallocate_single`] or [`MultiItemStorage::deallocate`] must
    ///   not have been called with the handle.
    /// - The handle type must be a valid type to read the allocated item as. This is the same
    ///   as the restrictions on dereferencing a cast or unsized pointer.
    ///
    /// Certain extension traits may loosen these requirements (See [`LeaksafeStorage`] for an
    /// example)
    type Handle<T: ?Sized>: Copy + PartialEq;

    /// Convert a handle into a raw pointer.
    ///
    /// # Safety
    ///
    /// The returned pointer, in general, is only valid as long as the following conditions are
    /// upheld:
    /// - The handle must be valid. See [`Self::Handle`].
    /// - This storage is not moved or dropped while the pointer is in use
    /// - The handle must not be deallocated while the pointer is in use
    ///
    /// Specific implementations *may* loosen these requirements.
    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T>;

    // TODO: The four below should really be implemented on the handles, however,
    //       there's currently no clean to express the correct bounds to allow this to work
    //       in generic contexts - `Handle::This<U>` can't be related to `Storage::Handle<U>`

    /// Create a handle from a handle pointing to `()` and a metadata
    fn from_raw_parts<T: ?Sized + Pointee>(handle: Self::Handle<()>, meta: T::Metadata) -> Self::Handle<T>;

    /// Convert this handle into any sized type. This is equivalent to [`NonNull::cast`]
    fn cast<T: ?Sized + Pointee, U>(handle: Self::Handle<T>) -> Self::Handle<U>;

    /// Convert this handle into a different type with the same metadata. This is roughly equivalent
    /// to [`NonNull::from_raw_parts`] with a different type but same metadata.
    fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U>;

    /// Convert unsizing on a handle. This function is a (hopefully) temporary solution until Rust
    /// supports better custom unsizing.
    #[cfg(feature = "unsize")]
    fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U>;

    /// Attempt to allocate an element into this storage, returning a [`StorageError`] on failure.
    ///
    /// If an element has already been allocated, this *may* overwrite the existing item, allocate
    /// a new item, or return `Err(`[`StorageError::NoSlots`]`)`, at the discretion of the
    /// implementation.
    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>>;

    /// Deallocate a previously allocated element
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`].
    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>);

    /// Attempt to grow a previously allocated range up to the size of `capacity`.
    ///
    /// # Safety
    ///
    /// The following conditions must be upheld:
    /// - The provided handle must be valid. See [`Self::Handle`]
    /// - `capacity` must be greater than or equal to the allocation's current length
    #[allow(unused_variables)]
    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
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
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        Err(StorageError::Unimplemented)
    }

    create_drop!(
        create_single, create_single_range, create_single_dyn, drop_single;
        allocate_single, deallocate_single
    );
}

// SAFETY: Referenced item promises to fulfill safety guarantees
unsafe impl<S: Storage> Storage for &mut S {
    type Handle<T: ?Sized> = S::Handle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        S::get(self, handle)
    }

    fn from_raw_parts<T: ?Sized + Pointee>(handle: Self::Handle<()>, meta: T::Metadata) -> Self::Handle<T> {
        S::from_raw_parts(handle, meta)
    }

    fn cast<T: ?Sized + Pointee, U>(handle: Self::Handle<T>) -> Self::Handle<U> {
        S::cast(handle)
    }

    fn cast_unsized<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        S::cast_unsized(handle)
    }

    #[cfg(feature = "unsize")]
    fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        S::coerce(handle)
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        S::allocate_single(self, meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        S::deallocate_single(self, handle)
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        S::try_grow(self, handle, capacity)
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> error::Result<Self::Handle<[T]>> {
        S::try_shrink(self, handle, capacity)
    }
}

// SAFETY: Referenced item promises to fulfill safety guarantees
unsafe impl<S> MultiItemStorage for &mut S
where
    S: MultiItemStorage,
{
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>> {
        S::allocate(self, meta)
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        S::deallocate(self, handle)
    }
}

impl<S> ExactSizeStorage for &mut S
where
    S: ExactSizeStorage,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        S::will_fit::<T>(self, meta)
    }

    fn max_range<T>(&self) -> usize {
        S::max_range::<T>(self)
    }
}

// SAFETY: Referenced item promises to fulfill safety guarantees
unsafe impl<S> LeaksafeStorage for &mut S where S: LeaksafeStorage {}

/// An extension to [`Storage`] for storages that can store multiple distinct items at once
///
/// # Safety
///
/// Implementations must not cause memory unsafety as long as the user follows the unsafe method
/// invariants documented on this trait. Valid handles must be returned from [`Self::allocate`],
/// UB must not be caused when [`Self::deallocate`] is called on a valid handle, etc.
pub unsafe trait MultiItemStorage: Storage {
    /// Attempt to allocate an item into this storage, returning [`StorageError`] on failure.
    fn allocate<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>>;

    /// Deallocate a previously allocated item
    ///
    /// # Safety
    ///
    /// The provided handle must be valid. See [`Self::Handle`](`Storage::Handle`).
    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>);

    create_drop!(
        create, create_range, create_dyn, drop;
        allocate, deallocate
    );
}

/// An extension to [`Storage`] for storages that know the exact maximum size that can be stored
/// within them.
pub trait ExactSizeStorage: Storage {
    /// Given a type and metadata, return whether the item would fit in this storage.
    ///
    /// This does not guarantee that a call to [`Storage::allocate_single`] or
    /// [`MultiItemStorage::allocate`] would succeed, as they may fail for other reasons such as
    /// alignment, or all possible slots already being in-use.
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool;

    /// Return the largest range of a sized type that could fit in this storage.
    ///
    /// This does not guarantee that a call to [`Storage::allocate_single`] or
    /// [`MultiItemStorage::allocate`] would succeed, as they may fail for other reasons such as
    /// alignment, or all possible slots already being in-use.
    fn max_range<T>(&self) -> usize;
}

/// An extension to [`Storage`] for storages that may have their handles dereferenced or deallocated
/// by any clone of the storage that allocated them.
///
/// # Safety
///
/// Handles from this storage must be safe to [`get`][Storage::get] or
/// [`deallocate`][Storage::deallocate_single] through any clone of the original storage. This is
/// used for things such as an Rc, where handles may be deallocated by any clone of the original.
pub unsafe trait ClonesafeStorage: Storage + Clone {}

/// An extension to [`Storage`] for storages that may have their handles leaked. This allows a
/// handle to outlive the [`Storage`] that created it, though does not guarantee that such handles
/// can be dereferenced on their own.
///
/// Note that this does not mean the handles are *never* invalidated, just that they are not
/// invalidated when the storage is dropped. A storage also needs to fulfill a `'static` bound for
/// its handles to be valid forever
///
/// # Safety
///
/// Handles from this storage, as well as their referenced data, must outlive this storage.
/// This means a storage may be moved, or even dropped, while a pointer to its data lives.
/// This removes the second safety invariant on [`Storage::get`] for this type.
pub unsafe trait LeaksafeStorage: Storage {}

/// An extension for storages that can restore allocations from leaked pointers. This is a
/// specialization of both [`LeaksafeStorage`] and [`ClonesafeStorage`]. Implementations may define
/// certain safety requirements on when pointers are valid to unleak, however the following
/// situations are required to work:
///
/// # Safety
///
/// - If [`Default`] is implemented, any default instance must unleak any other default instance
/// - If using some separate 'backing', any storage with the same backing as another must be able
///   unleak pointers from the other.
pub unsafe trait FromLeakedStorage: LeaksafeStorage + ClonesafeStorage {
    /// Convert a pointer back into a handle into this storage. One should be very careful with this
    /// method - implementations may define requirements on exactly what counts as a storage with
    /// the 'same backing' as another.
    ///
    /// # Safety
    ///
    /// The pointer provided must come from an instance that is 'unleak-compatible' with this
    /// instance. Two situations are required to be unleak-compatible:
    /// - If [`Default`] is implemented on this type, any two default instances are
    ///   unleak-compatible
    /// - If this type uses some 'backing', any two instances with the same backing are
    ///   unleak-compatible
    ///
    /// Other situations may be valid or not depending on the type, and one should check the
    /// implementor's documentation for any further details.
    unsafe fn unleak_ptr<T: ?Sized>(&self, leaked: *mut T) -> Self::Handle<T>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline::SingleInline;

    type Store = SingleInline<[usize; 4]>;

    #[test]
    fn create_single() {
        let mut storage = Store::default();

        let handle = storage.create_single(1.0f32).unwrap();
        unsafe { storage.drop_single(handle) };
    }

    #[test]
    fn create_single_range() {
        let mut storage = Store::default();

        let handle = storage.create_single_range::<u8, _>([1, 2, 3, 4]).unwrap();
        unsafe { storage.drop_single(handle) };
    }

    #[test]
    fn create_single_dyn() {
        let mut storage = Store::default();

        let handle = storage
            .create_single_dyn::<dyn fmt::Debug, _>("Hello!")
            .unwrap();
        unsafe { storage.drop_single(handle) };
    }
}
