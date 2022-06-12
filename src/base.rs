use core::{fmt, ptr};
use core::marker::Unsize;
use core::ptr::{NonNull, Pointee};

use crate::error::StorageError;
use crate::error;

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

pub unsafe trait Storage {
    type Handle<T: ?Sized>: Copy;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T>;

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(&self, handle: Self::Handle<T>) -> Self::Handle<U>;
    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> Self::Handle<U>;

    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> error::Result<Self::Handle<T>>;
    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>);

    #[allow(unused_variables)]
    unsafe fn try_grow<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> error::Result<Self::Handle<[T]>> {
        Err(StorageError::Unimplemented)
    }

    #[allow(unused_variables)]
    unsafe fn try_shrink<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> error::Result<Self::Handle<[T]>> {
        Err(StorageError::Unimplemented)
    }

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

    unsafe fn drop_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: `handle` is valid by safety requirements.
        let element = self.get(handle);

        // SAFETY: `element` is valid by safety requirements.
        ptr::drop_in_place(element.as_ptr());

        self.deallocate_single(handle);
    }
}

pub unsafe trait MultiItemStorage: Storage {
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> error::Result<Self::Handle<T>>;
    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>);

    fn create<T>(&mut self, value: T) -> core::result::Result<Self::Handle<T>, (StorageError, T)> {
        // Meta is always `()` for sized types
        let handle = match self.allocate(()) {
            Ok(handle) => handle,
            Err(e) => return Err((e, value)),
        };

        // SAFETY: `handle` is valid, as allocate just succeeded.
        let pointer = unsafe { self.get(handle) };

        // SAFETY: `pointer` points to a suitable memory area for `T` by impl guarantees.
        unsafe { ptr::write(pointer.as_ptr(), value) };

        Ok(handle)
    }

    unsafe fn drop<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: `handle` is valid by safety requirements.
        let element = self.get(handle);

        // SAFETY: `element` is valid by safety requirements.
        ptr::drop_in_place(element.as_ptr());

        self.deallocate(handle);
    }
}

pub unsafe trait ExactSizeStorage: Storage {
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool;
    fn max_range<T>(&self) -> usize {
        for i in 0.. {
            if self.will_fit::<[T]>(i) {
                return i;
            }
        }
        return usize::MAX;
    }
}

pub unsafe trait LeaksafeStorage: Storage {}

pub unsafe trait FromLeakedPtrStorage: LeaksafeStorage {
    unsafe fn unleak<T: ?Sized>(&self, leaked: *mut T) -> Self::Handle<T>;
}
