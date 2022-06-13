use core::alloc::Layout;
use core::marker::Unsize;
use core::mem;
use core::ptr::{NonNull, Pointee};

use super::StorageCell;
use crate::base::{ExactSizeStorage, MultiItemStorage, Storage, StorageSafe};
use crate::error::{Result, StorageError};
use crate::statics::traits::StaticStorage;
use crate::utils;

/// Static multi-element storage implementation
pub struct MultiStatic<S: 'static, const N: usize> {
    used: [bool; N],
    storage: &'static StorageCell<[S; N]>,
}

impl<S: 'static, const N: usize> StaticStorage<[S; N]> for MultiStatic<S, N> {
    fn take_cell(storage: &'static StorageCell<[S; N]>) -> MultiStatic<S, N> {
        MultiStatic {
            used: [false; N],
            storage,
        }
    }
}

unsafe impl<S, const N: usize> Storage for MultiStatic<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = MultiStaticHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let idx = core::ptr::addr_of_mut!((*self.storage.as_ptr().as_ptr())[handle.0]);
        let ptr: NonNull<()> = NonNull::new(idx).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata = T::Metadata>>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        MultiStaticHandle(handle.0, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(
        &self,
        handle: Self::Handle<T>,
    ) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        MultiStaticHandle(handle.0, meta)
    }

    fn allocate_single<T: ?Sized + Pointee>(
        &mut self,
        meta: T::Metadata,
    ) -> Result<Self::Handle<T>> {
        self.allocate(meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        self.deallocate(handle)
    }

    unsafe fn try_grow<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity >= handle.1);
        let new_layout = Layout::array::<T>(capacity).map_err(|_| StorageError::exceeds_max())?;

        if self.will_fit::<[T]>(capacity) {
            Ok(MultiStaticHandle(handle.0, capacity))
        } else {
            Err(StorageError::InsufficientSpace {
                expected: new_layout.size(),
                available: Some(self.max_range::<T>()),
            })
        }
    }

    unsafe fn try_shrink<T>(
        &mut self,
        handle: Self::Handle<[T]>,
        capacity: usize,
    ) -> Result<Self::Handle<[T]>> {
        debug_assert!(capacity <= handle.1);
        Ok(MultiStaticHandle(handle.0, capacity))
    }
}

unsafe impl<S, const N: usize> MultiItemStorage for MultiStatic<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;

        let pos = self
            .used
            .iter()
            .position(|i| !*i)
            .ok_or(StorageError::NoSlots)?;

        self.used[pos] = true;

        Ok(MultiStaticHandle(pos, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        self.used[handle.0] = false;
    }
}

impl<S, const N: usize> ExactSizeStorage for MultiStatic<S, N>
where
    S: StorageSafe,
{
    fn will_fit<T: ?Sized + Pointee>(&self, meta: T::Metadata) -> bool {
        let layout = utils::layout_of::<T>(meta);
        mem::size_of::<S>() >= layout.size()
    }

    fn max_range<T>(&self) -> usize {
        let layout = Layout::new::<T>();
        mem::size_of::<S>() / layout.size()
    }
}

impl<S, const N: usize> Drop for MultiStatic<S, N> {
    fn drop(&mut self) {
        self.storage.release()
    }
}

pub struct MultiStaticHandle<T: ?Sized + Pointee>(usize, T::Metadata);

impl<T: ?Sized + Pointee> Clone for MultiStaticHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized + Pointee> Copy for MultiStaticHandle<T> {}
