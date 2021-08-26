use core::{array, ptr, fmt};
use core::marker::Unsize;
use core::mem::MaybeUninit;
use core::ptr::{NonNull, Pointee};

use crate::error;
use crate::error::StorageError;

pub unsafe trait StorageSafe: Copy + fmt::Debug {}

unsafe impl StorageSafe for u8 {}
unsafe impl StorageSafe for u16 {}
unsafe impl StorageSafe for u32 {}
unsafe impl StorageSafe for u64 {}
unsafe impl StorageSafe for u128 {}
unsafe impl StorageSafe for usize {}

unsafe impl<T: StorageSafe, const N: usize> StorageSafe for [T; N] {}

pub trait ElementStorage {
    type Handle<T: ?Sized /*+ Pointee*/>: Clone + Copy;

    unsafe fn get<T: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> NonNull<T>;
    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U>;
}

pub trait SingleElementStorage: ElementStorage {
    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> error::Result<Self::Handle<T>>;
    unsafe fn deallocate_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>);

    fn create_single<T: Pointee>(
        &mut self,
        value: T,
    ) -> core::result::Result<Self::Handle<T>, (StorageError, T)> {
        let meta = NonNull::from(&value).to_raw_parts().1;

        let handle = match self.allocate_single(meta) {
            Ok(handle) => handle,
            Err(e) => return Err((e, value)),
        };

        //  SAFETY: `handle` is valid.
        let pointer = unsafe { self.get(handle) };

        //  SAFETY: `pointer` points to a suitable memory area for `T`.
        unsafe { ptr::write(pointer.as_ptr(), value) };

        Ok(handle)
    }

    unsafe fn drop_single<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: `handle` is assumed to be valid.
        let element = self.get(handle);

        // SAFETY: `element` is valid.
        ptr::drop_in_place(element.as_ptr());

        self.deallocate_single(handle);
    }
}

pub trait MultiElementStorage: ElementStorage {
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> error::Result<Self::Handle<T>>;
    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>);

    fn create<T: Pointee>(&mut self, value: T) -> core::result::Result<Self::Handle<T>, T> {
        let meta = NonNull::from(&value).to_raw_parts().1;

        if let Ok(handle) = self.allocate(meta) {
            //  SAFETY: `handle` is valid.
            let pointer = unsafe { self.get(handle) };

            //  SAFETY: `pointer` points to a suitable memory area for `T`.
            unsafe { ptr::write(pointer.as_ptr(), value) };

            Ok(handle)
        } else {
            Err(value)
        }
    }

    unsafe fn drop<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        // SAFETY: `handle` is assumed to be valid.
        let element = self.get(handle);

        // SAFETY: `element` is valid.
        ptr::drop_in_place(element.as_ptr());

        self.deallocate(handle);
    }
}

pub trait RangeStorage {
    type Handle<T>: Clone + Copy;

    fn maximum_capacity<T>(&self) -> usize;
    unsafe fn get<T>(&self, handle: Self::Handle<T>) -> NonNull<[MaybeUninit<T>]>;

    #[allow(unused_variables)]
    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> error::Result<Self::Handle<T>> {
        Err(StorageError::Unimplemented)
    }

    #[allow(unused_variables)]
    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<T>,
        capacity: usize,
    ) -> error::Result<Self::Handle<T>> {
        Err(StorageError::Unimplemented)
    }
}

pub trait SingleRangeStorage: RangeStorage {
    fn allocate_single<T>(&mut self, capacity: usize) -> error::Result<Self::Handle<T>>;
    unsafe fn deallocate_single<T>(&mut self, handle: Self::Handle<T>);

    fn create_single<T, const N: usize>(
        &mut self,
        arr: [T; N],
    ) -> core::result::Result<Self::Handle<T>, (StorageError, [T; N])> {
        let handle = match self.allocate_single(N) {
            Ok(handle) => handle,
            Err(e) => return Err((e, arr)),
        };

        // SAFETY: `handle` is valid.
        let mut pointer: NonNull<[MaybeUninit<T>]> = unsafe { self.get(handle) };

        // SAFETY: `pointer` points to a suitable memory area for `T`.
        for (idx, val) in array::IntoIter::new(arr).enumerate() {
            unsafe { pointer.as_mut()[idx].write(val) };
        }

        Ok(handle)
    }
}

pub trait MultiRangeStorage: RangeStorage {
    fn allocate<T>(&mut self, capacity: usize) -> error::Result<Self::Handle<T>>;
    unsafe fn deallocate<T>(&mut self, handle: Self::Handle<T>);

    fn create<T, const N: usize>(
        &mut self,
        arr: [T; N],
    ) -> core::result::Result<Self::Handle<T>, [T; N]> {
        if let Ok(handle) = self.allocate(N) {
            // SAFETY: `handle` is valid.
            let mut pointer: NonNull<[MaybeUninit<T>]> = unsafe { self.get(handle) };

            // SAFETY: `pointer` points to a suitable memory area for `T`.
            for (idx, val) in array::IntoIter::new(arr).enumerate() {
                unsafe { pointer.as_mut()[idx].write(val) };
            }

            Ok(handle)
        } else {
            Err(arr)
        }
    }
}
