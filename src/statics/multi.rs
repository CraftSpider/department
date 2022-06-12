use super::StorageCell;
use crate::base::{Storage, MultiItemStorage, StorageSafe};
use crate::error::StorageError;
use crate::statics::traits::StaticStorage;
use crate::{error, utils};
use std::marker::Unsize;
use std::ptr::{NonNull, Pointee};

/// Static multi-element storage implementation
pub struct MultiItem<S: 'static, const N: usize> {
    used: [bool; N],
    storage: &'static StorageCell<[S; N]>,
}

impl<S: 'static, const N: usize> StaticStorage<[S; N]> for MultiItem<S, N> {
    fn take_cell(storage: &'static StorageCell<[S; N]>) -> MultiItem<S, N> {
        MultiItem {
            used: [false; N],
            storage,
        }
    }
}

unsafe impl<S, const N: usize> Storage for MultiItem<S, N>
where
    S: StorageSafe,
{
    type Handle<T: ?Sized> = MultiStaticHandle<T>;

    unsafe fn get<T: ?Sized>(&self, handle: Self::Handle<T>) -> NonNull<T> {
        let idx = std::ptr::addr_of_mut!((*self.storage.as_ptr().as_ptr())[handle.0]);
        let ptr: NonNull<()> = NonNull::new(idx).unwrap().cast();
        NonNull::from_raw_parts(ptr, handle.1)
    }

    fn cast<T: ?Sized + Pointee, U: ?Sized + Pointee<Metadata=T::Metadata>>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        MultiStaticHandle(handle.0, handle.1)
    }

    unsafe fn coerce<T: ?Sized + Pointee + Unsize<U>, U: ?Sized + Pointee>(&self, handle: Self::Handle<T>) -> Self::Handle<U> {
        let element = self.get(handle);
        let meta = (element.as_ptr() as *mut U).to_raw_parts().1;
        MultiStaticHandle(handle.0, meta)
    }

    fn allocate_single<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> error::Result<Self::Handle<T>> {
        self.allocate(meta)
    }

    unsafe fn deallocate_single<T: ?Sized>(&mut self, handle: Self::Handle<T>) {
        self.deallocate(handle)
    }

    unsafe fn try_grow<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> error::Result<Self::Handle<[T]>> {
        todo!()
    }

    unsafe fn try_shrink<T>(&mut self, handle: Self::Handle<[T]>, capacity: usize) -> error::Result<Self::Handle<[T]>> {
        todo!()
    }
}

unsafe impl<S, const N: usize> MultiItemStorage for MultiItem<S, N>
where
    S: StorageSafe,
{
    fn allocate<T: ?Sized + Pointee>(&mut self, meta: T::Metadata) -> error::Result<Self::Handle<T>> {
        utils::validate_layout::<T, S>(meta)?;

        let pos = self
            .used
            .iter()
            .position(|i| !*i)
            .ok_or(StorageError::NoSlots)?;

        Ok(MultiStaticHandle(pos, meta))
    }

    unsafe fn deallocate<T: ?Sized + Pointee>(&mut self, handle: Self::Handle<T>) {
        todo!()
    }
}

impl<S, const N: usize> Drop for MultiItem<S, N> {
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
